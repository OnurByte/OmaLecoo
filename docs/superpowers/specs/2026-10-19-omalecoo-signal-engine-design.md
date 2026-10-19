# OmaLecoo Signal Engine Design

## Goal

Make OmaLecoo an event-aware GK301 control surface that explains its own HID activity and recovers cleanly from device disconnects, while keeping the implementation in Rust and Quickshell QML.

## Agreed constraints

- Rust owns device detection, HID reports, validation, locking, state persistence, event logging, and privileged udev operations.
- Quickshell QML owns Omarchy integration, event observation, panel controls, setup flow, and uninstall flow.
- Do not add standalone C, JavaScript/Node, or Python implementation code.
- Do not claim custom RGB, per-key RGB, or lighting readback; the GK301 firmware response is not current-lighting state.
- Keep all device writes serialized by the existing Rust file lock and pacing.
- Preserve user state unless the user explicitly selects its removal.

## Product behavior

### Signal Engine

Add a persistent Quickshell service alongside the current bar widget. It observes supported Hyprland window/fullscreen events and Quickshell idle state, resolves active rules by fixed priority, and sends only validated Rust CLI commands. Manual panel actions temporarily suppress automation. Duplicate events are coalesced and a minimum command interval prevents rapid HID writes. The panel exposes a single automation toggle and shows the active rule and its reason.

Default rules are conservative: a fullscreen window selects the existing `game` preset; idle selects `off`; otherwise the user's last manually selected state is restored. No rule is enabled until the user turns on automation. Manual panel actions pause automation for 30 minutes. A locked-state rule is excluded until Omarchy exposes a stable, documented third-party lock-state signal. State changes are coalesced and HID writes are separated by at least two seconds.

### HID Blackbox

Record a bounded, rotating `blackbox.jsonl` history (1 MiB maximum, with one previous file) under the existing state directory. Each entry includes UTC time, command origin (`panel`, `signal`, or `cli`), report bytes when a report was sent, syscall result, and a concise outcome. The recorder captures only OmaLecoo's vendor-HID control path; it never opens a keyboard input stream or records typed keys. A Rust `blackbox --json` command exposes the history to a diagnostics view in the QML panel. Firmware response entries are labeled as firmware acknowledgements, never lighting readback.

### Ghost Mode and recovery

The service polls the Rust device-presence command every 10 seconds. When the GK301 disappears, the bar and panel show a muted last-known state and a disconnected marker. On reconnect, the service reapplies the cached settings once only if the user enabled recovery. It does not send repeated HID writes while disconnected and does not describe the cached state as a hardware readback.

### Setup and removal

The panel can start `cargo install --git https://github.com/OnurByte/OmaLecoo --root <home>/.local --locked` with a fixed argument list, then verify the installed binary. If Cargo is unavailable, the panel shows the exact manual prerequisite. The device access action invokes a narrowly scoped Rust udev command through `pkexec`; the rule continues to match only vendor `1a2c`, product `9df4`, interface `01`. The Rust command writes or removes only `/etc/udev/rules.d/70-lecoo-gk301.rules` and runs `udevadm` reload/trigger. The user must authorize that system change.

The uninstall page requires a second confirmation and offers separate choices for the controller installed under `~/.local`, the plugin checkout, the optional udev rule, and OmaLecoo's state directory. The controller is removed with `cargo uninstall --root <home>/.local omalecoo`. State is removed through a Rust command only when selected. It invokes `omarchy plugin remove borat.lecoo-rgb --yes` as a detached process after any selected cleanup so removal can finish after Quickshell unloads the plugin. Unknown or manually configured binary locations are reported and left alone.

Omarchy's plugin installer clones and enables plugin files without running install hooks, so the panel is the explicit setup entry point. The README must explain the Cargo prerequisite and one-time polkit authorization plainly.

## Architecture and interfaces

- `manifest.json` declares both `service` and `bar-widget` entry points; the service remains in the same Quickshell process and no Rust daemon is added.
- `SignalService.qml` observes Hyprland and idle signals, arbitrates rules, checks device presence, and forwards validated operations to the Rust CLI.
- `src/main.rs` remains the sole HID writer and gains narrow commands for rule configuration, device presence, blackbox output, state cleanup, and udev install/remove. Rule configuration is atomically stored as `rules.json` under the existing state directory; Quickshell reads it at service startup and reloads it after panel changes.
- `Panel.qml` presents automation, connection state, the recent flight log, setup progress, and uninstall confirmation.
- State and recorder data stay under `XDG_STATE_HOME/lecoo-gk301` (or `~/.local/state/lecoo-gk301`).
- The README becomes a distinctive project landing page: one-line install, clear product story, live capability truth table, compact packet map, screenshots/preview instructions, setup/removal, privacy, and direct links to protocol evidence.

## Out of scope

- Agent Signals, Protocol Atlas, Nocturne, command palette, audio-reactive lighting, and arbitrary custom colors.
- Capturing keyboard input reports, adding a background Rust daemon, or changing Omarchy files outside the plugin and the explicitly authorized udev rule.
- Changing the already submitted marketplace commit or posting to the marketplace issue.

## Research basis and limitations

Omarchy's Quattro contract supports one plugin manifest with both a service and a bar widget, but plugin code runs unsandboxed in the long-lived shell. A third-party replacement bar receives a service-less widget facade, so the automation service is supported with Omarchy's built-in bar. The plugin installer does not execute install hooks. Quickshell exposes Hyprland raw events and an idle monitor; current Omarchy docs do not establish a stable public lock-state signal for third-party plugins, so lock-triggered behavior is deferred.

