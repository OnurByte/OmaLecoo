# Windows app ↔ OmaLecoo feature parity

**Question:** does the original Lecoo GK301 Windows tool still have features
that OmaLecoo lacks?

**Answer:** the three open differences are handled, but two of them are
**adaptations, not equivalents**, and one control is unconfirmed. The table
below says which is which.

| # | Windows app feature | Status | Why |
| --- | --- | --- | --- |
| 1 | **Macro editor** — per-key macro assignment with delays, groups, import/export (`macro.xml`, `record.xml`, the keyboard picture on the Macro tab) | **Adapted — lighting macros, not key macros** | The recovered macro worker calls Windows `keybd_event`: it plays keys on the Windows host, it does not program the keyboard. There is no on-device macro protocol to port, and on Linux the compositor owns keyboard input. OmaLecoo's macros are a named, timed sequence of lighting commands, built in the panel, played on the keyboard, importable and exportable as JSON. The original's *key* macros are **not** reproduced. |
| 2 | **Function/media key programming** — the `funkey` and `combkey` lists in `config.xml` (Volume, Mute, Media keys, Ctrl+C, Win+D, launching apps) | **Adapted — a key map plus a Hyprland fragment** | Those mappings were application-level and backed by the Windows filter driver. On Linux the compositor owns them. The plugin keeps the original's key names and codes, lets you pick the command per key, and writes a Hyprland fragment you include from your own config. The original's *combination* list (`ctrl+c`, `win+d`, …) is **not** reproduced, because on Linux those combinations are ordinary shortcuts the desktop already has. |
| 3 | **Settings → Restore (factory defaults)** | **Equivalent** | `lecoo-gk301 restore --factory` applies the same default record the Windows tool starts with (Steady, brightness 5, speed 2, first option) and makes it the new manual state. The panel exposes it as *Restore factory lighting defaults*. |

One control from the original has a **settled, narrower behavior** on this
unit: the **M1/M2/M3 loader**. Raw sweeps on 20 October 2026 lit the
special-key group only for the third loader value; the first two render
dark at every tested combination, and the original sends the same values
with the same result. The gaming path applies M3 and the panel keeps no
loader row. See [lighting-zone-capture.md](captures/lighting-zone-capture.md).

Everything else in the Windows tool is either matched, exceeded, or excluded
with evidence.

## Lighting controls

Source of truth: the packaged `dlg_main.xml` (SOUI definition) and the
disassembly notes in [custom-color-investigation.md](custom-color-investigation.md).

| Control | Windows app | OmaLecoo | Result |
| --- | --- | --- | --- |
| Panel color effects | 20 effects, `Steady` … `Sine wave` | the same 20, same order (`lecoo-gk301 catalog --json`), shown in the panel under the original names | **Equal** |
| Speed | 0–2 | 0–2 | **Equal** |
| Brightness | 0–5 in the UI (the stored record carries 13) | 0–13 | **Plugin exceeds it** — the clamp was application-side, and the user confirmed 5 vs 13 on the device ([experiment](captures/extended-brightness-experiment.md)) |
| Gaming-key selection | M1/M2 (M3 radio is zero-sized) | the gaming path applies the third loader value (M3) | **Equal in behavior** — on this unit only the third value lights the special-key group; the tool's M1/M2 render dark the same way |
| Direction (Left/Right) | A radio on the first settings page | not offered | **No device-level difference** — the Windows normal writer overwrites the direction byte with the selection byte and never transmits it (`0x41ff71..0x41ff93`). The app shows a control the device never receives. |
| M1/M2/M3 loader | Three values, offered for the gaming-key effect | M3 applied; no loader row | The raw sweeps settled it: wire loader 0 and 1 leave the board dark at every tested brightness and encoding, wire 2 lights the current FN-zone group, and values above 2 blink and fall back. It is not a zone selector, and the panel does not claim it is. |
| Per-effect control pages | Steady: brightness only · Gaming key: brightness + M1/M2 · Effects 2–19: speed + brightness | one uniform record: effect + brightness + speed + selection | **Plugin is a superset** — it can set every field for every effect; the Windows app hides fields per effect. |
| Side LED page | 12 side effects, brightness 0–7, on the hidden Mouse page | not offered | **Correctly excluded** — the side writer produced no physical change on this keyboard ([failure analysis](side-failure-analysis.md)) |
| Custom color / per-key RGB | `color.xml` is the SOUI UI palette, not a lighting control | not offered | **Correctly excluded** — no RGB-triplet or per-key sender exists in the recovered paths ([investigation](custom-color-investigation.md)) |

## Application features

| Windows app feature | OmaLecoo | Result |
| --- | --- | --- |
| Profile selector P1/P2/P3 (`cbx_in_lv`, hidden) | Software presets: silent, game, night, party, off, random | **Replaced** — profiles were removed from the tool in 1.4.0; presets compose the same packets |
| Tray speed readout (k/s) | — | **Not applicable** — a gaming-mode performance meter, not a keyboard control |
| "Upgrade" menu item (commented out) and the `DPInst` driver installer | — | **Not applicable** — firmware flashing and a Windows filter driver have no Linux counterpart; the plugin uses `hidraw` and needs no driver |
| Headset page | empty in the shipped XML | — |
| Macro group editor, per-key picture, import/export | — | **Difference 1** above |
| Function/media key mapping | — | **Difference 2** above |

## What OmaLecoo adds on top

These have no counterpart in the Windows tool:

- **Signal Engine** — the keyboard follows the desktop: fullscreen selects
  Game, five idle minutes select off, and a manual action always wins for
  30 minutes. Verified live on 20 October 2026; see
  [verification.md](verification.md).
- **HID Blackbox** — a local, bounded history of every vendor-HID exchange
  with UTC time, origin, report bytes and syscall outcome.
- **Ghost Mode** — a disconnected or permission-denied keyboard shows a
  muted, cached state instead of an error, with optional one-time recovery on
  reconnect.
- **Lighting macros** — timed lighting sequences, which the Windows tool
  could not do on the device side.
- **A function-key map with a generated Hyprland fragment**, which is the
  Linux shape of the original's key configuration.
- **Omarchy-native operation** — a bar widget, a seven-page panel, in-panel
  setup and in-panel removal, with no second process and no daemon.
- **A CLI and a JSON contract** — every panel action is reproducible and
  scriptable from the shell.
- **Published evidence** — the captures, disassembly notes and negative
  results that justify every boundary above.

## Method

- The Windows inventory comes from the shipped resource files under
  `reference/windows/` (`dlg_main.xml`, `config.xml`, `macro.xml`,
  `ledeffect.xml`, `sidemodel.xml`, `color.xml`, `positions.xml`,
  `record.xml`, `menu_more.xml`, `dlg_tray.xml`, `readme.txt`) read against
  the recovered disassembly records in [rea-findings.md](rea-findings.md),
  [custom-color-investigation.md](custom-color-investigation.md) and
  [side-failure-analysis.md](side-failure-analysis.md).
- The OmaLecoo inventory comes from the live controller (`lecoo-gk301
  capabilities --json`, `catalog --json`, `--help`), the panel and service
  sources, and the runtime records in the local Blackbox.
- No Windows binary was executed and no firmware was modified. The Windows
  tool was never run on this machine, so the comparison is a source-level
  inventory plus the device evidence already recorded in `docs/`.
