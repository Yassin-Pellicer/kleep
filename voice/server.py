#!/usr/bin/env python3
"""Local speech-to-text and text-to-speech service for Kleep.

Uses faster-whisper (CUDA) for transcription and Piper for speech synthesis.
Audio is captured/played through PipeWire/PulseAudio command-line tools.
"""

import array
import ctypes
import glob
import json
import math
import os
import re
import shutil
import signal
import subprocess
import sys
import sysconfig
import tempfile
import threading
import time
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import numpy as np

os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")

SAMPLE_RATE = 16000
MAX_RECORD_SECONDS = 120
PROTOCOL_VERSION = 16

# Automatic end-of-speech detection. Recording keeps going until the caller
# stops it, but once speech has been heard and the line stays quiet for
# AUTO_STOP_SILENCE_SECONDS the service reports that it is safe to stop.
FRAME_SECONDS = 0.03
FRAME_SAMPLES = int(SAMPLE_RATE * FRAME_SECONDS)
FRAME_BYTES = FRAME_SAMPLES * 2
SPEECH_MIN_SECONDS = 0.15
CALIBRATION_SECONDS = 0.35
AUTO_STOP_SILENCE_SECONDS = float(
    os.environ.get("KLEEP_AUTO_STOP_SILENCE", "1.2")
)
NO_SPEECH_TIMEOUT_SECONDS = float(
    os.environ.get("KLEEP_NO_SPEECH_TIMEOUT", "12")
)
MIN_SPEECH_RMS = float(os.environ.get("KLEEP_SPEECH_RMS", "200"))
INITIAL_NOISE_FLOOR = 120.0


def preload_cuda_libraries() -> None:
    """Make pip-installed CUDA libraries visible to CTranslate2 without
    requiring the caller to set LD_LIBRARY_PATH."""
    purelib = sysconfig.get_paths()["purelib"]
    patterns = (
        "nvidia/cublas/lib/libcublas.so.12",
        "nvidia/cudnn/lib/libcudnn.so.9",
    )
    for pattern in patterns:
        for path in glob.glob(os.path.join(purelib, pattern)):
            try:
                ctypes.CDLL(path, mode=ctypes.RTLD_GLOBAL)
            except OSError as error:
                print(f"could not preload {path}: {error}", flush=True)


preload_cuda_libraries()

from faster_whisper import WhisperModel  # noqa: E402
from piper import PiperVoice  # noqa: E402
from piper.config import SynthesisConfig  # noqa: E402
from piper.download_voices import download_voice  # noqa: E402

PORT = int(os.environ.get("KLEEP_VOICE_PORT", "8765"))
BASE_DIR = Path(__file__).resolve().parent
PIPER_DIR = BASE_DIR / "models" / "piper"
WHISPER_MODEL = os.environ.get("KLEEP_WHISPER_MODEL", "large-v3-turbo")
PIPER_VOICE_EN = os.environ.get("KLEEP_TTS_PIPER_VOICE_EN", "en_US-lessac-medium")
PIPER_VOICE_ES = os.environ.get("KLEEP_TTS_PIPER_VOICE_ES", "es_ES-davefx-medium")
PIPER_SPEAKER_ES = int(os.environ.get("KLEEP_TTS_PIPER_SPEAKER_ES", "1"))
PIPER_LENGTH_SCALE = float(os.environ.get("KLEEP_TTS_PIPER_LENGTH_SCALE", "1.05"))
SENTENCE_PAUSE_SECONDS = float(os.environ.get("KLEEP_TTS_SENTENCE_PAUSE", "0.3"))
CLAUSE_PAUSE_SECONDS = float(os.environ.get("KLEEP_TTS_CLAUSE_PAUSE", "0.14"))

print(f"loading Whisper {WHISPER_MODEL} on CUDA...", flush=True)
try:
    whisper = WhisperModel(
        WHISPER_MODEL,
        device="cuda",
        compute_type="int8_float16",
    )
except Exception as error:
    print(f"CUDA load failed, falling back to CPU: {error}", flush=True)
    whisper = WhisperModel(
        WHISPER_MODEL,
        device="cpu",
        compute_type="int8",
    )


