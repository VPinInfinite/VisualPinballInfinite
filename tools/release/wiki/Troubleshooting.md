# When something isn't right

## The app says what's missing

Tables needing a look have a triangle on their tile, and gather under **Needs attention**, by kind. A table's page says what's wrong, in plain words, and how to fix it:

- **Its ROM isn't installed:** get it from the table's VPS page and drop it onto the app.
- **Its ROM is missing files:** the zip you have is probably another version; get the right one from the table's VPS page.
- **Its ROM also needs another zip, the version it's based on:** get that one too.
- **PinMAME doesn't know the ROM:** the table's script may name it wrongly; **Change** on its ROM row picks one PinMAME knows.
- **A script it loads isn't there,** or **the folder its script names for its DMD's pictures:** both usually come with the table's download. Without that folder, some tables stop before the game starts.
- **Its PinUP pack has no layout chosen yet:** the pack shows nothing until one is; **Layout…** picks the one for your screens.
- **Its PinUP pack is there under another name:** a pack is found by the name the table's script asks for; **Fix** on its row, then **Rename it**.
- **Its table file is here twice:** one looks like the same file saved again; its page asks which to keep.

**Test it** on a table's page runs the table in VPX 10.8.1 for a few seconds, out of sight and silent, and says how it went: errors in its script and, for a ROM table, whether its machine starts.

## Tables that stop at start

