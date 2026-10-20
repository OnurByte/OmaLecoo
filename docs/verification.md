# Development verification — 1.5.0

Version `1.5.0` identifies the Signal Engine development line in the Cargo
package and plugin manifest. This document records implementation checks and
historical release notes; it does not announce publication of v1.5.0.

## The FN lighting zones and the loader byte — 20 October 2026
The keyboard lights a different key group for each of its own FN+1 … FN+9
keys. A live usbmon capture (control transfers only, raw file deleted right
after filtering) and a REA pass over the original Windows tool settled what
the host can and cannot set. The record and the reasoning are in
[docs/captures/lighting-zone-capture.md](captures/lighting-zone-capture.md).

- The report that reaches the device is `07 FF | effect | brightness | speed
  | loader | 00` as a Feature report, ID 7, interface 1 — confirmed on the
  wire, not only by the ioctl return value.
- REA confirms the loader mapping: the stored values 1, 2 and 3 become the
  wire bytes 0, 1 and 2, and the stored direction byte is never transmitted.
- The **loader is not the FN zone**. Changing the loader does not move the lit
  key group; a loader value above 2 makes the keyboard blink and fall back.
- The **FN zone lives in the keyboard**: after the user pressed FN+2, the
  plugin's `Gaming Special Key` effect lit the FN+2 group (TAB, SHIFT, CTRL,
  QWER, HOME, PRTSC, SCRLK, PGUP, END, PGDN). The host can enter the
  special-key lighting but cannot choose the group.

The plugin therefore keeps the loader at M1/M2/M3, exactly the three values
the original tool exposed, and the `game` preset and the fullscreen rule
apply `Gaming Special Key` so they light whichever group the keyboard is
in.

Two further results from the same session:

- The **loader's visible effect is now settled**. Repeated raw-feature sweeps
  on 20 October 2026 lit the special-key group only with wire loader value 2
  (M3): values 0 and 1 left the board dark at every tested brightness (0.05,
  0x08, 0x09) and encoding, byte 7 of the alternate writer's raw form changed
  nothing, and brightness 0x0D rendered no light on the gaming path while the
  19 October experiment had already confirmed 13 for the Steady effect. The
  panel applies M3 for the gaming path and offers no loader control of its
  own; the CLI keeps `--select` as a stored value for tool parity.
- The **original Windows tool's zone path is not settled by the static
  call graph**. Its `HidDevice::SetOutputReport` method is present in the
  binary (log tag `0x692cc0`, implementation `0x4050e0`) and the complete
  disassembly has five calls into the `SetFeature` wrapper and none into it,
  but the class also has a `Write` method (`0x404a54`), and `WriteFile` on a
  HID handle is the other way an output report reaches the device. The same
  program was seen sending `05 01 00…` … `05 04 00…` output frames at
  start-up for a sibling model, so the output path is live; only a dynamic
  recording on this hardware would settle it. The vendor interface's report
  ID 7 does declare an Output item, so the channel exists.
- The **zone cannot be observed from the host**. A 100-second recording of
  the vendor interface (`/dev/hidraw1`) produced **zero** input reports,
  while the boot interface (`/dev/hidraw0`) produced 110 in the same window.
  The descriptor declares input reports 1–6 on that interface — a consumer
  report, a vendor report (page `0xFF00`, byte `0xF1`–`0xF8`) and three
  keyboard-state reports — and this firmware uses none of them. The FN
  combinations add no distinguishable report to the ordinary boot traffic.
  There is no channel to watch the zone over, so a host-side zone *reader*
  is not possible on this unit; only a host-side zone *writer* would be,
  and no command for one has been found.

The keyboard's ten lighting layouts are now their own model
(`src/zones.rs`, `lecoo-gk301 zones list`, and the panel's *Gaming zones*
section), separate from the loader: FN+1 FPS 1, FN+2 FPS 2, FN+3 COD, FN+4
RTS, FN+5 LOL, FN+6 Racing, FN+7 NBA, FN+8 LOL II, FN+9 Custom I, FN+0
Custom II.

The lighting write path was also tightened: a panel command now has to move
exactly one eight-byte report, and a short transfer is a failure, the same
rule the firmware query already used. A successful transfer still only means
the device accepted the report, never that the LEDs changed.

## Panel fixes and the missing features — 20 October 2026