def load_piper_voice(name: str) -> PiperVoice:
    path = PIPER_DIR / f"{name}.onnx"
    if not path.exists():
        PIPER_DIR.mkdir(parents=True, exist_ok=True)
        print(f"downloading Piper voice {name}...", flush=True)
        download_voice(name, PIPER_DIR)
    return PiperVoice.load(str(path))


def _get_piper(code: str) -> PiperVoice:
    """Return the Piper voice for ``code``, loading it on first use."""
    global piper_en, piper_es
    if code == "es":
        if piper_es is None:
            piper_es = load_piper_voice(PIPER_VOICE_ES)
        return piper_es
    if piper_en is None:
        piper_en = load_piper_voice(PIPER_VOICE_EN)
    return piper_en


piper_en = None
piper_es = None
print(f"loading Piper English voice {PIPER_VOICE_EN}...", flush=True)
piper_en = load_piper_voice(PIPER_VOICE_EN)
print(f"loading Piper Spanish voice {PIPER_VOICE_ES}...", flush=True)
piper_es = load_piper_voice(PIPER_VOICE_ES)

record_lock = threading.Lock()
playback_lock = threading.Lock()
transcribe_lock = threading.Lock()
tts_lock = threading.Lock()
record_status_lock = threading.Lock()

recording = None
playback = None
record_status = {
    "auto_stop": False,
    "speech": False,
    "silence": 0.0,
    "level": 0.0,
}


def _raw_player_command(sample_rate: int) -> list[str] | None:
    if shutil.which("paplay"):
        return [
            "paplay",
            "--raw",
            "--format=s16le",
            f"--rate={sample_rate}",
            "--channels=1",
        ]
    if shutil.which("pw-play"):
        return [
            "pw-play",
            "--raw",
            "--format=s16",
            f"--rate={sample_rate}",
            "--channels=1",
        ]
    if shutil.which("aplay"):
        return [
            "aplay",
            "-q",
            "-f",
            "S16_LE",
            "-r",
            str(sample_rate),
            "-c",
            "1",
        ]
    return None


def _start_raw_player(sample_rate: int) -> subprocess.Popen:
    global playback

    command = _raw_player_command(sample_rate)
    if command is None:
        raise RuntimeError("no audio player found (paplay/pw-play/aplay)")

    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    with playback_lock:
        previous = playback
        playback = process

    if previous is not None and previous.poll() is None:
        previous.terminate()

    def cleanup() -> None:
        global playback
        process.wait()
        with playback_lock:
            if playback is process:
                playback = None

    threading.Thread(target=cleanup, daemon=True).start()
    return process


def _write_audio(process: subprocess.Popen, audio: np.ndarray) -> bool:
    if process.stdin is None:
        return False
    pcm = (np.clip(audio, -1.0, 1.0) * 32767).astype("<i2").tobytes()
    try:
        process.stdin.write(pcm)
        process.stdin.flush()
        return True
    except (BrokenPipeError, ValueError):
        return False


def stop_playback() -> None:
    global playback
    with playback_lock:
        process = playback
        playback = None
    if process is not None and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()


def strip_markdown(text: str) -> str:
    text = re.sub(r"!\[[^\]]*\]\([^)]*\)", "", text)
    text = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"(?m)^\s*[-*+]\s+", "", text)
    text = text.replace("`", "").replace("*", "").replace("#", "").replace(">", "")
    text = re.sub(r"\s+", " ", text)
    return text.strip()


SENTENCE_SPLIT = re.compile(r"(?<=[.!?…])\s+")
CLAUSE_SPLIT = re.compile(r"(?<=[,;:])\s+")
MAX_CHUNK_CHARS = 180


