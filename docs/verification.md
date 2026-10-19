# Development verification — 1.5.0

Version `1.5.0` identifies the Signal Engine development line in the Cargo
package and plugin manifest. This document records implementation checks and
historical release notes; it does not announce publication of v1.5.0.

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
parser in `src/main.rs`; setup/removal arguments were compared to `Panel.qml`,
and service registration/version to `manifest.json`. This is static inspection,
not execution of installation, removal or HID commands.

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
uninstall or system mutation was performed during these tasks. Fullscreen,
idle, reconnect, runtime launch failures and setup/removal remain pending
live validation. Historical test results below do not validate these features.

## Historical — 1.4.1 (marketplace layout)

- Moved `manifest.json`, `BarWidget.qml` and `Panel.qml` to the repository root
  so the Git clone is a plugin folder. Added `license: "MIT"` and version
  `1.4.1` to the manifest.
- `install.sh` built from source and skipped the plugin-copy step when run
  from an Omarchy plugin checkout.
- README documented the marketplace install command, separate Cargo/prebuilt
  controller options and brightness 0–13. The earlier release record states
  that v1.4.1 was published with an x86_64 binary and `checksums.txt`; no v1.5
  asset is inferred from that history.

## Historical — 1.4.0 (profiles removed; Rust tooling)

- Removed lighting profiles from the CLI and panel; `state.json` retained the
  last applied settings. Profile JSON was no longer read or written.
- Replaced Python offline integration and transport harnesses with Rust tests,
  preserving the existing C interception shim. The brightness probe became
  the hidden `lecoo-gk301 probe-brightness` diagnostic.
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
