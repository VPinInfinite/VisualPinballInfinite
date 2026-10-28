# A table's page

Click a table in the library to open its page. The arrow keys, or a controller, move to the previous and next table.

## PLAY

**PLAY** starts the table on your screens, with its backglass, score display and PinUP pack where Settings › Screens puts them. The app steps aside while you play and comes back when you quit, on the same table. See [Playing](Playing).

For a table with achievements, PLAY says whether it plays **Hardcore**, as its author made it, or **Softcore**, and why (point at it to see). Its **▾** plays one game either way on purpose: **Play Hardcore** leaves your table settings, script edits and physics changes out for that game, and **Play Softcore** keeps its achievements as softcore, for a guest, say. See [Achievements](Achievements#hardcore-and-softcore).

## Its pictures and videos

The big picture shows its playfield video or picture; the arrows on it step through all its media. Below it, a stack for each kind (playfield, backglass, score display, topper, wheel and the rest) holds what's there: VPS's pictures, PinUP Popper's media for it, and media the app made. An empty square stands for each kind it hasn't yet.

- **Choosing the one shown.** Open a stack and pick **Show this one**: the one with the blue check shows on its tile, its page and in cabinet mode. It doesn't change the table, or what plays in the game. **PinUP Popper shows this one** marks the one in Popper's own folder. To put yours there, see [Media](Media).
- **Right-click** a picture or video to turn it a quarter right or left, or **Show it as its file has it**. Turning only changes how the app shows it; the file stays as it is. The same menu offers **Hide it from this page**, **Show the file** in its folder, or **Delete…** to the Recycle Bin. VPS's pictures can only be hidden.
- **Sound:** a video with sound plays muted; the speaker at its bottom right turns the sound on.
- **Make its media…** has the app's own player make what it lacks: see [Media](Media).

## Its facts

Its name, maker, year, type, version and authors, from VPS, with links **On VPS** and **On IPDB**. When its VPS match is only a guess, the page asks **Is this …?** first: see [Your library](Your-library#matching-tables-to-vps).

**Its rating:** the five stars under its version and authors. Click a star to give the table that many, and click the same star again to clear it. Beside them: **Your rating**, **Not rated yet**, or **Its rating in PinUP Popper** (or PinballX, or PinballY) for stars you gave it there. See [Your library](Your-library#ratings).

## What it holds

Every file it plays with, each checked: its backglass, ROM, PinUP pack, AltSound pack, color DMD, achievement packs, POV, settings and more.

- **Show** opens a file's folder. For something missing, **Find** looks for it on VPS.
- **The backglass row:** its file, and **Change…** where a download brought several. See [Its backglass](#its-backglass).
- **The ROM row:** its check, and **Change…** to choose another ROM. See [ROM machines](ROM-machines).
- **The AltSound row:** its pack's check, **Listen** to hear its sounds, and for a machine without one, **Make one from its ROM…** See [AltSound](AltSound).
- **The PinUP pack row:** its layout, **Layout…** and an **Enable** switch that turns the pack off for this table alone. A pack named differently from what the table asks for says so, with **Rename it**. See [PinUP packs](PinUP-packs).
- **The Color DMD row:** its coloring, and **Color it…** to color the machine's display yourself. See [Score displays](Score-displays#color-dmds).
- **The Achievements rows:** each of its packs, with its kind on VPIDB and how many you've earned of it, or **Add a pack…** where it has none. Its bin deletes the pack for good, after asking. See [Achievements](Achievements#deleting-a-pack).
- **The bin** on a row moves that one file to the Recycle Bin, after asking.
- **Other:** what else it has. That's the music its script plays, scripts of its own, the folders its DMD pictures come from, other files beside it named for it, and what the drop window added for it.
- **More on VPS:** everything VPS lists for it, kind by kind, once its VPS match is confirmed, and first the achievement packs VPIDB has for its game, each with **Add**. See [Achievements](Achievements#adding-a-pack).

## Achievements, high scores and plays

One card holds its **Achievements** and its **High scores**: click either at its top.

- **Achievements:** its pack's achievements, those you've earned lit up. See [Achievements](Achievements#on-a-tables-page). The trophy button beside PLAY jumps to them.
- **High scores:** **Yours** lists your best games here, from its play log. **All** lists the scores the table keeps: a ROM machine's are read from its memory, through the Pinball Memory Maps, and a table's own come from what its script saves. Tick **Played in VPX** to see the scores you made in VPX beside them.
- **Your plays:** its play log, each time you played it.

## Its settings

**Table settings** (the sliders button beside PLAY) holds the settings for this table alone, over your usual ones:

- **Its own options:** the options its author built into its script.
- **Its view** and **Its screens:** your usual screens, a preset, or its own.
- **Its look:** tone mapper, exposure, scene lighting and difficulty, VPX's own settings for each table. The difficulty is how steep the table is, how big the flippers and how much the ball scatters.
- **Its packs:** its AltSound pack, color DMD and PinUP pack, each on or off.
- **Screenshots:** whether to take one when a game makes the high scores, and which screens.
- **Physics:** the table's own, or the physics file beside it that plays (**Show**), and **Change…**: see [Its physics](#its-physics). A `.vpp` dropped on the table goes beside it.
- **Reaching outside its files:** off unless you turn it on. See [Settings](Settings#general).
- **The machine,** for a ROM table: its sound, its volume, older machines' recorded sounds, skipping its start-up tests, PinMAME's cheat mode, and its color on a real DMD panel.

They're kept beside the table in `<table>.ini`, as VPX keeps them. The machine's settings go in `Machine settings.ini`.

## Its backglass

When a table has more than one backglass, its **Backglass** row says how many more are kept, and **Change…** shows a picture of each. **Play this one** makes it the one that plays, in Visual Pinball X too; **Play none** keeps them all beside the table and plays none (a table with a PinUP pack shows that instead).

## Its script

**Edit its script** (the `<>` button) opens a table's script in two tabs:

- **Its options:** the settings its authors left near the top of its script for players to change, each with what its comment says, as a box, a switch or a list of choices. Its ROM is there too, with **Change…**, and a note pointing to its settings where it has an options menu of its own.
- **Its whole script:** every line, colored, with line numbers, **Find** (Ctrl+F) and **Go to line**.

Both tabs show the same script: change an option and its line changes in the whole script. **Save** keeps it beside the table as `<table>.vbs`, which plays in place of the table's own script, in Visual Pinball X too; the table file itself never changes. **Check it** plays the script for a few seconds with no window and lists any errors by line; click one to go there. **Open in Notepad** opens the same file there. **Put back the table's own script** sends the one beside it to the Recycle Bin.

When a table you changed is updated (its file replaced by a newer one), its page asks whether to carry your option changes over to its new script.

## Its physics

**Change…** on the Physics row of its settings opens its physics window. It shows the values its author set beside the ones that play: for the table (gravity, friction, bounciness, scatter, and its slope at the easiest and hardest difficulty) and for its flippers (mass, strength, bounciness, scatter, how they hold at the end of their stroke, how fast they fall back, friction, and how fast the coil comes to full strength). A value that differs from its author's is marked.

- **Save beside it** keeps yours as the table's own physics file (`<table>.vpp`), which VPX reads too, from the next game on. The table file stays as it is.
- **Back to its author's** sends that file to the Recycle Bin; **Fill in its author's** puts its values back in the boxes.
- **Presets:** **Keep these as a preset…** keeps the values under a name, to **Use** on any table. **Bring one in…** adds a physics file from elsewhere, one shared or exported from VPX's editor.

The window says when the table plays `autovpp.vpp`, the physics file for every table in its folder without one of its own, or one of VPX's physics sets from your settings, over everything else (see [Settings](Settings#physics-sets)). A table without flippers has no physics of its own to change. Achievements earned with changed physics count as softcore (see [Achievements](Achievements#hardcore-and-softcore)).

## Test it

**Test it** runs the table for about 15 seconds with no window (its physics, its script, and its machine for a ROM table), then says how it went.

## The buttons beside PLAY

**Favourite** (the heart), **Edit its details**, **Table settings**, **Edit its script**, **Show its files**, **Make media**, **Its achievements** (the trophy), and **…**, which holds **Add to a playlist…**, **Open on VPS's site**, **Make media with our player…**, **Open in the table inspector**, **Show the player's last log**, **Report a problem with it…**, **Copy where its file is** and **Remove from library…**. Its screenshots open from its page too.