**The bar and panel shifting.** The panel scrolled its content with a fixed
step and no bound, so a page shorter than the viewport could be pushed out of
view: the content slid up and left a gap. The scroll is now clamped to the
content, so a short page cannot move at all and a long page stops exactly at
its end. Two further sources of movement were removed: the speed row and the
loader row now keep their place for every effect instead of appearing and
disappearing, and the bar tooltip no longer appends a state that changes
while the pointer rests on it. The effect names in the panel come from the
controller's catalog now, so the panel shows the original tool's own names
(`Gaming Special Key`, `Marquee effect`, `Diagonal transformation`, …)
instead of a second, shorter list kept in the QML.

**Self-setup.** The marketplace review classified the listing's setup as
manual because the native controller and the udev rule required directed
steps. The first load now performs both: when the controller is missing it
builds from the checkout the shell cloned (`cargo install --path`,
`--target-dir ~/.cache/lecoo-gk301`) and starts with `omarchy pkg add rust`
if Cargo is absent, and while the keyboard is connected but read-only it
raises the one polkit prompt for the udev rule. Each step goes through its
own approval prompt, a failed chain stops and does not retry this session,
and the Setup page keeps the same steps as buttons.

**The missing features.** The three differences that the Windows app still
had are handled:

- `lecoo-gk301 restore --factory` applies the original tool's default record
  and makes it the new manual state.
- Lighting macros: a named, timed sequence of lighting states, stored in
  `macros.json`, built in the panel's Macros page, played with `macro play`,
  importable and exportable as JSON. The original recorded key sequences and
  replayed them on the Windows host; there is no on-device macro protocol, so
  the macros here are the device-side sequence the hardware can perform.
- Function keys: the GK301's function and media keys with the names and codes
  from the original `config.xml`, a command per key, and a generated Hyprland
  fragment (`keys export`) that the user includes from their own config.

Checks for this work: `cargo fmt --check`,
`cargo check --all-targets --locked --offline`, `cargo build --release
--locked --offline`, `omarchy plugin validate .`, the three-file QML parse,
the service-only `qmllint` and `git diff --check` all passed. A macro was
saved and played against the connected keyboard: the Blackbox recorded the
four expected panel reports for a two-step macro repeated twice. The macro
editor, the key map and the clamped scroll were exercised in a live Omarchy
session; the panel pages render and the key command save was confirmed in
the panel.

Still not live-validated: the idle rule, the reconnect recovery, the pause
expiry, and the setup and removal flows.

## Live session — 20 October 2026

The plugin was installed into a live Omarchy session (`~/.config/omarchy/plugins/borat.lecoo-rgb`,
controller built from this branch into `~/.local/bin/lecoo-gk301`) and
exercised against the connected GK301.

Observed and confirmed:

- The panel opens from the bar, reads the device, and shows firmware
  **1.16**, the 20-effect grid, brightness, speed, the Gaming M1/M2 selector,
  the presets and the auto-cycle control.
- The service runs inside the same Quickshell process and checks presence
  every ten seconds with `dev --json` — no HID report is sent by that check.
- Rule arbitration works on the real keyboard. The Blackbox recorded
  `origin: "signal"` writes in both directions: the default rule sent
  `07 FF FF 0D 05 02 00 00` (the saved manual record), and focusing a
  fullscreen window sent `07 FF FF 01 05 02 00 00` — the Game preset.
- Manual panel actions are tagged `origin: "panel"`, and the service is
  tagged `origin: "signal"`, so the Blackbox distinguishes the two writers.
- A signal-origin write did not replace the saved manual restore point: the
  service applied effect 1 while `state.json` kept the user's effect.
- The 30-minute manual pause is observable: after a manual panel action the
  service stopped writing, and the next rule transition only happened after
  the pause state was cleared by a fresh service start.

Not observed, so still pending:

- The **idle rule** needs five idle minutes; it was not reached in this
  session.
- The **reconnect recovery** needs the keyboard to be unplugged; it was not
  exercised.
- The **expiry of the 30-minute manual pause** was not waited out.
- The **setup and removal flows** (Cargo availability, the polkit
  authorization, the plugin removal) were not run in this session. The udev
  rule was already installed, and the device was writable.
- The **lock-screen rule** remains deferred by design: no stable public
  Omarchy lock-state signal is assumed.

The session was left with automation off, the same as before it started, and
the saved manual lighting state reapplied to the device.

