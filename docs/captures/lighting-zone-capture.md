# The loader byte, the FN lighting zones, and what the host can set

2026-10-20. Device VID:PID 1a2c:9df4, USB bus 8, address 1, hidraw5 =
vendor interface 1. Keyboard firmware 1.16.

## Question

The GK301 lights a different group of keys for each of its own **FN+1 … FN+9**
keys: FN+1 lights ESC, WASD and the arrow keys; FN+2 lights TAB, SHIFT, CTRL,
QWER, HOME, PRTSC, SCRLK, PGUP, END and PGDN. The plugin should be able to
produce that lighting. Which protocol field carries it?

## Capture

Wireshark CLI 4.7.4 with the Linux usbmon module, dumpcap on `usbmon8`. The
raw record was filtered to this device's **control** transfers only and the
unfiltered file was deleted immediately, so no keyboard input report is
retained. Control-only record: `loader-selection-capture.pcapng`.

| Frame | Time | Transfer | Data |
| --- | --- | --- | --- |
| 7 | 17.278807 | SET_REPORT, wValue 0x0307, wIndex 1, wLength 8 | `07 ff ff 01 08 02 00 00` |
| 9 | 19.396630 | SET_REPORT, wValue 0x0307, wIndex 1, wLength 8 | `07 ff ff 01 08 02 01 00` |
| 11 | 21.513836 | SET_REPORT, wValue 0x0307, wIndex 1, wLength 8 | `07 ff ff 01 08 02 02 00` |
| 13 | 23.630690 | SET_REPORT, wValue 0x0307, wIndex 1, wLength 8 | `07 ff ff 01 08 02 02 00` |

`wValue 0x0307` is report ID 7, report type Feature; `wIndex 1` is the vendor
interface. The three commands were the same panel record with the loader byte
0, 1 and 2, and the fourth re-applied the saved state. The device answered
every transfer with a status response and **sent no data back** — there is no
lighting acknowledgement to read.

A second 50-second record captured the user pressing FN+1, FN+2 and FN+3 on
the keyboard produced **no control transfer at all**: the zone change is
local to the keyboard and never reaches the host.

## REA

REA 6.7.0 (Hopper) on `reference/windows/ShinetekTools.exe`, SHA-256
`89a0c7792ceff1adde158e7ef28f08ca7fa2d6945d065c7f566434e96b54ac88`. The
normal panel writer at `0x41feb0` reads the stored record's selection byte and
maps it onto the wire:

```
0x41ff71: mov al, byte [esi+3]      ; stored record byte 3 = "selection"
0x41ff74: mov byte [ebp+var_6], al  ; wire byte 5 (the loader)
0x41ff77: cmp al, 0x1
0x41ff7b: mov byte [ebp+var_6], 0x0 ; 1 -> 0
0x41ff81: cmp al, 0x2
0x41ff85: mov byte [ebp+var_6], 0x1 ; 2 -> 1
0x41ff8b: cmp al, 0x3
0x41ff8f: mov byte [ebp+var_6], 0x2 ; 3 -> 2
0x41ff9d: mov byte [ebp+var_5], 0x0 ; last byte
0x41ffa1: call sub_404cb0            ; the 8-byte Feature transfer
```

The stored record's **direction** byte is never copied: the wire byte that
would carry it receives the loader value instead. The report is therefore
`07 FF | effect | brightness | speed | loader | 00`, exactly what the
controller sends.

## Live result

| Action | Result |
| --- | --- |
| Panel `Gaming Special Key` + M1 | Lights ESC, WASD and the arrow keys — the FN+1 group |
| Panel `Gaming Special Key` + M2 | The keyboard does **not** change to the FN+2 group |
| Panel `Gaming Special Key` + loader 3, 4 … 8 | The keyboard **blinks** and falls back |
| Keyboard FN+2, then the panel's `Gaming Special Key` | Lights the **FN+2 group** (TAB, SHIFT, CTRL, QWER, HOME, PRTSC, SCRLK, PGUP, END, PGDN) |

## Conclusion

- The **loader** byte carries the original tool's M1/M2/M3 values. It is not
  the FN zone selector: changing it does not move the lit key group, and a
  loader value above 2 makes the keyboard blink and fall back, so the wire
  byte carries exactly the three values the original Windows tool exposed.
- The **FN zone lives in the keyboard**. The host can enter the special-key
  lighting with the `Gaming Special Key` effect, and the keyboard then lights
  whichever group its own FN key last selected.
- The recovered protocol has no field for the zone.
- The **vendor interface sends no input reports at all**. A 100-second
  recording of `/dev/hidraw1` (interface 1, the one the controller writes to)
  produced **zero** input reports, while the boot interface
  (`/dev/hidraw0`) produced 110 in the same window. The report descriptor
  declares input reports 1–6 on interface 1 — a consumer report, a vendor
  report (page `0xFF00`, byte `0xF1`–`0xF8`) and three keyboard-state
  reports — and this firmware uses none of them. **There is no channel to
  observe the zone from.** The only key events the keyboard sends are the
  ordinary boot-protocol reports on interface 0, and the FN combinations do
  not add a distinguishable report to them.
