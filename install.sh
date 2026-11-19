#!/usr/bin/env bash
# Installs Kleep from this checkout: build dependencies, the release binary,
# desktop integration, clipboard paste support and the user configuration.
#
# Models are never downloaded by this script. Pick them in the generated
# config file (~/.config/kleep/.env) and install/pull them yourself.
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

PREFIX="${KLEEP_PREFIX:-$HOME/.local}"
INSTALL_DEPS=1
BUILD=1
INSTALL_AUTOSTART=1
SETUP_UINPUT=1
CONFIG_ONLY=0
DRY_RUN=0
NEED_RELOGIN=0

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
CONFIG_DIR="$CONFIG_HOME/kleep"
CONFIG_FILE="$CONFIG_DIR/.env"
AUTOSTART_DIR="$CONFIG_HOME/autostart"
REPO_ENV="$ROOT/.env"

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Install Kleep from this checkout and make it ready to use.

Usage: ./install.sh [options]

Options:
  --prefix DIR      Install under DIR (default: ~/.local).
  --config-only     Only write ~/.config/kleep/.env from the current setup.
  --no-deps         Do not install system build/runtime packages.
  --no-build        Use an existing src-tauri/target/release/kleep binary.
  --no-autostart    Do not add Kleep to the XDG autostart directory.
  --no-uinput       Do not set up the /dev/uinput rule for direct paste.
  --dry-run         Print what would be done without changing anything.
  -h, --help        Show this help.

Models are not installed by this script. Choose them in the generated
config file, or pull them yourself (for example with `ollama pull`).
EOF
}

while (($#)); do
  case "$1" in
    --prefix)
      [[ $# -ge 2 ]] || die "--prefix needs a directory"
      PREFIX="$2"
      shift 2
      ;;
    --prefix=*)
      PREFIX="${1#*=}"
      shift
      ;;
    --config-only)
      CONFIG_ONLY=1
      shift
      ;;
    --no-deps)
      INSTALL_DEPS=0
      shift
      ;;
    --no-build)
      BUILD=0
      shift
      ;;
    --no-autostart)
      INSTALL_AUTOSTART=0
      shift
      ;;
    --no-uinput)
      SETUP_UINPUT=0
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1 (see ./install.sh --help)"
      ;;
  esac
done

