# Installing and first setup

## What you need

- Windows 10 or 11, 64-bit.
- A graphics card that runs DirectX 12.
- Your tables (`.vpx` files) and, for tables of real machines, their ROMs. Nothing else to install: no VPX, no PinMAME, no extra programs for backglasses, PinUP packs or toys.

## Installing

1. Download the installer from the [Releases page](https://github.com/VPinInfinite/VisualPinballInfinite/releases) on GitHub.
2. Run it. The installer isn't signed yet, so Windows may say it protected your PC: choose **More info**, then **Run anyway**.
3. It installs for your Windows user only, so it doesn't ask for an administrator. The app goes in `%LOCALAPPDATA%\Visual Pinball Infinite`. If your PC lacks Microsoft's WebView2, which draws the app's window, the installer fetches it.

Everything comes with it: the player, PinMAME for ROM tables, and what backglasses, score displays, PinUP packs, AltSound packs, color DMDs, real DMD panels and cabinet toys need. If you have VPX, it stays as it is: both live side by side on the same PC.

## First setup

The first time the app opens, it takes a minute to set up, in four steps.

**Where are your tables?**

- **You already use VPX:** the app looks for VPX on this PC: its tables folder, VPinMAME's folder with your ROMs, VPX's scripts folder and PinUP's `PUPVideos` folder. It uses them where they are. Nothing is moved or copied, and VPX, PinUP Popper and VPin Studio keep working as before. If it can't find one, choose it yourself: **Choose VPX's folder…**, or each folder on its own.
- **You're starting fresh:** **Start a new library** makes the folders (tables, ROMs, scripts, PinUP packs) in a folder you choose, laid out as VPX lays them out, so other programs can use them too. You add tables by dropping their downloads on the app. It asks how the new library keeps each table's files: **shared folders**, as VPX keeps them (recommended), or **a folder for each game** with its ROM, packs, coloring and music, as Visual Pinball X 10.8.1 keeps them (VPX 10.8.0, and PinUP Popper starting tables through it, can't start ROM tables kept that way). Settings › Library can switch later, either way.

**Your screens.** With more than one screen, tick **Open Settings › Screens when setup is done** to place the backglass, score display and topper next. See [Screens](Screens).

**Your controls.** VPX's keys work as they do in VPX, and so do gamepads and cabinet boards. The step shows the main ones; any action can go on any key or button in Settings › Controls. See [Controls](Controls).

**All set.** Settings › General › **Run it again** runs the setup again at any time, and Settings › Library › Folders changes any folder.

## If you already play in VPX

- **Your machines' memory comes with you.** The first time a ROM table plays here, its high scores, settings and audits are taken from VPinMAME's copy. From then on the app keeps its own, so VPX's is never changed.
- **A table's own files are read as VPX reads them:** its settings file (`<table>.ini`), its camera file (`<table>.pov`), a replacement script (`<table>.vbs`) and its physics settings (`<table>.vpp`).
- **High scores you made in VPX** show beside the ones you make here, on each table's page.
- **Your ratings come with you,** from PinUP Popper (and VPin Studio, which keeps its ratings there), PinballX and PinballY.
- **Your DOF Config Tool files** are read where VPX keeps them, so a cabinet's toys keep working.

## Updates

The app looks for a new version when it starts, and Settings › General › **Look for a new version** checks at any time. When one is out, it says so, and installs it only when you say, after checking the download was signed with the project's key. Your library and settings stay as they are.

## Where things are

- **The app:** `%LOCALAPPDATA%\Visual Pinball Infinite` (the player itself is `vpi-player.exe` there).
- **Your tables and their files:** your own folders, as first setup found or made them. Settings › Library › Folders shows them.
- **The app's own data:** `%APPDATA%\Visual Pinball Infinite`:
  - `Settings.ini`: every setting, under VPX's own names; `Screen presets\` beside it.
  - `Machine settings.ini`: PinMAME's settings for each game.
  - `VPReg.ini`: what tables' scripts save between plays, their own high scores among them.
  - `PinMAME\`: the machines' memory.
  - `Library\`: your favourites, playlists, ratings, plays and choices.
  - `Achievements\`: your achievement packs, and what you've earned.
  - `Physics presets\`: the physics you've kept as presets.
  - `VPS\`: a copy of the Virtual Pinball Spreadsheet's list, and its pictures.
  - `Pictures\` and `Screenshots\`: media the app made, and your screenshots, by table.
  - `Logs\`: the app's and the player's logs, and the problem reports you make.

## Removing it

Uninstall it from Windows' Settings › Apps, like any other program. Your tables, ROMs and their files live in your own folders, and stay.
