#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$(realpath -- "$0")")"
[[ $EUID != 0 ]] || { echo 'Run the installer as your normal user.' >&2; exit 1; }
cargo build --release
# One elevation for the device rule and reload; ordinary use never needs sudo.
if ! cmp -s udev/70-lecoo-gk301.rules /etc/udev/rules.d/70-lecoo-gk301.rules; then
  sudo -- bash "$PWD/scripts/install-udev.sh"
fi
# Omarchy's plugin folder is a user-owned copy; when this script runs inside
# it (e.g. from `omarchy plugin add` clones), the copy step is skipped.
plugin_path="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/borat.lecoo-rgb"
backup="${XDG_STATE_HOME:-$HOME/.local/state}/lecoo-gk301/backups/$(date +%Y%m%d-%H%M%S)"
binary_tmp="$HOME/.local/bin/.lecoo-gk301-install-$$"
trap 'rm -f -- "$binary_tmp"' EXIT
install -m755 target/release/lecoo-gk301 "$binary_tmp"
mv -f -- "$binary_tmp" "$HOME/.local/bin/lecoo-gk301"
case "$PWD" in
  "${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins"/*)
    echo "Running from an enabled plugin directory; plugin files already live here."
    ;;
  *)
    mkdir -p "$backup" "$plugin_path"
    if [[ -f "$plugin_path/manifest.json" ]]; then cp -a "$plugin_path/." "$backup/"; fi
    install -m644 manifest.json BarWidget.qml Panel.qml "$plugin_path/"
    # Retired decorative components; the backup above retains their copies.
    rm -f -- "$plugin_path/ProfileControls.qml"
    rm -f -- "$plugin_path/StudioCard.qml"
    ;;
esac
echo "Installed. Source directory: $PWD"
"$HOME/.local/bin/lecoo-gk301" doctor
