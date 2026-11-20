# Kleep

A keyboard-first desktop palette for Linux. Launch applications, browse
clipboard history, look up words, and chat with a local or hosted assistant.

Kleep is a small always-on-top palette. Press the shortcut, type, and act.

## Download and install

Download the artifact for your distribution from the project's Releases page.

### Debian / Ubuntu / Linux Mint

```bash
sudo apt install ./kleep_0.1.0_amd64.deb
```

Then launch **Kleep** from your application menu or run `kleep` in a terminal.

### Fedora / RHEL

```bash
sudo dnf install ./kleep-0.1.0-1.x86_64.rpm
```

Then launch **Kleep** from your application menu or run `kleep`.

### AppImage (any distribution)

```bash
chmod +x kleep_0.1.0_amd64.AppImage
./kleep_0.1.0_amd64.AppImage
```

If your system does not have FUSE installed, run it with:

```bash
./kleep_0.1.0_amd64.AppImage --appimage-extract-and-run
```

## First run

- A normal launch opens the palette immediately.
- Press `Esc` or click outside the palette to hide it.
- On X11, while Kleep is running, `Alt+Space` toggles the palette
  (`Alt+Shift+Space` is used if that combination is unavailable).
- On Wayland, compositors do not allow the built-in global shortcut. Bind your
  own desktop shortcut to `kleep --toggle` (or the full AppImage path).
- `kleep --version` prints the version, and `kleep --help` lists the flags.
- `kleep --listen` opens the palette and starts recording a voice request right away.
- To start Kleep in the background on login, use `kleep --hidden`.

Example autostart entry (`~/.config/autostart/kleep.desktop`):

```ini
[Desktop Entry]
Type=Application
Name=Kleep
Exec=kleep --hidden
X-GNOME-Autostart-enabled=true
```

## Configure the assistant and dictionary

The assistant and dictionary need a DeepSeek API key. The key is read only by
the Rust backend and is never exposed to the webview.

Create `~/.config/kleep/.env`:

```bash
mkdir -p ~/.config/kleep
cat > ~/.config/kleep/.env <<'EOF'
DEEPSEEK_API_KEY=your-key-here
EOF
```

Get a key at <https://platform.deepseek.com/>.

You can also export `DEEPSEEK_API_KEY` in the environment. Environment variables
already set take precedence over `.env` files.

To use a local Ollama model for chat instead, add:

```bash
KLEEP_ASSISTANT_PROVIDER=ollama
KLEEP_OLLAMA_MODEL=qwen3.5:9b
# Optional, defaults to http://127.0.0.1:11434
KLEEP_OLLAMA_URL=http://127.0.0.1:11434
# Optional, defaults to 16384.
KLEEP_OLLAMA_NUM_CTX=16384
```

The model must support tool calling. `DEEPSEEK_API_KEY` is still used for web
search when present; without it the local model keeps app launching and shell
command tools but loses `web_search`.

Kleep warms the local model in the background at startup and keeps it loaded
for 30 minutes after each request.

Set `KLEEP_DEEPSEEK_MODEL` for the hosted chat model and
`KLEEP_DICTIONARY_MODEL` for the dictionary (both default to `deepseek-flash`).
Voice models can be chosen with `KLEEP_WHISPER_MODEL`,
`KLEEP_TTS_PIPER_VOICE_EN`, and `KLEEP_TTS_PIPER_VOICE_ES`. The `install.sh`
script writes these settings to `~/.config/kleep/.env`; it never downloads the
models themselves.

## Voice input and read-aloud setup

Voice is provided by a local Python service. On first launch Kleep writes it to:

```
~/.local/share/kleep/voice/
├── server.py
├── setup.sh
├── requirements.txt
├── .venv/          (created by the first-run setup)
└── models/         (downloaded by the first-run setup)
```

The setup runs automatically the first time you use voice (microphone or
read-aloud); the palette shows a notice while it works. It creates a virtual
environment and downloads the Whisper (`large-v3-turbo`) and Piper voice
models, so it can take several minutes and a few GB on the first run.

To disable automatic setup and run it yourself:

```bash
export KLEEP_VOICE_AUTO_SETUP=0
~/.local/share/kleep/voice/setup.sh
```

You need:

- Python 3 with `venv` and `pip`
  - Debian/Ubuntu: `sudo apt install python3 python3-venv python3-pip`
  - Fedora: `sudo dnf install python3 python3-pip`
