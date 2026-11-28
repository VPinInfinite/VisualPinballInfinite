# Cabinet hardware

What a cabinet gets, besides [cabinet mode](Cabinet-mode). In a game, VPX does the cabinet's work: its screens, its toys, a real DMD panel, the nudge and the plunger. The app sets it all up in one place, in VPX's own settings.

## The cabinet view

Settings › Screens › The table's view › **Cabinet** has VPX draw the table for a screen lying in a cabinet. Settings › Cabinet › **The playfield screen and you** tells VPX's window mode the screen's width, its tilt, and where you stand: across, back, and your eyes' height, so it can draw the table at its real size, behind the glass.

## Screens

Up to four: the playfield, the backglass, the score display (or a real DMD panel) and a topper, placed in Settings › Screens. See [Screens](Screens).

## Toys (DOF)

Contactors, knockers, shakers, fans, flashers and LED strips, through DOF and the DOF Config Tool's files:

- **VPX's own DOF add-on** (Settings › Library › its add-ons › **Toys (DOF)**) reads them from the `directoutputconfig` folder in VPX's settings folder. It goes with VPX's own backglass add-on.
- **The classic DOF,** which runs inside the classic B2S Server, reads DOF's own folder, so a cabinet set up that way keeps working.
- **Settings › Cabinet › Toys (DOF)** says where the configuration is found. **Import a DOF configuration…** takes the Config Tool's download (its zip), or any file of a set you've unzipped, and copies it into VPX's; a file it replaces goes to the Recycle Bin. Where a set is found that VPX doesn't read, **Bring it to VPX** copies it in, and **Bring in DOF's own** brings the one in DOF's own folder.

## Boards and sensors

Pinscape and Open Pinball Device boards, key encoders and gamepads work through Settings › Controls, nudge and plunger included: see [Controls](Controls).

## Real DMD panels

ZeDMD, PIN2DMD (with VPX 10.9, not an [older VPX build](Installing-and-first-setup#which-build-of-vpx)), Pixelcade and other panels through VPX's own DMDUtil add-on, or a display device such as DMD Extensions' with the classic VPinMAME. See [Score displays](Score-displays#real-dmd-panels).

## Sound

Exciters under the playfield (SSF), surround speakers, and a bass shaker on a sound output of its own: Settings › Sound. See [Sound](Sound).

## Front ends

To start tables from PinUP Popper, PinballX or PinballY: see [Front ends](Front-ends).