`preview.png` at the repository root is a screenshot of that live session: the
Lighting page with the keyboard connected, the firmware read and the effect
grid. It is the image the marketplace uses. The project banner in the README
(`docs/assets/omalecoo-banner.png`, source `docs/assets/omalecoo-banner.svg`)
is a designed image, not a screenshot.

## Windows app parity — 20 October 2026

The original Windows tool was inventoried from its shipped resource files
(`dlg_main.xml`, `config.xml`, `macro.xml`, `ledeffect.xml`,
`sidemodel.xml`, `color.xml`, `positions.xml`, `record.xml`) against the
existing disassembly records, and compared with the live controller. The
result is in [windows-app-parity.md](windows-app-parity.md).

Three differences remain. The macro editor and the function/media key
programming are not lighting controls and have no portable device protocol
behind them. The third one, the Windows app's *Settings → Restore*, was
closed: `lecoo-gk301 restore --factory` now applies the same default record
the Windows tool starts with (Steady, brightness 5, speed 2, first option)
and makes it the new manual state. The panel exposes it as *Restore factory
lighting defaults*.

The Windows app's *Direction* control is not a difference in practice: its
own normal writer overwrites the direction byte with the selection byte and
never transmits it, so the device never receives a direction value.

## Rust-only cleanup — 20 October 2026

The plugin is now Rust and Quickshell only. No standalone C, JavaScript/Node
or Python implementation file remains in the repository.

| Removed | Replacement |
| --- | --- |
| `install.sh` | The panel's **Setup** page: `cargo install --git … --root "$HOME/.local" --locked` with a fixed argument list, then `capabilities --json` verification. |
| `scripts/install-udev.sh` | `lecoo-gk301 udev install` / `udev remove`, run through the panel's polkit action. |
| `scripts/probe-windows-startup.py` | The hidden `lecoo-gk301 probe-startup [--send]` diagnostic in `src/probe.rs`. Display-only by default; `--send` replays the recovered startup record. |
| `tests/ioctl_capture.c` | An in-process `HidTransport` substitution: `src/transport.rs` defines the trait, `tests/transport.rs` supplies the fixture. No `LD_PRELOAD`, no C compiler needed. |
| `tests/ui-state.cjs` | Removed. It asserted a retired `--panel-only` routing flag and a `worker.command` shape that the current panel no longer has, so it could not validate the present panel. The stale-response handling it covered lives in `Panel.qml` and still needs a live session to verify. |
| `udev/70-lecoo-gk301.rules` | The rule content now lives in `src/udev.rs` as the single source of truth. Two copies of the same rule could drift, and the install/remove path compares the file byte for byte before it touches it. |

The Rust controller was split into focused modules: `protocol` and `transport`
(the wire format), `device` (discovery and writes), `state` (saved settings
and rules), `blackbox` (command history), `lock` (the shared process lock),
`udev` (the owned rule), `cli` (argument helpers), `commands` (the command
implementations), `probe` (hidden diagnostics) and `logging`. `src/lib.rs` is
the library root; `src/main.rs` is now a thin binary that takes the lock and
dispatches.

The README gained a marketplace-facing landing page, an explicit
**What this plugin touches** table (one privileged file, one user binary, one
state directory) and a root `preview.png` — the image the marketplace uses
when it cannot find a supported preview. The banner is a project image, not a
screenshot of the panel; `docs/assets/omalecoo-banner.svg` is its source. The
root preview was later replaced with a real screenshot of a live session; see
the live-session section above.

Checks for this cleanup: `cargo fmt --check`,
`cargo check --all-targets --locked --offline`, `cargo build --release
--locked --offline`, `omarchy plugin validate .`, the three-file QML parse
and the service-only `qmllint` above, and `git diff --check` all passed. The
release binary was also started for its local, device-free commands
(`--help`, `capabilities --json`, `catalog --json`, `list`, `status --json`,
`rules --json`, `dev --json`) and for the rejection of retired commands
(`side`, `raw`, `reset`), with `XDG_STATE_HOME` pointed at a temporary
directory. No test suite was run, and no live panel, polkit, install or
uninstall flow was exercised.

