# Side lighting failure audit — 2026-10-19

## Conclusion

The current Rust side command matches the recovered ordinary Windows side command byte-for-byte for valid inputs. No evidence supports changing its brightness shift, subtype, report ID, or Feature transport. The actual cause of physically unchanged side lighting is unresolved; successful Linux ioctl `rc=8` establishes transport acceptance, not firmware execution or visible effect. Color names attached to side effect IDs require physical corroboration and should not be presented as verified.

This is read-only analysis of the existing REA disassembly evidence (`ev_d7c97e5d45eb13f7b6b838d17b276c105a9373cbc860797875a0fd003c1ab82d`, `rea-color-evidence.json`) with focused local Intel-syntax objdump excerpts and current `src/main.rs`. No app execution or keyboard writes.

## Exact side encoding

- Windows UI `bar_side_brightness` declares integer range **0..7** (`reference/windows/dlg_main.xml`). At `0x41f5e8` the slider byte is read, shifted left **4** at `0x41f5f0`, combined with the old byte's low nibble at `0x41f5f3..0x41f5f8`, and stored. No scale, inversion, addition, or nonlinear lookup occurs.
- Windows side sender `0x41ffd0` admits effect **0..11**. At `0x420060..0x42006d` the expression is exactly `(stored & 0xf0) | (effect & 0x0f)` (implemented with XOR/AND/XOR).
- `0x42007a` writes DWORD **0x20ffff07**, whose little-endian bytes are `07 FF FF 20`. Subtype is therefore **hex 0x20 = decimal 32**, not decimal 20.
- `0x42006f` zeroes bytes 4/5, `0x420081` stores packed value at byte6, `0x420073` zeroes byte7. Complete packet:

```
07 FF FF 20 00 00 ((brightness << 4) | effect) 00
```

- Rust `send_side` computes `((side_brightness & 7) << 4) | (side_effect & 15)` and sends exactly those bytes; validation limits brightness0..7/effect0..11. Its bit7 is always zero, matching the Windows slider range. No side direction or RGB triplet appears in this path.
- Packaged `sidemodel.xml` stores per-effect brightness, defaults4 for first3 effects and7 for remaining9. Rust stores one common brightness. That persistence difference cannot explain complete failure with brightness7 across effects; the actual packet still matches a valid Windows packet.

## Transport and handle

Both side (`0x420084`) and panel (`0x41ffa1`) call the same routine `0x404cb0`. It takes object+`0x60` as handle (`0x404d36..0x404d39`), pushes length8, buffer, handle (`0x404d44..0x404d47`) and calls **HidD_SetFeature** at `0x404d48`, IAT`0x64a0d8`. There is no side-only second handle or output transport in these paths.

Windows initialization passes the enumerated device path to `0x4042e0` at `0x41d243`, with the same HID object offset+`0x3a8` later passed by senders. In `0x4042e0`, object+`0x60` is assigned the result of CreateFile on that path at `0x4043e6..0x4043e8`. It separately opens object+`0x48` on the **same path** at `0x404460..0x404462`. Thus Feature and output/read handles are access-mode variants of one enumerated device path, not separate side/panel devices.

`config.xml` declares VID1a2c, PID9df4, interface1, subdevice7. Current Rust detection matches VID/PID and HID_PHYS ending `/input1`; it does not parse the HID collection/report descriptor. The exact meaning and enumeration predicate for Windows `subdevice=7` has not been recovered here, so complete equivalence of collection selection is still unknown. A responding firmware query on the chosen Linux node reduces, but does not logically eliminate, this concern. Linux hidraw groups reports by HID interface rather than necessarily mirroring Windows per-collection device paths.

## Panel comparison

Rust `send_panel` also matches normal Windows packet layout:

```
07 FF FF effect brightness speed selection_wire 00
```

Windows normal path clamps brightness to5 and maps stored selections1/2/3 to0/1/2. Rust already uses brightness0..5 and wire selection0..2, so it should not subtract selection again. Windows stored direction is overwritten by selection in normal sender; no direction byte is transmitted.

Windows UI only exposes brightness for effect0; brightness+M1/M2 for effect1; speed+brightness for2..19. Effects without a displayed control must not be assumed to implement it merely because the Linux packet contains a value.

## Best next discriminating evidence

1. Compare an operation that visibly works in the original Windows software against captured USB traffic, including exact setup fields and interface number. Static similarity alone cannot prove live firmware equivalence.
2. Record whether any side preset changes **physical** LEDs; keep this distinct from report transmission logs. A panel screenshot is not evidence of keyboard lighting.
3. Preserve report descriptor/interface identity and firmware version alongside physical results. If only panel commands work, a firmware/model side-capability mismatch or wrong side effect semantics remains plausible. Neither is proven here.
4. Do not cycle undocumented raw payloads or switch normal side commands to output reports to make `rc=8` look different. No recovered evidence supports that change.

