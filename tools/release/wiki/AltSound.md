# AltSound packs

A ROM machine's sound comes from its sound board, which PinMAME runs from the ROM. An AltSound pack replaces it with recorded files: the machine's music, callouts and effects, each a sound file the pack's list names, played in place of PinMAME's sound as the game asks for them. Some packs are remastered, re-recorded or remixed by their makers; others are made from the ROM itself, and can then be changed sound by sound.

## Getting a pack

- **Download one:** the table's page on the Virtual Pinball Spreadsheet (VPS) lists the AltSound packs made for it. **Find** on the table's AltSound row looks there.
- **Drop it onto the app,** with its table or onto the table's page. It goes into VPinMAME's `altsound` folder, in a folder named for the ROM (`altsound\<rom>`), and is checked as PinMAME reads it.
- **Made for another version of the game:** dropped with its table, it goes in under the ROM the table plays, as packs play across a game's versions. A pack for another machine is left out, and the drop window says so.
- **A pack that's there already:** the drop window asks: **Replace** (the old one to the Recycle Bin), **Keep both**, **Append** (its new files added) or **Leave**.
- **Or make one from the table's ROM:** see below.

## Where the player looks for packs

A pack is found under its ROM's name, as VPX looks: in an `altsound` folder beside the table, then in its `pinmame` folder, then in the folder VPX's AltSound setting names; then beside the player, and in VPinMAME's `altsound` folder, where the drop window puts packs, as VPin Studio does.

## The kinds of pack

All three kinds play:

- **`altsound.csv`**, the original list of a pack's sounds;
- **`g-sound.csv`**, the newer G-Sound list, with its music, callouts and effects on channels of their own;
- **PinSound's folders**, with their own gain and ducking files.

The first time a pack plays, its settings file (`altsound.ini`) is written into its folder, as VPinMAME writes it.

## Its check

A table's page checks its pack on its AltSound row, and says what it found:

- **Complete:** its sounds are there. A pack missing a few of the sounds its list names is common, the pack's own slip: the row says so, and those few don't play.
- **Needs a look:** it won't play anything, because its list names no sounds, its folders hold no sounds, or it has no list at all (neither `altsound.csv` nor `g-sound.csv`) and none of PinSound's folders.
- **Not right:** half or more of the sounds its list names are missing, so most won't play: it may be incomplete, or made for another version.

Tables whose pack needs a look gather under **Needs attention** in the library. **Show** opens the pack's folder, and the bin moves it to the Recycle Bin.

## Every table's pack: Tools › AltSound

**Tools** › **AltSound** lists every ROM table's pack, checked as the player reads it. **Which tables show** picks **All ROM tables**, those that **Have a pack**, those that **Need a look**, or those that **Can make one** from their ROM.

- **A table with a pack:** its check and its file. **Listen** for a pack made from its ROM, **Show** for its folder, and the bin to move it to the Recycle Bin, after which the machine plays its own sound (a pack other tables share says so first).
- **A table without one:** **Find on VPS** where VPS lists one, **Make one…** where one can be made from its ROM (see below), or why not. A table whose ROM isn't there yet says so.

## Turning packs on or off

A pack plays whenever its game has one, unless you say otherwise:

- **Every game:** Settings › Sound › **Play a game's AltSound pack when it has one**.
- **Every machine, PinMAME's way:** Settings › Machines › **Its sound comes from**: its own sound, or its AltSound pack if it has one.
- **One table:** its settings › **AltSound pack**: your usual, on or off.

Turned off, the machine's own sound plays. While a pack plays, the pause menu's **Machine (AltSound)** level sets its volume.

## Making a pack from a table's ROM

A machine without a pack can have one made from its own ROM:

1. On the table's page, the AltSound row offers **Make one from its ROM…** for a machine whose pack would play (see below); so does **Make one…** on its row in Tools › AltSound.
2. The window says what happens: every sound the machine's sound board can play is recorded, one at a time, into a pack the table plays in place of PinMAME's sound, at the volume the machine plays out of the box.
3. **Make the pack.** It takes a minute or so, in the background; the bar at the top of the app counts its sounds.

The pack goes into VPinMAME's `altsound` folder under the ROM's name, never over a pack that's there. The table's row then says it was made from its ROM, with **Listen**, which opens a page of all its sounds in your browser.

- **It sounds about the same** as the machine's own sound: the point is that each sound is now a file. Replace one with your own, under the same name, and the table plays yours.
- **Its size:** up to a few hundred MB. Music is cut to its loop; a song with an intro is kept as the intro and a few minutes of its loop, which is what takes the room. Attack from Mars's pack holds 576 sounds and comes to 429 MB.
- **Made by rom2altsound** (BSD-3-Clause), by Sylvain Gargasson, which comes with the app.

## Which machines it works for

Packs play on the machines whose games send their sound board commands a pack can follow: Williams and Bally's WPC machines, Williams' System 9 and 11, Data East, Sega and Stern's Whitestar, early Bally machines (and their 6803 boards), Gottlieb, Zaccaria and the other older makers.

Williams' older System 3 to 7 machines are half and half: they play packs made with PinMAME's own AltSound tool, as the packs shared for them are, but not packs made from their ROM, because what their games send the sound board carries other signals too. On those tables the app doesn't offer to make one, and their AltSound row says why.

They don't work, and a table's page says so, where:

- **the machine has no sound board for a pack to follow:** its own processor makes its sounds, as on Stern's SAM machines (2006 to 2014: AC/DC, Metallica, The Walking Dead and others), or it rings chimes;
- **it's a Pinball 2000 machine,** whose sounds don't go through the commands a pack follows;
- **its game reaches the sound board some other way,** or what it sends isn't what a pack made from its ROM is keyed by.

Those tables play their own sound.

## When a pack stays silent

- **Check it's on:** Settings › Sound, Settings › Machines, and the table's own settings.
- **Check its row** on the table's page: a pack missing sounds, or with no list, says so.
- **Check its folder's name:** it must be the name of the ROM the table plays (its ROM row shows it). A pack for another version of the ROM is found only under that version's name.
- **Check the machine:** Stern SAM and Pinball 2000 machines can't play packs.

Still not playing? **Report a problem with it…** in the table's **…** menu sends us what's needed to find out why.
