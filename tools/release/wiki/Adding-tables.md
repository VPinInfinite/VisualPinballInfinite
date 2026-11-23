# Adding tables

Drop what you downloaded onto the app: table files, zips, 7-Zip and RAR archives (multi-part RARs too), or a whole folder of downloads at once. Or click **Add tables** at the top and drop them onto its window. The drop window opens and shows where every file will go **before anything is copied**.

Looking for tables? The Virtual Pinball Spreadsheet lists them with their files, each linked to where its authors released it. The app's **VPS browser** shows it all: see [Your library](Your-library#the-vps-browser).

## What you can drop

What the Virtual Pinball Spreadsheet (VPS) lists for a table:

- the table itself (`.vpx`) and its backglass (`.directb2s`);
- its ROM (a `.zip`, or a set's files), and colorings for its score display;
- AltSound packs and its PinUP pack;
- music, a POV (camera) file, a physics file (`.vpp`), its own settings file (`.ini`), a replacement script (`.vbs`) and the scripts it loads;
- manuals (`.pdf`) and read-mes;
- pictures, videos and sounds for front ends: playfield, backglass, wheel, topper and the rest;
- achievement packs, which are added at once (see [Achievements](Achievements#adding-a-pack));
- VPUniverse's table patches (`.dif`).

## How the app sorts it out

- **Which table each file is for.** A backglass named for its table, the ROM its table's script asks for, a PinUP pack, music, a POV or a coloring: each is tied to its table, whether that table is in the same drop or already in your library. A folder holding many downloads is split into one per table. Files dropped on a table's page are for that table, and so are files dropped on its row on one of the **Tools** pages, and on its tile when Settings › Library allows it (**Files dropped onto a table's tile are added to that table**).
- **The table's entry on VPS,** for its name, pictures and details. When the match is only a guess, the window says so, and the table's page asks you to confirm it later.
- **Another release of a machine you have.** The window says so and asks: **Add it as a build of …** keeps both, the new one a build of the same table (see [Your library](Your-library#a-table-with-several-table-files)), or **Replace …** puts the new one in its place: the one you have and its own files (its backglass, its settings file) go to the Recycle Bin once the new one is in, and your plays, heart, stars and playlists carry over to it.
- **Where each file goes,** in VPX's layout, or in each game's own folder when your library keeps a folder for each game (see [Your library](Your-library#a-folder-for-each-game)): the table and its backglass in the tables folder; ROMs in VPinMAME's `roms` folder; AltSound packs and colorings in VPinMAME's folders for them; PinUP packs in `PUPVideos`; front-end pictures and videos in your front end's media folders, named for the table (PinUP Popper's, else PinballX's or PinballY's: see [Media](Media#where-they-come-from)); music in the `music` folder, where the table's script plays it from; a manual beside the table, named for it.
- **ROM sets** go in as a zip, the only kind PinMAME reads. A set from another kind of archive, or ROM files dropped loose, is made a zip named for the set. A set that comes with a read-me (a maker's code update often has one) is still taken for a ROM set: a zip goes in as it is, and a set made a zip leaves the read-me out.
- **Several backglasses.** When a table's downloads bring more than one backglass, all of them go in, a 2-screen and a 3-screen one dropped together included. The one VPX plays is named for the table: the one made for your screens where their names say so (`2scr` or `3scr`, three screens being where your score display has a window of its own), else the one already there, else the first. The others are kept beside it, numbered (`My Table (1).directb2s`), as VPin Studio keeps them. Choose which one plays on the table's page.
- **Several of a kind.** Two wheels for one table, say, or two playfield videos, whether in one download or several: all go in. The first is named for the table, and the others are kept beside it, numbered as your front end numbers its own (`My Table01.png` in PinUP Popper's folders, `My Table 1.png` in PinballX's or PinballY's). Choose which one shows on the table's page.
- **Media to place yourself.** Pictures, videos and sounds that aren't named the way front ends name their media come as cards, to place one by one: see [The drop window](#the-drop-window).
- **Pictures for the table.** A picture named after one of the table's own pictures (a different desktop backdrop, say) is offered, unticked, as a picture for the table. Ticked, it's kept in a `<table> pictures` folder beside the table. VPX plays a table's own pictures, so **Make the copy** on the table's page plays yours in a copy of the table: see [A table's page](Table-page#your-pictures-in-a-copy-of-the-table). The table file never changes.
- **A settings file that sets its own windows.** When a table's own settings file (`<table>.ini`) places VPX's windows for that table, the window shows how (**It sets its own windows**), as VPX would use them instead of your screens. **Use my screens** (the usual) leaves its window settings out as it goes in; **Keep the table's** copies it as it is. Its other settings, such as the table's own options, come in either way.
- **Read-mes** are shown, not copied (**Its read-me**, under the download), and so are pictures of a pack's own instructions.
- **Names put right.** A PinUP pack goes in under the name the table's script asks for, which isn't always the pack's own. A helper script named slightly wrongly goes in under the name tables ask for.
- **Checks.** A ROM is checked file by file against what PinMAME expects. A backglass that looks like another machine's is pointed out. A table whose ROM is neither in the drop nor among your ROMs says so: it won't start until its ROM is added.
- **A coloring zipped under a ROM's name.** A zip named for a ROM that holds only a coloring's files (PIN2DMD's `.pal`, `.vni` or `.pac`, Serum's `.cRZ`) isn't a ROM: the window says so and leaves it out, and the ROM you have stays. Where the same coloring comes as files of their own in the download, those go in place; else the window says to unzip them into VPinMAME's `altcolor\<rom>` folder.
- **Fitted to its ROM.** Colorings and AltSound packs made for another version of the table's game go where they'll play. A pack goes in under the ROM the table plays, as packs play across a game's versions. A coloring stays that version's, so changing the table's ROM to that version shows it, and one named for no ROM at all gets the name VPX looks for. Another machine's are left out, and the window says why.
- **Two files for one place.** Where two files would go to the same place, only one can be there: the second starts unticked. Tick both and the window says so, and **Add** waits until one is unticked.

## The drop window

Each download has a block: its name, what it holds, and either the table it brings, matched to VPS (found, a guess to confirm on its page once it's added, or not on VPS; its machine; **In your library already**), or **For**, the table its files are for. Then a row for each of its files: what it is, the file, where it goes, and what's there now.

- Untick anything you don't want.
- **For** ties a download to another table: one in your library, or one being added in the same drop.
- **Yours to place:** a row the app can't place. Choose where it goes from the list beside it, or **Leave it out**.
- **Media:** under a download's files, a box, folded to begin with, holds a card for each picture, video and sound that isn't named the way front ends name their media: its picture (or video, or a sound's mark), its size and shape, and the app's guess of where it goes and why. **Our guess** means its name says so after all (`logo` for a wheel, say, or a screen's name misspelt). **Check this** means it only looks so: by its shape, or, for a table already in your library, because it looks like the table's own backglass. **No place for it** means your front end has no place for what it looks like, and **Not told** that nothing says what it is. Each card starts at the app's guess, else at **The table's media**; choose its place: one of your front end's screens, **The table's media** (kept beside the table in a `<table> media` folder and shown on its page: see [Media](Media#a-tables-own-media)), **The table's screenshots**, or **Leave it out**. Click a card's picture to see it larger, with every place as a button. **Place all as** places every card at once.
- **See them on VPS** shows VPS's entry for the download's table, with every file VPS lists for it, beside the downloads.
- The **×** on a download takes it out of the window; the file stays where it was.
- Drop more onto the open window and it joins the same drop.

## When a file is there already

Every file that's already there asks what to do, on its row:

- **Replace it:** the old one goes to the Recycle Bin (for a pack, the whole pack is replaced).
- **Keep both:** the new one goes beside it, numbered.
- **Append:** for a pack, its new files are added to it (packs often come in parts), one of the same name replaced.
- **Leave it:** nothing of it is copied.

The bar at the bottom of the window chooses for all of them at once: **Replace**, **Keep both / Append**, **Leave** or **Ask each**. A file that's there already, and the same, says so and needs nothing. Settings › Library › Folders › **When a file is there already** sets what's picked to start with, or **Ask each time**.

## Adding

**Add** copies the ticked files, and says how many. Replaced files go to the Recycle Bin, where you can get them back. When all goes well, the window closes, a notice says what was added, and the library shows the new tables. When something needs a look (a table whose VPS match is a guess, a test that didn't go well, a file that couldn't be copied), the window stays and shows what was copied, download by download, with **Open its page** for a table to confirm; **Done** closes it. When the button can't add yet, it says why: **Nothing new to add**, **Choose for 1 there already**, or **Two go to the same place**.

Tick **Test new tables after adding them** and each new table runs in VPX for about 10 seconds, unseen and unheard (its script, and its machine for a ROM table), and you see how it went. Tests run in VPX 10.8.1, so it needs to be in your library.

## VPUniverse's table patches

Some table mods are shared as a patch (`.dif`) of the original table. Drop the patch with the table it's for, or onto that table's page: the app makes the modded table beside your original, checks it opens as a table, and leaves the original as it is.
