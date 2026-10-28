# Screens

Settings › Screens places every window the player opens: the playfield, the backglass, the score display (DMD) and a PinUP pack's topper. It draws your screens as they sit on your desk, with each window on them. One screen to four, landscape or portrait.

## The table's view

How the playfield is drawn:

- **Desktop:** the table seen from the front, for a monitor on a desk.
- **Cabinet:** for a screen lying in a cabinet. The table is drawn at its real size behind the glass, from the screen's size and where you stand (Settings › Cabinet: see [Cabinet hardware](Cabinets#the-cabinet-view)). On an upright screen, the table runs up it.
- **Full single screen:** the whole table on one screen.

## Placing the windows

- **Ready-made layouts:** One screen, Table and backglass, Ultrawide, Three screens, Four screens. A layout's button lights while the windows are as it put them.
- **On the map:** drag and size each window where it should go.
- **On your real screens:** **Arrange on your screens** opens each window on the real screens, to drag and size it there. **Identify screens** shows each screen's name on it.
- **For each window:**
  - what it **Shows**: **In its own window**, **Inside the table's window** (painted into the table's picture, as VPX does on one screen) or **Not at all**;
  - its **Screen**, and **Fill the screen**;
  - its **Shape**: it keeps its shape as you size it, or **Reset its shape** to its picture's.
- **The score display:** where a backglass file has a score panel, it shows the DMD **Over a score panel**. On one screen, for ROM tables, **With no window of its own, float it over the table**: a window you can drag with the mouse, its place kept for each game.
- **Save** writes only what changed; **Undo changes** puts it back. The file before is kept as `Settings.previous.ini`.

## Presets

Set the windows as you want them for some tables (the topper off, say) and **Save as preset…** A table plays with a preset when its settings choose it. Presets can be renamed, deleted or shown on the map. Cabinet mode can have one of its own: see [Cabinet mode](Cabinet-mode#its-screens).

## A table's own screens

A table's settings (**Its screens**) choose your usual screens, a preset, or **Custom…**: its own windows. Only what differs from your usual ones is kept, in its `<table>.ini`.

## Display timing

How frames reach the screen is on Settings › Picture:

- **The player's own:** as fast as it can, up to its limit.
- **As fast as possible:** the quickest; may tear.
- **No tearing, quick:** the newest frame at each refresh.
- **Each frame just before the screen shows it.**
- **The screen's own pace:** no tearing, more delay.

And at most how many frames a second: 240 unless you set another, the screen's refresh rate, or no limit. A key press always gets its frame at once.

## When something's on the wrong screen

**Identify screens** shows which screen is which; then move the window on the map, or with **Arrange on your screens**. A table with its own screens, or a preset, uses those: check its settings.