def speech_chunks(text: str) -> list[tuple[str, float]]:
    """Split text into TTS-sized chunks with a pause after each one."""
    chunks: list[tuple[str, float]] = []

    for sentence in SENTENCE_SPLIT.split(text):
        sentence = sentence.strip()
        if not sentence:
            continue

        if len(sentence) <= MAX_CHUNK_CHARS:
            chunks.append((sentence, SENTENCE_PAUSE_SECONDS))
            continue

        pieces = [piece.strip() for piece in CLAUSE_SPLIT.split(sentence) if piece.strip()]
        current = ""
        for piece in pieces:
            candidate = f"{current} {piece}".strip()
            if current and len(candidate) > MAX_CHUNK_CHARS:
                chunks.append((current, CLAUSE_PAUSE_SECONDS))
                current = piece
            else:
                current = candidate

        if current:
            chunks.append((current, SENTENCE_PAUSE_SECONDS))

    return chunks


MONTHS_ES = (
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
)

_ES_SMALL = (
    "cero",
    "uno",
    "dos",
    "tres",
    "cuatro",
    "cinco",
    "seis",
    "siete",
    "ocho",
    "nueve",
    "diez",
    "once",
    "doce",
    "trece",
    "catorce",
    "quince",
    "dieciséis",
    "diecisiete",
    "dieciocho",
    "diecinueve",
    "veinte",
    "veintiuno",
    "veintidós",
    "veintitrés",
    "veinticuatro",
    "veinticinco",
    "veintiséis",
    "veintisiete",
    "veintiocho",
    "veintinueve",
)
_ES_TENS = ("", "", "", "treinta", "cuarenta", "cincuenta")
_ES_DIGITS = (
    "cero",
    "uno",
    "dos",
    "tres",
    "cuatro",
    "cinco",
    "seis",
    "siete",
    "ocho",
    "nueve",
)
_ES_ORDINALS_M = (
    "",
    "primero",
    "segundo",
    "tercero",
    "cuarto",
    "quinto",
    "sexto",
    "séptimo",
    "octavo",
    "noveno",
    "décimo",
)
_ES_ORDINALS_F = (
    "",
    "primera",
    "segunda",
    "tercera",
    "cuarta",
    "quinta",
    "sexta",
    "séptima",
    "octava",
    "novena",
    "décima",
)


def _es_number(value: int) -> str:
    """Render 0-99 as Spanish words (used by the time normalizer)."""
    if 0 <= value < len(_ES_SMALL):
        return _ES_SMALL[value]
    if 0 <= value < 100:
        tens, ones = divmod(value, 10)
        word = _ES_TENS[tens]
        return word if ones == 0 else f"{word} y {_ES_SMALL[ones]}"
    return str(value)


def _normalize_time_es(match: re.Match) -> str:
    hour = int(match.group("hour"))
    minute = int(match.group("minute"))
    if hour > 23 or minute > 59:
        return match.group(0)

    hour = hour % 12 or 12
    hour_word = "una" if hour == 1 else _es_number(hour)
    article = "la" if hour == 1 else "las"
    lead = match.group("lead") or ""
    if match.group("article"):
        prefix = f"{lead}{article} {hour_word}"
    else:
        prefix = f"{article} {hour_word}"
    if minute == 0:
        body = "en punto"
    elif minute == 15:
        body = "y cuarto"
    elif minute == 30:
        body = "y media"
    elif minute == 45:
        body = "menos cuarto"
    else:
        body = f"y {_es_number(minute)}"
    return f"{prefix} {body}"


def _normalize_date_es(match: re.Match) -> str:
    day = int(match.group(1))
    month = int(match.group(2))
    year = match.group(3)
    if not (1 <= day <= 31 and 1 <= month <= 12):
        return match.group(0)
    return f"{day} de {MONTHS_ES[month - 1]} de {year}"


def _normalize_date_iso_es(match: re.Match) -> str:
    year = int(match.group(1))
    month = int(match.group(2))
    day = int(match.group(3))
    if not (1 <= day <= 31 and 1 <= month <= 12):
        return match.group(0)
    return f"{day} de {MONTHS_ES[month - 1]} de {year}"


def _normalize_ordinal_es(match: re.Match) -> str:
    value = int(match.group(1))
    table = _ES_ORDINALS_F if match.group(2) == "ª" else _ES_ORDINALS_M
    if 0 < value < len(table):
        return table[value]
    return match.group(0)


