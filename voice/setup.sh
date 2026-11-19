#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

python3 -m venv .venv
.venv/bin/pip install --upgrade pip
.venv/bin/pip install -r requirements.txt

# Download the Piper voices used for speech synthesis. The voice names can be
# overridden with KLEEP_TTS_PIPER_VOICE_EN and KLEEP_TTS_PIPER_VOICE_ES.
.venv/bin/python - <<'PY'
import os
from pathlib import Path
from piper.download_voices import download_voice

target = Path("models/piper")
target.mkdir(parents=True, exist_ok=True)
voices = {
    os.environ.get("KLEEP_TTS_PIPER_VOICE_EN", "en_US-lessac-medium"),
    os.environ.get("KLEEP_TTS_PIPER_VOICE_ES", "es_ES-davefx-medium"),
}
for voice in sorted(voices):
    download_voice(voice, target)
PY

# Download and warm the Whisper model. Override it with KLEEP_WHISPER_MODEL.
.venv/bin/python - <<'PY'
import os
from faster_whisper import WhisperModel

model = os.environ.get("KLEEP_WHISPER_MODEL", "large-v3-turbo")
WhisperModel(model, device="cpu", compute_type="int8")
PY

# Marks the setup as complete so the app does not run it again.
touch .setup-complete

echo "Kleep voice service is ready."
