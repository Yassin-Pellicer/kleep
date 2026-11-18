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

SAMPLE_RATE = 16000
MAX_RECORD_SECONDS = 120
PROTOCOL_VERSION = 2

# Automatic end-of-speech detection. Recording keeps going until the caller
# stops it, but once speech has been heard and the line stays quiet for
# AUTO_STOP_SILENCE_SECONDS the service reports that it is safe to stop.
FRAME_SECONDS = 0.03
FRAME_SAMPLES = int(SAMPLE_RATE * FRAME_SECONDS)
FRAME_BYTES = FRAME_SAMPLES * 2
SPEECH_MIN_SECONDS = 0.15
AUTO_STOP_SILENCE_SECONDS = float(
    os.environ.get("KLEEP_AUTO_STOP_SILENCE", "1.8")
)
NO_SPEECH_TIMEOUT_SECONDS = float(
    os.environ.get("KLEEP_NO_SPEECH_TIMEOUT", "12")
)
MIN_SPEECH_RMS = float(os.environ.get("KLEEP_SPEECH_RMS", "300"))
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
from piper.download_voices import download_voice  # noqa: E402

PORT = int(os.environ.get("KLEEP_VOICE_PORT", "8765"))
BASE_DIR = Path(__file__).resolve().parent
PIPER_DIR = BASE_DIR / "models" / "piper"

print("loading Whisper large-v3-turbo on CUDA...", flush=True)
try:
    whisper = WhisperModel(
        "large-v3-turbo",
        device="cuda",
        compute_type="int8_float16",
    )
except Exception as error:
    print(f"CUDA load failed, falling back to CPU: {error}", flush=True)
    whisper = WhisperModel(
        "large-v3-turbo",
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


print("loading Piper voices...", flush=True)
voices = {
    "en": load_piper_voice("en_US-lessac-medium"),
    "es": load_piper_voice("es_ES-davefx-medium"),
}

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


def _player_command(path: str) -> list[str] | None:
    if shutil.which("paplay"):
        return ["paplay", path]
    if shutil.which("pw-play"):
        return ["pw-play", path]
    if shutil.which("aplay"):
        return ["aplay", "-q", path]
    return None


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


def _play(path: str) -> None:
    global playback

    command = _player_command(path)
    if command is None:
        Path(path).unlink(missing_ok=True)
        raise RuntimeError("no audio player found (paplay/pw-play/aplay)")

    process = subprocess.Popen(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    with playback_lock:
        if playback is not None and playback.poll() is None:
            playback.terminate()
        playback = process

    def cleanup() -> None:
        process.wait()
        try:
            os.unlink(path)
        except OSError:
            pass

    threading.Thread(target=cleanup, daemon=True).start()


def strip_markdown(text: str) -> str:
    text = re.sub(r"!\[[^\]]*\]\([^)]*\)", "", text)
    text = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"(?m)^\s*[-*+]\s+", "", text)
    text = text.replace("`", "").replace("*", "").replace("#", "").replace(">", "")
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def speak(text: str, language: str) -> None:
    text = strip_markdown(text)
    if not text:
        raise ValueError("no text to speak")

    voice = voices["es" if str(language).lower().startswith("es") else "en"]
    file_descriptor, path = tempfile.mkstemp(prefix="kleep-tts-", suffix=".wav")
    os.close(file_descriptor)

    try:
        with tts_lock:
            with wave.open(path, "wb") as wav:
                voice.synthesize_wav(text, wav)
    except Exception:
        Path(path).unlink(missing_ok=True)
        raise

    _play(path)


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
                    level = min(1.0, math.sqrt(rms / 5000.0))
                    elapsed += FRAME_SECONDS
                    speaking = rms >= max(MIN_SPEECH_RMS, noise_floor * 1.8)

                    if speaking:
                        speech_run += FRAME_SECONDS
                        silence = 0.0
                        if speech_run >= SPEECH_MIN_SECONDS:
                            speech_started = True
                    else:
                        speech_run = 0.0
                        if speech_started:
                            silence += FRAME_SECONDS
                        noise_floor = noise_floor * 0.9 + rms * 0.1

                    auto_stop = (
                        speech_started and silence >= AUTO_STOP_SILENCE_SECONDS
                    ) or elapsed >= MAX_RECORD_SECONDS or (
                        not speech_started and elapsed >= NO_SPEECH_TIMEOUT_SECONDS
                    )

                    with record_status_lock:
                        record_status["speech"] = speaking
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