def _spell_digits_es(match: re.Match) -> str:
    digits = [char for char in match.group(0) if char.isdigit()]
    return " ".join(_ES_DIGITS[int(digit)] for digit in digits)


def normalize_for_speech(text: str, code: str) -> str:
    """Rewrite written forms that the TTS phonemizer reads poorly.

    Piper/espeak already handles most numbers. This fills the gaps that sound
    wrong in Spanish: dates, clock times, dotted decimals, ordinals and phone
    numbers.
    """
    if code != "es":
        return text

    text = re.sub(
        r"\b(\d{4})-(\d{2})-(\d{2})\b", _normalize_date_iso_es, text
    )
    text = re.sub(
        r"\b(\d{1,2})/(\d{1,2})/(\d{2,4})\b", _normalize_date_es, text
    )
    text = re.sub(
        r"(?:(?P<lead>\ba\s+)?(?P<article>\blas?)\s+)?(?P<hour>\d{1,2}):(?P<minute>\d{2})(?:\s*h(?:oras?)?\b)?",
        _normalize_time_es,
        text,
        flags=re.IGNORECASE,
    )
    text = re.sub(r"\b(\d{1,2})\.?\s*(º|ª)", _normalize_ordinal_es, text)
    # Phone numbers and long digit strings are unreadable as a single number.
    text = re.sub(
        r"(?<![\w+])\+\d{1,3}(?:[ .-]?\d{2,4}){2,}(?![\w])",
        _spell_digits_es,
        text,
    )
    text = re.sub(r"(?<!\d)\d{9,}(?!\d)", _spell_digits_es, text)
    # A dot followed by one or two digits is a decimal; dot thousands
    # separators (1.234) are left alone because espeak reads them correctly.
    text = re.sub(r"(?<=\d)\.(?=\d{1,2}(?!\d))", " coma ", text)
    return re.sub(r"\s+", " ", text).strip()


def speak(text: str, language: str) -> None:
    text = strip_markdown(text)
    if not text:
        raise ValueError("no text to speak")

    code = "es" if str(language).lower().startswith("es") else "en"
    text = normalize_for_speech(text, code)
    chunks = speech_chunks(text)
    if not chunks:
        raise ValueError("no text to speak")

    with tts_lock:
        process: subprocess.Popen | None = None
        sample_rate = 24_000

        for index, (chunk, pause) in enumerate(chunks):
            piper = _get_piper(code)
            speaker_id = None
            if code == "es" and int(getattr(piper.config, "num_speakers", 1)) > 1:
                speaker_id = PIPER_SPEAKER_ES
                if speaker_id >= piper.config.num_speakers:
                    speaker_id = 0

            parts: list[np.ndarray] = []
            for audio_chunk in piper.synthesize(
                chunk,
                syn_config=SynthesisConfig(
                    speaker_id=speaker_id,
                    length_scale=PIPER_LENGTH_SCALE,
                ),
            ):
                sample_rate = audio_chunk.sample_rate
                parts.append(
                    np.asarray(
                        audio_chunk.audio_float_array, dtype=np.float32
                    )
                )
            if not parts:
                continue
            audio = np.concatenate(parts)

            if process is None:
                process = _start_raw_player(sample_rate)

            if not _write_audio(process, audio):
                return

            if index < len(chunks) - 1 and pause > 0:
                silence = np.zeros(
                    int(sample_rate * pause), dtype=np.float32
                )
                if not _write_audio(process, silence):
                    return

        if process is not None and process.stdin is not None:
            try:
                process.stdin.close()
            except OSError:
                pass


def _frame_rms(frame: bytes) -> float:
    samples = array.array("h")
    samples.frombytes(frame)
    if not samples:
        return 0.0
    total = sum(sample * sample for sample in samples)
    return math.sqrt(total / len(samples))


def _reset_record_status() -> None:
    with record_status_lock:
        record_status["auto_stop"] = False
        record_status["speech"] = False
        record_status["silence"] = 0.0
        record_status["level"] = 0.0


