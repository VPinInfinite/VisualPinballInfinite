# Questions

**Is it free?** Yes: free and open source under the GNU General Public License, version 3 or later, for good. It will never be sold, and it has no account and no ads.

**Do I need VPX installed?** No. Everything comes in the installer. If you have VPX, the app can use your tables folder and files where they are.

**Can I keep using VPX?** Yes. They live side by side and share your folders, so a table you add in the app plays in VPX too. Each keeps its own machines' memory, saved scores and main settings, so a game in one doesn't change the other. What sits beside a table (its own settings file, or a script file made by Change the ROM or the script editor) is read by both, as VPX reads it.

**Which tables does it play?** Visual Pinball X tables (`.vpx`), with their scripts, ROMs (through PinMAME, built in), backglasses, score displays, color DMDs, PinUP packs, FlexDMD, UltraDMD and AltSound. Older Visual Pinball tables (`.vpt`) and Future Pinball tables don't play.

**Does it have achievements?** Yes: goals to earn as you play, from achievement packs made by a table's author or anyone else, announced on the backglass as you earn them. Ten sample packs show what they can do, and the table editor makes your own by clicking the table's parts. See [Achievements](Achievements).

**Does it play the same as VPX?** That's the aim: VPX's physics are rewritten in Rust line by line and checked against VPX's own, and tables are drawn as VPX draws them, on average within 0.1% of VPX's own picture on the tables we test. The website lists the [confirmed working tables](https://visualpinballinfinite.org/resources/tested-tables). If a table plays or looks different, please report it.

**Does it change my tables?** Never the table files. A changed ROM, or a script you change in the script editor, goes in a script file beside the table, and a table is only renamed when you choose to rename it for all players. Files it replaces go to the Recycle Bin.

**Does it work in a cabinet?** Yes: up to four screens, a portrait playfield, real DMD panels, toys, nudge and plunger sensors, cabinet mode, and starting from PinUP Popper, PinballX or PinballY. The toys, boards and panels haven't been tried in a real cabinet yet: see [Cabinet hardware](Cabinets).

**What PC does it need?** Windows 10 or 11, 64-bit, and a graphics card that runs DirectX 12. It runs on Windows only.

**Where are my settings?** In `%APPDATA%\Visual Pinball Infinite\Settings.ini`, under VPX's own names, with a table's own settings beside the table in `<table>.ini`, as VPX keeps them. See [Settings](Settings).

**Does it go online?** Only to fetch the Virtual Pinball Spreadsheet's list and pictures, to look for updates, to send a problem report when you choose to, and, once you connect it to VPIDB, to share what you choose there: your achievements, and your ratings and which tables you have.

**Where do I get tables and ROMs?** Tables come from their authors, on sites such as VPUniverse. A machine's page on the Virtual Pinball Spreadsheet links to its tables and their files, and VPIDB (vpidb.org) lists them too, each linked to where its authors released it. ROMs belong to the machines' makers, and you supply them yourself, as for VPX.

**What's VPIDB?** A free, non-commercial site at vpidb.org, a separate project from the same maker: every table and file in the Virtual Pinball Spreadsheet, each linked to where its authors released it, a free library of parts for building tables, and a place for achievement packs. The app connects to it only if you choose, to share your achievements, and your ratings and which tables you have: see [Achievements](Achievements#sharing-on-vpidb).

**Who makes it?** An independent project, not part of the Visual Pinball X project, whose work it credits throughout. It's developed with Claude, Anthropic's AI.

**How can I help?** Play your tables in it and tell us what doesn't work: **Report a problem** in the app, a post on the [Discord server](https://discord.gg/GyZTZtsJag), or an issue on GitHub.
