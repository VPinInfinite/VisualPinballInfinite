# Achievements

Goals to earn as you play a table: a score, a multiball, an extra ball, the table's lights, the machine's own lamps, switches and coils, each once or so many times, even one after another against the clock. They come in **achievement packs**, made by a table's author or anyone else, you included (see [Making a pack](#making-a-pack)), and a table can have several. Ten [sample packs](Sample-achievement-packs) show what a pack can do.

They count as you play in VPX 10.9 (or a VPX 10.8.1 build). The app puts an add-on of its own into VPX, which follows each game, shows each achievement the moment it's earned and plays its sound. It hears the game through VPX's own add-ons for the machine and the backglass, so Settings › Library › **Its add-ons** needs **VPX's own** for **Machines' ROMs** and **Backglasses** (where the classic VPinMAME or B2S Server was installed, the app keeps using it at first). With VPX 10.8.0, or with the classic programs, achievements are counted once the game is over, and only those counting games and scores: see [What can be counted](#what-can-be-counted) and [Settings](Settings#library).

They count in the games the app starts: **PLAY**, **Play Hardcore** and **Play Softcore** on a table's page, and Start in cabinet mode. A table started from a front end plays in VPX as usual, without them. Future Pinball's tables have no achievements.

## Adding a pack

- **Drop it onto the app,** like any other download: the pack's zip, or its folder. It's added at once, with a notice saying so, and anything else you dropped with it goes on to the drop window.
- **Add a pack…** chooses one with Windows' file picker instead: a pack's zip, or its `achievements.json` with its pictures and sounds beside it. It's under **Player hub** › **Achievements**, and on a table's page while it has no pack (its **Achievements** card, and **What it holds**).
- **Make your own** in the achievements editor: see [Making a pack](#making-a-pack).

The app checks a pack before keeping it, and says what's wrong with one it can't take. A newer version of a pack you have takes over from the one before.

A pack names the game it's for, so it goes with every table file of that game you have. A pack made for one table file, its parts and lights named as that file names them, goes with that file alone; one made for some of a game's ROMs goes with the tables playing those.

## On a table's page

One card on a table's page holds its **Achievements** and its **High scores**: click **Achievements** at its top, or the trophy button beside PLAY. It shows the achievements of the table's pack: those you've earned lit up, with the day you earned each (and **softcore** where it was earned so), the others dimmed, as a grid or a list. A secret one reads **Secret achievement** until it's earned. Above them are how many you've earned and their points, the pack's name, version and makers, and when you mastered it. Where a table has several packs, choose which one shows from the list.

Under an achievement that can't be earned as the table plays now, a line says why: what it counts, and what would count it (VPX 10.9 in VPX 10.8.0's place, a [newer VPX build](Installing-and-first-setup#which-build-of-vpx), or VPX's own PinMAME or B2S add-on, chosen in Settings › Library), or that VPX doesn't share it with add-ons yet. Where the table plays softcore, a line above them says why.

**What it holds** lists each pack too, with how many you've earned of it and its bin, or **Add a pack…** where it has none. Where a table has several table files, shown once with a tab for each of its builds, its page shows the packs of the build you pick.

## In the Player hub

**Player hub** › **Achievements** lists every pack you have: the tables of your library it's made for (click one to open it), how many of its achievements you've earned and their points, and the versions you've mastered. A pack's arrow opens it to show its achievements, those earned lit up. **Add a pack…** adds one, and a pack's bin deletes it. **Hardcore** and **Softcore** at the top count what you've earned each way: click one to count only those.

## Deleting a pack

The bin on a pack's row, in a table's **What it holds** or in **Player hub** › **Achievements**, deletes it after asking (**Delete it**). It's deleted for good: it doesn't go to the Recycle Bin, every version of it goes, and it goes from every table it's made for. What you've earned with it stays earned.

## While you play

Each achievement is announced the moment it's earned, with its sound: a card over the backglass for about four seconds, with its picture, its name and its points, and **softcore** where it's earned so. Settings › Screens › **The achievement popup** chooses where the card shows, **Backglass** or **Score display**, and its size, **Small**, **Medium** or **Large** (see [Screens](Screens)). VPX lets add-ons draw on its backglass and score display but never on the playfield: with **Playfield** chosen, or where the chosen window isn't showing, VPX's own notice shows it on the playfield instead. Mastering a pack gets a card of its own.

What you earn is kept at once, even if the game ends early. As play begins, VPX's notice says how this game counts: **hardcore**, with how many there are to earn, or **softcore**, with why. When the game is paused, a notice says where they stand: how many you've earned, and those you're closest to.

The app's pause menu (see [Playing](Playing#the-pause-menu)) has an **Achievements** row saying how this game counts and how far you've got (**Hardcore · 3 of 8 earned · 30 of 80 points**, say). Its page lists each one with its points and how it stands: **Earned**, how far you've got (**12 of 25**), **Not yet**, or **Can't be earned here** where it counts what this game doesn't share. A secret one reads **A secret achievement** until it's earned. VPX's own menu has an **Achievements** page with the same list.

When the game is over, the app says what you earned.

## Its sound

Each achievement is announced with its own sound, else its pack's, else the app's own chime. Settings › Sound › **Achievements**: **Play a sound as each achievement is earned** turns it on or off, and **Its volume** goes from 0 to 200% (100% plays it as it comes). It plays through VPX's backglass sound, at VPX's music volume, and VPX's own sound settings give it a level of its own, **Achievements**. The app's pause menu has an **Achievements** level among the parts of the sound too, kept for that table. See [Sound](Sound#achievements).

## Hardcore and softcore

An achievement counts as **Hardcore** while the table plays as its author made it: its own script, physics, options and difficulty, with no cheats. Played any other way, it's earned as **Softcore**: still shown and kept for you, but never counted towards a mastery. A game is softcore when:

- its physics are changed: a physics file of its own beside it, or a VPX physics set you've changed that it plays;
- its script is edited (a script changed only to play another of its game's ROMs, with **Change** on its ROM row, still counts as its author's);
- its options or its difficulty are changed in its table settings;
- PinMAME's cheat mode is on for its game.

A table's achievements say when they're softcore there, and why. PLAY says which way it plays (point at it to see), and its **▾** plays one game either way on purpose:

- **Play Hardcore:** as its author made it. Your table settings, script edits and physics changes are left out for this game, VPX's physics sets are as VPX made them and cheat mode is off; everything is put back once the game is over. Your screens, sound and controls stay yours.
- **Play Softcore:** its achievements kept as softcore this game: for a guest, say.

During a game started with **Play Hardcore**, changing a table option, the difficulty, a physics set or cheat mode in VPX's own menu turns the rest of that game softcore, and VPX's notice says so.

## Masteries

Earning every achievement of a pack in Hardcore masters it, and a card says so. When a pack's author updates it, what you earned with the earlier version stays earned, and a mastery keeps the version it was made with. An achievement whose rules a newer version changes is earned again for that version's mastery.

## What can be counted

As VPX 10.9 plays a table, the app's add-on hears:

- **A ROM table's machine,** through VPX's own PinMAME add-on: its game (a game starting and ending, the ball, the scores, credits, extra balls, a tilt) where a memory map knows the machine, and its lamps, switches and coils, by their numbers.
- **A table without a ROM,** through VPX's own B2S add-on: what its script tells its backglass (a game starting and ending, the ball, the scores, credits, a tilt).
- **The balls in play,** on any table, in VPX 10.9 (or VPX 10.8.1 from build 5575: see [Which build of VPX](Installing-and-first-setup#which-build-of-vpx)). A ball counts once it's travelled well away from where it rested, until it's been still for ten seconds: a ball served, plunged or kicked out counts, the balls waiting in the trough never do, and a ball held in a lock stops counting.
- **The table's lights,** by their names in the table: on, off or blinking, in VPX 10.9 (or VPX 10.8.1 from build 6148).

Not yet: a table's own parts (a hit, a spinner's spins, a slingshot's kicks) and the events a table's script tells. VPX doesn't share them with add-ons, so an achievement counting them can't be earned in VPX yet.

With VPX 10.8.0 (chosen in Settings › Library, or for one table with its **…** menu › **Play it with VPX 10.8.0**), or where the classic VPinMAME or B2S Server plays the table instead of VPX's own add-ons, the game itself isn't heard. Its achievements are counted once it's over, from what it leaves: the games played and their scores. Nothing shows while you play. A table's page says beside each achievement when it can't be earned as the table plays now, and why.

The table inspector (**Table editor** at the top, a tile's right-click › **Open in the table editor**, or a table's **…** menu › **Open in the table inspector**) shows a table from above, drawn from its own parts. Click a part to see its name and what an achievement could count of it: each hit, a spinner's spins, a slingshot's kicks. A pack can name them, but playing in VPX doesn't count them yet. A part the ball passes straight through, such as a decoration, can't be counted, and nor can a spinner held between two angles, such as a flap on a ramp.

## Making a pack

**Table editor** at the top (or a tile's right-click › **Open in the table editor**): open a table, then **Its achievements**, beside **Its parts**. Across the top are the table's name, **Another table**, and three buttons:

- **Save** keeps what you've made as a draft, with no version, as often as you like. A draft stays until you finish the pack or throw it away, even when the app closes. Leaving with changes not saved asks first.
- **Try it** plays the table in VPX with that pack alone, as softcore. Nothing earned in a try is kept; when you quit, the editor says what you earned.
- **Finalize and pack as 1.1** (or whichever version comes next) makes the pack a version others can play with. What was earned with the version before stays earned, and its masteries keep their version.

**The pack**, at the top right, shows its picture, name, version, achievements and points. Click it to open its details:

- **Pack:** which pack to work on: one of the table's, a draft, or **A new pack of my own**, which starts from VPS's facts for the table.
- Its picture (click it to choose one), **Its name**, **Made by** and **What it's about**.
- **Its sound when one's earned:** **Choose…** a sound of your own, or **Take it out** for the app's own chime. A WAV, Ogg Vorbis, MP3 or FLAC file.
- **Only for this table file — its parts are named as this table names them:** tick it when the pack counts the table's parts or lights by their names, so it goes with this table file alone.
- **Show its files**, **Throw the draft away…** (it goes to the Recycle Bin), and **Done**.

What's missing before it can be tried or packed is listed there under **Incomplete**, and a warning mark shows on the pack until it's done.

Below, in three columns:

- **The table's parts, on the left,** by kind, with **Find a part by name**. Drag one onto a condition, or right-click it to **Hide it** on the playfield, to reach the parts under it; the eye beside a hidden one shows it again.
- **The playfield, in the middle,** seen from above: **Its look**, the table drawn from straight above (**Draw its look** makes that picture the first time, in a few seconds), or its parts' **Outlines** alone. Click a part to see what an achievement could count of it, then **Count it in this achievement**; right-click one to hide it.
- **The achievements, on the right.** Click one to open it, or **New achievement** to start one: its picture (click it to choose one), its name, what to do in a few words, its points (1 to 100), and **Secret: its name hidden until it's earned**.

Under **Earned when** come its conditions, from **+ Add a condition…**. What the table can't tell is greyed, with why, and a line below says what it can tell:

- **A part on the playfield:** its parts, what they do (**is hit**, **spins**, **fires its slingshot**, **is rolled over**, **is entered**, **is passed through**, **does anything**). Pick them with **Pick object** (click the parts on the playfield or in the list, a second click takes one out, then **Done**), drag them from the list, or find one by name. VPX doesn't share a table's parts with add-ons yet, so these aren't counted in VPX.
- **A light on the table:** one or more of its lights by name (**Pick light**, or type its name), and what it does: **turns on**, **goes out**, **starts blinking**, **is on**, **is off** or **is blinking**. Several lights on one row mean any one of them. "Turns on" is the moment it lights; "is on" lasts while it's lit, so an **is on** row for each light, all at the same moment, means every one lit together. Counted in VPX 10.9 (or VPX 10.8.1 from build 6148).
- **The score**; **A game starting or ending, or a ball begun**; **Balls in play at once (multiball)**; **An extra ball earned**; **Credits on the machine**; or **A tilt**.
- On a ROM table, **A lamp of the machine**, **A switch of the machine** or **A coil of the machine**, by its number as the machine's manual and PinMAME number it. The box suggests numbers from the names the table gives its lights and switches, and its coils by the names its script gives them.
- **An event the table's script tells,** for tables written for achievements. Not counted in VPX yet.
- A group of conditions of its own, to put one inside another: **A group: one after another**, **A group: all of these, in any order** or **A group: any one of these**.

Each condition is met once, or a number of times **in one ball**, **on ball…** (one ball by its number), **in one game**, **over all your games** (kept from one play to the next) or **within some seconds**.

With more than one condition, choose how they go together:

- **One after another:** with no time limit, or **in one ball**, **on ball…**, **in one game** or **within some seconds**. Tick **In exactly this order: anything else on the list starts it over** to make it strict.
- **All of these, in any order:** within one ball, one ball by its number, one game or some seconds.
- **All at the same moment**, or **Any one of these**.

**Optional: start it over when something happens, a ball going down an outlane, say…** adds what sets it back to nothing: **It starts over each time this happens**, a count over all your games included. Its **×** takes it out again.

**As the pack file has it** shows the achievement as the file keeps it, and **Take it out of the pack** removes it, after asking. What's been earned of it stays earned.

## The add-on on its own

The add-on that follows your games also works without the app, in VPX alone: a download of its own on each release, beside the installer (`vpi-achievements-<version>.zip`), with a read-me saying how to put it in. It needs VPX 10.9, or a recent VPX 10.8.1 build, and VPX's own add-ons for the tables you play (its PinMAME add-on for ROM machines, its B2S add-on for a table whose backglass tells its scores).

- **Putting it in:** its `vpi-achievements` folder goes into the `plugins` folder of VPX's settings folder (`%APPDATA%\VPinballX\10.9\plugins`), and `[Plugin.VPIAchievements]` `Enable = 1` in VPX's settings switches it on, or switch it on where VPX lists its add-ons.
- **Packs:** in an `achievements` folder beside the table, or in VPX's settings folder (`%APPDATA%\VPinballX\10.9\achievements`). A pack plays with a table when the table runs one of the ROM sets it names, or else when the game's name it gives is in the table file's name (and its year, where both have one).
- **What you earn** is kept in VPX's settings folder, and hardcore and softcore count as they do in the app.
- **Its settings** are in VPX's menu (F12): the sound on or off, where the card shows and its size.

With the app, there's nothing to do: the app puts the add-on in for you and hands it each table's packs.
