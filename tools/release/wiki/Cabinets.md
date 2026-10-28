# Cabinet hardware

What Visual Pinball Infinite does in a cabinet, besides [cabinet mode](Cabinet-mode). Screens and controllers are tried on real hardware. The toys, boards and real DMD panels are built and checked against the same libraries the hardware uses, but haven't been tried in a real cabinet yet: tell us how it goes.

## The cabinet view

Settings › Screens › The table's view › **Cabinet** draws the table for a screen lying in a cabinet: at its real size, behind the glass, as VPX's window mode draws it. Settings › Cabinet › **The playfield screen and you** tells it the screen's width, its tilt, and where you stand: across, back, and your eyes' height.

## Screens

Up to four: the playfield, the backglass, the score display (or a real DMD panel) and a topper, placed in Settings › Screens. See [Screens](Screens).

## Toys (DOF)

Contactors, knockers, shakers, fans, flashers and LED strips, through DOF (libdof) and the DOF Config Tool's files:

- They're read where VPX keeps them (the `directoutputconfig` folder in VPX's settings folder), so a cabinet set up for VPX keeps working.
- No VPX set up? **Import a DOF configuration…** in Settings › Cabinet takes the Config Tool's download (a zip), any file of an unzipped set, or DOF's own folder.
- Philips Hue lights, and SSF impact sounds on exciters, are included.
- Older tables that read their toy settings from Windows' registry get them too.

## Boards and sensors

Pinscape and Open Pinball Device boards, key encoders and gamepads work through Settings › Controls, nudge and plunger included: see [Controls](Controls). Settings › Cabinet's own page for boards and sensors is still being built.

## Real DMD panels

ZeDMD, Pixelcade and other panels through libdmdutil, or a VPinMAME display device such as DMD Extensions'. See [Score displays](Score-displays#real-dmd-panels).

## Sound

Exciters under the playfield (SSF) and surround speakers: Settings › Sound. See [Sound](Sound).

## Front ends

To start tables from PinUP Popper, PinballX or PinballY: see [Front ends](Front-ends).
