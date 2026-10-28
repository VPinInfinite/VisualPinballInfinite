# Score displays

The score display is the dot-matrix display (DMD) of machines from the 1990s on, or the number displays of older ones, with digits made of lit bars. Original tables made for VPX often draw their own, with FlexDMD or UltraDMD.

## Where it shows

On one screen, it shows where the table's own design puts it, or floats over the table in a window you can drag (ROM tables). With more screens, it gets a window of its own, or sits in the backglass's score panel. Settings › Screens places it: see [Screens](Screens).

## How it looks

It's drawn as VPX's newer versions draw it: glowing plasma dots behind glass, or LEDs (Settings › Picture › the score view's layouts). Older machines' segment displays are drawn digit by digit, in their own layouts. **The DMD made smoother** sharpens it for big screens with ScaleFX, MMPX, xBRZ or Super-xBR; it's off, as in VPX. A game's own DMD color is in its table's settings.

## Color DMDs

Colored score displays, from a machine's coloring:

- **Serum** colorings (`.cRZ`, `.cROMc`).
- **PIN2DMD** colorings (`.pal`, `.vni`, and `.pac` with your own key: Settings › Picture › **Your PIN2DMD key**).

They go in VPinMAME's `altcolor` folder under the ROM's name, where VPX looks for them too, or in the game's own folder when your library keeps a folder for each game; the drop window puts them there. A coloring named for another version of the game stays that version's, so changing the table's ROM to that version shows it; one named for no ROM at all gets the name the player looks for. Settings › Picture turns Serum and PIN2DMD colorings on or off, and a table's settings can turn its color DMD off.

**Tools** › **Color DMD** lists every ROM table's coloring, and what you've recorded of each machine to color it yourself. **Which tables show** picks **All ROM tables**, those **Colored**, those **Not colored**, or those where **VPS lists one**. Drop a coloring onto a table's row there, or onto its page, to add it.

## Coloring your own

**Color it…** on a table's Color DMD row, or on its row in Tools › Color DMD, colors its machine's display in three steps:

1. **Record the display.** **Record it by itself** runs the table with no window for about four minutes, in the background: its attract mode, then a game it plays on its own. **Play and record** adds what only a player reaches, such as the modes, multiballs and jackpots. The pause menu's **Record the display for coloring** records from any ROM table's game too. **Show the recordings** opens their folder.
2. **Color it in a coloring program,** by hand, scene by scene. These are programs by other people, not part of Visual Pinball Infinite:
  - **ColorizingDMD,** which most colorings were made with: free, for Windows. **Install ColorizingDMD** fetches it from its makers' page on GitHub the first time and unpacks it into the app's folder, and **Open ColorizingDMD** opens it after that. It opens a recording as one text file named for the ROM: **Save the recordings for it** makes that file and shows where it is.
  - **PPUC's Colorizing DMD,** the new program from the people who make Serum, isn't released yet: its test versions save a newer kind of file that no player plays yet. **Its page** shows where it stands, and once it's released the app installs it the same way.
3. **Put the finished coloring in place.** The program saves a file named for the ROM (`<rom>.cRZ`). Drop it onto the table's page, or onto its row in Tools › Color DMD, or use **Choose the finished file…**: it goes where the player finds it, and a coloring there before goes to the Recycle Bin. Then **Play it**.

## Real DMD panels

- **Panels through libdmdutil,** such as ZeDMD and Pixelcade: Settings › Picture.
- **A VPinMAME display device** (`DmdDevice64.dll`, such as DMD Extensions' or a panel maker's), with its own settings file: Settings › Picture.
- **For one game:** Settings › Machines › **A real DMD panel for it**, and its color there in the table's settings.

Real panels are checked with libdmdutil's virtual panel, but haven't been tried with a real one yet: tell us how it goes.

## FlexDMD and UltraDMD

Original tables that draw their own DMD scenes use FlexDMD or UltraDMD. Both are built in, so there's nothing to install: FlexDMD's own code, rewritten in Rust, checked dot for dot against the original, with its stand-in for UltraDMD.
