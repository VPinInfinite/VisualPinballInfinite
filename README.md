<p align="center"><img src="app/icons/icon.png" width="128" alt="The Visual Pinball Infinite logo: an infinity sign in a glowing ring"></p>

<h1 align="center">Visual Pinball Infinite</h1>

<p align="center"><strong>Everything around Visual Pinball X, in one app.</strong> Free and open source, for Windows 10 and 11.</p>

<p align="center"><img src="docs/images/library-1600.webp" width="100%" alt="The app's library, showing VPin Workshop's tables: each table as a card with its playfield picture, its maker and year, and what it holds"></p>
<p align="center"><sub>The library. Tables shown: AC/DC (LUCI Premium), Andromeda and Bad Cats by VPin Workshop, American Graffiti by Onevox, Scottacus, Leojreimroc, VPin Workshop and team. Pictures from the Virtual Pinball Spreadsheet.</sub></p>

Visual Pinball Infinite is a front end, a table and media manager and one place for every setting, for Visual Pinball X. Your tables play in VPX itself — VPX 10.8.1, or VPX 10.8.0 beside it — and the app does everything around them: it finds and sorts your tables' files, places your screens, sets VPX up, makes your tables' media, follows achievements live as you play, gives every game a pause menu with its rules and its guide, and browses the Virtual Pinball Spreadsheet.

