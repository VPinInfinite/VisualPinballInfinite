# Screens

Settings › Screens places every window VPX opens: the playfield, the backglass, the score display (DMD) and the topper. It draws your screens as they sit on your desk, with each window on them. One screen to four, landscape or portrait. The places go into VPX's own settings, and VPX takes them from there whenever a table starts.

- **Playfield:** the table itself.
- **Backglass:** the backglass picture, or a PinUP pack's backglass screen.
- **Score display:** the DMD or the score digits, or a PinUP pack's DMD screen.
- **Topper:** a PinUP pack's topper screen.

Future Pinball's games play where this page puts the playfield, and the backglass as Future Pinball's backbox: see [Future Pinball](Future-Pinball#playing). Where Future Pinball is installed, the parts that set its games too are tagged **VPX + FP**: the ready-made layouts and the windows.

## The table's view

How VPX draws the playfield:

- **Desktop:** the table seen from in front, for a monitor on a desk, its score display on its backdrop.
- **Cabinet:** for a cabinet's playfield screen, looking down on the table as a player standing at it. On an upright screen, the table runs up it. The screen's size and where you stand are in Settings › Cabinet: see [Cabinet hardware](Cabinets#the-cabinet-view).
- **Full single screen:** the whole table and its backglass on one portrait screen, as some tables offer.

## The achievement popup

Where each achievement you earn shows while VPX plays (see [Achievements](Achievements)):

- **Shows on:** **Backglass** or **Score display** puts a card over what that window shows, for about four seconds: the achievement's picture, its name and its points. Where that window isn't showing, VPX's own notice on the playfield shows instead. **Playfield** is VPX's own notice, at its own size: VPX lets add-ons draw on its backglass, score display and topper, never on the playfield.
- **Size:** the card **Small**, **Medium** or **Large**.

It's the same for every table: a table's own screens and presets leave it as it is.

## Placing the windows

- **Ready-made layouts** for the screens you have: **One screen**, **Table and backglass**, **Ultrawide** (the topper, the backglass and the score display side by side on an ultrawide screen), **Three screens** and **Four screens**. A layout's button lights while the windows are as it put them; choose it again to go back to what was there before.
- **On the map:** drag each window where it should go, onto another screen too, and its corner to size it. Double-click a window to make it fill its screen.
- **On your real screens:** **Arrange on your screens** opens a stand-in for each window on the real screens: drag it by its middle and size it by its edges or corner, then **Done** (or Enter) on any of them, or **Cancel**. Close to a screen's edge, it's taken to the edge. **Identify screens** shows each screen's name on it, with the windows set for it.
- **For each window,** its card (click it to open it):
  - what it **Shows**: **In its own window**, **Inside the table's window** (drawn into the table's picture, as VPX does on one screen) or **Not at all**;
  - its **Screen**, and **Fill the screen**; or where it sits there, in pixels: **Across**, **Down**, **Width** and **Height**;
  - its **Shape**: while **Keep its shape** is ticked, it keeps its shape as you size it, and **Reset its shape** puts back the one it had.
- **The score display over the backglass:** the backglass's card holds **The score display in it** and **Show the backglass picture's speaker panel**, and the score display's card holds **Over a score panel**. See [Score displays](Score-displays#over-the-backglass).
- **Save** writes only what changed; **Undo changes** puts the page back. The file before is kept as `VPinballX.previous.ini`, and **Show the file** shows VPX's settings file in Explorer.

A window kept inside the table's window is drawn at the wrong place and size while VPX's supersampling (Settings › Picture) is anything but 1×: the page says so.

Where the classic B2S Server shows the backglasses (in VPX 10.8.0, or where Settings › Library › Its add-ons has B2S Server play them), it places them by its own screen file, `ScreenRes.txt`. The app writes the backglass's place there as this page has it, when the page saves and before a table plays, keeping the file before beside it as `ScreenRes.previous.txt`. B2S Server's second window, a backglass file's score panel, stays where B2S Server's own settings put it, and so does a table that has a screen file of its own (`<table>.res`).

## Presets

Set the windows as you want them for some tables (the topper off, say), then **Save as preset…**, name it and **Keep it**. A table plays with a preset when its settings choose it (**Its screens**). **Show it here** puts a preset on the page, to change and keep again or to save as your usual; presets can be renamed, or deleted to the Recycle Bin. Cabinet mode can have one of its own: see [Cabinet mode](Cabinet-mode#its-screens). Presets are for VPX 10.9 (and 10.8.1): VPX 10.8.0 plays with its own settings.

## A table's own screens

- **Chosen in its settings:** a table's settings (**Table settings** on its page) choose its **View**, and **Its screens**: your usual screens, or a preset. **Custom…** beside its view opens this page for that table alone. Only what differs from your usual screens (and its preset) is kept, in its `<table>.ini`.
- **Saved from VPX's own menu:** changing a window in VPX's own menu during a game can save it for that table alone, in the same file. The table's page then says **This table has its own screen settings**, with each window for that table and yours side by side. **Use my screens** takes the table's out, so yours apply again; **Keep this table's** keeps them, and the page says no more of them until they change. See [A table's page](Table-page#its-own-screen-settings).

## Display timing

How frames reach the screen is on Settings › Picture, under **Display timing**, as VPX does it:

- **Frames:** VPX's own, each frame drawn just before the screen shows it (Frame Pacing); as fast as possible, the quickest, which may tear; the screen's own pace, with no tearing but more delay (Vertical Sync); or the screen's pace with late frames shown at once (Adaptive Sync).
- **At most:** VPX's own, the screen's refresh rate; 60, 120, 144, 165 or 240 frames a second; or no limit.
- **Frames queued:** how many frames the graphics card may hold before showing them. 1 is the quickest.

## When something's on the wrong screen

- **Identify screens** shows which screen is which; then move the window on the map, or with **Arrange on your screens**.
- The page warns of a window set for a screen that isn't connected (VPX puts it on the main screen meanwhile, as the map shows), and of a window set for one of two screens with the same name, which VPX can't tell apart.
- After a game, a table's page says when VPX couldn't find a screen your settings name and which one it used instead, with **Open Settings › Screens**.
- A table with its own screens, or a preset, uses those: check its settings and its page.
