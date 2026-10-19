# Physical user validation — 2026-10-19

Earlier transfer tests proving rc=8 did not prove the LEDs changed. The
physical observations in this round were recorded directly with the user's
own answers:

1. Panel 0 / brightness 5 and side 3 / brightness 7 as separate CLI calls via
   Feature: the user reported no change at all.
2. The same two known light packets as Output transfers: the user reported
   no change.
3. F1 Set/Get on the same open handle, then the same panel/side packets at
   150 ms intervals: the user reported a brief flicker, then a return to
   the previous state.
4. The delayed startup path recovered from Windows: on one kept-open handle,
   F1 Set/Get, wait 2 seconds, then Feature `07 FF FF 00 0D 00 00 00`, hold
   open for 2 seconds. This time the user reported a permanent change.

The last experiment changed both the wait and the brightness byte at once,
so the cause cannot be attributed to delay alone, to the 0D value alone, or
only to the open connection. No new lighting protocol has been enabled
automatically yet.

The user also confirmed the existing effect controls work. Removing all
controls would have over-corrected that feedback and was reverted; effects,
brightness, the existing side-effect list and profiles were restored. Only
the decorative keyboard diagram at the top and the unproven new color
shortcuts were removed. The prior UI is preserved under archive/0.4.0-plugin.

Remaining split: compare the current effect commands against the startup
packet's byte4 0D value and its physical effect; determine whether the
device's real LED state can be read back. No random raw command scanning was
performed.

## Final user validation and model boundary

The user confirmed the restored main-effect selector changes the physical
keyboard and that the side LED controls did not react.

New XML finding: dlg_main.xml lines 15–124 are the Keyboard page; Panel LED
lives there. Side LED appears at line 126 inside the Mouse page (line 125)
behind visible=0. That strongly suggests shared application code that does
not represent GK301 side-LED support; it is not a claim of firmware absence.
The side LED UI was removed; main effects were kept. The side writer was
removed from the tool entirely, so no side packet can leave the preset,
restore or effect paths. Legacy research CLI and references are preserved.

Macro investigation: worker 0x420580 calls Windows keybd_event at 0x420621;
macros execute on the host. Recovering an on-device macro programming
protocol is not established, so no on-device macro feature was added.

`lecoo-gk301 probe-startup` defaults to display-only mode; with --send it
replays the earlier physical experiment on one open connection. Because the
last packet's byte4 0D and the delay changed within the same experiment,
their individual effects remain unresolved.
