# GK301 analysis conclusions — 19 October 2026

## Confirmed

The connected device is SEMICO 1a2c:9df4, firmware1.16. It exposes a normal
keyboard HID interface and a vendor-capable interface. The latter's report7
has seven payload bytes for Feature and Output transfers (eight bytes with ID).

Actual usbmon capture and REA analysis confirm SET_REPORT Feature7,
wValue0x0307, interface1, length8. The user independently confirmed that
main effect changes are visible on the physical keyboard.

Main packet: `07 FF FF effect brightness speed selection 00`.
The recovered Windows UI exposes brightness for effect0, brightness and
M1/M2 for effect1, and brightness/speed for effects2–19. That is the scope
exposed by this release. Shared process locking and pacing are retained.

## New readback experiment

The final experiment kept the same HID handle open and performed:

1. SET_FEATURE `07 F1 00 00 00 00 00 00`.
2. GET_FEATURE -> `07 01 16 00 00 00 00 00`.
3. Reapply current main settings `07 FF FF 02 05 00 00 00`.
4. GET_FEATURE without another F1 query -> the same firmware response.

The primary sequence is present in final-readback-control.pcapng, frames7–14.
Capture contains15 records, no reported drops; frame15 is a subsequent
background firmware query whose completion falls outside the capture window.
That final incomplete exchange is not used to claim a result.

REA Evidence:
`ev_9c07241caa6f57914f9eaf6704694feff1bcdefad7ec05eb6591c576d0649822`.
Complete analysis: captures/rea-final-readback.json.
Direct ioctl observations: captures/final-readback-probe.json.
Earlier complete main-command capture:
`ev_1e20977fbfca2c4906f3cebd2230dd6062c5378f9ca0083fde0886fcf4a61e1c`.

This demonstrates that the tested GET_FEATURE path does not report changed
lighting settings. It does not prove that no other firmware query could.
Local settings remain explicitly labeled as last sent.

## Excluded capabilities

- Side LED controls are under the original XML's hidden Mouse page, not
  the Keyboard/Panel LED page. They produced no physical change on this
  keyboard. They are excluded from the UI and GUI combined operations.
- No arbitrary RGB triplet or per-key RGB sender was established.
- The recovered macro worker calls Windows keybd_event. That establishes
  host-side playback, not an on-device macro programming protocol.
- The delayed Windows startup record `07 FF FF 00 0D 00 00 00` caused a
  user-observed change in an earlier experiment. Timing and byte4 changed
  together, so their individual effects remain unresolved. It remains a
  separate research probe and is not automatically applied at startup.

## Boundaries

REA analyzed captured files and the Windows executable. USB acquisition was
performed by dumpcap, and physical LED observation came from the user.
There is no firmware dump or firmware decompilation. No random command fuzzing,
firmware flashing, or keyboard-input capture was performed. Capture filters
selected only this device's control URBs and exclude interrupt key reports.
All capture processes have ended. No continuous monitoring is installed.

This concludes the current release investigation; unsupported features are
not presented as working controls. Further capability work requires matching
new firmware evidence or a specific physical test with a clear hypothesis.
