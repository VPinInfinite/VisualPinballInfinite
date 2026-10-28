# Visual Pinball Infinite

**A new pinball engine, and everything around it, in one app.** Free and open source, for Windows 10 and 11.

<p align="center"><img src="https://visualpinballinfinite.org/images/library-1600.webp" width="100%" alt="The app's library: each table as a card with its playfield picture, its maker and year, and what it holds"></p>
<p align="center"><sub>The library. Tables shown: AC/DC (LUCI Premium) and Andromeda by VPin Workshop, Apollo 13 by UnclePaulie and team, Attack from Mars by JPSalas and team. Pictures from the Virtual Pinball Spreadsheet.</sub></p>

Visual Pinball Infinite plays your Visual Pinball X tables on its own engine, built from the ground up, with VPX's own code rewritten in Rust. The same program finds and sorts your tables' files, manages your screens and settings, makes your tables' media, adds achievements to your tables and browses the Virtual Pinball Spreadsheet. Nothing else to install.

**Version 0.1.0 is the first release.** Confirmed working on 115 tables, at 99.9% visual accuracy on average.\*

Website: [visualpinballinfinite.org](https://visualpinballinfinite.org)

What's new: [the latest version's release notes](https://github.com/VPinInfinite/VisualPinballInfinite/releases/latest)

Questions, bug reports and ideas: the [Discord server](https://discord.gg/GyZTZtsJag) or this repository's issues.

## Download

The installer is on the [releases page](https://github.com/VPinInfinite/VisualPinballInfinite/releases): one file with everything inside, installed for your Windows user, with no administrator rights needed. It needs Windows 10 or 11 (64-bit) and a DirectX 12 graphics card. Tables, ROMs and media aren't included: get them from their authors.

## What it does

- **Plays your VPX tables on a new engine.** VPX's physics, rewritten in Rust line by line and checked bit for bit against VPX's own test runs; VPX's look, on average within 0.1% of VPX's own picture in our tests (about 1 pixel in 1,000)\*; each table's own script; ROM machines through PinMAME; backglasses, PinUP packs, FlexDMD and UltraDMD, color DMDs and AltSound (a pack made from a table's own ROM too; Stern's SAM machines and Pinball 2000 can't use AltSound); cabinet toys through DOF, real DMD panels, analog plungers and nudging. Your `.vpx` files play as they are and are never changed.
- **Built for low latency.** Buttons are read on a thread of their own, never held up by drawing, and each press gets its frame at once. In our tests on the same PC, with sync off, a button press reached the screen in under 1.5 ms, where VPX 10.8.0 took about 19 ms.†
- **1 to 4 screens:** the playfield, backglass, score display and topper placed by dragging them on a map or on the screens themselves, kept as presets to switch between, and a table can keep its own.
- **Achievements:** goals to earn as you play, from achievement packs made by a table's author or anyone else, announced on the backglass, Hardcore while the table plays as its author made it. An achievements editor makes them by clicking the table's own parts, ten sample packs show what they can do, and connected to VPIDB with your Discord account, what you earn can show there if you choose.
- **Your library:** each table's pictures, facts and designers from the Virtual Pinball Spreadsheet, favourites, ratings, playlists, a play log and high scores read from the machine's own memory, and a Player hub with every table's high scores, your achievements and your stats.
- **Adding tables:** drop in a download — a zip, 7-Zip or RAR archive, or a folder — and each file is told apart, matched to its table and shown where it'll go before anything moves.
- **Your folders, as they are:** VPX's tables, ROMs, backglasses and PinUP packs, and PinUP Popper's media, where they already are. Front ends launch it as they launch VPX.
- **Settings in one place:** picture, sound, controls, ROM machines and PinUP packs on plain pages — each PinUP pack's layout chosen for your screens — with every one of VPX's own settings there when you want them.
- **A table's script, made simple:** its options as switches and lists, or its whole script in an editor, checked before it's kept and saved beside the table, so the table file never changes.
- **Cabinet mode:** the app full screen across a cabinet's screens, tables chosen and started with the cabinet's buttons.
- **Media made for you:** a table's playfield, backglass and score display drawn by the engine, a preview video and a wheel.
- **Tools for every table:** ROMs checked file by file, PinUP packs, media, color DMDs (colored by you, in a coloring program the app installs) and AltSound packs, made from a table's own ROM too.
- **A table inspector:** any table seen from above, drawn from its own parts, the first part of a table editor.

<p align="center">
<img src="https://visualpinballinfinite.org/images/table-1600.webp" width="49%" alt="Attack from Mars's page in the app: its playfield picture, PLAY, its rating and its facts">
<img src="https://visualpinballinfinite.org/images/play-screens-1600.webp" width="49%" alt="Attack from Mars on three screens: the playfield, its backglass and its colored score display">
<img src="https://visualpinballinfinite.org/images/screens-1600.webp" width="49%" alt="The Screens settings page: a map of the screens with the player's windows on them">
<img src="https://visualpinballinfinite.org/images/ach-editor-1600.webp" width="49%" alt="The achievements editor on Attack from Mars: the pack on the left, the playfield in the middle, Bumper Flurry on the right">
</p>
<p align="center"><sub>A table's page, Attack from Mars on three screens, the screens map and the achievements editor. Table: Attack from Mars, by JPSalas and team.</sub></p>

More screenshots are on the [website](https://visualpinballinfinite.org/#screenshots). For the full features view, see the [Features page](https://visualpinballinfinite.org/resources/features) on the website.

## Privacy

It goes online to read the Virtual Pinball Spreadsheet's database and pictures, and to look for a new version on this repository's releases. Nothing about you or your PC is sent with them. Connected to VPIDB with your Discord account, which is up to you, it shares what you earn and, if you choose, your ratings and which tables you have; either can be switched off at any time. A coloring program is downloaded from its makers' own page only when you ask for it. Report a problem gathers a file you can look through first, and sends it only when you press Send. "Use this PC's location", for the light of day in tables that follow it, asks Windows only when you click it.

## Building from source

On Windows 10 or 11, 64-bit, from the repository's folder:

1. **The tools.** This installs Rust, Visual Studio's C++ build tools, CMake, Ninja, Node.js, the GitHub CLI and MSYS2, asking once for administrator rights. The release tools also need [Python 3](https://www.python.org).
   ```
   powershell -ExecutionPolicy Bypass -File tools\setup\install-dev-tools.ps1
   ```
2. **The libraries**, each built from its source at the version its script pins, into `vpx-build\`, in this order:
   ```
   powershell -ExecutionPolicy Bypass -File tools\thirdparty\setup.ps1
   powershell -ExecutionPolicy Bypass -File tools\pinmame\setup.ps1
   powershell -ExecutionPolicy Bypass -File tools\altsound\setup.ps1
   powershell -ExecutionPolicy Bypass -File tools\libwinevbs\setup.ps1
   powershell -ExecutionPolicy Bypass -File tools\dof\setup.ps1
   powershell -ExecutionPolicy Bypass -File tools\dmdutil\setup.ps1
   powershell -ExecutionPolicy Bypass -File tools\libarchive\setup.ps1
   powershell -ExecutionPolicy Bypass -File tools\rom2altsound\setup.ps1
   ```
3. **The app's screens:** in `app\ui`, `npm ci --ignore-scripts`, then `npm run build`.
4. **The programs:** `cargo build --release`. They land in `target\release`; `vpi-app.exe` is the app.
5. **The installer:** `tools\release\stage.ps1` gathers everything the installer installs and checks it, and `tools\release\installer.ps1` builds the installer. Its updates are signed with the project's key; for builds of your own, make a key with `tools\release\make-updater-key.ps1` and put its public key in `app\tauri.conf.json`.

## Licence

GPL-3.0-or-later: see [LICENSE](LICENSE). Free for life: no price, no ads, no subscriptions. Every project it builds on is credited in [CREDITS.md](CREDITS.md), and the installer carries each one's licence in full.

## About

Made by VPinInfinite and Claude (Anthropic's AI). An independent project: not made by or affiliated with the Visual Pinball X team, the Virtual Pinball Spreadsheet or any pinball maker. It's possible because they share their work. Machine names and artwork belong to their owners.

Takedown requests go to vpininfinite@gmail.com: we'll change or remove anything a table's author or a rights holder asks us to.

<sub>\* On average across the tables we test, comparing each picture with VPX's pixel by pixel. A few tables name a room picture they don't include; VPX's built-in one can't be shared, so ours shows a room photo of its own, and their reflections of it differ.</sub>

<sub>† Key press to picture on screen, the median of many presses on an example table, measured with Microsoft's PresentMon during development, both programs on the same desktop PC with sync off (VPX 10.8.0 in its full-screen mode). With tear-free sync on, ours took about 20 ms, about the same as VPX in a window.</sub>
