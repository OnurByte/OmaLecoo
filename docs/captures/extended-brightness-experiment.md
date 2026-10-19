# Extended-brightness experiment — 19 October 2026

## Static findings (REA / Hopper, ShinetekTools.exe sha256 89a0c779…)

New this round, in addition to the earlier records:

1. `sub_4206f0` decompiled (Evidence
  `ev_79f9fb77b15bb0b5f6c164fdc002c84fca2733b06db63e5373972a1aa19489b4`):
   it resolves one of the 20 four-byte stored records
   (`records + index * 4 + 0x1c`, index bounds-checked `< 0x14`) and hands it
   to the 8-byte Feature wrapper `sub_404cb0`. It neither clamps brightness
   nor remaps selection. Named callers list is empty because it is registered
   as a *task*, not called directly: the open-success startup path
   (`loc_41d8fd` in the open path body) runs
   `sub_421110(task-table, sub_4206f0, 0, records[0x1b])` — i.e. the tool
   applies the stored record through this writer during startup.
2. Alongside it the startup path also registers the macro worker
   `sub_420580` and the second poller `sub_4207a0` through the same task
   table, and refreshes the panel (`sub_41f650(1,1)`, `sub_41f450(...)`).
3. `reference/windows/ledeffect.xml` stores 20 identical defaults
   `speed=0 brightness=13 direction=0 select=0`. Together with the record
   serialization order (brightness, speed, direction, selection), the
   previously recorded startup wire packet
   `07 FF FF 00 0D 00 00 00` is exactly "apply record 0": index 0,
   brightness 13 (0x0D), speed 0, direction 0, selection 0.
   The Windows panel writer clamps brightness to 5 and overwrites the
   direction byte; the startup writer sends the stored bytes unchanged.
4. HID descriptor of the vendor interface (hidraw5, report ID 7, page
   0xff01) defines three transport channels grouped under the two vendor
   usages, read straight from `docs/hid-descriptors.json`:
   - usage 03, 7 payload bytes, **Feature** (`b1 02`) — the channel the
     plugin uses;
   - usage 04, 7 payload bytes, **Output** (`91 02`) — the channel the
     recovered reset writer uses via `WriteFile`;
   - report ID 3 on page 0xff00: an 8-bit **Input** report whose bits are
     usages F1..F8 (interpreted as a vendor status/event mask, unconfirmed).
5. Evidence bundle exported to `docs/rea-startup-path-evidence.json`
   (3 records, 25,714 bytes) and the REA target was closed.

## Hardware experiment (same attached keyboard, firmware 1.16)

`scripts/probe-extended-brightness.py` (uses `command.lock`, same as the
controller) on `/dev/hidraw5`, uaccess only, no root:

| t (monotonic) | direction | report |
|---|---|---|
| 72194.120 | SET | `07 F1 00 00 00 00 00 00` |
| 72194.246 | GET | `07 01 16 00 00 00 00 00` (firmware 1.16 ack) |
| 72195.246 | SET | `07 FF FF 00 05 02 00 00` (baseline Steady 5) |
| 72196.450 | SET | `07 FF FF 00 08 00 00 00` (startup form) |
| 72197.653 | SET | `07 FF FF 00 0A 00 00 00` |
| 72198.855 | SET | `07 FF FF 00 0D 00 00 00` (exact Windows startup record) |
| 72200.058 | SET | `07 FF FF 06 05 02 00 00` (restore user's own last state) |

Full transcript: `/tmp` session + `extended-brightness-probe.json` events;
the ioctls all returned without error on the physical device. A concurrent
non-blocking reader on the same hidraw node observed **no vendor input
reports** (report ID 3 events) during the sweep — the device did not push
any state-report while lighting changed.

Full transcript: /tmp/opencode/extended-brightness-probe.json (session
artifact; the sequence is reproduced in the table above).

Wire-level confirmation attempted with dumpcap (`-a duration:20`, control
URB filter `link[9]=2 and link[11]=7` on usbmon8). Two elevated runs
(polkit and sudo) were refused by dumpcap's own capture-file write policy
before packets were recorded; no capture file was produced and no packet
data was lost otherwise. Transport-level acceptance for every probe was
observed through HIDIOCSFEATURE return values, performed on one open
handle — the same observation method as the previously accepted round.
usbmon availability (module loaded) and dumpcap identities were checked;
no daemon was installed and no capture process remains.

## Interpretation and limits

**Result confirmed by the user (19 October 2026):** during a second,
deliberate observation run (3-second holds: brightness 5 → 13 → 5 → 13 on
effect 0 with speed unchanged) the user reported a clear visible brightness
difference between 5 and 13. The firmware brightness byte is therefore not
an application-limited 0..5 slider: the original Windows UI simply clamped.
The extended scale is wired into the tool and panel as brightness 0–13 from
v1.3.0. After the final restore packet the user also observed the expected
animation start for their own restored effect (a flowing/wave effect), which
matches the restored record rather than a fault.

Accepted does not replace observation, and the remaining byte semantics stay
unproven:

- the firmware brightness byte is not limited to 0..5; the Windows tool's
  own startup carries 13 on the wire (XML-stored default), and the panel
  clamp to 5 is application-side; **user-confirmed visually — wired as a
  0..13 control from v1.3.0**;
- byte 3 (index), byte 5 (speed/direction carry) and byte 6 (selection)
  semantics beyond the panel mapping are still unproven;
- the device did not report any vendor input event during lighting changes,
  so the F1..F8 input bits are not tied to lighting state changes
  (single-observation limit).

No other packet families were probed; no firmware flashing, no factory
reset, and no per-key or custom-RGB writes were attempted in this round.
