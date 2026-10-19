#!/usr/bin/env bash
set -euo pipefail
[[ $EUID == 0 ]] || { echo 'This step requires administrator privileges.' >&2; exit 1; }
install -m644 "$(dirname "$(realpath "$0")")/../udev/70-lecoo-gk301.rules" /etc/udev/rules.d/70-lecoo-gk301.rules
udevadm control --reload-rules
udevadm trigger --action=add --subsystem-match=hidraw
udevadm settle
