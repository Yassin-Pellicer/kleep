#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

python3 -m venv .venv
.venv/bin/pip install --upgrade pip
.venv/bin/pip install -r requirements.txt

# Download the Piper voices used for fallback and references.
.venv/bin/python - <<'PY'
from pathlib import Path
from piper.download_voices import download_voice

target = Path("models/piper")
target.mkdir(parents=True, exist_ok=True)
for voice in ("en_US-lessac-medium", "es_ES-davefx-medium"):
    download_voice(voice, target)
PY

# Generate English and Castilian reference clips for Chatterbox cloning.
.venv/bin/python - <<'PY'
import wave
from pathlib import Path

from piper import PiperVoice
from piper.config import SynthesisConfig

references = {
    "en": (
        "models/piper/en_US-lessac-medium.onnx",
        0,
        "Hello, I am an English voice reference. I speak clearly and naturally, "
        "with a calm and friendly tone.",
    ),
    "es": (
        "models/piper/es_ES-davefx-medium.onnx",
        0,
        "Hola, soy una voz castellana de España. "
        "Me gusta hablar con calma y de forma natural.",
    ),
}

for code, (model, speaker, text) in references.items():
    reference = Path(f"models/chatterbox/{code}_reference.wav")
    reference.parent.mkdir(parents=True, exist_ok=True)
    voice = PiperVoice.load(model)
    speaker_id = speaker if voice.config.num_speakers > 1 else None
    with wave.open(str(reference), "wb") as wav:
        voice.synthesize_wav(
            text,
            wav,
            syn_config=SynthesisConfig(speaker_id=speaker_id),
        )
PY

# Download and warm the Whisper model. This can take a while the first time.
.venv/bin/python - <<'PY'
from faster_whisper import WhisperModel

WhisperModel("large-v3-turbo", device="cpu", compute_type="int8")
PY

echo "Kleep voice service is ready."
