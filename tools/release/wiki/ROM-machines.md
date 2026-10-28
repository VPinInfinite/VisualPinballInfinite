# ROM machines

A table that recreates a real machine runs that machine's own program, its ROM, in PinMAME, which comes with the app. ROMs belong to the machines' makers, so no installer may include them: you supply them yourself, as for VPX. The table's script names the ROM it wants, and a table's VPS page lists the ROMs for it.

## Getting a ROM in

Drop the ROM's `.zip` onto the app, with its table or onto the table's page. It goes into VPinMAME's `roms` folder, checked file by file against what PinMAME expects for it. Keep ROMs zipped, named as the table asks: the drop window does both.

## When a ROM isn't right

The app checks every table's ROM in the background, file by file, and a table's page says what's wrong in plain words:

- **Its ROM isn't installed:** get it from the table's VPS page and drop it onto the app.
- **Its ROM is missing files:** the zip you have is probably another version of the set; get the right one.
- **It also needs another zip, the version it's based on** (its parent): get that one too.
- **PinMAME doesn't know the ROM:** the table's script may name it wrongly. **Change…** on its ROM row picks one PinMAME knows.

## Every ROM: Tools › ROMs

**Tools** › **ROMs** lists every ROM set your library has or its tables ask for, checked file by file against PinMAME's own list, with the tables that play it. **Which sets show** picks **All**, those that **Need a look**, those **No table plays**, or those **Not installed**.

- **Its status:** **Good**; a **Spare version** no table plays; **Files missing**; **Wrong files**; **Holds no ROM PinMAME knows**; **Can't be read**; or **Not installed**, where **Its table** opens the table's page, which links to VPS for its ROM.
- **Files** lists its files, each **Right**, **From its parent's zip**, **Missing**, **Another file under its name**, or **Not there; it plays without it**, with **Show in Explorer**.
- **Fixes** always ask first, and what's removed goes to the Recycle Bin: **Rename it …** for a set under a name PinMAME doesn't look for (the tables asking for the set it holds play it after), **Remove the copy…** for a second copy of a set, and **Remove…** for a set no table plays, or a zip with no ROM PinMAME knows.

## Change the ROM

**Change…** on a table's ROM row opens **Change the ROM**. It lists the ROMs PinMAME knows for its game, which of them you have, and which are on VPS; **Select from all ROMs…** shows every ROM you have. Choose another and the app changes only the name the table's script hands PinMAME, in a script file beside the table (`<table>.vbs`, which VPX reads too). The table file itself never changes, and you can go back to its own ROM here any time.

## Its machine's settings

Settings › Machines sets them for every machine, and a table's settings (**The machine**) for one game, under VPinMAME's own names:

- **Its sound** and **its volume.**
- **Its sound comes from:** its own sound, its AltSound pack if it has one, or older machines' recorded sounds.
- **Skip its start-up tests** (PinMAME's cheat mode, which skips some games' tests at start).
- **A real DMD panel for it,** and its color there.
- **Its floating score window,** on one screen.

They're kept in `Machine settings.ini` in the app's data folder.

## Its memory

A machine keeps its high scores, settings and audits in its memory (NVRAM). The first time a table plays here, the app takes VPinMAME's copy, if you play in VPX; from then on it keeps its own, so VPX's is never changed. A machine with no memory at all restores its factory settings the first time; the player gives it about 15 quiet seconds for that, so the game then starts as it should.

## High scores

A machine's high scores are read from its memory, through the [Pinball Memory Maps](https://github.com/tomlogic/pinball-memory-maps) (ODbL), and shown on its table's page beside the ones you made in VPX. See [A table's page](Table-page#achievements-high-scores-and-plays). **Player hub** › **High scores** lists every table's best.

## AltSound packs

AltSound packs replace a machine's music and sounds with recorded ones, remastered by their makers or made from the ROM itself on the table's page (**Make one from its ROM…**). They play on most machines, though not Stern's SAM machines or Pinball 2000. Getting one, its check, turning it on or off, making one, and which machines it works for: see [AltSound](AltSound).

## Color DMDs

Colorings for a machine's score display (Serum's and PIN2DMD's) go in VPinMAME's `altcolor` folder under the ROM's name, and a machine without one can be colored by hand: **Color it…** on its table's page. See [Score displays](Score-displays#color-dmds).
