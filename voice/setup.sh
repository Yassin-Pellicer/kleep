#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

python3 -m venv .venv
.venv/bin/pip install --upgrade pip
.venv/bin/pip install -r requirements.txt

# Download the Piper voices used by the service.
.venv/bin/python - <<'PY'
from pathlib import Path
from piper.download_voices import download_voice

target = Path("models/piper")
for voice in ("en_US-lessac-medium", "es_ES-davefx-medium"):
    download_voice(voice, target)
PY

# Download and warm the Whisper model. This can take a while the first time.
.venv/bin/python - <<'PY'
from faster_whisper import WhisperModel

WhisperModel("large-v3-turbo", device="cpu", compute_type="int8")
PY

echo "Kleep voice service is ready."
