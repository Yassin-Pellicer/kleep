# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Dictionary setup

Lookups are powered by the DeepSeek API from the Rust backend. The key is never
exposed to the webview.

1. Copy `.env.example` to `.env`.
2. Set `DEEPSEEK_API_KEY` to a key from <https://platform.deepseek.com/>.
3. Run the app normally (`npm run tauri dev`).

Without a key the palette shows a configuration error in the results area.

## Local voice input and read-aloud

Kleep uses a local Python service for speech:

- **STT:** faster-whisper `large-v3-turbo` on CUDA
- **TTS:** Piper voices for English and Spanish
- **Audio:** PipeWire/PulseAudio `parec` and `paplay`

First-time setup:

```bash
cd voice
./setup.sh
```

Requires Python 3, an NVIDIA GPU with CUDA support, and PipeWire or PulseAudio. The first run downloads the Whisper model and Piper voices. The service starts automatically with the app and loads models in the background; check `voice/voice.log` if it fails.

Click the microphone button in the palette to start recording. Recording stops automatically after you pause for about 0.8 seconds (or click again to stop right away), then the transcript is sent to the assistant. If no speech is detected at all it gives up after about 8 seconds. Replies to voice prompts are read aloud; typed prompts stay silent.

The detection thresholds can be tuned with environment variables when starting the voice service: `KLEEP_AUTO_STOP_SILENCE` (pause in seconds), `KLEEP_NO_SPEECH_TIMEOUT` (seconds to wait for speech) and `KLEEP_SPEECH_RMS` (loudness floor).

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