def _monitor_recording(path: str, stop_event: threading.Event) -> None:
    """Watch the captured audio and flag when the speaker has gone quiet.

    The recorder writes raw samples to ``path``; this thread tails that file in
    small frames and measures loudness. It never stops the recorder itself, so
    the caller can still stop at any time and get the full buffer.
    """

    buffer = bytearray()
    noise_floor = INITIAL_NOISE_FLOOR
    speech_run = 0.0
    speech_started = False
    speaking_now = False
    silence = 0.0
    elapsed = 0.0

    try:
        with open(path, "rb") as stream:
            while not stop_event.is_set():
                chunk = stream.read(FRAME_BYTES * 4)
                if not chunk:
                    stop_event.wait(0.02)
                    continue

                buffer.extend(chunk)

                while len(buffer) >= FRAME_BYTES and not stop_event.is_set():
                    frame = bytes(buffer[:FRAME_BYTES])
                    del buffer[:FRAME_BYTES]

                    rms = _frame_rms(frame)
                    elapsed += FRAME_SECONDS

                    if elapsed < CALIBRATION_SECONDS:
                        # Estimate the room noise before accepting speech.
                        noise_floor = noise_floor * 0.7 + rms * 0.3
                        speaking_now = False
                    else:
                        # Hysteresis keeps room noise from flickering the meter
                        # and the auto-stop detector into a speech state.
                        start_threshold = max(MIN_SPEECH_RMS, noise_floor * 1.1)
                        stop_threshold = max(
                            MIN_SPEECH_RMS * 0.6, noise_floor * 1.02
                        )
                        speaking_now = (
                            rms >= stop_threshold
                            if speaking_now
                            else rms >= start_threshold
                        )

                    level = (
                        min(1.0, math.sqrt(rms / 5000.0))
                        if speech_started
                        else 0.0
                    )

                    if speaking_now:
                        speech_run += FRAME_SECONDS
                        silence = 0.0
                        if speech_run >= SPEECH_MIN_SECONDS:
                            speech_started = True
                    else:
                        speech_run = 0.0
                        if speech_started:
                            silence += FRAME_SECONDS
                        if elapsed >= CALIBRATION_SECONDS:
                            noise_floor = noise_floor * 0.9 + rms * 0.1

                    auto_stop = (
                        speech_started and silence >= AUTO_STOP_SILENCE_SECONDS
                    ) or elapsed >= MAX_RECORD_SECONDS or (
                        not speech_started and elapsed >= NO_SPEECH_TIMEOUT_SECONDS
                    )

                    with record_status_lock:
                        record_status["speech"] = speaking_now
                        record_status["silence"] = silence
                        record_status["level"] = level
                        if auto_stop:
                            record_status["auto_stop"] = True

                    if auto_stop:
                        return
    except OSError:
        # The recording was stopped and its temp file removed while reading.
        return


def get_recording_status() -> dict:
    with record_lock:
        active = recording is not None

    if not active:
        return {
            "recording": False,
            "autoStop": False,
            "speech": False,
            "silence": 0.0,
            "level": 0.0,
        }

    with record_status_lock:
        return {
            "recording": True,
            "autoStop": record_status["auto_stop"],
            "speech": record_status["speech"],
            "silence": round(record_status["silence"], 3),
            "level": round(record_status["level"], 3),
        }


def start_recording() -> None:
    global recording

    with record_lock:
        if recording is not None:
            raise RuntimeError("already recording")

        file_descriptor, path = tempfile.mkstemp(prefix="kleep-rec-", suffix=".raw")
        os.close(file_descriptor)
        output = open(path, "wb")

        try:
            process = subprocess.Popen(
                [
                    "parec",
                    "--format=s16le",
                    f"--rate={SAMPLE_RATE}",
                    "--channels=1",
                    "--latency-msec=20",
                ],
                stdin=subprocess.DEVNULL,
                stdout=output,
                stderr=subprocess.DEVNULL,
            )
        except Exception:
            output.close()
            Path(path).unlink(missing_ok=True)
            raise

        _reset_record_status()
        stop_event = threading.Event()
        monitor = threading.Thread(
            target=_monitor_recording,
            args=(path, stop_event),
            daemon=True,
        )
        monitor.start()

        recording = (process, output, path, stop_event, monitor)