As a parity check, the previous monolithic `src/main.rs` was built as a
separate crate and both binaries were run over the same command list. The
outputs were identical for every command, including the usage text, the JSON
reports, the retired-command rejections and the error strings. The only
differences were the newly added `probe-startup` command, the measured
firmware round-trip time, and the effect index of `next`/`prev`, which
advanced the saved state between the two runs. That comparison ran against
the connected keyboard, so it did send firmware queries and a few panel
commands to the device; the saved manual lighting state was restored
afterwards with `restore`.

## What changed in 1.5.0

- **Signal Engine:** a Quickshell service and bar widget in one plugin. Rules
  default to disabled; idle for five minutes selects `off`, active fullscreen
  selects `game`, and the default restores the saved manual settings. Manual
  panel/bar controls pause automation for 30 minutes. Events are coalesced for
  250 ms, with at least two seconds after a command finishes before the next
  service HID command. Rust retains its cross-process lock and 110 ms pacing.
- **Manual restore point:** automatic writes do not replace the user's saved
  manual state. Unsupported or malformed rules load all-disabled defaults.
- **HID Blackbox:** best-effort vendor-HID history with UTC time, origin,
  report bytes, syscall return/error and outcome. `blackbox.jsonl` rotates at
  1 MiB, retaining one previous file; CLI output is newest first and the panel
  displays the newest 40 entries. It does not record keyboard input or typed
  keys. Firmware replies are acknowledgements, not lighting readback.
- **Ghost Mode:** a non-HID presence/writability check every ten seconds,
  cached/disconnected presentation, distinct permission/controller failures,
  and an optional single restore on reconnect. Presence checks do not send
  reports; the service suppresses writes while absent or denied.
- **Explicit setup:** Cargo source installation into `~/.local`, followed by
  verification, and a separate polkit action for the exact owned GK301 udev
  rule. Cargo availability can be refreshed in Setup.
- **Removal choices:** second confirmation, optional controller/rule/state
  cleanup, then detached plugin removal. State and device access are preserved
  unless selected. Unknown configured binary paths are left in place.
- **README:** native Omarchy setup, product preview instructions, capability
  boundaries, packet map, maintenance commands and direct evidence links.

Automation and service-driven reconnect recovery require Omarchy's built-in
bar. Automatic/recovery writes are gated on a live built-in widget attachment;
detaching the last widget cancels pending recovery. No stable public lock-state
signal is assumed. The new software features
do not establish custom RGB, per-key RGB, on-device macros or lighting-state
readback. See [hardware conclusions](FINAL-ANALYSIS.md) and the
[extended-brightness observation](captures/extended-brightness-experiment.md)
for the existing physical evidence.

## Checks to run from the project directory

These are non-test development checks. Listing a command is not a claim that
it passed on a particular revision:

```bash
cargo fmt --check
cargo check --all-targets
cargo build --release
omarchy plugin validate .
git diff --check
```

With the Qt tools available at these paths, the QML parse and service lint
checks used during implementation are:

```bash
/usr/lib/qt6/bin/qmlformat SignalService.qml Panel.qml BarWidget.qml >/dev/null
/usr/lib/qt6/bin/qmllint SignalService.qml
```

`qmlformat` without its in-place option parses and formats to stdout; the
redirection discards that output and leaves the files untouched. Bare
`qmllint Panel.qml BarWidget.qml` cannot resolve the host's `qs.Ui` and
`qs.Commons` runtime namespaces here, so its unresolved-type warnings must not
be reported as successful semantic validation. Review those files with the
installed Omarchy host contract and verify them in a live shell session.

