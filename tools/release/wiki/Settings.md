# Settings

Every setting is in one place: **Settings**, the sliders button at the top right of the app. The pages hold the settings most people need; **Advanced** holds every one.

VPX plays your tables, so most settings are VPX's own, under VPX's own names. The pages write them where VPX reads them, and VPX plays with them from the next table it starts:

- **VPX's settings:** VPX's own settings file, `%APPDATA%\VPinballX\10.8\VPinballX.ini`. **Show the file**, on Screens and Sound, shows it in Explorer. VPX keeps a settings folder for each version: a VPX 10.9 build's is `%APPDATA%\VPinballX\10.9`, brought over from 10.8's the first time, and the pages set the settings of whichever VPX plays.
- **The app's own few,** for what VPX has no setting of its own (where the achievement popup shows, the screenshot key and the like): `%APPDATA%\Visual Pinball Infinite\Settings.ini`.
- **A table's own:** `<table>.ini` beside it, which VPX reads over your usual ones.
- **The classic VPinMAME's, for each machine:** where VPinMAME keeps them, in your part of Windows' registry.

**Save** writes only what changed, and the file as it was before is kept beside it (`VPinballX.previous.ini` for VPX's). VPX 10.8.0 keeps settings of its own: when it plays your tables, these pages still set VPX 10.8.1's, and a note at their top says so. Change 10.8.0's in its own Preferences.

The pages, in order: General, Library, Screens, Picture, Controls, Sound, Machines, PinUP packs, Cabinet and Advanced, with Physics sets under Advanced.

## General

- **Start-up:** first setup again (**Run it again**: where your library lives, and the screens and controls in a word), the app's window (in a window or full screen; **F11** switches), and what it opens on: **The library** or **The last table's page**.
- **Where you are,** for the light of day: a table's lighting can follow the sun where you are (Picture › **Light level**, **From the sun where you are**). **Use this PC's location** asks Windows for this PC's place, or type the latitude and longitude. Only those two numbers are kept.
- **VPX's readout:** frame rate and timings in a corner while playing, **Hidden** or **Shown**.
- **Visual Pinball Infinite:** the version, **Its release notes**, and **Look for a new version**.

The screenshot key is on Settings › Controls, and whether a table takes a screenshot when a game makes the high scores is in its own settings.

## Library

- **VPX:** which VPX plays your tables, VPX 10.8.1 or VPX 10.8.0 where both are there; **How they differ** says what changes before you switch. One table can play with VPX 10.8.0 on its own, from its page's **…** menu (see [A table's page](Table-page)). **Choose VPX's folder…** chooses it by hand. **Get VPX 10.8.1** downloads build 6758, the VPX 10.8.1 build the app is made for ([Which VPX 10.8.1 build](Installing-and-first-setup#which-vpx-1081-build)), checks it against the checksum GitHub keeps of it, and puts it beside the VPX there, keeping every file it replaces; where your VPX 10.8.1 is an older build, it's offered in its place; **Undo Get VPX 10.8.1** takes out what it brought, to the Recycle Bin, and puts those files back.
- **Its add-ons:** VPX 10.8.1 carries its own add-ons, and each part plays with **VPX's own** or with the classic program installed for it: **Machines' ROMs** (VPinMAME), **Backglasses** (B2S Server), **PinUP packs** (PinUP Player), **FlexDMD and UltraDMD** (FlexDMD), **Media Player's music** (Windows Media Player) and **Toys (DOF)** (DOF, through B2S Server). The first time tables play, each part gets VPX's own where no classic program is installed for it, and keeps the classic one where it is. Achievements as you play need VPX's own PinMAME and B2S add-ons. VPX 10.8.0 takes no add-ons: it plays with the classic programs.
- **Your settings:** settings made in an earlier version of the app that aren't in VPX's settings yet. **Bring them to VPX** copies them over once.
- **The library:** tiles or a list, what tiles show and their size, whether files dropped on a tile go to that table, a table with more than one table file shown once or each file on its own, and your ratings (whether they go in PinUP Popper's, PinballX's or PinballY's list too).
- **Playlists:** your playlists, the smart playlists made from VPS's tags, and hidden tables.
- **Folders:** what the drop window picks when a file is there already, whether each game has a folder of its own, and where each kind of file lives.

Every table's media are under Tools › Media (see [Media](Media)). See [Your library](Your-library) and [Adding tables](Adding-tables).

## Screens

Where each of VPX's windows goes (the playfield, the backglass, the score display and the topper), ready-made layouts, presets, the table's view, and where the achievement popup shows. See [Screens](Screens).

## Picture

- **The look, for every table:** the tone mapper (each table's own, or Reinhard, AgX, Filmic, Neutral or AgX Punchy for all), exposure, and the light level: each table's own, fixed, or from the sun where you are.
- **On an HDR screen,** with Windows' HDR on: bright light by VPX's HDR curve or by each table's own tone mapper, and the HDR brightness.
- **Quality:** smoothing (multisampling and supersampling), a smoothing filter (FXAA, NFAA, DLAA, SMAA or FAAA), sharpening, ambient occlusion, reflections, bloom, the balls' motion blur, sharper pictures at an angle, and the detail level. Each starts at VPX's own.
- **The balls:** unstretch the ball, VPX's older ball trail, lighting on balls, and your own ball pictures.
- **The score displays:** the DMD made smoother, the score view's layouts, Serum and PIN2DMD colorings, your PIN2DMD key, the looks of older machines' segment displays, and a real DMD panel. See [Score displays](Score-displays).
- **Display timing:** how frames reach the screen, at most how many a second, and how many the graphics card may queue. See [Screens](Screens#display-timing).

When VPX runs short of graphics memory in a game, the table's page says so afterwards. Where supersampling is on, **Set supersampling to 1×** there frees a lot of it.

## Controls

Every game action with its keys and buttons, and a live test; which pause menu VPX's menu key opens; and the screenshot key. See [Controls](Controls).

## Sound

Which speakers the playfield's and the backglass's sounds play through, a bass shaker, the playfield's speaker layout, the volumes, the sound achievements are announced with, and AltSound packs. See [Sound](Sound).

## Machines

PinMAME's settings for every machine: VPX's own PinMAME's and the classic VPinMAME's, each where it reads them, and where each machine's memory is kept. See [ROM machines](ROM-machines).

## Physics sets

Under Advanced, since they change how tables play. VPX's eight shared physics sets: each one's name and its table and flipper values, with the tables that play it. A table can play one of them in its own physics' place (its Physics set, set in VPX's editor), and few do. A game played with them changed counts as softcore for achievements. A table's own physics are changed in its physics window, from its settings (see [A table's page](Table-page#its-physics)).

## PinUP packs

How VPX's own PinUP add-on plays packs: on or off, which comes first on the backglass and on the score display where a table has both a pack and a backglass file, and the room around a pack's screens. Every pack, its layout and its status are under Tools › PinUP packs, and their volume is on Sound. See [PinUP packs](PinUP-packs).

## Cabinet

Cabinet mode's settings (what its playfield screen shows, its volume, the order its sound plays in, its screens, its picture and more), the playfield screen and where you stand, for the cabinet view, and the toys' configuration. A real DMD panel is on Picture. See [Cabinet mode](Cabinet-mode) and [Cabinet hardware](Cabinets).

## Advanced

Every setting: all of VPX's, VPinMAME's for every machine, and the app's own, by category, each with what it does, its default and its name in the file. A wrong value can make tables look, sound or play wrongly, so they're locked against a slip: **Unlock…** to change one, and **Lock** to lock them again.

- Search them by name or by a word of what they do, or show **Only those set in the file**.
- **Take it out** of the file to have its default again.
- **Add one by name** for a setting the lists don't show.
- The file before each save is kept beside it.

## A table's own settings

On its page, **Table settings** (the sliders button beside PLAY): see [A table's page](Table-page#its-settings). They're kept in its `<table>.ini`, which VPX reads too, and win over your usual ones, for that table.
