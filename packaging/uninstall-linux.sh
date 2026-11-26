#!/usr/bin/env bash
# Remove Ferrum Paint installed by install-linux.sh (current user).
# Settings in ~/.config/ferrum-paint are kept unless you pass --remove-settings.
set -u
rm -f ~/.local/bin/ferrum-paint
rm -f ~/.local/share/applications/ferrum-paint.desktop
rm -f ~/.local/share/mime/packages/ferrum-paint.xml
for s in 48 128 256; do rm -f ~/.local/share/icons/hicolor/${s}x${s}/apps/ferrum-paint.png; done
update-mime-database ~/.local/share/mime >/dev/null 2>&1 || true
update-desktop-database ~/.local/share/applications >/dev/null 2>&1 || true
gtk-update-icon-cache ~/.local/share/icons/hicolor >/dev/null 2>&1 || true
if [ "${1:-}" = "--remove-settings" ]; then rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/ferrum-paint"; fi
echo "Ferrum Paint has been removed."
