# OmaLecoo Rust-Only Cleanup Plan

**Goal:** Make the plugin Rust and Quickshell only, split the monolithic
controller into focused modules, and give the marketplace a README and a
root preview that explain the plugin honestly.

**Architecture:** `src/lib.rs` becomes the library root of the `omalecoo`
crate; `src/main.rs` stays the `lecoo-gk301` binary and only takes the shared
lock before dispatching. Every module owns one responsibility. The Quickshell
side is unchanged: `SignalService.qml`, `BarWidget.qml` and `Panel.qml` keep
their current behaviour.

**Tech Stack:** Rust 2021, Cargo, Quickshell QML, Omarchy Quattro plugin
manifest. No new dependency.

## Global Constraints

- Rust owns device detection, HID reports, validation, locking, state
  persistence, event logging, and the privileged udev operation.
- Quickshell QML owns Omarchy integration, event observation, panel controls,
  setup flow, and uninstall flow.
- No standalone C, JavaScript/Node, or Python implementation file may remain.
- Do not claim custom RGB, per-key RGB, or lighting readback.
- Keep all device writes serialized by the existing Rust file lock and pacing.
- Preserve user state unless the user explicitly selects its removal.
- The README must describe the privileged action, the files written and the
  privacy boundary; the marketplace review reads it.
- Do not run the test suite. Static and build verification only.

## Review Focus

- The ported startup probe must stay display-only unless `--send` is given.
- The transport substitution must not change the real ioctl path.
- The udev rule content must have exactly one owner.
- The removed installer paths must still be reachable through the panel.

---

### Task 1: Split the controller into modules

**Files:**
- Create: `src/lib.rs`, `src/protocol.rs`, `src/transport.rs`, `src/device.rs`,
  `src/state.rs`, `src/blackbox.rs`, `src/logging.rs`, `src/lock.rs`,
  `src/udev.rs`, `src/cli.rs`, `src/commands.rs`, `src/probe.rs`, `src/sys.rs`
- Modify: `src/main.rs`, `Cargo.toml`

- [x] **Step 1:** Move the wire constants, effect catalog and the
  capability/catalog reports into `src/protocol.rs`.
- [x] **Step 2:** Introduce the `HidTransport` trait in `src/transport.rs` with
  the production `hidraw` ioctl implementation behind it.
- [x] **Step 3:** Move discovery, the write paths and the firmware query into
  `src/device.rs`; keep the exact transfer-length validation.
- [x] **Step 4:** Move the saved settings and the automation rules into
  `src/state.rs`, keeping the atomic writes and the all-disabled fallback.
- [x] **Step 5:** Move the Blackbox recorder, the debug log, the JSON helpers
  and the command lock into their own modules.
- [x] **Step 6:** Move the udev rule and its install/remove path into
  `src/udev.rs`, keeping the root check and the exact-content comparison.
- [x] **Step 7:** Split argument helpers from command implementations
  (`src/cli.rs`, `src/commands.rs`) and keep the hidden diagnostics in
  `src/probe.rs`.
- [x] **Step 8:** Reduce `src/main.rs` to signal handling, the lock and the
  dispatch call.
- [x] **Step 9:** Run `cargo fmt --check` and `cargo check --all-targets`.

### Task 2: Remove the non-Rust implementation files

**Files:**
- Delete: `install.sh`, `scripts/install-udev.sh`,
  `scripts/probe-windows-startup.py`, `tests/ioctl_capture.c`,
  `tests/ui-state.cjs`, `udev/70-lecoo-gk301.rules`

- [x] **Step 1:** Confirm the panel's Setup and Uninstall pages already cover
  the installer and the udev rule, so no capability is lost.
- [x] **Step 2:** Port the startup probe to the hidden
  `lecoo-gk301 probe-startup [--send]` diagnostic.
- [x] **Step 3:** Replace the C interception shim with the in-process
  `HidTransport` substitution in `tests/transport.rs`.
- [x] **Step 4:** Remove the stale Node panel test; record why it could not
  validate the current panel.
- [x] **Step 5:** Make `src/udev.rs` the single owner of the rule content and
  delete the duplicate rule file.
- [x] **Step 6:** Run `cargo fmt --check` and `cargo check --all-targets`.

### Task 3: Marketplace-facing README and preview

**Files:**
- Modify: `README.md`
- Create: `preview.png`, `docs/assets/omalecoo-banner.png`, `docs/assets/omalecoo-banner.svg`
- Modify: `docs/verification.md`, `docs/physical-validation.md`

- [x] **Step 1:** Add a root `preview.png` so the marketplace stops
  falling back to its generic preview. It was first a designed banner and was
  later replaced with a real screenshot of a live session.
- [x] **Step 2:** Add the landing page, the feature grid and an explicit
  **What this plugin touches** table covering the one privileged file, the
  controller path and the state directory.
- [x] **Step 3:** Keep every capability boundary, and state plainly that the
  banner is a project image rather than a panel screenshot.
- [x] **Step 4:** Document the removal of controllers installed outside
  `~/.local`, and keep the honest "no published v1.5 release" wording.
- [x] **Step 5:** Update the verification record, the plan and the physical
  validation note that referenced the removed script.
- [x] **Step 6:** Run `cargo fmt --check`, `cargo check --all-targets
  --locked --offline`, `cargo build --release --locked --offline`,
  `omarchy plugin validate .`, the QML parse and lint, and `git diff --check`.

## Not covered

No test suite was run for this cleanup. The QML service and panel were not
started, no polkit, install or uninstall flow was exercised, and no HID
report was sent to the keyboard. The runtime behaviour of the Signal Engine
still needs live validation, as recorded in
[docs/verification.md](../verification.md).

---

### Task 4: Live session and Windows app parity

**Files:**
- Create: `docs/windows-app-parity.md`
- Modify: `src/commands.rs`, `src/cli.rs`, `Panel.qml`, `README.md`,
  `docs/verification.md`, `preview.png`

- [x] **Step 1:** Install the plugin into a live Omarchy session and open the
  panel against the connected keyboard.
- [x] **Step 2:** Record what the session proves: the panel reads the device,
  the service arbitrates rules, and the Blackbox separates the `panel` and
  `signal` origins.
- [x] **Step 3:** Capture the panel as the root `preview.png` for the
  marketplace.
- [x] **Step 4:** Inventory the original Windows tool from its shipped
  resource files and compare it with the live controller.
- [x] **Step 5:** Close the one difference that belongs to a lighting
  controller: `restore --factory`, plus the matching panel action.
- [x] **Step 6:** Record the remaining differences and why they are out of
  scope on Linux.

The idle rule, the reconnect recovery, the pause expiry and the setup and
removal flows still need live validation; they are listed as pending in
[docs/verification.md](../verification.md).