- PipeWire or PulseAudio tools: `parec` and `paplay`
  - Debian/Ubuntu: `sudo apt install pulseaudio-utils`
  - Fedora: `sudo dnf install pulseaudio-utils`
- Optional: an NVIDIA GPU with CUDA. Without one, transcription runs on CPU
  (slower).

An NVIDIA GPU is not required. The CUDA libraries are installed by the setup
script, and the service falls back to CPU if CUDA is unavailable.

Set `KLEEP_VOICE_DIR` to use a different voice directory.

If voice setup fails, check `~/.local/share/kleep/voice/voice.log`. Kleep still
works for app launching, clipboard history, and text chat without voice.

## Clipboard

Kleep polls the X11 clipboard through XWayland, so it works on both X11 and
Wayland desktops.

- Copying an entry uses `wl-copy` (from `wl-clipboard`) or `xclip`.
  Install one of them:
  - Debian/Ubuntu: `sudo apt install wl-clipboard` (or `xclip`)
  - Fedora: `sudo dnf install wl-clipboard` (or `xclip`)
- Direct paste into the previously focused window (press `Enter` on a clipboard
  entry) uses a virtual keyboard through `/dev/uinput`.
- If direct paste is unavailable, copying still works.

To allow direct paste for your user, add a udev rule and join the `input` group:

```bash
echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/99-kleep-uinput.rules
sudo modprobe uinput
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input "$USER"
```

Log out and back in for the group change to take effect.

## What it does

- **Apps:** search installed desktop applications and launch them.
- **Clipboard:** browse recent clipboard text, copy an entry, delete entries, or
  paste directly into the focused window.
- **Dictionary:** look up a word or quoted phrase; results come from DeepSeek.
- **Chat:** type a multi-word prompt to talk to the assistant. It can open apps
  and links, play videos and music from YouTube, run shell commands, and search
  the web.
- **Voice:** press the microphone, speak, and the transcript is sent to the
  assistant. Replies to voice prompts are read aloud.

Spanish replies use Peninsular Spanish. Dates, times, decimals, ordinals, and
phone numbers are normalized before speech.

## Troubleshooting

- **Nothing appears after launch:** run `kleep` from a terminal to see errors.
  If you started it with `--hidden`, use your toggle shortcut.
- **`invalid response` or missing key errors:** check
  `~/.config/kleep/.env` and make sure `DEEPSEEK_API_KEY` is set.
- **Voice button fails:** the first use installs the voice service
  automatically; check `~/.local/share/kleep/voice/voice.log`. If you disabled
  auto-setup (`KLEEP_VOICE_AUTO_SETUP=0`), run
  `~/.local/share/kleep/voice/setup.sh` manually.
- **Direct paste fails:** the `wl-copy`/`xclip` copy still works; see the
  `/dev/uinput` instructions above.
- **Wayland shortcut does nothing:** bind a desktop shortcut to
  `kleep --toggle`; the built-in global shortcut is X11-only.

## Build from source

### One-shot install

```bash
./install.sh
```

This installs the build and runtime packages, builds the release binary,
installs it under `~/.local` (override with `--prefix DIR`), adds desktop and
autostart entries, and writes `~/.config/kleep/.env`. It never downloads
models: choose them in the config file and pull them yourself. Use
`./install.sh --help` for the available flags.

### Prerequisites

- Node.js 20 or newer
- Rust stable
- Tauri Linux dependencies (WebKitGTK 4.1, GTK 3, etc.)
- Python 3 with `venv` for voice
- `yt-dlp` for video playback (`python3 -m pip install --user yt-dlp`)
- `libfuse2` only if you want to run the generated AppImage locally

### Build

```bash
npm ci
npm run tauri build
```

Artifacts are written to `src-tauri/target/release/bundle/`:

- `deb/kleep_0.1.0_amd64.deb`
- `rpm/kleep-0.1.0-1.x86_64.rpm`
- `appimage/kleep_0.1.0_amd64.AppImage`

Use `npm run tauri build -- --bundles deb,appimage` to build a subset.

### Develop

```bash
npm install
npm run tauri dev
```

For development, create a `.env` in the repository root (copy `.env.example`).
The repository `voice/` directory is used directly in development; run
`cd voice && ./setup.sh` once to set it up.

See `docs/RELEASING.md` for the release checklist.