case "$PREFIX" in
  "~") PREFIX="$HOME" ;;
  "~/"*) PREFIX="$HOME/${PREFIX#\~/}" ;;
  /*) ;;
  *) PREFIX="$ROOT/$PREFIX" ;;
esac

if ((EUID == 0)); then
  SUDO=()
elif command -v sudo >/dev/null 2>&1; then
  SUDO=(sudo)
else
  SUDO=()
  warn "sudo was not found; system-wide steps may fail."
fi

run() {
  if ((DRY_RUN)); then
    printf '+ '
    printf '%q ' "$@"
    printf '\n'
  else
    "$@"
  fi
}

run_sudo() {
  if ((${#SUDO[@]})); then
    run "${SUDO[@]}" "$@"
  else
    run "$@"
  fi
}

make_dir() {
  local dir="$1"
  [[ -d "$dir" ]] && return 0
  if ((DRY_RUN)); then
    log "Would create $dir"
    return 0
  fi

  local probe="$dir"
  while [[ ! -e "$probe" ]]; do
    probe="$(dirname -- "$probe")"
  done

  if [[ -w "$probe" ]]; then
    mkdir -p "$dir"
  else
    run_sudo mkdir -p "$dir"
  fi
}

put_file() {
  local source="$1" destination="$2" mode="${3:-644}"
  if ((DRY_RUN)); then
    log "Would install $destination"
    return 0
  fi

  if [[ -w "$(dirname -- "$destination")" ]]; then
    install -m "$mode" "$source" "$destination"
  else
    run_sudo install -m "$mode" "$source" "$destination"
  fi
}

install_system_dependencies() {
  ((INSTALL_DEPS)) || {
    log "Skipping system dependencies (--no-deps)"
    return 0
  }

  if command -v pacman >/dev/null 2>&1; then
    local packages=(
      base-devel curl wget file openssl gtk3 libsoup3 webkit2gtk-4.1
      librsvg wl-clipboard xclip libpulse python python-pip
    )
    if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
      packages+=(nodejs npm)
    fi
    if ! command -v cargo >/dev/null 2>&1; then
      packages+=(rust)
    fi
    log "Installing build and runtime packages with pacman"
    run_sudo pacman -S --needed --noconfirm "${packages[@]}"
  elif command -v apt-get >/dev/null 2>&1; then
    local packages=(
      build-essential curl wget file libssl-dev libgtk-3-dev libsoup-3.0-dev
      libwebkit2gtk-4.1-dev librsvg2-dev python3 python3-venv python3-pip
      wl-clipboard xclip pulseaudio-utils
    )
    if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
      packages+=(nodejs npm)
    fi
    if ! command -v cargo >/dev/null 2>&1; then
      packages+=(rustc cargo)
    fi
    log "Installing build and runtime packages with apt"
    run_sudo apt-get update
    run_sudo apt-get install -y "${packages[@]}"
  elif command -v dnf >/dev/null 2>&1; then
    local packages=(
      gcc gcc-c++ make curl wget file openssl-devel gtk3-devel libsoup3-devel
      webkit2gtk4.1-devel librsvg2-devel python3 python3-pip wl-clipboard
      xclip pulseaudio-utils
    )
    if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
      packages+=(nodejs npm)
    fi
    if ! command -v cargo >/dev/null 2>&1; then
      packages+=(rust cargo)
    fi
    log "Installing build and runtime packages with dnf"
    run_sudo dnf install -y "${packages[@]}"
  elif command -v zypper >/dev/null 2>&1; then
    local packages=(
      gcc gcc-c++ make curl wget file libopenssl-devel gtk3-devel
      libsoup-3_0-devel webkit2gtk4.1-devel librsvg-devel python3 python3-pip
      wl-clipboard xclip pulseaudio-utils
    )
    if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
      packages+=(nodejs npm)
    fi
    if ! command -v cargo >/dev/null 2>&1; then
      packages+=(rust cargo)
    fi
    log "Installing build and runtime packages with zypper"
    run_sudo zypper install -y "${packages[@]}"
  else
    warn "Unknown package manager. Install Node.js 20+, Rust, WebKitGTK 4.1,"
    warn "GTK 3, wl-clipboard/xclip, Python 3 with venv, and parec/paplay."
  fi
}

check_build_tools() {
  command -v node >/dev/null 2>&1 || die "Node.js 20+ is required to build Kleep."
  command -v npm >/dev/null 2>&1 || die "npm is required to build Kleep."
  command -v cargo >/dev/null 2>&1 || die "Rust/Cargo is required to build Kleep."

  local major
  major="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
  if [[ "$major" -lt 20 ]]; then
    die "Node.js $(node --version) is too old; Kleep needs Node.js 20 or newer."
  fi
}

build_app() {
  if ((DRY_RUN)); then
    log "Would build the frontend and release binary"
    return 0
  fi

  check_build_tools
  if [[ -f "$ROOT/package-lock.json" ]]; then
    npm ci
  else
    npm install
  fi
  npm run tauri build -- --no-bundle
}

install_icons() {
  local source
  while IFS=: read -r size source; do
    [[ -f "$ROOT/$source" ]] || continue
    make_dir "$PREFIX/share/icons/hicolor/${size}x${size}/apps"
    put_file "$ROOT/$source" "$PREFIX/share/icons/hicolor/${size}x${size}/apps/kleep.png" 644
  done <<'EOF'
32:src-tauri/icons/32x32.png
128:src-tauri/icons/128x128.png
256:src-tauri/icons/128x128@2x.png
512:src-tauri/icons/icon.png
EOF
}

install_desktop_entry() {
  local tmp
  tmp="$(mktemp)"
  cat >"$tmp" <<EOF
[Desktop Entry]
Type=Application
Name=Kleep
Comment=Keyboard-first desktop palette
Exec="$PREFIX/bin/kleep"
Icon=kleep
Terminal=false
Categories=Utility;
StartupWMClass=kleep
EOF
  make_dir "$PREFIX/share/applications"
  put_file "$tmp" "$PREFIX/share/applications/kleep.desktop" 644
  rm -f "$tmp"

  if command -v update-desktop-database >/dev/null 2>&1; then
    run update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    run gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
  fi
}

install_autostart_entry() {
  ((INSTALL_AUTOSTART)) || {
    log "Skipping autostart entry (--no-autostart)"
    return 0
  }

  local tmp
  tmp="$(mktemp)"
  cat >"$tmp" <<EOF
[Desktop Entry]
Type=Application
Name=Kleep
Comment=Keyboard-first desktop palette
Exec="$PREFIX/bin/kleep" --hidden
Terminal=false
Categories=Utility;
EOF
  make_dir "$AUTOSTART_DIR"
  put_file "$tmp" "$AUTOSTART_DIR/kleep.desktop" 644
  rm -f "$tmp"
}

setup_uinput() {
  ((SETUP_UINPUT)) || {
    log "Skipping /dev/uinput setup (--no-uinput)"
    return 0
  }

  if [[ -w /dev/uinput ]] || id -nG "$USER" 2>/dev/null | tr ' ' '\n' | grep -qx input; then
    log "Direct paste support is already available"
    return 0
  fi

  local rule="/etc/udev/rules.d/99-kleep-uinput.rules"
  if [[ ! -f "$rule" ]]; then
    log "Adding the /dev/uinput udev rule"
    if ((DRY_RUN)); then
      log "Would write $rule"
    else
      printf '%s\n' \
        'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' \
        | run_sudo tee "$rule" >/dev/null
    fi
  fi

  if ! id -nG "$USER" 2>/dev/null | tr ' ' '\n' | grep -qx input; then
    log "Adding $USER to the input group (log out and back in to apply)"
    run_sudo usermod -aG input "$USER"
    NEED_RELOGIN=1
  fi

  if [[ ! -f /etc/modules-load.d/kleep-uinput.conf ]]; then
    if ((DRY_RUN)); then
      log "Would write /etc/modules-load.d/kleep-uinput.conf"
    else
      printf 'uinput\n' | run_sudo tee /etc/modules-load.d/kleep-uinput.conf >/dev/null
    fi
  fi

  run_sudo modprobe uinput || true
  run_sudo udevadm control --reload-rules || true
  run_sudo udevadm trigger || true
}

# Reads KEY=value from a .env file without executing it.
env_value() {
  local file="$1" key="$2" line
  [[ -f "$file" ]] || return 1
  line="$(grep -E "^[[:space:]]*${key}[[:space:]]*=" "$file" | tail -n 1 || true)"
  [[ -n "$line" ]] || return 1
  line="${line#*=}"
  printf '%s' "$line" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' \
    -e 's/^"//' -e 's/"$//' -e "s/^'//" -e "s/'$//"
}

# Priority: environment, existing user config, this checkout's .env, default.
resolve_value() {
  local key="$1" default="${2-}" value
  if [[ -v "$key" ]] && [[ -n "${!key}" ]]; then
    printf '%s' "${!key}"
    return 0
  fi
  if value="$(env_value "$CONFIG_FILE" "$key")" && [[ -n "$value" ]]; then
    printf '%s' "$value"
    return 0
  fi
  if value="$(env_value "$REPO_ENV" "$key")" && [[ -n "$value" ]]; then
    printf '%s' "$value"
    return 0
  fi
  printf '%s' "$default"
}

OLLAMA_MODELS=""
CURRENT_OLLAMA_MODEL=""
FIRST_OLLAMA_MODEL=""
if command -v ollama >/dev/null 2>&1; then
  OLLAMA_MODELS="$(ollama list 2>/dev/null | awk 'NR > 1 && $1 != "" { print $1 }' || true)"
  CURRENT_OLLAMA_MODEL="$(ollama ps 2>/dev/null | awk 'NR > 1 && $1 != "" { print $1; exit }' || true)"
  FIRST_OLLAMA_MODEL="$(printf '%s\n' "$OLLAMA_MODELS" | awk 'NF { print; exit }')"
fi

ollama_model_available() {
  [[ -n "$1" ]] && printf '%s\n' "$OLLAMA_MODELS" | grep -Fxq -- "$1"
}

DEEPSEEK_KEY="$(resolve_value DEEPSEEK_API_KEY "")"
PROVIDER="$(resolve_value KLEEP_ASSISTANT_PROVIDER "")"
PROVIDER="${PROVIDER,,}"
OLLAMA_MODEL="$(resolve_value KLEEP_OLLAMA_MODEL "")"
OLLAMA_URL="$(resolve_value KLEEP_OLLAMA_URL "http://127.0.0.1:11434")"
OLLAMA_NUM_CTX="$(resolve_value KLEEP_OLLAMA_NUM_CTX "16384")"
DEEPSEEK_MODEL="$(resolve_value KLEEP_DEEPSEEK_MODEL "deepseek-flash")"
DICTIONARY_MODEL="$(resolve_value KLEEP_DICTIONARY_MODEL "deepseek-flash")"
WHISPER_MODEL="$(resolve_value KLEEP_WHISPER_MODEL "large-v3-turbo")"
PIPER_VOICE_EN="$(resolve_value KLEEP_TTS_PIPER_VOICE_EN "en_US-lessac-medium")"
PIPER_VOICE_ES="$(resolve_value KLEEP_TTS_PIPER_VOICE_ES "es_ES-davefx-medium")"

if [[ -n "$OLLAMA_MODEL" ]] && [[ -n "$OLLAMA_MODELS" ]] && ! ollama_model_available "$OLLAMA_MODEL"; then
  warn "Ollama model '$OLLAMA_MODEL' is not installed; using an available model."
  OLLAMA_MODEL=""
fi
if [[ -z "$OLLAMA_MODEL" ]]; then
  OLLAMA_MODEL="${CURRENT_OLLAMA_MODEL:-$FIRST_OLLAMA_MODEL}"
fi
if [[ -z "$PROVIDER" ]]; then
  if [[ -n "$OLLAMA_MODEL" ]]; then
    PROVIDER="ollama"
  else
    PROVIDER="deepseek"
  fi
fi
if [[ "$PROVIDER" == "ollama" ]]; then
  if ! command -v ollama >/dev/null 2>&1; then
    warn "KLEEP_ASSISTANT_PROVIDER=ollama but the 'ollama' command was not found."
  elif [[ -z "$OLLAMA_MODEL" ]]; then
    warn "No Ollama model was found. Pull one yourself, then set"
    warn "KLEEP_OLLAMA_MODEL in $CONFIG_FILE."
  fi
fi

write_config() {
  local tmp
  make_dir "$CONFIG_DIR"

  tmp="$(mktemp)"
  chmod 600 "$tmp"
  {
    printf '# Kleep configuration, generated by install.sh.\n'
    printf '# Models are not downloaded by this installer. Choose them here and\n'
    printf '# pull/install them yourself; local Ollama models use `ollama pull`.\n'
    printf '\n# Assistant model.\n'
    printf '# ollama = local model served by Ollama; deepseek = hosted API.\n'
    printf 'KLEEP_ASSISTANT_PROVIDER=%s\n' "$PROVIDER"
    printf 'KLEEP_OLLAMA_MODEL=%s\n' "$OLLAMA_MODEL"
    printf 'KLEEP_OLLAMA_URL=%s\n' "$OLLAMA_URL"
    printf 'KLEEP_OLLAMA_NUM_CTX=%s\n' "$OLLAMA_NUM_CTX"
    printf 'KLEEP_DEEPSEEK_MODEL=%s\n' "$DEEPSEEK_MODEL"
    printf '\n# DeepSeek API key, also used by the dictionary.\n'
    printf 'DEEPSEEK_API_KEY=%s\n' "$DEEPSEEK_KEY"
    printf 'KLEEP_DICTIONARY_MODEL=%s\n' "$DICTIONARY_MODEL"
    printf '\n# Voice models, downloaded only when you set up or use voice.\n'
    printf 'KLEEP_WHISPER_MODEL=%s\n' "$WHISPER_MODEL"
    printf 'KLEEP_TTS_PIPER_VOICE_EN=%s\n' "$PIPER_VOICE_EN"
    printf 'KLEEP_TTS_PIPER_VOICE_ES=%s\n' "$PIPER_VOICE_ES"
  } >"$tmp"

  if ((!DRY_RUN)) && [[ -f "$CONFIG_FILE" ]] && cmp -s "$tmp" "$CONFIG_FILE"; then
    log "Config unchanged: $CONFIG_FILE"
    rm -f "$tmp"
    return 0
  fi

  if ((DRY_RUN)); then
    log "Would write $CONFIG_FILE:"
    sed -E 's/^(DEEPSEEK_API_KEY=).+$/\1<redacted>/' "$tmp"
  else
    if [[ -f "$CONFIG_FILE" ]]; then
      local backup="$CONFIG_FILE.bak.$(date +%Y%m%d-%H%M%S)"
      cp -p "$CONFIG_FILE" "$backup"
      log "Backed up the previous config to $backup"
    fi
    install -m 600 "$tmp" "$CONFIG_FILE"
    log "Wrote $CONFIG_FILE"
  fi
  rm -f "$tmp"
}

if ((CONFIG_ONLY)); then
  write_config
  exit 0
fi

install_system_dependencies

if ((BUILD)); then
  log "Building Kleep"
  build_app
else
  log "Skipping build (--no-build)"
fi

BINARY="$ROOT/src-tauri/target/release/kleep"
if ((!DRY_RUN)); then
  [[ -x "$BINARY" ]] || die "release binary not found at $BINARY (run without --no-build)"
fi

log "Installing Kleep into $PREFIX"
make_dir "$PREFIX/bin"
put_file "$BINARY" "$PREFIX/bin/kleep" 755
install_icons
install_desktop_entry
install_autostart_entry
setup_uinput
write_config

if [[ ":$PATH:" != *":$PREFIX/bin:"* ]]; then
  warn "$PREFIX/bin is not in your PATH. Add it to use the 'kleep' command."
fi

if [[ "${XDG_SESSION_TYPE:-}" == "wayland" ]] || [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
  log "Wayland: bind a desktop shortcut to \"$PREFIX/bin/kleep --toggle\"."
else
  log "X11: while Kleep runs, Alt+Space toggles the palette."
fi

if ((NEED_RELOGIN)); then
  warn "Log out and back in once so the 'input' group membership applies."
fi

log "Kleep is installed."
printf '  binary: %s\n' "$PREFIX/bin/kleep"
printf '  config: %s\n' "$CONFIG_FILE"
printf '  models: chose %s' "$PROVIDER"
if [[ "$PROVIDER" == "ollama" ]] && [[ -n "$OLLAMA_MODEL" ]]; then
  printf ' (%s)' "$OLLAMA_MODEL"
fi
printf '; no model files were downloaded.\n'
