#!/usr/bin/env bash
# Build and install Ferrum Paint for the current user on Ubuntu (26.04 and other recent releases).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release
BIN="${CARGO_TARGET_DIR:-target}/release/ferrum-paint"

mkdir -p ~/.local/bin ~/.local/share/applications
install -m 755 "$BIN" ~/.local/bin/ferrum-paint
for s in 48 128 256; do
  d=~/.local/share/icons/hicolor/${s}x${s}/apps
  mkdir -p "$d"
  "$BIN" --write-icon "$d/ferrum-paint.png" "$s" >/dev/null
done
install -m 644 packaging/ferrum-paint.desktop ~/.local/share/applications/ferrum-paint.desktop
update-desktop-database ~/.local/share/applications >/dev/null 2>&1 || true
gtk-update-icon-cache ~/.local/share/icons/hicolor >/dev/null 2>&1 || true
echo "Installed. Make sure ~/.local/bin is on your PATH, then run: ferrum-paint"
