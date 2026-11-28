# Media

A table's media are what a front end shows while you browse: its playfield picture or video, backglass, score display, topper, wheel (its logo), flyer and instruction card. The app shows them on tiles, on table pages and in cabinet mode. A PinUP pack is different: it plays during the game. See [PinUP packs](PinUP-packs).

## Where they come from

- **VPS's pictures,** fetched once and kept on your PC.
- **PinUP Popper's media folders,** if you use Popper: its pictures and videos for each game. Popper's playfield media made for a cabinet often lie on their side; the app shows them upright. Copies Popper keeps twice show once.
- **PinballX's and PinballY's media folders,** where either plays your library's tables: their pictures and videos for each game, named as their game lists name it.
- **The app itself,** which draws them for you, or records them from the game for a Future Pinball table: see [Making media](#making-media).
- **What you drop in:** pictures and videos dropped with a table, or onto its row in Tools › Media, go into your front end's media folders for it, named for the table: PinUP Popper's, else PinballX's, else PinballY's. With more than one, Tools › Media › **Media you drop in go in** chooses. Several for one screen all go in, numbered as your front end numbers its own, and you choose which one shows. Those not named the way front ends name their media come as cards in the drop window, to place one by one: on one of your front end's screens, or among [the table's own media](#a-tables-own-media). See [Adding tables](Adding-tables#the-drop-window).

## Making media

**Make media** on a table's page (the film button beside PLAY, or the empty square of a kind it hasn't got) has the app draw a table's media itself, with no window and nothing heard:

- **Playfield picture:** the playfield seen from straight above, flippers at the bottom, cut to the playfield as VPS's pictures show tables.
- **Playfield video, with its sound:** twenty seconds of it starting a game, seen the same way, with its sound (off until you turn it on), to preview it in the app, in front ends and in cabinet mode.
- **Backglass picture:** drawn from the table's backglass file, lit as its script lights it.
- **Score display picture:** its DMD or score digits, once it's up.
- **Wheel:** a round logo for front ends, its backglass in a ring and its name across it.

Tick what to make (**Everything** ticks them all); what you don't tick stays as it is. It takes about a minute a table with the video, and asks before replacing media it made before. Media from VPS and your front ends are never touched. What a table turns out to have none of (no backglass file, no score display) is remembered. On the table's page, what the app drew is marked **drawn by the app**.

**A Future Pinball table's media** are recorded from the game itself, as it plays on your screens:

- **Recorded from the game:** the table plays in its attract mode for about a minute, then closes, with nothing to press. The app waits until the table plays, not its loading screen, and keeps only what Future Pinball and PinUP show: the playfield's picture and twenty seconds of its video, and the backglass's and score display's pictures where it shows them.
- **Wheel:** a round logo, made from its pictures.

See [Future Pinball](Future-Pinball#its-media).

## Media for every table

Tools › Media › **Your media** lists every table's media, kind by kind (playfield, playfield video, backglass, score display, topper, wheel, flyer, instruction card, and its sound), with an empty square for what it hasn't got yet and a dash for what it has nothing to make from. Show **All** tables, those **Missing some**, or those **Complete**, and search them by name. Toppers, flyers and instruction cards come from VPS or a front end, so a table counts as complete without them. **Make missing** on a table's row makes what it lacks, and media files dropped onto its row are added for it. A Future Pinball table's row makes its wheel alone: its other media are recorded from its page's **Make media**.

Above it, **Make missing media for all tables** makes what's missing for every table, one table at a time in the background, and says first how long it will take. The bar at the top of the app shows how far it's got, and what's waiting its turn.

## Choosing which one shows

Where a kind has several, open its stack on the table's page and pick **Show this one**. The one with the blue check shows on its tile, its page and in cabinet mode. It's for browsing only: when you play, the table shows its own backglass and score display.

## Turning a picture

Right-click a picture or video on a table's page to turn it a quarter right or left, or **Show it as its file has it**. The app remembers the turn; the file stays as it is. Where the app stood one of a front end's sideways playfields upright the wrong way, its stack has **Turn it the other way**.

## A table's own media

Pictures, videos and sounds that came with a table, but for none of a front end's screens, can be kept beside it in a `<table> media` folder: the drop window's **The table's media** puts them there. Its pictures and videos show on its page as **Its media**, and its sounds under **Other** › **Its sounds**, each with **Listen**. They're renamed and removed with the table.

## Media for PinUP Popper

PinUP Popper shows the pictures and videos kept in its own folders, so it doesn't see VPS's or the ones the app makes unless they're copied over. Tools › Media › PinUP Popper › **Show the same media in PinUP Popper** does that. When it's on:

- The one with the blue check of each kind is copied into Popper's folder for that screen, named for Popper's name for the game, whenever you choose another, so Popper shows the same one.
- Media you make go in where Popper has none.
- The one Popper had there is kept beside it, numbered (`Name01.png`), never deleted, so choosing it again swaps them back. A copy the app put in there before goes to the Recycle Bin instead, as the picture it copied is still on the table's page.
- VPS's pictures go in as PNGs.

**Put them in for every table now** does it for every table at once, saying first how many would go in. Turned off, Popper's folders are left alone, though **Put it in PinUP Popper's folder**, in a kind's stack on a table's page, still copies that one over on your word.

## Media for PinballX and PinballY

Where PinballX or PinballY plays your library's tables, Tools › Media shows where their media are, and offers **Put the media a table uses in PinballX's folders too** (or PinballY's, or both): the same as for Popper, in each screen's folder, named for the game as their game lists name it. Their own one there before is kept beside it, numbered (`Name 1.png`); a copy the app put in before goes to the Recycle Bin. **Put them in for every table now** does it for every table at once. If either is installed somewhere unusual, its folder can be set in the settings (`[VPI]` `PinballXFolder`, `PinballYFolder`).
