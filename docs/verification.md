# Release verification — 1.4.0

Run from the project directory:

```bash
cargo test                       # unit + offline CLI + intercepted-transport tests
cargo clippy --all-targets -- -D warnings
cargo build --release
node tests/ui-state.cjs
bash -n install.sh scripts/install-udev.sh
```

## What changed in 1.4.0 (profiles removed; Rust-only tooling)

- **Profile feature removed entirely** (maintainer request): the CLI lost
  `profiles`, `profile-save`, `profile-load` and `profile-delete`; the panel
  lost its lighting-profiles section (`plugin/ProfileControls.qml` deleted,
  stale installs are cleaned by `install.sh`); no profile JSON is read or
  written anymore. `state.json` keeps the last applied settings only.
- **No Python in the project.** `tests/integration.py` and
  `tests/panel-only.py` were ported to Rust (`tests/integration.rs` and
  `tests/transport.rs`, keeping the C interception shim
  `tests/ioctl_capture.c`); `scripts/probe-extended-brightness.py` became the
  hidden `lecoo-gk301 probe-brightness` diagnostic. `--panel-only` flag and
  the interception/checks semantics are unchanged.
- Stale brightness prints fixed to `/13` (status, next/prev, preset), and
  `catalog --json` now reports `panel_brightness_max` 13.

## Historical — 1.3.0 (extended brightness)

## What changed in 1.3.0 (extended brightness)

- **Brightness 0–13**: the firmware accepts the full stored-record scale the
  Windows tool only carried internally (its UI clamps to 5). Confirmed by
  the user on the physical keyboard during the observation run in
  `docs/captures/extended-brightness-experiment.md`. The CLI bounds, panel
  slider (0–13) and capability report were extended
  (`panel_brightness_max` 13, `panel_brightness_max_windows` 5).
- `brightness <0-13>` and `panel --brightness` updated; old saved states and
  profiles remain valid.

## What changed in 1.2.0 (OmaLecoo)

- Project renamed to **OmaLecoo**; Cargo crate is `omalecoo` while the
  installed binary keeps the historical name `lecoo-gk301`, so the running
  Quickshell plugin and the user's state directory stay valid across
  updates. Plugin id `borat.lecoo-rgb` unchanged.
- Manifest name/panel header/bar tooltip are OmaLecoo with the GK301
  identity in the description.
- Every remaining document under `docs/` is English; the Turkish-named
  sample profile was removed from local state and no default profile is
  seeded.
- New research round documented in
  `docs/captures/extended-brightness-experiment.md`: REA resolved the
  startup apply chain (`sub_4206f0` task via the open-success path) and the
  vendor descriptor's three channels (Feature/Output usage 04/Input
  report 3 F1..F8). Hardware probes replayed the exact Windows startup
  record `07 FF FF 00 0D 00 00 00` and an intermediate-brightness sweep;
  the device accepted all of them without vendor input reports. UI stays
  limited to physically confirmed controls pending the user's visual
  confirmation of the extended scale.
- `scripts/probe-extended-brightness.py` added: bounded, lock-respecting
  probe with concurrent vendored input-report listening.

## Executed release checks

19 October 2026 (1.2.0): Rust tests, Clippy (all targets, -D warnings),
release build, UI state tests, offline CLI integration, panel-only
transport interceptors and installer shell syntax all passed. Product
QML/manifest/install scripts pass the English-text scan. No sudo was
needed for the udev rule (already installed and matching), and no capture
daemon was installed during the research round.

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

