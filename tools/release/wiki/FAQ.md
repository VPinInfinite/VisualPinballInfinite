# Questions

**What is it?** A front end and a table and media manager for Visual Pinball X (VPX), and one place for VPX's settings. Your tables play in VPX itself: **PLAY** starts VPX with the table, as a front end does, and the app looks after everything around it: adding tables and their files, pictures and videos, high scores, achievements, your screens, sound and controls.

**Is it free?** Yes: free and open source under the GNU General Public License, version 3 or later, for good. It will never be sold, and it has no account and no ads.

**Does it replace VPX?** No: VPX plays every table. The app finds the VPX you have, or gets VPX 10.9 for you, and starts it with the table. VPX's own menus and editor work as they always have, and the app's settings pages change VPX's own settings, the same ones VPX's menus change.

**Do I need VPX installed?** Yes, VPX 10.9 or 10.8.0 (a VPX 10.8.1 build works too). If it isn't on your PC, first setup gets VPX 10.9 for you: build 7001, the one the app is made for, from VPX's automatic builds ([Which build of VPX](Installing-and-first-setup#which-build-of-vpx)). Settings › Library › **Get VPX 10.9.0** does it later. VPX 10.8.0 also needs the programs it plays with (VPinMAME, B2S Server and the rest); VPX 10.9 brings its own add-ons for them. See [Installing and first setup](Installing-and-first-setup).

**Which VPX does it play with?** VPX 10.9, unless you choose VPX 10.8.0: for every table in Settings › Library, or for one table in its page's **…** menu. VPX 10.9 gives you achievements as you play, screenshots and VR; in VPX 10.8.0, achievements count once the game is over. The app's pause menu works in both. A table that stops at start in 10.9 can play in 10.8.0 beside it. See [Playing](Playing#which-vpx-plays-it).

**Can I keep using VPX on its own, and my front end?** Yes. The app shares VPX's folders and settings, so a table you add in the app goes where VPX and your front end look for tables (your front end lists a new table once you add it to its own list, as always), and a table you add in VPX's folders shows in the app. PinUP Popper, PinballX and PinballY start VPX as they always have: see [Front ends](Front-ends).

**Which tables does it play?** Visual Pinball X tables (`.vpx`), as VPX plays them: with their scripts, ROMs, backglasses, score displays, color DMDs, PinUP packs, FlexDMD, UltraDMD and AltSound, through VPX's own add-ons or the classic programs you have. And Future Pinball's tables (`.fpt`), in Future Pinball itself.

**Does it play Future Pinball's tables?** Yes, where Future Pinball is installed: they sit in your library beside VPX's, and play in Future Pinball through BAM, on your screens, with your sound, keys and controller, the pause menu over them and their media recorded from the game. Future Pinball and BAM aren't part of the app: you install them, and Settings › Library links to their makers' pages where it doesn't find them. See [Future Pinball](Future-Pinball).

**Does it play tables in VR?** Yes: VPX 10.9 plays its tables in a VR headset, through Meta Horizon Link, SteamVR, Virtual Desktop or another VR program on the PC, as Settings › VR sets it up, or one game at a time with **Play it in VR**. Future Pinball's tables play in VR through BAM. See [VR](VR).

**Do tables play as they do in VPX?** They play in VPX itself, so yes: with VPX's physics, picture and sound. The app sets VPX's settings and starts it; how a table plays changes only where you change it yourself, such as its options, its physics or its script.

**Does it have achievements?** Yes: goals to earn as you play, from achievement packs made by a table's author or anyone else. In VPX 10.9 they're followed as you play, through an add-on the app puts into VPX, and each one shows the moment it's earned: over the backglass or the score display, or as VPX's own notice on the playfield, as Settings › Screens chooses. In VPX 10.8.0 they count once the game is over. Add a pack by dropping its zip on the app, or make your own in the table editor by clicking the table's parts. Ten sample packs, a download of their own beside each release, show what they can do, and the add-on works in VPX on its own too. See [Achievements](Achievements).

**Does it change my tables?** Never the table files. A changed ROM, or a script you change in the script editor, goes in a script file beside the table. Pictures of your own go in a copy of the table (**Make the copy**), the table itself left as it is. A table is only renamed when you choose to rename it for all players. Where a backglass made for a score display has a window for it, the app writes the score display's place into the backglass file, keeping the file as it came first; Settings › Screens can turn that off. Files it replaces go to the Recycle Bin.

**Does it work in a cabinet?** Yes. VPX plays on up to four screens, a portrait playfield among them, with toys, real DMD panels, and nudge and plunger sensors, through its own add-ons or the classic programs. The app places VPX's windows on your screens and keeps every setting in one place. Its cabinet mode runs the library with the cabinet's buttons, and front ends start VPX as before. See [Cabinet hardware](Cabinets) and [Cabinet mode](Cabinet-mode).

**What PC does it need?** Windows 10 or 11, 64-bit, one that plays VPX. For the pictures and videos the app makes of your tables, a graphics card that runs DirectX 12. It runs on Windows only.

**Where are my settings?** VPX's are in VPX's own file, `%APPDATA%\VPinballX\10.9\VPinballX.ini` for VPX 10.9, which the app's settings pages and VPX's menus both change, with a table's own beside the table in `<table>.ini`, as VPX keeps them. The app's own choices are in `%APPDATA%\Visual Pinball Infinite\Settings.ini`. VPX 10.8.0 keeps settings of its own, changed in its own Preferences. See [Settings](Settings).

**Does it go online?** Only to fetch the Virtual Pinball Spreadsheet's list and pictures, to look for updates and the team's short notice, if there is one, to get VPX from VPX's automatic builds when you ask, to show a table's guide from Pinball Primer when you open it in the pause menu, to show a file's page you open with **Get** and download what you choose there, to show Future Pinball's and BAM's pages when you open them from Settings › Library, to fetch a coloring program from its makers' releases when you ask, and to send a problem report when you choose to, then to ask for the team's replies to it.

**Where do I get tables and ROMs?** Tables come from their authors, on sites such as VPUniverse. A machine's page on the Virtual Pinball Spreadsheet links to its tables and their files, and the app's VPS browser and each table's page show them too, a file's page opening inside the app. ROMs belong to the machines' makers, and you supply them yourself, as for VPX.

**Who makes it?** An independent project, not part of the Visual Pinball X project, whose work it credits throughout. It's developed with Claude, Anthropic's AI.

**How can I help?** Play your tables through it and tell us what doesn't work: **Report a problem** in the app, a post on the [Discord server](https://discord.gg/GyZTZtsJag), or an issue on GitHub.
