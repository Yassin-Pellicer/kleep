# Releasing Kleep

Kleep ships Linux desktop bundles: Debian package, RPM package, and AppImage.

## Before you build

1. Pick a version and keep these files in sync:
   - `package.json` → `version`
   - `src-tauri/tauri.conf.json` → `version`
   - `src-tauri/Cargo.toml` → `version`
   - Run `cargo check` once so `src-tauri/Cargo.lock` updates.
2. Confirm the application metadata in `src-tauri/tauri.conf.json`:
   `productName`, `identifier`, `publisher`, descriptions, category, icons.
3. Add a `LICENSE` file and set `bundle.license` if this release will be
   distributed publicly. The repository currently has no license file.
4. Make sure the release machine has the Tauri Linux build dependencies.
   The Tauri CLI produces the `.deb`, `.rpm`, and AppImage bundles itself;
   `libfuse2` is only needed to run the AppImage locally.

## Verify

Run the full check from the repository root:

```bash
npm ci
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
python3 -c "import ast, pathlib; ast.parse(pathlib.Path('voice/server.py').read_text())"
bash -n voice/setup.sh
```

Then smoke test in development:

```bash
npm run tauri dev
```

Check app search/launch, clipboard list/copy/paste/delete/clear, dictionary,
text chat, voice record/reply, and `Esc`/click-outside to hide.

## Build the bundles

```bash
npm run tauri build
```

Artifacts:

```
src-tauri/target/release/bundle/deb/kleep_<version>_amd64.deb
src-tauri/target/release/bundle/rpm/kleep-<version>-1.x86_64.rpm
src-tauri/target/release/bundle/appimage/kleep_<version>_amd64.AppImage
```

Build a subset when needed:

```bash
npm run tauri build -- --bundles deb,appimage
```

## Test the artifacts on a clean system

For each artifact:

1. Install it (or run the AppImage).
2. Run `kleep --version` and `kleep --help`.
3. Launch normally and confirm the palette appears.
4. Hide it, then toggle with the desktop shortcut. On Wayland, bind a shortcut
   to `kleep --toggle` and confirm it works.
5. Configure `~/.config/kleep/.env` and verify dictionary + assistant requests.
6. Run `~/.local/share/kleep/voice/setup.sh` and verify voice input and
   read-aloud.
7. Verify clipboard history, copy, delete, clear, and direct paste.
8. Confirm `~/.local/share/kleep/voice/` was created with the embedded scripts.

## Publish

Generate checksums:

```bash
cd src-tauri/target/release/bundle
sha256sum \
  deb/kleep_*_amd64.deb \
  rpm/kleep-*_x86_64.rpm \
  appimage/kleep_*_amd64.AppImage > SHA256SUMS
```

Upload the three artifacts plus `SHA256SUMS` to the release page and include
the install instructions from `README.md`.

## Notes

- The voice service scripts are embedded into the Rust binary at compile time.
  Rebuild the application after changing `voice/server.py`, `voice/setup.sh`, or
  `voice/requirements.txt`.
- The Python virtual environment and downloaded models are intentionally **not**
  bundled: they are created per user by the setup script. This keeps the
  download small and avoids shipping architecture-specific wheels.
- `--hidden` is the intended autostart flag; a normal launch shows the palette.
- The global shortcut is X11-only. Wayland users must bind `kleep --toggle`
  themselves.
