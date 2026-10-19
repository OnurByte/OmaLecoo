# Connected GK301: live USB capture and REA analysis

2026-10-19. Device VID:PID 1a2c:9df4, USB bus8/device7; hidraw5 = vendor
interface1. Firmware query response 07 01 16 00 00 00 00 00 (1.16).

## Direct hardware observation

Wireshark CLI 4.7.4 was installed and the Linux usbmon module loaded.
A 15-second dumpcap record containing only this device's CONTROL URBs:
`link[9] = 2 and link[11] = 7`. 16 records, 0 drops reported. Interrupt
keyboard input is outside the filter. The capture ended; no background
capture remains.

During that window the firmware query, a re-send of the current Breathing
setting and a second firmware query were issued. The current effect was not
changed.

REA 6.7.0 CLI analyzed the real record with REA_TSHARK_COMMAND=/usr/bin/tshark
selected explicitly; the MCP server configuration was not modified.

REA Evidence: ev_1e20977fbfca2c4906f3cebd2230dd6062c5378f9ca0083fde0886fcf4a61e1c
Complete record: live-control.pcapng
REA output: rea-live-control.json
USB/HID inventory: live-hid-inventory.json

| Record | Observation |
|---|---|
|7–8|SET_REPORT, wValue0307, interface1, 8 bytes: 07 F1 00 00 00 00 00 00; completion status0|
|9–10|GET_REPORT Feature7, interface1; real device response 07 01 16 00 00 00 00 00|
|11–12|SET_REPORT Feature7, interface1; 07 FF FF 02 05 00 00 00; completion status0|
|13–16|Second firmware query with the same firmware response|

So the Linux command really reaches USB interface1 as Feature report7 with
length8. This no longer rests on ioctl return values alone.

## Boundaries

REA itself does not capture live USB; dumpcap took the hardware record and
REA analyzed the file. This is not firmware dumping/decompilation and not an
optical measurement of the LEDs. Additional commands, custom RGB or side LED
capability cannot be inferred from this record. No hardware fault or new
feature is claimed. No new random command was sent.
