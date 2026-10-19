# Similar open-source applications — 2026-10-19

- OpenRGB: https://github.com/CalcProgrammer1/OpenRGB
  Main README: color/effect control, profiles, CLI, SDK and a graphic LED
  view. Model for this panel's device card and local save/load profile flow.
- OpenRazer: https://github.com/openrazer/openrazer
  Driver + DBus + user interface separation. Supported Razer devices use VID
  1532; installing that driver for Lecoo 1a2c:9df4 would not be a solution.
- Polychromatic: https://github.com/polychromatic/polychromatic
  Lighting and device-capability UI on top of OpenRazer. Model for the panel
  showing controls only for what the hardware really offers.
- rgb_keyboard: https://github.com/dokutan/rgb_keyboard
  Per-key color and profiles on some 0c45 VID keyboards. No protocol match
  with this model; their packets were never sent to the Lecoo.
- Same-vendor different-PID protocol research:
  https://gitlab.com/CalcProgrammer1/OpenRGB/-/issues/2470
  The Redragon 1a2c:4b4d example uses a `07 F7` command family and fixed
  color codes. Different from Lecoo's evidenced `07 FF FF` family. A shared
  VID does not prove compatible commands.

Search boundary: GitHub/OpenRGB for Lecoo/GK301/9df4/Shinetek and the known
Linux RGB management projects. This search found no verified free RGB driver
for the GK301; that result is not a claim that none exists anywhere.

No source code was copied; only the general interface approach and the
capability model were reviewed.