- The keyboard's ten zones are hardware layouts, listed in the manufacturer's
  GK301 guide: FN+1 FPS 1, FN+2 FPS 2, FN+3 COD, FN+4 RTS, FN+5 LOL, FN+6
  Racing, FN+7 NBA, FN+8 LOL II, FN+9 Custom I, FN+0 Custom II.

### What the original tool does about it

The Windows tool's `HidDevice` class exposes `Open`, `Close`, `Read`,
`Write`, `GetFeature`, `SetFeature`, `SetOutputReport` and `GetInputReport`.
The `SetOutputReport` method (log tag `0x692cc0`, implementation `0x4050e0`)
has no direct call site — the disassembly has five calls into the
`SetFeature` wrapper (`0x404cb0`) and none into it — but the class also has a
`Write` method (`0x404a54`), and `WriteFile` on a HID handle is the other
way an output report reaches the device. The same program family
(`ShinetekTools.exe`) was seen sending `05 01 00…` … `05 04 00…` output
frames at start-up by another researcher, so the original tool's output
path is live and the static call graph alone cannot rule it out.

That output path is **not** the loader either: the loader rides in byte 6 of
the feature report, exactly as the disassembly of the normal writer shows
(`0x41ff71..0x41ff93`).

### The sibling model in the same program family

`ShinetekTools.exe` is shared across keyboards from vendor `0x1A2C`. The
research on the AULA F3009 (`0x1A2C:7F05`) documents the same frame layout
`07 FF FF <effect> <brightness> <speed> 00 00`, the same 20 effects, the
same 0–5 brightness range and the same 0–2 speed range as this keyboard,
and it records the same behaviour that matters here: **on that keyboard the
M1/M2 keys produce no HID events at all in wired mode**, and their meaning
depends on a mode the keyboard itself keeps.

The one field that differs is byte 6. The F3009's official software always
writes `00 00` there and the meaning of those two bytes is listed as
unknown; the GK301's writer puts the loader in byte 6 and the original tool
exposes M1/M2/M3 for it. So the loader is a GK301-side use of a byte the
shared software leaves open — which fits the observation that only one of
the three values does anything visible.

Frames captured for the F3009 are **not** sent to this keyboard: different
model, different firmware, and the report lengths differ.

OmaLecoo therefore keeps the loader at M1/M2/M3, labels it as the original
tool's loader rather than a zone selector, and lists the ten hardware zones
as information (`lecoo-gk301 zones list`, and the panel's *Gaming zones*
section). The `game` preset and the Signal Engine's fullscreen rule both
apply `Gaming Special Key`, so they reproduce the special-key lighting of
whichever zone the keyboard is in.

## What is still open

The loader's *visible* effect is unconfirmed. The byte is sent exactly as the
original sends it, the wire value is documented by the disassembly, and the
keyboard accepts the transfer — but the lit key group does not follow it.

Repeated observation, same brightness and speed, `Gaming Special Key`:

| Loader | Wire byte | Observed |
| --- | --- | --- |
| M1 | 0 | no visible change |
| M2 | 1 | no visible change |
| M3 | 2 | **a visible change** |

So the loader is not a plain zone index either: M3 does something the other
two do not, while M1 does not move the keyboard to the FN+1 group even
though the FN+1 group is the one the keyboard was showing. What M3 changes
exactly is still unknown — the report does not say, and the plugin does not
claim it.

The plugin therefore presents the control as **Loader** (the original tool's
name for it), states that its effect was not confirmed, and lists the ten
hardware zones separately as information.

Two paths would still settle it, and neither is a code change:

- Someone watching the keyboard while the three loader values are sent in
  turn, with the FN zone known beforehand. That needs eyes on the keyboard,
  which this session did not have.
- A dynamic recording of the original Windows tool on this hardware, the way
  `HidDevice::Write` and `HidDevice::SetOutputReport` were hooked for the
  sibling model in the same program family. That needs a Windows session
  with this keyboard attached, which this session did not have either.

Until one of them exists, the plugin says "not confirmed" instead of guessing.

## Boundaries

No firmware was modified, no key report was retained, and no random command
was sent. The visual observations come from the user; the capture only proves
which bytes reached the device. The ten zones are hardware presets, and this
record does not claim a protocol for choosing them.

The input-report recording that closed the observation channel was made with
a small Rust reader on the two `hidraw` nodes of the keyboard. It wrote
every boot report it saw to a log file that was deleted afterwards, and only
the *shape* of the traffic is quoted here — the vendor interface produced no
input reports at all, so nothing about the typed keys is part of this
record.
