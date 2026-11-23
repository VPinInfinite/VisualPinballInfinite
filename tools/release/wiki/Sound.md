# Sound

Settings › Sound: which speakers each part of a table plays through, and how loud. These are VPX's own settings, which it reads whenever a table starts. **-** and **=** change both volumes while you play.

## Speakers

Two outputs, as in VPX:

- **The playfield's sounds:** the ball, the flippers, bumpers and the rest of the table.
- **The backglass's sounds:** music, the machine's own sound from its ROM, PinUP packs, and music played through Media Player.

Each is Windows' default output, or one you choose by name. A cabinet often sends the playfield's sounds to the speakers or shakers in its body and the backglass's to the speakers in its backbox. One set of speakers can play both. On Windows' default, VPX plays through whatever Windows plays through as a table starts, which Windows can change by itself (a screen with speakers waking up, headphones plugged in): choose your speakers by name to keep them.

## Bass shaker

A copy of the playfield's sounds (the flippers, the ball, the bumpers) sent to a bass shaker or another tactile transducer on a sound output of its own, so you feel them: **Its output** (**None — off** until you choose one), **Its layout** (one of the layouts below) and **Its volume**. It's VPX's third output, in a [newer VPX 10.8.1 build](Installing-and-first-setup#which-vpx-1081-build): with an older one, the page says so.

## The playfield's speaker layout

- **Stereo:** two speakers, a desk's, a TV's, or a cabinet's own pair. The table's sounds move left and right.
- **Stereo on the rear outputs:** for a cabinet with one sound card, the table's sounds on its rear outputs, the front ones left for the backglass.
- **Surround, the rear speakers by the lockbar:** a 5.1 or 7.1 set with the front pair at the table's far end and the rear pair by you, as in a home cinema: the ball rolling toward you comes from behind.
- **Surround, the front speakers by the lockbar:** the same, plugged in the other way round.
- **Cabinet surround (SSF), VPX's older mixing,** and **VPX's newer mixing:** for cabinets with speakers or exciters (bass shakers) under the playfield. The newer is what most surround-sound cabinets are set up for.

## Volumes

- **Playfield**, **Backglass** and **PinUP packs** (a pack's videos and sounds, on top of the backglass volume), for every table. A table's settings can give it a playfield and a backglass volume of its own.
- **In the pause menu,** while a game plays: **Volume** turns VPX's volume down or up, both together as **-** and **=** do, and it's kept for your next games. Below it, a level for each part of the sound the table plays (**Table sounds**, **Machine** or **Machine (AltSound)**, **Music**, **PinUP pack**, **Media Player**, **Backglass sounds**, **Achievements**), from 0 to 100%, 100% being as the table came, kept for that table. **Put the parts back as they came** sets them all to 100% again. See [Playing](Playing#the-pause-menu).
- **A machine's own volume,** its ROM's sound louder or softer, is the classic VPinMAME's, on Settings › Machines. With VPX's own PinMAME, the pause menu's **Machine** level turns it down.

## Achievements

Each achievement earned as you play in VPX 10.8.1 is announced with a sound: its pack's own, or the app's chime, played through VPX's backglass sound at VPX's music volume. **Play a sound as each achievement is earned** turns it off or on, and **Its volume** goes from 0 to 200% (100% plays it as it comes). VPX's own sound menu gives it a level of its own too (**Achievements**), and so does the pause menu. See [Achievements](Achievements#its-sound).

## Sound packs

**Play a game's AltSound pack when it has one** is for VPX's own AltSound add-on. AltSound packs replace a machine's music and sounds with remastered ones; turned off, the machine's own sound plays. For one game, its table's settings choose; with the classic VPinMAME, Settings › Machines does. A machine without a pack can have one made from its ROM, on its table's page. See [AltSound](AltSound).

## A table's music

A table that plays music files finds them where VPX looks for them: in a `music` folder beside it, then in VPX's own `Music` folder. The drop window puts them where the table's script plays them from, and a table's page says when songs its script plays aren't there. Music a table plays through Windows Media Player plays through VPX's own Media Player add-on, or through Windows Media Player itself, as Settings › Library › its add-ons choose.
