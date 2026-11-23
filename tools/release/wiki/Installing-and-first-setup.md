# Installing and first setup

## What you need

- Windows 10 or 11, 64-bit.
- Visual Pinball X (VPX): your tables play in VPX itself, 10.8.1 or 10.8.0. If you don't have it, first setup gets VPX 10.8.1 for you.
- With VPX 10.8.0, the programs it plays with, as you have them now: VPinMAME, B2S Server, PinUP Player, FlexDMD, DOF. VPX 10.8.1 brings its own add-ons for all of them.
- Your tables (`.vpx` files) and, for tables of real machines, their ROMs.
- For the pictures and videos the app makes of your tables, a graphics card that runs DirectX 12.

## Which VPX 10.8.1 build

VPX 10.8.1 isn't finished yet, and it has no release: it's VPX's newest code, which VPX's team builds automatically, many times a week. The 10.8.1 builds on VPX's GitHub releases page, the newest of them 5436 from August 2026, are older test builds that VPX's team names "DO NOT USE".

The app is made for **VPX 10.8.1 build 6758**, one of those automatic builds, and **Get VPX 10.8.1** downloads that build. It comes through nightly.link, a free service that hands out a GitHub project's automatic builds without a GitHub sign-in, and the app checks it against the checksum GitHub keeps of it before it's kept. GitHub keeps an automatic build for 90 days, and a newer version of the app brings a newer build. VPX's newest code has been numbered 10.9 since November 2026, so no newer 10.8.1 builds come: where GitHub no longer keeps 6758, Get VPX 10.8.1 says so and asks you to update the app, whose newer version brings the VPX it's made for.

Where your library's VPX 10.8.1 is an older build, Settings › Library says so and offers **Get VPX 10.8.1 (build 6758)** in its place, keeping each file it replaces so yours can be put back. Settings › Library shows the build your tables play in.

What an older 10.8.1 build, such as 5436, doesn't have:

- the app's pause menu (F12 opens VPX's own);
- achievements that count the balls in play or the table's lights (the rest count);
- the score display placed in each backglass file;
- the bass shaker;
- PIN2DMD panels through VPX's own DMDUtil add-on.

## Installing