- **In VPX 10.8.1, but not in 10.8.0:** some older tables' scripts do something VPX 10.8.1 no longer allows (setting a ball's ID, for one), and stop with an error as the table starts. After the game, the table's page says so. Where VPX 10.8.0 is in your library, **Play it with VPX 10.8.0** plays that table there, and the app tries it again by itself in each newer 10.8.1 build. See [Which VPX plays it](Playing#which-vpx-plays-it).
- **In VPX 10.8.0, a ROM table kept in a folder of its own:** VPX 10.8.0 can't start ROM tables kept in a folder for each game. Play it with VPX 10.8.1.
- **In both:** often something it needs is missing, such as a script it loads, the folder of its DMD's pictures and videos, or its ROM. Its page says what (above). Otherwise VPX's log says what stopped it (below), and **Report a problem with it…** in its **…** menu puts the table's details and the log in a report for us.
- **Every table, on a PC with SteamVR or another VR program:** VPX's Enable VR setting may be starting tables for a headset. After the game, the table's page says so, with **Turn VR off**: see [After a game](Playing#after-a-game).

## Common problems

- **Windows says it protected your PC when you run the installer:** the installer isn't signed yet. Choose **More info**, then **Run anyway**.
- **PLAY says VPX wasn't found:** Settings › Library › VPX shows the VPX your tables play in. **Choose VPX's folder…** where it's somewhere else, or **Get VPX 10.8.1**.
- **"VPX couldn't start: a file it needs isn't on this PC":** VPX needs Microsoft's Visual C++ runtime, which most PCs already have, and for the games it starts the app lends VPX its own copy where Windows has none. So this usually means one of VPX's own files is missing: **Get VPX 10.8.1** in Settings › Library brings the build the app is made for again. A front end that starts VPX itself, on a PC that has never had the runtime, needs Microsoft's own installed: the Visual C++ Redistributable (x64), from Microsoft's website.
- **"Windows' VBScript is off on this PC":** VPX runs every table's script with Windows' VBScript. Windows 11 keeps VBScript as an optional part that can be turned off, and without it a table shows in VPX but nothing on it moves. To turn it on, open Windows' Settings › System › Optional features, choose **View features**, and add **VBSCRIPT**. Microsoft plans to leave it off on new Windows installs from around 2027.
- **A window opens on the wrong screen:** Settings › Screens › **Identify screens** shows which screen is which; move the window there, and VPX takes it from the next game. A table with screen settings of its own uses those: its page says so, and **Use my screens** gives it yours. After a game, the table's page says when VPX couldn't find a screen the settings name (see [After a game](Playing#after-a-game)).
- **No sound, or sound from the wrong speakers:** Settings › Sound › Speakers chooses where the playfield's and the backglass's sounds go. In the pause menu, each part of the sound has a level for that table: check none is at 0%.
- **A PinUP pack shows nothing:** choose its layout (**Layout…** on its row, or **Fit all packs…** in Tools › PinUP packs), and check its status in Tools › PinUP packs, and that packs are on in Settings › PinUP packs and in the table's own settings. Settings › Library › Its add-ons says whether VPX's own add-on or PinUP Player plays them.
- **The backglass doesn't show:** its file should be named as the table is, `<table>.directb2s`, beside it (the drop window does this), and Settings › Screens should show the backglass window. Where a table also has a PinUP pack, Settings › PinUP packs chooses which shows first on that window.
- **The score display isn't where it should be on the backglass:** where your VPX keeps its place in each backglass file, the app finds the place in the picture by itself, and **Score display…** on the table's backglass row places it by hand. In Settings › Screens, the backglass window's **The score display in it** chooses whether the app places it at all. See [Score displays](Score-displays#over-the-backglass).
- **A ROM table shows its factory settings, or won't start a game:** the first time a machine runs with no memory, it restores its factory settings. With VPX 10.8.1 in your library, the app starts each machine once, out of sight, before its first game, so the game then starts as it should. If its ROM is the wrong version, its page says so.
- **An AltSound pack doesn't play:** check it's on (Settings › Sound › Sound packs, and the table's own settings), what its row says on the table's page, and that its folder is named for the ROM the table plays. VPX's own AltSound goes with VPX's own PinMAME (Settings › Library › Its add-ons). Stern SAM and Pinball 2000 machines can't play packs. See [AltSound](AltSound#when-a-pack-stays-silent).
- **The color DMD doesn't show colors:** its coloring belongs in PinMAME's `altcolor` folder under the ROM's name (or in the game's own folder, when your library keeps a folder for each game), Serum or PIN2DMD colorings must be on in Settings › Picture, and the table's own settings mustn't turn its color DMD off. Those colorings go with VPX's own PinMAME (Settings › Library › Its add-ons). Tools › Color DMD shows which coloring each ROM table plays.
- **A table's script stops with an error after you changed it:** **Check it** in its script editor (**Edit its script** on its page) runs it in VPX, out of sight, and lists the errors by line; **Put back the table's own script** undoes your changes.
- **Achievements don't show as you play:** they're followed as you play in games started with PLAY, in VPX 10.8.1, with VPX's own add-ons for **Machines' ROMs** and **Backglasses** (Settings › Library › Its add-ons). With the classic programs, or in VPX 10.8.0, they count once the game is over.
- **An achievement counts as softcore:** the table isn't playing as its author made it (physics of your own or a VPX physics set you've changed, an edited script, its options or difficulty changed, PinMAME's cheat mode). Its achievements on its page say why, and PLAY's **▾** › **Play Hardcore** plays a game as its author made it; see [Achievements](Achievements#hardcore-and-softcore).
- **An achievement pack doesn't show on a table:** a pack made for one table file, its parts named as that file names them, goes with that file alone. **Player hub** › **Achievements** lists every pack, with the tables it fits.
- **F12 opens VPX's own menu, not the app's:** the app's pause menu comes with games started with PLAY, in a [newer VPX 10.8.1 build](Installing-and-first-setup#which-vpx-1081-build), while Settings › Controls › The pause menu is on **The app's**. In VPX 10.8.0, an older 10.8.1 build, or a game started from a front end, VPX's own menu shows.

## VPX's log

VPX writes what happens in each game to its log, `vpinball.log`, in VPX's settings folder, `%APPDATA%\VPinballX\10.8\`. A table's **…** menu › **Show VPX's last log** shows it. After each game, the app reads it for the notes on the table's page (see [After a game](Playing#after-a-game)), and a problem report takes it along. VPX 10.8.0 keeps its log beside its own program, and only where its settings switch it on (its editor's Preferences).

## Reporting a problem

The bug button at the top (**Report a problem**), **Report a problem with it…** in a table's **…** menu, or **Report it** after a game makes one file to send. Write what happened, untick anything you'd rather leave out, then **Make the report**:

- your note of what happened;
- the app's and VPX's logs, from this time and the time before;
- your settings, the app's and VPX's (never a PIN2DMD key);
- this PC: Windows, processor, memory, graphics card and screens;
- the table's details: what its page shows, and its own settings.

Your Windows user folder is taken out of every path in it. **Show it** to look through it first. Then **Send it to us**, which posts it with your note to the team's private channel on Discord, where only the team sees it, or attach it to your post in the [Discord server](https://discord.gg/GyZTZtsJag)'s #bug-reports or a GitHub issue. Nothing is sent until you say so. A build of the app made from its source code alone has no **Send it to us**, and asks for no replies or notices: attach the file instead.

**The team's replies.** The team can answer a report you send with **Send it to us**: their reply shows under **Report a problem › Your reports**, and the bug button at the top counts the new ones. Open a report there to read what the team wrote, and reply: your reply goes to the team with your report. Each report goes with a key made on your PC and kept there, so the team needs neither your name nor your Discord to answer it. The app asks for replies as it starts and every hour while it runs, and only once you've sent a report.

**News from the team.** Now and then the team puts up a short notice — a known problem, a fix on the way — which the app shows once as it starts.

The file is kept in the data folder's `Logs\Reports`. The app's own logs are in `%APPDATA%\Visual Pinball Infinite\Logs`.

## A table plays or looks wrong

Tables play in VPX itself, so how a table plays and looks comes from VPX and from its author. Tell us all the same, with **Report a problem with it…** in its **…** menu: say which table and version, and what's wrong. The report takes the table's details and VPX's log along.
