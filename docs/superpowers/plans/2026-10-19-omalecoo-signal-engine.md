# OmaLecoo Signal Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the approved Signal Engine, HID Blackbox, Ghost Mode, explicit setup/removal flows, and a distinctive README using Rust and Quickshell QML.

**Architecture:** `src/main.rs` remains the only HID writer and owns persisted rules, bounded command history, device-presence checks, and narrowly scoped udev operations. `SignalService.qml` observes Quickshell/Hyprland state and forwards validated CLI commands; `Panel.qml` exposes controls, diagnostics, setup, and removal. No separate Rust daemon is added.

**Tech Stack:** Rust 2021, Cargo, Quickshell QML, Omarchy Quattro plugin manifest.

**Spec:** `docs/superpowers/specs/2026-10-19-omalecoo-signal-engine-design.md`

## Global Constraints

- Rust owns device detection, HID reports, validation, locking, state persistence, event logging, and privileged udev operations.
- Quickshell QML owns Omarchy integration, event observation, panel controls, setup flow, and uninstall flow.
- Do not add standalone C, JavaScript/Node, or Python implementation code.
- Do not claim custom RGB, per-key RGB, or lighting readback; the GK301 firmware response is not current-lighting state.
- Keep all device writes serialized by the existing Rust file lock and pacing.
- Preserve user state unless the user explicitly selects its removal.
- State and recorder data stay under `XDG_STATE_HOME/lecoo-gk301` (or `~/.local/state/lecoo-gk301`).
- Do not edit synced project files under the separate `sources/` mirror.

## Review Focus

- Malformed or missing rules JSON must fall back safely without enabling automation.
- Repeated Hyprland, idle, or reconnect events must not flood the keyboard with HID writes.
- A disconnected or permission-denied device must show Ghost Mode without presenting cached settings as device readback.
- Blackbox rotation must remain bounded and must never capture keyboard input reports or typed keys.
- Setup/removal and udev cleanup must touch only paths and rules owned by OmaLecoo and must preserve state unless selected.

---

### Task 1: Rust controller services and command contracts

**Files:**
- Modify: `src/main.rs`
- Modify: `Cargo.toml` only if a Rust crate is required; prefer standard library and current dependencies.
- Modify: `README.md` later in Task 3; keep CLI behavior aligned with this task.

**Interfaces:**
- Produces `lecoo-gk301 rules [--json]` returning `{"automation_enabled":false,"restore_on_reconnect":false}` by default.
- Produces `lecoo-gk301 rules set --automation <on|off> --restore-on-reconnect <on|off>` with atomic persistence under the existing state directory.
- Produces `lecoo-gk301 dev --json` returning `{"connected":bool,"path":string|null,"writable":bool}` without sending a HID report.
- Produces `lecoo-gk301 blackbox --json` returning newest-first structured entries from a 1 MiB `blackbox.jsonl` plus one rotated file.
- Adds optional `--origin <panel|signal>` to HID-writing commands; commands without the flag are logged as `cli`.
- Produces `lecoo-gk301 state clear` for explicitly requested state cleanup.
- Produces `lecoo-gk301 udev install|remove`, allowed only as root, limited to `/etc/udev/rules.d/70-lecoo-gk301.rules`, and followed by udev reload/trigger.

- [ ] **Step 1: Add typed rules parsing, defaults, and atomic persistence** in `src/main.rs`; malformed files load the all-disabled defaults.
- [ ] **Step 2: Add device-presence JSON output** using existing device discovery without opening a HID report path or sending a command.
- [ ] **Step 3: Add bounded structured Blackbox recording and JSON output**; record UTC time, origin, report bytes, syscall outcome, and concise result.
- [ ] **Step 4: Thread command origin through HID writes** while preserving current command lock and 110 ms protocol pacing.
- [ ] **Step 5: Add explicit state cleanup and root-only udev install/remove commands** with exact-content ownership checks before deleting a rule.
- [ ] **Step 6: Run `cargo fmt --check` and `cargo check --all-targets`**; resolve compile or formatting errors without adding a non-Rust test harness.

### Task 2: Quickshell service, panel controls, and lifecycle

**Files:**
- Create: `SignalService.qml`
- Modify: `manifest.json`
- Modify: `BarWidget.qml`
- Modify: `Panel.qml`

**Interfaces:**
- `SignalService.qml` publishes `automationEnabled`, `restoreOnReconnect`, `connected`, `activeRule`, `lastOutcome`, and a callable `reloadState()` to the matching bar widget through Omarchy's own-service lookup.
- The service observes `Hyprland.rawEvent`, active toplevel state, and `IdleMonitor`; rule priority is idle/off > fullscreen/game > restore cached default.
- Manual panel action pauses automation for 30 minutes. State transitions are coalesced and HID writes are separated by at least two seconds.
- Device presence is checked every 10 seconds. Reconnect sends one `restore --origin signal` only when recovery is enabled.
- The panel starts Cargo, pkexec, and Omarchy commands as fixed argument arrays, never a shell-assembled command string.

- [ ] **Step 1: Register one `service` and one `bar-widget` manifest entry point** using the existing plugin id and root plugin layout.
- [ ] **Step 2: Implement the persistent service** with safe startup defaults, rule arbitration, event coalescing, manual override expiry, and reconnect recovery.
- [ ] **Step 3: Expose the service to the bar widget and panel** through the documented scoped service lookup, with null-safe behavior when running under a replacement bar.
- [ ] **Step 4: Add an automation section and Ghost Mode status** to `Panel.qml`, including the active rule reason and a truthful disconnected/permission-denied message.
- [ ] **Step 5: Add the Blackbox diagnostics view** with timestamp, origin, report bytes, and outcome; label firmware replies as acknowledgements.
- [ ] **Step 6: Add setup and uninstall panels**: source install into `~/.local`, explicit polkit authorization, a second removal confirmation, optional state/udev cleanup, Cargo uninstall, and detached Omarchy plugin removal.
- [ ] **Step 7: Run `omarchy plugin validate .` when available and inspect QML/manifest diffs**; otherwise report the missing validator and perform static manifest/QML review.

### Task 3: Distinctive README and release notes

**Files:**
- Modify: `README.md`
- Modify: `docs/verification.md`

**Interfaces:**
- README installation, CLI and removal examples must match the commands from Tasks 1 and 2.
- Feature claims must distinguish confirmed hardware behavior, new software behavior, and unsupported hardware behavior.

- [ ] **Step 1: Rebuild the opening as a strong product landing page** with a concise identity, native Omarchy use case, and clear feature snapshot.
- [ ] **Step 2: Add a factual capability table and compact packet map** that preserve all current evidence limits.
- [ ] **Step 3: Explain the setup flow, Cargo prerequisite, polkit udev authorization, in-panel removal choices, preserved state, and privacy boundary.**
- [ ] **Step 4: Link protocol and hardware claims directly to the existing evidence documents** and document a real preview/screenshot path without inventing product screenshots.
- [ ] **Step 5: Run `git diff --check`, inspect all changed docs against the CLI and manifest, and review the final diff.**