1. Download the installer from the [Releases page](https://github.com/VPinInfinite/VisualPinballInfinite/releases) on GitHub.
2. Run it. The installer isn't signed yet, so Windows may say it protected your PC: choose **More info**, then **Run anyway**.
3. It installs for your Windows user only, so it doesn't ask for an administrator. The app goes in `%LOCALAPPDATA%\Visual Pinball Infinite`. If your PC lacks Microsoft's WebView2, which draws the app's window, the installer fetches it.

The installer holds the app and the tools it works with: the add-on it puts into VPX for achievements, and its tools for making media and for ROMs and AltSound packs. VPX itself isn't in it: first setup finds yours, or gets VPX 10.8.1.

## First setup

The first time the app opens, it takes a minute or two to set up.

**Where are your tables?**

- **You already use VPX:** **Use your VPX setup**. The app finds VPX on this PC: its tables folder, VPinMAME's folder with your ROMs, VPX's scripts folder and PinUP's `PUPVideos` folder, and uses them where they are. Nothing is moved or copied, and VPX, PinUP Popper and VPin Studio keep working as before. **Change** beside a folder picks another one. If it found none, **Choose VPX's folder…**: VPX's own folder or your tables' folder, and it finds the rest.
- **You're starting fresh:** **Start a new library** makes the folders (tables, ROMs, scripts, PinUP packs) in a folder you choose, laid out as VPX lays them out, so other programs can use them too. You add tables by dropping their downloads on the app. It asks how the new library keeps each table's files: **Shared folders**, which every VPX reads (recommended), or **A folder for each game**, with its ROM, packs, coloring and music. A folder for each game needs VPX 10.8.1: VPX 10.8.0, and PinUP Popper starting tables through it, can't start ROM tables kept that way. Settings › Library can move a library of shared folders into a folder for each game later, and put it back.

**VPX.** The app says which VPX it found, and where.

- **VPX 10.8.1:** it plays your tables. The app starts it when you press PLAY, and comes back when the game ends.
- **VPX 10.8.0 alone:** it plays your tables, and the app offers **Get VPX 10.8.1** to put beside it. VPX 10.8.0 stays, and Settings › Library chooses which one plays. **How they differ** compares them.
- **VPX 10.8.1 already:** it plays your tables. Where it's an older build than the one the app is made for, the app offers **Get VPX 10.8.1 (build 6758)** in its place.
- **No VPX, or one older than 10.8.0:** **Get VPX 10.8.1** downloads build 6758, the VPX 10.8.1 build the app is made for (see [Which VPX 10.8.1 build](#which-vpx-1081-build)), checks it against the checksum GitHub keeps of it, and puts it in VPX's folder, keeping each file it replaces. Where there's no VPX yet, that's the folder your tables folder is in. If VPX is on the PC somewhere the app didn't look, **It's in another folder…** points to it.

The download shows in the bar at the top, and you can carry on meanwhile. **Later** goes on without VPX: Settings › Library › VPX has the same **Get VPX 10.8.1**, and **Undo Get VPX 10.8.1** there takes out what it brought, to the Recycle Bin, and puts back the files it replaced.

**Your screens.** With more than one screen, tick **Open Settings › Screens when setup is done** to place the backglass, score display and topper next. See [Screens](Screens).

**Your controls.** VPX's keys work as they do in VPX, and so do gamepads and cabinet boards, through VPX's own layouts. The step shows the main ones; any action can go on any key or button in Settings › Controls, with a live test of every button, stick and plunger. See [Controls](Controls).

**All set.** Settings › General › **Run it again** runs the setup again at any time, to change a folder too. Settings › Library shows the VPX your tables play in, and where each kind of file lives.

## If you already play in VPX

- **Your tables play in your own VPX,** with your VPX settings. The app's settings pages change VPX's own settings file, the one VPX's menus change too, so a setting changed in either place is the same setting. VPX 10.8.0 keeps settings of its own, which you change in its own Preferences: the app's pages set VPX 10.8.1's.
- **High scores and machines' memory** stay where VPX keeps them, and each table's page shows them.
- **VPX reads what sits beside a table** as it always has: its own settings (`<table>.ini`), a replacement script (`<table>.vbs`), its physics (`<table>.vpp`). The table's page shows each.
- **The programs you play with stay yours.** The first time a table plays in VPX 10.8.1, each part that has a classic program installed (VPinMAME, B2S Server, PinUP Player, FlexDMD, Windows Media Player, DOF through B2S Server) keeps it, and each part without one gets VPX's own add-on. Settings › Library › Its add-ons switches them, part by part.
- **Your ratings come with you,** from PinUP Popper (and VPin Studio, which keeps its ratings there), PinballX and PinballY.

## Updates

The app looks for a new version when it starts, and Settings › General › **Look for a new version** checks at any time. When one is out, it says so, and installs it only when you say, after checking the download was signed with the project's key. Your library and settings stay as they are.

**From an earlier version of the app:** what it kept carries over to VPX. Where there's something to bring, Settings › Library › Your settings › **Bring them to VPX** brings over, once, the screens, sound, controls and picture you set then, keeping VPX's file as it was beside it. Each machine's memory and each table's saved scores go to VPX the first time the table plays, where VPX keeps none of its own.

## Where things are

- **The app:** `%LOCALAPPDATA%\Visual Pinball Infinite`.
- **VPX:** where first setup found it, or where Get VPX 10.8.1 put it. Settings › Library shows it.
- **VPX's settings and log:** in `%APPDATA%\VPinballX\10.8\`. `VPinballX.ini` holds every VPX setting (the app's settings pages and VPX's menus both change it), and `vpinball.log` is VPX's log. VPX 10.8.0 keeps its own `VPinballX.ini`, in `%APPDATA%\VPinballX` or beside its program.
- **The achievements add-on:** `vpi-achievements`, in the `plugins` folder beside VPX's settings (or in VPX's own `plugins` folder, for an older 10.8.1 build), put there by the app when a table plays.
- **Your tables and their files:** your own folders, as first setup found or made them. Settings › Library › Folders shows them.
- **The app's own data:** `%APPDATA%\Visual Pinball Infinite`:
  - `Settings.ini`: the app's own settings, your library's folders among them; `Screen presets\` beside it.
  - `Library\`: your favourites, playlists, ratings, plays and choices.
  - `Achievements\`: your achievement packs, and what you've earned.
  - `Physics presets\`: the physics you've kept as presets.
  - `VPS\`: a copy of the Virtual Pinball Spreadsheet's list, and its pictures.
  - `Pictures\` and `Screenshots\`: media the app made, and your screenshots, by table.
  - `Backglasses as they came\`: each backglass file as it was before the app placed the score display in it.
  - `VPX\`: the files Get VPX 10.8.1 replaced, kept so they can be put back.
  - `Logs\`: the app's logs, and the problem reports you make.

## Removing it

Uninstall it from Windows' Settings › Apps, like any other program. Your tables, ROMs, VPX and their files live in your own folders, and stay. So does the achievements add-on in VPX's `plugins` folder: delete its `vpi-achievements` folder to take it out too.