The CLI examples in the [README](../README.md#cli) were compared to the command
parser in `src/cli.rs` and `src/commands.rs`; setup/removal arguments were
compared to `Panel.qml`, and service registration/version to `manifest.json`.
This is static inspection, not execution of installation, removal or HID
commands.

## Recorded checks — 19 October 2026

- Rust controller implementation (`e12366e`): `cargo fmt --check` and
  `cargo check --all-targets` passed. These results precede the subsequent
  manual-state persistence adjustment; they are not a full final-tree compile
  result.
- QML/lifecycle implementation (`f177269`): `omarchy plugin validate .`, the
  three-file QML parse command above, service-only `qmllint`,
  `cargo fmt --check`, and `git diff --check` passed. Static inspection covered
  CLI argument arrays, rule priorities, pacing, conservative defaults,
  service absence, cleanup order and removal confirmation.
- Launch-failure/Cargo refresh correction (`d2622a6`): plugin validation,
  parsing the two changed QML files (`SignalService.qml` and `Panel.qml`),
  service-only lint, and `git diff --check` passed. Static review traced failed
  process launch through presence/rules completion and panel busy-state
  cleanup, and checked automatic/manual Cargo availability refresh.
- README/verification update: `git diff --check` passed; commands, paths,
  feature claims and evidence links were inspected against the repository.
- Final integration correction (based on `5a2590b`): `cargo fmt --check`,
  `cargo check --all-targets --locked --offline`, three-file QML parsing, service-only
  lint, plugin validation and `git diff --check` passed. Checkout installation
  includes the service. Static review traced pending reloads and stale process
  results after controller changes, built-in widget attachment/removal, and a
  reconnect that becomes writable later. Blackbox output now validates each
  JSON object with `serde_json` and skips malformed entries. These checks do
  not verify the asynchronous flows or hardware effects at runtime.

No test suite was added or run for this v1.5 implementation. No live QML
session, new physical GK301 validation, source installation, polkit action,
uninstall or system mutation was performed during the implementation tasks.
The 20 October 2026 live session covered the panel, the service and the rule
arbitration; see the live-session section above for what it did and did not
cover. Historical test results below do not validate these features.

## Historical — 1.4.1 (marketplace layout)

- Moved `manifest.json`, `BarWidget.qml` and `Panel.qml` to the repository root
  so the Git clone is a plugin folder. Added `license: "MIT"` and version
  `1.4.1` to the manifest.
- `install.sh` built from source and skipped the plugin-copy step when run
  from an Omarchy plugin checkout. That script was removed in the Rust-only
  cleanup of 20 October 2026; the panel's Setup page took over the role.
- README documented the marketplace install command, separate Cargo/prebuilt
  controller options and brightness 0–13. The earlier release record states
  that v1.4.1 was published with an x86_64 binary and `checksums.txt`; no v1.5
  asset is inferred from that history.

## Historical — 1.4.0 (profiles removed; Rust tooling)

- Removed lighting profiles from the CLI and panel; `state.json` retained the
  last applied settings. Profile JSON was no longer read or written.
- Replaced Python offline integration and transport harnesses with Rust tests,
  preserving the existing C interception shim. The brightness probe became
  the hidden `lecoo-gk301 probe-brightness` diagnostic. The C shim itself was
  replaced by the in-process transport substitution in the Rust-only cleanup
  of 20 October 2026.
- Fixed brightness text to `/13` and catalog `panel_brightness_max` to 13.

## Historical — 1.3.0 (extended brightness)

- Extended the CLI, panel and capabilities to brightness 0–13 after the user
  confirmed a visible difference between 5 and 13 on the physical keyboard.
  The Windows UI's 0–5 clamp was application-side. Evidence:
  [extended-brightness experiment](captures/extended-brightness-experiment.md).

## Historical — 1.2.0 (OmaLecoo identity)

- Renamed the project to OmaLecoo. Cargo crate `omalecoo`, binary
  `lecoo-gk301` and plugin id `borat.lecoo-rgb` preserved the existing controller
  and state paths.
- Converted remaining product documentation to English and removed the
  Turkish-named sample profile; no default profile was seeded.
- Recorded the Windows startup-apply chain, vendor descriptor channels,
  startup-record replay and intermediate-brightness sweep. That phase observed
  accepted transfers and no vendor input reports; visual confirmation of the
  extended scale was recorded later for 1.3.0.

The historical 19 October 2026 record for 1.2.0 reports that Rust tests,
all-target Clippy with warnings denied, release build, UI state tests, offline
CLI integration, transport interception and installer shell syntax passed.
It also records an English-text scan, an already-installed matching udev rule,
and no installed capture daemon. These checks were not rerun for v1.5.

## Historical — 1.0.0

The original release record reports five Rust tests, all-target Clippy,
release build, UI state tests, offline integration, transport interception,
installer shell syntax and manifest JSON validation passing. QML lint had no
errors, with host dynamic-property warnings. An English-text scan also passed.

That version was installed and its live panel was visually checked. Doctor
returned firmware 1.16. The then-current profile, search and effect controls
rendered with native styling; no relevant QML runtime errors were found.
User-created profile names and historical logs were preserved. No dumpcap
process remained running. Profiles have since been removed from the product.

Readback sequence and REA evidence: [FINAL-ANALYSIS.md](FINAL-ANALYSIS.md).
