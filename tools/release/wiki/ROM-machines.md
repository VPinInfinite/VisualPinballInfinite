# ROM machines

A table that recreates a real machine runs that machine's own program, its ROM, in PinMAME: VPX 10.8.1's own PinMAME add-on, or the classic VPinMAME where Settings › Library › its add-ons choose it (VPX 10.8.0 always uses the classic one). ROMs belong to the machines' makers, so no installer may include them: you supply them yourself, as for VPX. The table's script names the ROM it wants, and a table's VPS page lists the ROMs for it.

## Getting a ROM in

Drop the ROM's `.zip` onto the app, with its table or onto the table's page. It goes into VPinMAME's `roms` folder, where both PinMAMEs look (or into the table's own `pinmame\roms`, for a table kept in a folder of its own that has one), checked file by file against what PinMAME expects for it. Keep ROMs zipped, named as the table asks: the drop window does both, and makes a zip of a set that comes in another kind of archive or as loose files. A download's read-me shows in the drop window (**Its read-me**). A zip named for a ROM that holds only a coloring's files isn't a ROM: the drop window says so, leaves it out and keeps your ROM (see [Score displays](Score-displays#color-dmds)).

## When a ROM isn't right

The app checks every table's ROM in the background, file by file, and a table's page says what's wrong in plain words:

- **Its ROM isn't installed:** get it from the table's VPS page and drop it onto the app.
- **Its ROM is missing files:** the zip you have is probably another version of the set; get the right one.
- **It also needs another zip, the version it's based on** (its parent): get that one too.
- **PinMAME doesn't know the ROM:** the table's script may name it wrongly. **Change** on its ROM row picks one PinMAME knows.

## Every ROM: Tools › ROMs

**Tools** › **ROMs** lists every ROM set your library has or its tables ask for, checked file by file against PinMAME's own list, with the tables that play it. **Which sets show** picks **All**, **Needs a look**, **No table plays it** or **Not installed**.

- **Its status:** **Good**; a **Spare version** no table plays; an **Extra copy** of another set; **Files missing**; **Wrong files**; **Needs** its parent's zip; **Named wrongly**, for a set under another set's name; **Holds no ROM PinMAME knows**; **Can't be read**; or **Not installed**, where **Its table** opens the table's page, which links to VPS for its ROM.
- **Files** lists its files, each **Right**, **From its parent's zip**, **In its parent's zip, which isn't here**, **Missing**, **Another file under its name**, **Not there; it plays without it**, or **Not this set's**, with **Show in Explorer**.
- **Fixes** always ask first, and what's removed goes to the Recycle Bin: **Rename it …** for a set under a name PinMAME doesn't look for (the tables asking for the set it holds play it after), **Remove this extra copy…** for a second copy of a set, and **Remove…** for a set no table plays, or a zip with no ROM PinMAME knows.

## Change the ROM

**Change** on a table's ROM row opens **Change the ROM** for it. It lists the ROMs PinMAME knows for its game, which of them you have, and which are on VPS; **Select from all ROMs…** shows every ROM you have. Choose another and **Change it**: the app changes only the name the table's script hands PinMAME, in a script file beside the table (`<table>.vbs`, which VPX plays in place of the table's own script). The table file itself never changes, and the same window takes you back to its own ROM any time.

Its PinUP pack, AltSound pack and color DMD go with it when they play on the ROM you choose: a PinUP pack or an AltSound pack on any version of the same game, a color DMD on a version VPS lists a coloring made for, as a coloring is made for one version's pictures. Each one's folder is renamed for the new ROM, or copied when another of your tables still plays the old one, and going back puts them back. The window says what goes with it before anything changes.

## Its machine's settings

Settings › Machines sets them for every machine, where each PinMAME reads them:

- **VPX's own PinMAME:** **Skip their start-up tests** (PinMAME's cheat mode, which skips some games' tests at start), in VPX's settings.
- **The classic VPinMAME, for every machine:** **Its sound**; **Its volume**, only the ROM's sound, louder or softer, in decibels; **Its sound comes from**: its own sound, or its AltSound pack if it has one; **Older machines' recorded sounds**; **Skip its start-up tests**; **A real DMD panel for it**; and **Its own score window**, VPinMAME's own window showing the score display. They're kept where VPinMAME keeps them, in your part of Windows' registry.

A game's own, in a table's settings (**The machine**: its sound, its volume, recorded sounds, skipping its start-up tests, and with a display device, the real DMD and its color there), are the classic VPinMAME's: kept as it keeps them for each game, shared by every table that plays it, and winning over those for every machine. VPX's own PinMAME keeps none for each game, so a table's settings show them only where the classic VPinMAME plays its machine. A table's script may set some of them itself each time it starts, whichever PinMAME plays; its settings say so.

## Its memory

A machine keeps its high scores, settings and audits in its memory (NVRAM), a small file for each ROM. VPX's PinMAME keeps it beside the table's own ROMs (in its `pinmame\nvram` folder) where the table has its own, and else in PinMAME's own `nvram` folder, where the classic VPinMAME keeps it too: a machine's high scores and settings are the same whichever plays. Memory kept by an earlier version of the app comes along the first time a table plays in VPX, where VPX has none, so your high scores and settings carry on.

A machine with no memory at all restores its factory settings the first time it's switched on, and many then show them until they're switched off and on. So before its first game, the app switches it on once by itself, unseen, with VPX 10.8.1: the game then starts as it should.

## High scores

A machine's high scores are read from its memory, through the [Pinball Memory Maps](https://github.com/tomlogic/pinball-memory-maps) (ODbL), and shown on its table's page. See [A table's page](Table-page#achievements-high-scores-and-plays). **Player hub** › **High scores** lists every table's best.

While a game plays in VPX 10.8.1, VPX's own PinMAME add-on shares the machine's game and its high scores with the app's achievements add-on through the same kind of map: VPX's own maps where its PinMAME brings one for the machine, else one the app puts in for it. That's how achievements follow a ROM machine as you play, and how a screenshot is taken when a game makes the high scores.

## AltSound packs

AltSound packs replace a machine's music and sounds with recorded ones, remastered by their makers or made from the ROM itself on the table's page (**Make one from its ROM…**). They play on most machines, though not Stern's SAM machines or Pinball 2000. Getting one, its check, turning it on or off, making one, and which machines it works for: see [AltSound](AltSound).

## Color DMDs

Colorings for a machine's score display (Serum's and PIN2DMD's) go in VPinMAME's `altcolor` folder under the ROM's name, and a machine without one can be colored by hand: **Color it…** on its table's page. See [Score displays](Score-displays#color-dmds).
