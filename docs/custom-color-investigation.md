# Custom color and capability investigation — 2026-10-19

## Boundary and evidence

Read-only static inspection of `reference/windows/ShinetekTools.exe`, SHA-256 `89a0c7792ceff1adde158e7ef28f08ca7fa2d6945d065c7f566434e96b54ac88`. No Windows application execution, firmware modification, or hardware writes.

REA opened and identified the artifact. REA GNU objdump disassembly completed and retained Evidence `ev_d7c97e5d45eb13f7b6b838d17b276c105a9373cbc860797875a0fd003c1ab82d`. Its MCP response exceeded transport size; the complete record was exported to `rea-color-evidence.json` (35,638,959 bytes). `inspect_analysis_view` cannot page this particular operation. Focused human-readable excerpts in `custom-color-disassembly.txt` were independently generated with local GNU objdump using Intel syntax. The latter are direct file observations, not a separate REA evidence record. The logical REA target was closed; no other Hopper session was touched.

## Result

Arbitrary RGB and individual-key RGB are **unproven**. The recovered Windows lighting writer carries effect, brightness, speed, and selection values, without an RGB triplet. This is a bounded negative result for this executable's recovered direct paths, not proof that the firmware or physical LEDs cannot support another protocol. A working custom RGB control must not be claimed on this evidence.

## Proven model and packet mapping

`0x41ab20` serializes 20 four-byte records. String references establish record byte 0 = `brightness` (`0x41abf7`), byte 1 = `speed` (`0x41abd9`), byte 2 = misspelled `deriction` (`0x41ac16`), byte 3 = `selection` (`0x41ac35`). Loader at `0x4293e0..0x42952f` reads those same attributes; its local brightness limit is 13, whereas the ordinary sender clamps its wire brightness to 5. Local XML limits must not be mistaken for firmware capability.

Normal panel writer `0x41feb0` sends:

```
07 FF FF effect min(brightness,5) speed selection_wire 00
```

It initially copies all four bytes but overwrites the third payload byte (direction) with selection at `0x41ff71..0x41ff93`, and overwrites the last payload byte with zero at `0x41ff9d`. Selections 1,2,3 map to wire 0,1,2. Other selection values pass through. Thus this normal wire path does not transmit the stored direction at all. Do not advertise a verified direction control merely because XML has a direction attribute.

The alternate callback `0x4206f0` sends `07 FF FF index` followed by the four stored record bytes unchanged (`0x42075a..0x42077e`). Unlike the ordinary sender it neither clamps brightness nor remaps selection. Its callers/activation semantics have not been established. It is not an RGB packet merely because four opaque bytes can be copied.

Side writer `0x41ffd0` uses `07 FF FF 20 00 00 packed 00`, preserving the packed high nibble and replacing its low nibble with an effect in 0..11.

## Original application's effect controls

The panel refresh/update routine `0x41f650` uses a 20-entry byte dispatch table at `0x41f91c`: `00 01` followed by 18 `02` values. Cases are:

| Zero-based effect | Original settings page | Visible controls in dlg_main.xml |
| --- | --- | --- |
| 0 | 2 (`0x41f70d`, page pushed at `0x41f713`) | Brightness only |
| 1 | 3 (`0x41f731`, page pushed at `0x41f737`) | Brightness and M1/M2 selection; speed and M3 zero-sized |
| 2..19 | 1 (`0x41f783`, page pushed at `0x41f789`) | Speed and brightness |

The reverse update branch uses the analogous table at `0x41f93c`, with the same grouping. Page 0 exists in XML (speed, brightness, direction), but no case in this recovered switch selects it. These are proven Windows UI capabilities; they do not prove every firmware effect responds identically.

## Feature/output transport coverage

All direct calls to eight-byte wrapper `0x404cb0` found in the complete .text disassembly:

- `0x41d32c`: startup firmware query `07 F1 00 00 00 00 00 00`, followed by `0x404be0` GetFeature.
- `0x41ffa1`: normal panel writer.
- `0x420084`: side writer.
- `0x420106`: repeated firmware query, followed by GetFeature.
- `0x42077e`: alternate stored-effect writer.

Direct calls to HidD_SetFeature's IAT slot `0x64a0d8` occur at `0x40326f` (generic argument wrapper), `0x404d48` (8-byte wrapper), and `0x405096` (generic variable-length wrapper). There is also the imported jump thunk at `0x4b946c`. No direct call to entry `0x403260` was found. The variable-length routine appears to begin at `0x404fe0`; no direct call to that exact entry was found. Indirect calls remain unresolved, so these absent calls do not establish dead code or whole-program completeness.

Reset is now traced beyond the previous uncertainty: `0x41fe86` calls `0x4049f0`, which sets length 8 and invokes `0x4032a0` at `0x404a90`; that transport calls WriteFile (`0x64a2e4`) at `0x403318`/`0x403404`. Reset therefore uses an output write path, not the normal SetFeature wrapper. Do not change its transport to Feature based on matching packet length.

## color.xml and key programming

`reference/windows/color.xml` contains named UI colors red/green/blue/white/gray. UTF-16 and ASCII exact filename searches in the executable did not find `color.xml`; `\\ledeffect.xml` does exist at `0x6955e8`, referenced at `0x42b3cc`. A generic `rgb(` string exists in UI parsing data. These observations alone do not connect color.xml to a keyboard sender. The color file is not evidence of writable LED RGB channels.

`config.xml` exposes Windows virtual-key shortcuts, shell launch paths, and combinations. These declarations establish application configuration, not on-device macro or remapping packets. No additional proven key-programming writer was recovered in this pass. Host-side remapping would require a separate explicitly implemented input service and should be labeled as such.

## Remaining work

- Resolve alternate effect callback and generic variable-length sender indirect callers with a working deep disassembler session.
- Observe authorized Windows application USB traffic for a specific operation to corroborate static paths, if a usable Windows runtime is available.
- Determine LED/controller physical RGB capability or obtain matching firmware/protocol evidence before introducing raw color writes.
- Keep Linux panel claims limited to recovered effect controls and distinguish sent/accepted command status from visibly observed lighting.