What's new: [the latest version's release notes](https://github.com/VPinInfinite/VisualPinballInfinite/releases/latest)

Questions, bug reports and ideas: the [Discord server](https://discord.gg/GyZTZtsJag) or this repository's issues.

## Download

The installer is on the [releases page](https://github.com/VPinInfinite/VisualPinballInfinite/releases): one file, installed for your Windows user, with no administrator rights needed. It needs Windows 10 or 11 (64-bit) and a DirectX 12 graphics card. Ten sample achievement packs are a download of their own beside it.

It works with your own Visual Pinball X. Where your library has none of VPX 10.8.1 yet, or an older build of it, Settings › Library's Get VPX 10.8.1 downloads VPX 10.8.1 build 6758, the build the app is made for, and puts it beside the VPX you have, keeping every file it replaces so the VPX before can be put back. Tables, ROMs and media aren't included: get them from their authors.

VPX's team hasn't released 10.8.1 yet: build 6758 is one of its automatic builds of its newest code, which the pause menu, multiball achievements, achievements from a table's lights, the score display placed in each backglass and the bass shaker need. See [Which VPX 10.8.1 build](https://github.com/VPinInfinite/VisualPinballInfinite/wiki/Installing-and-first-setup#which-vpx-1081-build).

## What it does

- **Plays your tables in VPX itself.** PLAY starts VPX with the table: VPX 10.8.1, or VPX 10.8.0 beside it, chosen for all your tables or one at a time. A table that stops at start in 10.8.1 is offered 10.8.0, and is tried again by itself in each newer 10.8.1 build. Your `.vpx` files play as they are and are never changed.
- **Achievements, live as you play:** goals to earn from achievement packs made by a table's author or anyone else, followed as the game plays in VPX 10.8.1 and announced on the backglass or the score display, with a sound through VPX's own speakers. Hardcore while the table plays as its author made it, Softcore otherwise; multiball goals, and goals from a table's own lights. An achievements editor makes them by clicking the table's own parts.
- **A pause menu over every game:** started from the app, F12 or a controller's menu button pauses VPX and shows the volume and a level for each part of the sound, the table's rules and instruction cards, its guide from Pinball Primer where the Spreadsheet links one, its controls and its achievements.
- **1 to 4 screens:** the playfield, backglass, score display and topper placed by dragging them on a map or on the screens themselves, kept as presets to switch between, and a table can keep its own. The score display is placed in each backglass that has a window for it.
- **Your library:** each table's pictures, facts and designers from the Virtual Pinball Spreadsheet, favourites, ratings, playlists, a play log and high scores read from the machine's own memory, a Player hub with every table's high scores, your achievements and your stats, and a table with several table files shown once, its versions side by side on its page.
- **Adding tables:** drop in a download — a zip, 7-Zip or RAR archive (locked ones too, with their password), or a folder — and each file is told apart, matched to its table and shown where it'll go before anything moves, media placed one by one.
- **Finding what's missing:** the Virtual Pinball Spreadsheet's machines to browse, what each of your tables is missing, and tables recommended by what you play; a file's page opens inside the app, and what you download there is put in place with its table.
- **Your folders, as they are:** VPX's tables, ROMs, backglasses and PinUP packs, and your front end's media, where they already are. PinUP Popper, PinballX and PinballY keep starting VPX as they always have; the app keeps their media, ratings and game names in step with your library.
- **Settings in one place:** screens, picture, sound (a bass shaker too), controls, ROM machines and PinUP packs on plain pages — each PinUP pack's layout chosen for your screens, VPX's own add-ons or the classic programs chosen part by part — saved into VPX's own settings, with every one of VPX's settings there when you want them.
- **A table's script, made simple:** its options as switches and lists, or its whole script in an editor, checked before it's kept and saved beside the table, so the table file never changes.
- **Cabinet mode:** the app full screen across a cabinet's screens, tables chosen and started with the cabinet's buttons.
- **Media made for you:** a table's playfield, backglass and score display drawn by the app with no window, a preview video and a wheel; and a copy of a table with your own pictures in it, kept beside the original.
- **Tools for every table:** ROMs checked file by file, PinUP packs, media, color DMDs (colored by you, in a coloring program the app installs) and AltSound packs, made from a table's own ROM too.
- **A table inspector:** any table seen from above, drawn from its own parts, the first part of a table editor.
- **Report a problem:** your note, with the app's and VPX's logs and settings in one file you can look through first; the team's replies come back to the app, and you can answer them there.

<p align="center">
<img src="docs/images/table-1600.webp" width="49%" alt="Attack from Mars's page in the app: its playfield picture, PLAY, its rating and its facts">
<img src="docs/images/pause-1600.webp" width="49%" alt="The pause menu over Attack from Mars: the volume and each part of the sound, then Rules, Guide, Achievements, Controls and the table's settings">
<img src="docs/images/screens-1600.webp" width="49%" alt="The Screens settings page: a map of the screens with VPX's windows on them, and where the achievement card shows">
<img src="docs/images/ach-editor-1600.webp" width="49%" alt="The achievements editor on Attack from Mars: the table's parts on the left, the playfield in the middle, The Invasion Begins on the right">
</p>
<p align="center"><sub>A table's page, the pause menu, the screens map and the achievements editor. Table: Attack from Mars, by JPSalas and team.</sub></p>

## Privacy

It goes online to read the Virtual Pinball Spreadsheet's database and pictures, to look for a new version on this repository's releases, and to read the team's short notice as it starts, if there is one. Nothing about you or your PC is sent with them. A file's page you open in the app, with Get, loads from its own site in a browser of the app's own, whose sign-ins stay on your PC until you sign out. VPX 10.8.1 is downloaded only when you click Get VPX 10.8.1: VPX's own automatic build, through nightly.link (a service that hands out GitHub's automatic builds without a sign-in), or VPX's newest automatic build where GitHub no longer keeps that one; a coloring program from its makers' own page only when you ask for it, and a table's guide from Pinball Primer only while you have it open, nothing of it kept. Report a problem gathers a file you can look through first, and sends it only when you press Send; then the app asks for the team's replies to it, with a key made on your PC for that report alone. "Use this PC's location", for the light of day in tables that follow it, asks Windows only when you click it.

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
4. **The programs:** `cargo build --release`. They land in `target\release`; `vpi-app.exe` is the app, and `vpi_vpx_addon.dll` the add-on it puts into VPX for live achievements.
5. **The installer:** `tools\release\stage.ps1` gathers everything the installer installs and checks it, and `tools\release\installer.ps1` builds the installer. Its updates are signed with the project's key; for builds of your own, make a key with `tools\release\make-updater-key.ps1` and put its public key in `app\tauri.conf.json`.

A build of your own has no address for problem reports, which only the project's own builds are given: its Report a problem makes the file for you to attach to a post, and it asks nobody for replies or notices.

## Licence

GPL-3.0-or-later: see [LICENSE](LICENSE). Free for life: no price, no ads, no subscriptions. Every project it builds on is credited in [CREDITS.md](CREDITS.md), and the installer carries each one's licence in full.

## About

Made by VPinInfinite and Claude (Anthropic's AI). An independent project: not made by or affiliated with the Visual Pinball X team, the Virtual Pinball Spreadsheet, Pinball Primer or any pinball maker. It's possible because they share their work. Machine names and artwork belong to their owners.

Takedown requests go to vpininfinite@gmail.com: we'll change or remove anything a table's author or a rights holder asks us to.
