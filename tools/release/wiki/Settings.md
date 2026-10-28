# Settings

Every setting is in one place: **Settings** at the top right of the app. The pages hold the settings most people need; **Advanced** holds every one. They're kept in plain files under VPX's own names, so they can be read, shared and backed up: `%APPDATA%\Visual Pinball Infinite\Settings.ini`, and a table's own beside it as `<table>.ini`. The website lists [every setting](https://visualpinballinfinite.org/resources/settings) with what it does.

The pages, in order: General, Library, Screens, Picture, Controls, Sound, Machines, PinUP packs, Cabinet and Advanced, with Physics sets under Advanced.

## General

- **Start-up:** first setup again (**Run it again**), the app's window, and what it opens on: the library, or the last table's page.
- **Esc while playing:** opens the pause menu with Quit the table picked (the usual), or quits the table at once. A front end that starts a table the way it starts VPX, with VPX's `-minimized` switch, gets VPX's way: Esc quits at once.
- **Where you are,** for the light of day: a table's lighting can follow the sun where you are (Picture › Light level). **Find your place** asks Windows for this PC's location, or type the latitude and longitude. Only those two numbers are kept.
- **Screenshots:** which screens the Screenshot key saves (F12 unless Controls gives it another), whether to take one when a game makes the high scores, and where they're kept.
- **Table scripts:** a table's script is a small program. Out of the box it uses only its own files and what tables need to play. **Tables reaching outside their files** lets it use the internet, other programs and every file. It's off unless you turn it on, for every table here or for one in its own settings.
- **The player's readout:** frame rate and timings on screen, shown or hidden.
- **VPIDB:** your link to VPIDB (vpidb.org), and what you share there: the achievements you earn, and your ratings and which tables you have. **VPIDB** at the top of the app opens it too. See [Achievements](Achievements#sharing-on-vpidb).
- **Visual Pinball Infinite:** the version, and **Look for a new version**.

## Library

How the library shows your tables (tiles or a list, what tiles show, their size, whether files dropped on a tile go to that table), your playlists and VPS's smart playlists, hidden tables, your ratings (whether they go in PinUP Popper's, PinballX's or PinballY's list too), and your folders: where each kind of file lives, whether each game has a folder of its own, and what the drop window picks when a file is there already. Every table's media are under Tools › Media now (see [Media](Media)). See [Your library](Your-library) and [Adding tables](Adding-tables).

## Screens

Where every window goes, ready-made layouts, presets and the table's view. See [Screens](Screens).

## Picture

- **The look, for every table:** the tone mapper (each table's own, or Reinhard, AgX, Filmic, Neutral or AgX Punchy for all), exposure, and the light level: fixed, or from the sun where you are.
- **On an HDR screen:** HDR output for a screen with Windows' HDR on, its brightness, and the score window's look.
- **Quality:** smoothing (multisampling and supersampling), smoothing filters (FXAA, NFAA, DLAA, SMAA, FAAA), sharpening, ambient occlusion, reflections, bloom, the balls' motion blur, sharper pictures at an angle, and the detail level. As in VPX, the extras are off until you turn them on.
- **The balls:** unstretch the ball, lighting on balls, your own ball pictures.
- **The score displays:** the DMD made smoother, the score view's layouts, Serum and PIN2DMD colorings, your PIN2DMD key, segment displays, and real panels. See [Score displays](Score-displays).
- **Display timing:** how frames reach the screen, at most how many a second (240 unless you set another), and whether the other windows show from the start or with their first picture. See [Screens](Screens#display-timing).

## Controls

Every action, its keys and buttons, and a live test. See [Controls](Controls).

## Sound

Two sets of speakers, the playfield's speaker layout, volumes, sound packs, and the sound achievements are announced with. See [Sound](Sound).

## Machines

PinMAME's settings for every machine, and the machines' memory. A game's own settings, on its table's settings, win over these. See [ROM machines](ROM-machines).

## Physics sets

Under Advanced, since they change how tables play. VPX's eight shared physics sets: each one's name and its table and flipper values, with the tables that play it. A table can play one of them in its own physics' place (its Physics set, set in VPX's editor), and few do. A table's own physics are changed in its physics window, from its settings (see [A table's page](Table-page#its-physics)).

## PinUP packs

How packs play: on or off, which draws first where a table has a pack and a backglass file, and the room around a pack's screens. Every pack, its layout and its status are under Tools › PinUP packs. See [PinUP packs](PinUP-packs).

## Cabinet

Cabinet mode's settings (what its screens show, the order its sound plays in, its volume and more), the cabinet view's screen and where you stand, and the toys. See [Cabinet mode](Cabinet-mode) and [Cabinet hardware](Cabinets).

## Advanced

Every setting: all of VPX's, VPinMAME's for every machine, and ours, by category, each with what it does and its default. A wrong value can make tables look, sound or play wrongly, so they're locked against a slip: **Unlock…** to change one.

- Search them by name or by a word of what they do.
- **Take it out** of the file to have its default again.
- **Add one by name** for a setting the lists don't show.
- Saved settings go into the settings file every table plays with; the file before each save is kept as `Settings.previous.ini`.

## A table's own settings

On its page, **Table settings**: see [A table's page](Table-page#its-settings). They win over your usual ones, for that table.
