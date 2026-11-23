# Score displays

The score display is the dot-matrix display (DMD) of machines from the 1990s on, or the number displays of older ones, with digits made of lit bars. Original tables made for VPX often draw their own, with FlexDMD or UltraDMD. VPX shows them all; the app sets where and how, places them over backglasses, and puts colorings in place.

## Where it shows

VPX shows it where Settings › Screens puts its windows (see [Screens](Screens)):

- **In the table's own design:** on one screen, many tables draw it on the backdrop the desktop view shows.
- **In the score display's own window,** on a screen of its own, or kept inside the table's window.
- **Over the backglass's picture,** in the window the picture keeps for it: see below.
- **Over a score panel:** a backglass file may hold a picture for the score display's window, with the score display in its dark window.

With the classic VPinMAME playing the machines, VPinMAME can show it in a window of its own too, which you drag where you like: Settings › Machines › **Its own score window** turns it on or off.

## Over the backglass

Many backglass files are made for a machine with a DMD, and keep a window for it in their picture, often in the speaker panel at its foot. VPX's B2S add-on shows the score display there. [Newer VPX 10.8.1 builds](Installing-and-first-setup#which-vpx-1081-build) keep its place in each backglass file, and Settings › Screens › the backglass's **The score display in it** chooses:

- **Off:** none over the picture.
- **Found in its picture:** the app finds that window in each backglass made for a DMD, and writes the score display's place into the backglass file, as VPX's own menu writes one. It looks at every table's backglass in the background once the library is read, and at a table's again just before it plays. Each file is kept as it came first, in the app's data folder (`Backglasses as they came`), and choosing **Off** again puts back each file it changed. A backglass that shows scores of its own, or has no window for one, is left alone, and so is a place set by hand or in VPX's menu.

The score display's own card, **Over a score panel**, does the same for a backglass file's score panel. With an older 10.8.1 build, VPX's B2S add-on finds the place in the picture by itself as the score display first shows (**Found in its picture**), or puts it where the settings place it (**Where the settings file places it**).

**To place it by hand,** **Score display…** on a table's Backglass row shows the backglass's picture with a box for the score display. Drag the box onto the window in the picture, and its corner to size it: it keeps the DMD's shape while **Keep its shape** is ticked. Or type its **Left**, **Top**, **Width** and **Height**, or move it with the arrow keys, a pixel at a time (ten with Shift). **Find it in the picture** looks for the place again. **Save** writes it into the backglass file, kept as it came the first time: VPX shows it there from the table's next game, and the app leaves it as you set it.

**In VPX's own menu,** during a game: Plugin Settings › B2S › Edit, then the table's backglass file, with its X and Y position, width and height. A place you set there stays where you put it.

## How it looks

VPX draws it, in its own looks. **Score view layouts** (Settings › Picture) are VPX's layouts for the score display's window. Older machines' segment displays have eight looks of VPX's, from neon plasma to LEDs, each with its lit and unlit colors, brightness and glow in Settings › Picture › **Segment displays**; a table chooses the look its displays use. **The DMD made smoother** sharpens it for big screens with ScaleFX, MMPX, xBRZ or Super-xBR; it's off, as in VPX.

## Color DMDs

Colored score displays, from a machine's coloring:

- **Serum** colorings (`.cRZ`, `.cROMc`).
- **PIN2DMD** colorings (`.pal`, `.vni`, and `.pac` with your own key: Settings › Picture › **Your PIN2DMD key**).

They go in VPinMAME's `altcolor` folder under the ROM's name, or in the game's own folder when your library keeps a folder for each game: the drop window puts them there, and VPX's coloring add-ons find them in either. A coloring named for another version of the game stays that version's, so changing the table's ROM to that version shows it; one named for no ROM at all gets the name VPX looks for. A coloring zipped under a ROM's name isn't taken for the ROM: the drop window says so, leaves it out, and keeps your ROM.

Settings › Picture turns Serum and PIN2DMD colorings on or off, and a table's settings can turn its color DMD off. Those are VPX's own add-ons, which go with VPX's own PinMAME. The classic VPinMAME colors nothing itself: its display device (such as DMD Extensions) colors the display, by its own settings.

**Tools** › **Color DMD** lists every ROM table's coloring, and what you've recorded of each machine to color it yourself. **Which tables show** picks **All ROM tables**, those **Colored**, those **Not colored**, or those where **VPS lists one**. Drop a coloring onto a table's row there, or onto its page, to add it.

## Coloring your own

**Color it…** on a table's Color DMD row, or on its row in Tools › Color DMD, colors its machine's display in three steps:

1. **Record the display.** **Record it by itself** has the app run the table with no window for about four minutes, in the background: its attract mode, then a game it plays on its own. **Play and record** plays it in VPX 10.8.1 and records the display as you play, to add what only a player reaches, such as the modes, multiballs and jackpots. **Show the recordings** opens their folder.
2. **Color it in a coloring program,** by hand, scene by scene. These are programs by other people, not part of Visual Pinball Infinite:
   - **ColorizingDMD,** which most colorings were made with: free, for Windows. **Install ColorizingDMD** fetches it from its makers' page on GitHub the first time and unpacks it into the app's folder, and **Open ColorizingDMD** opens it after that. It opens a recording as one text file named for the ROM: **Save the recordings for it** makes that file and shows where it is.
   - **PPUC's Colorizing DMD,** the new program from the people who make Serum, isn't released yet: its test versions save a newer kind of file that no player plays yet. **Its page** shows where it stands.
3. **Put the finished coloring in place.** The program saves a file named for the ROM (`<rom>.cRZ`). Drop it onto the table's page, or onto its row in Tools › Color DMD, or use **Choose the finished file…**: it goes where VPX finds it, and a coloring there before goes to the Recycle Bin. Then **Play it**.

## Real DMD panels

- **A real DMD panel, built in** (Settings › Picture): VPX's own DMDUtil add-on shows the score display on a real LED panel plugged in by USB or on the network, such as ZeDMD, PIN2DMD or Pixelcade, as well as on screen (PIN2DMD panels with a [newer VPX 10.8.1 build](Installing-and-first-setup#which-vpx-1081-build)). It finds the panel by itself, and there's nothing to install. Leave it off without a panel.
- **With the classic VPinMAME** playing the machines (Settings › Library › its add-ons), VPinMAME drives a panel through its own display device (`DmdDevice64.dll`, such as DMD Extensions'), set up in its own settings. Settings › Machines › **A real DMD panel for it** turns it on or off for every machine.

## FlexDMD and UltraDMD

Original tables that draw their own DMD scenes use FlexDMD or UltraDMD. VPX 10.8.1's own FlexDMD add-on draws both, so there's nothing to install; Settings › Library › its add-ons can use the classic FlexDMD instead, where it's installed. VPX 10.8.0 needs the classic FlexDMD installed.
