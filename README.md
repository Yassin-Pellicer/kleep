# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Dictionary setup

Lookups are powered by the DeepSeek API from the Rust backend. The key is never
exposed to the webview.

1. Copy `.env.example` to `.env`.
2. Set `DEEPSEEK_API_KEY` to a key from <https://platform.deepseek.com/>.
3. Run the app normally (`npm run tauri dev`).

Without a key the palette shows a configuration error in the results area.

## Assistant model

Assistant chat uses DeepSeek by default. To run it on a local Ollama model
instead, add these to `.env`:

```bash
KLEEP_ASSISTANT_PROVIDER=ollama
KLEEP_OLLAMA_MODEL=qwen3.5:9b
# Optional, defaults to http://127.0.0.1:11434
KLEEP_OLLAMA_URL=http://127.0.0.1:11434
# Optional, defaults to 16384. Ollama's own default for many models is 4096,
# which is too small once the prompt, tools and command output share the window.
KLEEP_OLLAMA_NUM_CTX=16384
```

The model must support tool calling. `DEEPSEEK_API_KEY` is still used for web
search when present; without it the local model keeps app launching and shell
command tools but loses `web_search`.

Kleep warms the local model in the background at startup and keeps it loaded
for 30 minutes after each request, so the first prompt does not wait for a cold
model load.

## Local voice input and read-aloud

Kleep uses a local Python service for speech:

- **STT:** faster-whisper `large-v3-turbo` on CUDA
- **TTS:** Piper on CPU for both languages: `es_ES-davefx-medium` (Castilian)
  and `en_US-lessac-medium`. Chatterbox remains available as an optional engine
  (`KLEEP_TTS_ENGINE_EN=chatterbox` / `KLEEP_TTS_ENGINE_ES=chatterbox`); it loads
  on first use and is released after 5 minutes of inactivity. Both default
  voices run on CPU, so only Whisper uses VRAM (~1.2 GB).
- **Audio:** PipeWire/PulseAudio `parec` and `paplay`

First-time setup:

```bash
cd voice
./setup.sh
```

Requires Python 3, an NVIDIA GPU with CUDA support, and PipeWire or PulseAudio. The first run downloads the Whisper model, the Piper voices and references, and the optional Chatterbox multilingual model when that engine is enabled. The service starts automatically with the app and loads models in the background; check `voice/voice.log` if it fails.

Click the microphone button in the palette to start recording. Recording stops automatically after you pause for about 0.8 seconds (or click again to stop right away), then the transcript is sent to the assistant. If no speech is detected at all it gives up after about 8 seconds. Replies to voice prompts are read aloud; typed prompts stay silent.

The detection thresholds can be tuned with environment variables when starting the voice service: `KLEEP_AUTO_STOP_SILENCE` (pause in seconds), `KLEEP_NO_SPEECH_TIMEOUT` (seconds to wait for speech) and `KLEEP_SPEECH_RMS` (loudness floor).

Text-to-speech can be tuned too:

- `KLEEP_TTS_ENGINE_ES` / `KLEEP_TTS_ENGINE_EN` — `piper` or `chatterbox`
  (defaults: `piper` for both languages).
- `KLEEP_TTS_PIPER_VOICE_ES` — Castilian Piper voice
  (default `es_ES-davefx-medium`).
- `KLEEP_TTS_PIPER_VOICE_EN` — English Piper voice
  (default `en_US-lessac-medium`).
- `KLEEP_TTS_PIPER_SPEAKER_ES` — speaker id for multi-speaker Piper voices
  (default `1`; ignored by single-speaker voices).
- `KLEEP_CHATTERBOX_IDLE_SECONDS` — unload Chatterbox after this many idle
  seconds to free VRAM (default `300`; `0` keeps it loaded).

Spanish replies are normalized before synthesis, so dates, clock times,
decimal points, ordinals and phone numbers are read as Castilian Spanish
instead of digit-by-digit.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
