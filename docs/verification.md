# Release verification — 1.1.0

Run from the project directory:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
node tests/ui-state.cjs
python3 tests/integration.py
python3 tests/panel-only.py
bash -n install.sh scripts/install-udev.sh
```

The QML import folder is a local lint aid pointing to the installed Omarchy
modules; runtime loading remains the decisive check for host dynamic
properties.

## What changed in 1.1.0

- New `lecoo-gk301 response [--json]`: performs the recovered firmware query
  (`07 F1 00 00 00 00 00 00`, then GET_FEATURE), validates the transfer byte
  count, report ID and BCD version format, and prints the device reply with
  device path, raw report, parsed firmware, kind and roundtrip time. Defaults
  to a human-readable report; `--json` exposes the same fields for the panel.
- Removed every user-facing semantic that shipped without physical effect:
  the `side`, `side-next`, `raw` and `reset` commands and all side-brightness
  state. The side writer belongs to the Windows tool's hidden Mouse page and
  never changed this keyboard's LEDs. `capabilities` now reports
  `side_control: "unsupported"` and keeps `custom_rgb`/`per_key_rgb`
  marked `unverified`.
- Profiles saved by 1.0 with side fields remain loadable; new saves are
  main-panel only. `doctor` now validates the firmware reply and prints the
  device node. Pacing (100 ms Windows gate) and the process-wide command lock
  are unchanged.

## Executed release checks

19 October 2026 (1.1.0): six Rust tests passed (including the new
firmware-format validation test); Clippy all targets passed with no
warnings; release build passed; UI state tests, offline CLI integration and
panel-only transport interception passed, including the `response` fixture
matrix (short transfer, wrong report ID, empty payload, bad version, short
set). Installer shell syntax valid. No sudo was required: the udev rule
already matched the installed rule, so `install.sh` performed a
password-free update.

Version 1.1.0 was installed from this tree. Live checks on the attached
keyboard: `doctor` returned the actual firmware 1.16 acknowledgment through
the new validated path. The historical 1.0.0 notes below remain accurate for
that build.

## Historical — release 1.0.0

All five Rust tests passed; Clippy all targets passed; release build passed;
UI state tests, offline CLI integration and panel-only transport
interception passed. Installer shell syntax and manifest JSON were valid.
Product QML/manifest/install scripts passed the English-text scan. QML lint
reported no errors (host dynamic-property warnings remain).

Version 1.0.0 was installed. Doctor returned the actual firmware 1.16
response. The live Omarchy panel was opened and visually checked: English
profile, search and effect controls rendered using native styling, with no
decorative keyboard or side-lighting controls. No relevant QML runtime
errors were found. No dumpcap process remained running. User-created profile
names and historical research logs were preserved rather than translated.

Readback sequence and REA evidence are documented in FINAL-ANALYSIS.md.