## Bounded startup follow-up: short flicker and return

User physically observed no sustained response from both Feature and Output variants. A same-open-handle sequence F1 SetFeature/GetFeature, then panel0/brightness5 and side3/brightness7 with150ms delays, caused only a short flicker and a return to prior lighting. This is evidence against calling the protocol verified or a handshake fixed.

Additional static finding: startup does enqueue a **delayed alternate panel apply** after the F1 query and UI initialization. At `0x41d98b` the callback address `0x4206f0` is loaded, and at `0x41d9c1` it is queued through `0x421110` with selected effect. Queue constructor writes delay`0x7d0` (2000ms) at `0x421170`. Worker `0x421b90` removes the item from the queue, sleeps its delay at `0x421c76..0x421c7b`, then invokes the callback at `0x421c96`. This callback sends the already documented raw four-byte stored effect record (brightness/speed/direction/selection), not the normalized ordinary UI report. This is a concrete startup difference, but has **not** been physically validated and is **not** a demonstrated enable/unlock/keepalive command.

A second startup item uses callback`0x4207a0` queued at`0x41da03`; that callback only ensures side state allocation and returns. No side hardware write occurs there.

Within the directly inspected startup path from F1 query through this queue setup, no additional explicit enable/lock/apply opcode or extra Feature write was recovered. Generic wrappers and indirect callbacks prevent a whole-program absence claim. The identified startup callback is queued once; its presence does not prove a periodic resend or that keeping a Linux fd open repairs persistence. Original application's persistent HID handles and asynchronous worker differ architecturally from one-shot CLI calls, but observed same-handle failure already shows fd lifetime alone is insufficient.

## Further pass after main effects physically confirmed

Latest user evidence supersedes the earlier broad failure: main effect selector now visibly changes keyboard LEDs; side commands still have no physical result. Main-effect transport is therefore physically corroborated in that tested path, while side support remains unresolved.

New semantic correction: the **same side high-nibble parameter is speed for effects0..2, brightness for3..11**. `0x41f492..0x41f4c1` compares selected side effect with2, selects UTF-16 label`Speed` at`0x694918` for<=2, and `Brightness` at`0x694924` otherwise. Thus the field name side_brightness overstates its meaning for first three effects. This does not explain failure of side3/parameter7, which is still the recovered brightness packet.

Side event ordering is explicit: side selector event calls state save`0x41c310`, UI refresh`0x41f450`, then sender`0x41ffd0` (`0x41f3a9..0x41f3ba`). Slider callback`0x41f410` calls update`0x41f450` with update-model flag, reads selected effect, then calls same sender at`0x41f446`. The update routine shifts the raw slider value4 bits. Neither path has an additional enable command or firmware-version guard. The only recovered firmware guard compares version with`0x100` at`0x41e450`, in a different reopen/configuration action; it is not inside the two side send event paths. Firmware-dependent whole-app behavior remains incompletely traced.

`0x4207a0` is fully bounded by next aligned routine`0x4207e0`: it allocates/initializes the singleton when absent and returns; there is no subsequent hardware call hidden in that callback. No second side report writer was recovered in direct feature wrapper call coverage. There is consequently **no additional evidence-backed raw side command to recommend** from this pass.

New macro/remapping evidence: background worker callback`0x420580` drains host key-event records and invokes imported **keybd_event** at`0x420621`, followed by each record's delay via Sleep at`0x42062b`; special code`0x2b67` (=11111, config Lock window) takes a separate OS path. This is direct evidence of Windows **host-side** macro execution, not onboard macro programming. Linux host macros could be separately implemented using authorized input/uinput handling, but no recovered device upload protocol justifies advertising onboard macro storage.

## Capability boundary correction: hidden Mouse page

The XML tree places Side LED outside the Keyboard page: outer `tabctrl2` defaults to`curSel=0`; first`page2 tip=Keyboard` contains Macro/Panel LED/Settings. Second`page2 tip=Mouse` contains the Side LED page with`visible=0` (`dlg_main.xml:125..126`). This is positive evidence that these side controls belong to a hidden generic/mouse branch, **not evidence of supported GK301 keyboard side lighting**. The previous inference from shared executable sender to keyboard side capability was too broad. Physical main-success/side-failure is consistent with this boundary, although exact firmware rejection behavior remains unknown.

Code initialization resolves named side combobox at`0x41cc62` and`0x41cc8d`; binding an existing hidden control does not prove it is shown or supported. No model-specific selection/show of the Mouse page was established in this bounded pass. The shared executable's side packet should remain a historical/research observation and should not be exposed as verified keyboard capability.
