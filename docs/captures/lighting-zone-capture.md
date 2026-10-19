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
  the FN zone selector: changing it does not move the lit key group.
- The **FN zone lives in the keyboard**. The host can enter the special-key
  lighting with the `Gaming Special Key` effect, and the keyboard then lights
  whichever group its own FN key last selected.
- The recovered protocol has no field for the zone. A loader value above 2 is
  rejected by the firmware (the keyboard blinks), so the wire byte carries
  exactly the three values the original Windows tool exposed.

OmaLecoo therefore keeps the loader at M1/M2/M3 and documents that the FN
zones are chosen on the keyboard. The `game` preset and the Signal Engine's
fullscreen rule both apply `Gaming Special Key`, so they reproduce the
special-key lighting of whichever zone the keyboard is in.

## Boundaries

No firmware was modified, no key report was retained, and no random command
was sent. The visual observations come from the user; the capture only proves
which bytes reached the device. The nine zones are hardware presets, and this
record does not claim a protocol for choosing them.