def _stop_recording_process() -> tuple[bytes, str]:
    global recording

    with record_lock:
        if recording is None:
            raise RuntimeError("not recording")
        process, output, path, stop_event, monitor = recording
        recording = None

    stop_event.set()
    monitor.join(timeout=1)

    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()

    output.flush()
    output.close()

    data = Path(path).read_bytes()
    Path(path).unlink(missing_ok=True)
    _reset_record_status()
    return data, path


def cancel_recording() -> None:
    try:
        _stop_recording_process()
    except RuntimeError:
        pass


def stop_recording() -> dict:
    data, _ = _stop_recording_process()

    max_bytes = SAMPLE_RATE * 2 * MAX_RECORD_SECONDS
    if len(data) > max_bytes:
        data = data[:max_bytes]

    if len(data) < SAMPLE_RATE * 2 // 4:
        return {"text": "", "language": "en"}

    file_descriptor, wav_path = tempfile.mkstemp(prefix="kleep-stt-", suffix=".wav")
    os.close(file_descriptor)

    try:
        with wave.open(wav_path, "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(SAMPLE_RATE)
            wav.writeframes(data)

        with transcribe_lock:
            segments, info = whisper.transcribe(
                wav_path,
                beam_size=1,
                vad_filter=True,
                condition_on_previous_text=False,
                without_timestamps=True,
            )
            text = "".join(segment.text for segment in segments).strip()

        language = getattr(info, "language", "en") or "en"
        return {
            "text": text,
            "language": "es" if str(language).lower().startswith("es") else "en",
        }
    finally:
        Path(wav_path).unlink(missing_ok=True)


def _cleanup_and_exit() -> None:
    try:
        cancel_recording()
    except Exception:
        pass
    try:
        stop_playback()
    except Exception:
        pass
    os._exit(0)


class Handler(BaseHTTPRequestHandler):
    server_version = "KleepVoice/1.0"

    def log_message(self, format: str, *args) -> None:
        print(format % args, flush=True)

    def _json(self, status: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def do_GET(self) -> None:
        if self.path == "/health":
            self._json(200, {"ok": True, "version": PROTOCOL_VERSION})
        else:
            self._json(404, {"error": "not found"})

    def do_POST(self) -> None:
        try:
            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length) if length else b""
            body = json.loads(raw) if raw else {}

            if self.path == "/start":
                start_recording()
                self._json(200, {"ok": True})
            elif self.path == "/stop":
                self._json(200, stop_recording())
            elif self.path == "/status":
                self._json(200, get_recording_status())
            elif self.path == "/cancel":
                cancel_recording()
                self._json(200, {"ok": True})
            elif self.path == "/speak":
                speak(str(body.get("text", "")), str(body.get("language", "en")))
                self._json(200, {"ok": True})
            elif self.path == "/stop_speaking":
                stop_playback()
                self._json(200, {"ok": True})
            elif self.path == "/shutdown":
                self._json(200, {"ok": True})
                threading.Thread(target=_cleanup_and_exit, daemon=True).start()
            else:
                self._json(404, {"error": "not found"})
        except Exception as error:
            self._json(400, {"error": str(error)})


if __name__ == "__main__":
    parent_pid = os.getppid()

    def watch_parent() -> None:
        while True:
            time.sleep(2)
            if os.getppid() != parent_pid:
                _cleanup_and_exit()

    def handle_signal(_signum, _frame) -> None:
        threading.Thread(target=_cleanup_and_exit, daemon=True).start()

    signal.signal(signal.SIGTERM, handle_signal)
    signal.signal(signal.SIGINT, handle_signal)
    threading.Thread(target=watch_parent, daemon=True).start()

    server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    server.daemon_threads = True
    print(f"Kleep voice service ready on 127.0.0.1:{PORT}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        cancel_recording()
        stop_playback()
