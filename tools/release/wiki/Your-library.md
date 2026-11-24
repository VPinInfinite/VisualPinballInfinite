# Your library

The library shows every table in your tables folder: its picture, maker, year and what it holds. Its pictures and facts come from the Virtual Pinball Spreadsheet (VPS), from your front end's media (PinUP Popper, PinballX or PinballY), or from the app itself. **Library**, **VPS browser**, **Tools**, **Table editor** and **Player hub** at the top switch between them.

## Tiles or a list

- **Tiles** show a picture of each table: its playfield, backglass or wheel (**Playfield**, **Backglass** or **Wheel** above the tables), at the size you set (**Size**). Under it are its name, maker and year, its stars, tags for what it holds (backglass, ROM, PinUP pack, AltSound pack, color DMD, an achievement pack) and VPS's tags for its release (VR, FSS, nFozzy, SSF, DOF and more: point at one to see what it means). Point at a tile for its play button.
- **The list** shows a row for each table: maker, year, rating, version, authors, what it holds (backglass, ROM, PinUP pack, AltSound pack, color DMD, achievement pack) and when you last played it. **Columns** chooses which columns show, VPS's tags and each kind of media among them. Click a column's heading to sort by it.
- **Sort** by name, year, maker, recently added or rating.
- **Has**, above the tables, shows only the tables with a backglass, ROM, PinUP pack, AltSound pack, color DMD or achievement pack. Tick several to see only the tables that have them all.
- A heart on a tile marks a favourite. **2 builds** (or more) marks a table with several table files, shown once: see [A table with several table files](#a-table-with-several-table-files).
- A yellow or red triangle on a tile, or beside a name in the list, means the table needs a look; red means it won't play as it should. Point at it to see why, and see [Troubleshooting](Troubleshooting).

Settings › Library sets what the library opens as (**It opens as**: tiles or a list), what tiles show and their size.

## Groups down the side

- **All tables**, **Favourites**, **Top rated** (the tables you've given stars, the most stars first), **Recently played**, **Recently added** (the newest twenty), and **Needs attention**: the tables needing a look, each kind on its own (**VPS match to confirm**, **Files missing**, **Files not right**, **Failed its test**, **Couldn't be read**, **Here twice**). Where PinUP packs are there but named so VPX won't find them, **Rename all** renames them to the names their tables' scripts ask for.
- **Smart playlists:** VPin Workshop's tables, Originals, The 1990s, and playlists made from VPS's tags: a release's features such as SSF, VR or FastFlips, or a machine's themes. The five biggest show until you choose others in Settings › Library › Playlists › **Smart playlists from VPS's tags**.
- **Your playlists:** **New playlist** makes one. **Add to a playlist…**, in a table's **…** menu or on its right-click, puts a table in one or takes it out. Settings › Library › Playlists renames, reorders or deletes them (their tables stay in the library).
- **Browse by** maker, decade or type.
- **Hidden:** the tables you've hidden, once Settings › Library › Playlists › **Show the Hidden playlist in the library** is ticked.

**Smart playlists** and **Your playlists** fold away, and stay as you left them.

## A table with several table files

Most tables have one table file. For one with more (another release, a mod, a copy), Settings › Library › **Tables with more than one table file** chooses how the library shows them:

- **Each table file on its own** (the usual): every table file has its own tile and page.
- **One tile for each table:** a table shows once, however many table files you have for it. Its page has a tab for each, its builds.

Table files belong to one table where VPS matches them to the same machine, surely or because you chose the match. A table file VPS doesn't know stays on its own.

With one tile for each table, the tabs sit at the top of **What it holds** on the table's page, the newest version first. Picking one opens its page, which follows that build: its files, what PLAY plays, your plays and its achievements.

- **Make it the main build:** its tile shows and plays this one, whichever you played last. Without a main build, the tile shows the one played last, else the newest version.
- **Not this table:** this table file becomes a table of its own (the notice has **Undo**).
- **Add a build…:** puts another table file of your library with this table, for one VPS doesn't match to it.

The heart, the stars and playlists go with the table; your plays and scores stay with each build. **Needs attention** still lists each table file on its own.

**A table file saved twice.** In either view, a table file that looks like another saved again (the same name but for a copy's mark, such as ` (1)` or ` - Copy`, and the same ROM) shows the triangle, and its page says it's **here twice**, with both files' sizes, dates and your plays side by side:

- **Keep the one you've played** (or **Keep this one**, **Keep the other one**) opens the window to remove the other: see [Removing a table](#removing-a-table).
- **Keep both as builds** keeps them both, and it isn't asked again.
- **Not now** asks again the next time the app starts.

## Ratings

Give a table one to five stars on its page: see [A table's page](Table-page#its-facts). A table shown once with several builds has one rating. Ratings you gave tables in PinUP Popper (VPin Studio keeps its ratings there too), PinballX or PinballY show until you rate a table here yourself. VPX keeps no ratings.

The first time you rate a table, where one of those front ends is found, the app asks **Put your ratings in PinUP Popper too?** (naming the ones it found):

- **Yes, put them there too:** the stars you give here show there as well, and a rating you change there shows here.
- **No, keep them here:** they stay in this app.

Settings › Library › Ratings changes it whenever you like (**Put my ratings in PinUP Popper's list too**, naming the front ends it found). A front end that's open while you rate keeps its own rating until it's closed and the library is read again. Your ratings stay on this PC: in the app, and in those front ends' lists if you choose.

## Search

The search at the top finds tables by name, file name, maker, author or designer, and ROM. As you type, it lists the tables, makers, people, ROMs and files (backglasses, PinUP packs, AltSound packs, POVs and physics files) that match: pick one to go there. On the VPS browser, it searches VPS instead.

## A tile's right-click

On a tile, or a row of the list: **Open its page**, **Open in the table editor**, **Play**, **Play in Hardcore** (as its author made it: your table settings, script edits and physics changes left out for that game), **Add to a playlist…**, **Hide** (it then shows only in the Hidden playlist) or **Unhide**, and **Remove from library…**

## Matching tables to VPS

Each table is matched to its entry on VPS by its file's name, the name, version and authors inside the table, and its ROM. That gives it its picture, facts and authors. When the match is only a guess, its page asks **Is this …?** Answer **Yes, that's it**, or **Not this one** and pick it yourself: search VPS by a machine's name, maker, year or ROM, then choose which of its releases you have, or say **It isn't on VPS**. A page whose match is sure has **Not the right one?** beside it. There are two ways to keep your choice:

- **Associate in VPI only:** the app remembers it, and nothing is renamed.
- **Overwrite for all players (VPX, PinMAME, VPin Studio):** the table's files are renamed to VPS's name for it, `Name (Maker Year)`, so other programs match it the same way. PinUP Popper's game list is changed with them, and PinballX's or PinballY's where they list it, each list kept as a copy first. Nothing is renamed while one of those programs is running. **Put the names back**, in the table's settings, undoes it.

**Edit its details** on a table's page sets your own name, maker, year, type, version or authors over VPS's. They're kept in the app only: the table's file and other programs keep theirs. **Back to VPS's details** undoes them.

## The VPS browser

**VPS browser** at the top has three views:

- **All machines:** every machine VPS lists, as cards. Filter them by maker, era, machine type and table format, show only those **Not in my library**, and sort by newest release, name, year or most releases. **VPS has** shows only the machines VPS has a backglass, a PinUP pack or another kind of file for, each kind you tick shown as a chip.
- **For your tables:** your library against VPS's list. For each kind of file (backglasses, ROMs, PinUP packs, AltSound packs, color DMDs, POV files, wheels, toppers), how many of your tables have one, out of those VPS lists one for, and which tables have none: choose one to see VPS's files of that kind beside the list.
- **Recommended:** machines you don't have, picked by what you play (the makers, eras, themes and designers of your favourites, your best-rated and most-played tables) and by what VPS shows (VPin Workshop's tables, many releases, a new one this month, a ROM you have with no table), each saying why. **New list** picks again.

A machine's card opens a panel beside the cards: who designed it, every release and file VPS lists for it, **On VPS's site**, and **Open in my library** for one you have.

### A file's page inside the app

A file's **Get**, in the VPS browser or on a table's page, opens its page inside the app: VPUniverse's, VPForums' or wherever its authors released it. It's a browser of the app's own, with back, forward and **Load it again**. Sign in to a site there once and you stay signed in, apart from your usual browser: **Sign out** signs you out of every site shown there, and **In my browser** opens the page in your usual browser. The button at its left goes back to what showed before.

What you download there goes into `Downloads\Visual Pinball Infinite`, the bar at the top showing how much has come, and then waits under **to import** at the top of the app, each download with the table it was got for. **Import all** puts them in place through the drop window (see [Adding tables](Adding-tables)), each dropped on its own table, and what's added leaves that folder for the Recycle Bin. The cross beside a download takes it out.

## Tools

**Tools** at the top looks after every table at once, each page with a search box: **ROMs** (see [ROM machines](ROM-machines)), **PinUP packs** (see [PinUP packs](PinUP-packs)), **Media** (see [Media](Media)), **Color DMD** (see [Score displays](Score-displays#color-dmds)) and **AltSound** (see [AltSound](AltSound)). A table's own page does the same for that table.

## The table editor

Tables are made in VPX's editor. The app's own editor has two parts so far: the **table inspector** and the **achievements editor**. **Table editor** at the top opens the table you had there last (the first by name, the first time), and **Another table** opens a different one. Then:

- **Its parts:** the table seen from above, with every part listed by kind beside it (**Find a part by name**). The playfield shows **Its look**, the table drawn from straight above by the app (**Draw its look** makes that picture the first time, in a few seconds), or its parts' **Outlines** alone. Right-click a part to hide it, to reach the parts under it.
- **Its achievements:** the achievements editor, which makes the table's achievement packs: see [Achievements](Achievements#making-a-pack).

Click a part, on the playfield or in the list, to see:

- what it is, whether it's seen or hidden, and its layer;
- what an achievement can count of it (each hit, a spinner's spins, a slingshot's kicks), with the line a pack names it by (**In a pack**);
- every setting the table file keeps for it, read only.

A table's **…** menu › **Open in the table inspector**, or **Open in the table editor** on its right-click, opens it there too. The table file itself never changes.

## The Player hub

**Player hub** at the top:

- **High scores:** every table's best, as its machine's memory or its script keeps it, beside your own best from the games you played from the app, and what keeps it. Show the tables **With scores**, those **Played here**, or **All tables**. A table's page has its whole list.
- **Achievements:** your packs, each opening to show what you've earned of it, and how many you've earned in **Hardcore** and in **Softcore** (click one to count only those). **Add a pack…** adds one, and a pack's bin deletes it for good. See [Achievements](Achievements).
- **Stats:** how many tables you have and have played, time played, your most played tables, your latest plays, time played each week for the last twelve weeks, and your library by decade and by maker.

## A folder for each game

Settings › Library › **Where tables keep their files** chooses between two layouts:

- **Shared folders:** tables together, ROMs in VPinMAME's folder, PinUP packs in PinUP's, as VPX and PinUP Popper lay them out. Every VPX plays tables kept this way, 10.8.0 and 10.8.1 alike.
- **A folder for each game:** each game's tables in a folder of their own with their ROM, packs, coloring and music, easy to copy, back up or delete.

Choosing a folder for each game shows what would move first (**See each game's folder**), then **Move them** moves the files, showing how far it's got, and PinUP Popper's game list follows them. **Put them back** undoes it the same way. Nothing is deleted.

A folder for each game needs VPX 10.8.1. VPX 10.8.0, and PinUP Popper or any program that starts tables through VPX 10.8.0, can't start ROM tables kept this way, nor find their PinUP packs, AltSound packs or colorings: if you play some tables in VPX 10.8.0, keep shared folders. Nothing needs this layout: VPX 10.8.1 and VPin Studio read both.

## Removing a table

Right-click its tile, or use **Remove from library…** in its page's **…** menu. The window lists every file that goes with it, each with a tick box:

- Anything another table also uses starts unticked, with which table uses it.
- Its high scores, screenshots and play log start unticked too. Its play log, ticked, is forgotten for good.
- Everything else ticked goes to the Recycle Bin, where you can get it back.

Its game comes off PinUP Popper's list too, and off PinballX's or PinballY's where they list it, each list kept as a copy first.
