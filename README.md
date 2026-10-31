<div align="center">

<img src="docs/assets/omalecoo-banner.png" alt="OmaLecoo — Lecoo GK301 RGB control for Omarchy" width="900">

# OmaLecoo

[![Built for Omarchy: Plugin](https://raw.githubusercontent.com/tcballard/omarchy-badges/75975e5b5bf75e7ede3764bcd2950046f7abfe2c/badges/v1/omarchy-plugin.svg)](https://github.com/tcballard/omarchy-badges)

Lecoo GK301 lighting control for Omarchy. Rust controller, Quickshell panel.

</div>

## Install

```bash
omarchy plugin add https://github.com/OnurByte/OmaLecoo.git --enable
```

Open the keyboard icon in the bar. First-time setup builds the controller
and asks for authorization to install the GK301 udev rule. If Rust is missing,
setup installs it through Omarchy's package manager. **Setup** has manual controls
if a step needs retrying. Building may download Cargo dependencies.

## Controls

- 20 lighting effects, brightness, speed and presets.
- Fullscreen and idle lighting rules, with optional restore on reconnect.
- Lighting macros: save and replay a timed sequence of effects.
- Function-key commands and a local HID command log.

<img src="docs/assets/panel-lighting.png" alt="Current OmaLecoo Lighting panel" width="520">

The panel shows cached commands, not a reading of the LEDs. The controller
targets SEMICO `1a2c:9df4`; arbitrary RGB colours and independent hardware-zone
control are not established. [Hardware findings](docs/FINAL-ANALYSIS.md).

## CLI

```bash
lecoo-gk301 list
lecoo-gk301 panel 6 --brightness 5 --speed 2
lecoo-gk301 macro list --json
lecoo-gk301 macro play boot
```

For a manual controller build:

```bash
cargo install --path "$HOME/.config/omarchy/plugins/borat.lecoo-rgb" \
  --root "$HOME/.local" --locked --target-dir "$HOME/.cache/lecoo-gk301"
```

## Files and permissions

- Controller: `~/.local/bin/lecoo-gk301`.
- Settings, macros and logs: `~/.local/state/lecoo-gk301/`.
- Build cache: `~/.cache/lecoo-gk301/`.
- Device access: `/etc/udev/rules.d/70-lecoo-gk301.rules`, installed through polkit.

Routine controls run as your user. There are no runtime network calls or
keystroke recording. Macro collections are capped at 1 MiB, 64 macros,
256 steps per macro and 4,096 total steps; rejected imports leave saved data intact.

## Remove

Open **Uninstall**, choose what to remove, then review and confirm.
Settings and the udev rule remain unless you select them for removal.

## Documentation

- [Setup, controls, CLI and cleanup details](docs/usage.md)
- [Protocol and hardware limits](docs/FINAL-ANALYSIS.md)
- [Windows utility comparison](docs/windows-app-parity.md)
- [Validation and development notes](docs/verification.md)

Keyboard artwork comes from the original GK301 Windows utility; see
[asset provenance](docs/assets/README.md). Project code: [MIT](LICENSE).
