# Adding tables

Drop what you downloaded onto the app: table files, zips, 7-Zip and RAR archives (multi-part RARs too), or a whole folder of downloads at once. Or click **Add tables** at the top. The drop window opens and shows where every file will go **before anything is copied**.

Looking for tables? The Virtual Pinball Spreadsheet and VPIDB (vpidb.org) list them with their files, each linked to where its authors released it.

## What you can drop

What the Virtual Pinball Spreadsheet (VPS) lists for a table:

- the table itself (`.vpx`) and its backglass (`.directb2s`);
- its ROM (a `.zip`), and colorings for its score display;
- AltSound packs and its PinUP pack;
- music, a POV (camera) file, and a replacement script (`.vbs`);
- manuals (`.pdf`) and read-mes;
- pictures and videos for front ends: playfield, backglass, wheel, topper and the rest;
- achievement packs, which are added at once (see [Achievements](Achievements#adding-a-pack));
- VPUniverse's table patches (`.dif`).

## How the app sorts it out

- **Which table each file is for.** A backglass named for its table, the ROM its table's script asks for, a PinUP pack, music, a POV or a coloring: each is tied to its table, whether that table is in the same drop or already in your library. A folder holding many downloads is split into one per table. Files dropped on a table's page are for that table, and so are files dropped on its tile, when Settings › Library allows it.
- **The table's entry on VPS,** for its name, pictures and details. When the match is only a guess, the window says so, and the table's page asks you to confirm it later.
- **Where each file goes,** in VPX's layout, or in each game's own folder when your library keeps a folder for each game (see [Your library](Your-library#a-folder-for-each-game)): the table and its backglass in the tables folder; ROMs in VPinMAME's `roms` folder; AltSound packs and colorings in VPinMAME's folders for them; PinUP packs in `PUPVideos`; front-end pictures and videos in your front end's media folders, named for the table (PinUP Popper's, else PinballX's or PinballY's: see [Media](Media#where-they-come-from)); music in the `music` folder, where the table's script plays it from; a manual beside the table, named for it.
- **Several backglasses.** When a download brings more than one, all of them go in: the one that plays is named for the table, and the others are kept beside it, numbered (`My Table (1).directb2s`), as VPin Studio keeps them. Choose which one plays on the table's page.
- **Pictures for the table.** A picture named after one of the table's own pictures (a different desktop backdrop, say) is offered as a picture for the table: tick it, and it's kept in a `<table> pictures` folder beside the table and played in that picture's place. The table file never changes.
- **Read-mes** are shown, not copied, and so are pictures of a pack's own instructions.
- **Names put right.** A PinUP pack goes in under the name the table's script asks for, which isn't always the pack's own. A helper script named slightly wrongly goes in under the name tables ask for.
- **Checks.** A ROM is checked file by file against what PinMAME expects. A backglass that looks like another machine's, or another release of a machine you already have, is pointed out. A table whose ROM is neither in the drop nor among your ROMs says so: it won't start until its ROM is added.
- **Fitted to its ROM.** Colorings and AltSound packs made for another version of the table's game go where they'll play. A pack goes in under the ROM the table plays, as packs play across a game's versions. A coloring stays that version's, so changing the table's ROM to that version shows it, and one named for no ROM at all gets the name the player looks for. Another machine's are left out, and the window says why.

## The drop window

Each download has a block, with a row for each of its files: what it is, the file, where it goes, and what's there now.

- Untick anything you don't want.
- **Yours to place:** a row the app can't place. Choose where it goes from the list beside it, or **Leave it out**.
- **Choose its table…** ties a download to another table, or to one being added in the same drop.
- **See them on VPS** shows VPS's entry for the download's table, with every file VPS lists for it, beside the downloads.
- The **×** on a download takes it out of the window; the file stays where it was.
- Drop more onto the open window and it joins the same drop.

## When a file is there already

Every file that's already there asks what to do:

- **Replace:** the old one goes to the Recycle Bin.
- **Keep both:** the new one goes beside it, numbered.
- **Append:** for a pack, its new files are added to it (packs often come in parts).
- **Leave:** keep yours.

The bar at the bottom of the window chooses for all of them at once: **Replace**, **Keep both / Append**, **Leave** or **Ask each**. A file that's there already, and the same, says so and needs nothing. Settings › Library › Folders › **When a file is there already** sets what's picked to start with, or **Ask each time**.

## Adding

**Add** copies the ticked files. Replaced files go to the Recycle Bin, where you can get them back. A notice says what was added, and the library shows the new tables. When nothing ticked is new, the button says so.

Tick **Test new tables after adding them** and each new table runs for about 15 seconds with no window (its physics, its script, and its machine for a ROM table), and you see how it went.

## VPUniverse's table patches

Some table mods are shared as a patch (`.dif`) of the original table. Drop the patch with the table it's for, or onto that table's page: the app makes the modded table beside your original, checks it opens as a table, and leaves the original as it is.
