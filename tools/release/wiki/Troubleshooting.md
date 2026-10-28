# When something isn't right

## The app says what's missing

Tables needing a look have a triangle on their tile, and gather under **Needs attention**, by kind. A table's page says what's wrong, in plain words, and how to fix it:

- **Its ROM isn't installed:** get it from the table's VPS page and drop it onto the app.
- **Its ROM is missing files:** the zip you have is probably another version; get the right one from the table's VPS page.
- **Its ROM also needs another zip, the version it's based on:** get that one too.
- **PinMAME doesn't know the ROM:** the table's script may name it wrongly; **Change…** on its ROM row picks one PinMAME knows.
- **Its PinUP pack has no layout chosen yet:** the pack shows nothing until one is; **Layout…** picks the one for your screens.
- **Its PinUP pack is named for another name:** the player finds a pack by the name the table's script asks for; **Rename it** on its row.

**Test it** on a table's page runs it for about 15 seconds with no window and says how it went: errors in its script, and for a ROM table, whether its machine starts.

## Common problems

- **Windows says it protected your PC when you run the installer:** the installer isn't signed yet. Choose **More info**, then **Run anyway**.
- **A window opens on the wrong screen:** Settings › Screens › **Identify screens** shows which screen is which; move the window there. A table with its own screens, or a preset, uses those: check its settings.
- **No sound, or sound from the wrong speakers:** Settings › Sound › Speakers chooses where the playfield's and the backglass's sounds go.
- **A PinUP pack shows nothing:** choose its layout (**Layout…**), and check its status in Tools › PinUP packs.
- **The backglass doesn't show:** its file should be named as the table is, `<table>.directb2s`, beside it (the drop window does this), and Settings › Screens should show the backglass window. Where a table also has a PinUP pack, Settings › PinUP packs chooses which shows on that window.
- **A ROM table shows its factory settings, or won't start a game:** the first time a machine runs with no memory, it restores its factory settings; give it the first 15 seconds. If its ROM is the wrong version, its page says so.
- **An AltSound pack doesn't play:** check it's on (Settings › Sound, and the table's settings), what its row says on the table's page, and that its folder is named for the ROM the table plays. Stern SAM and Pinball 2000 machines can't play packs. See [AltSound](AltSound#when-a-pack-stays-silent).
- **The color DMD doesn't show colors:** its coloring belongs in VPinMAME's `altcolor` folder under the ROM's name (or in the game's own folder, when your library keeps a folder for each game), Serum or PIN2DMD colorings must be on in Settings › Picture, and the table's own settings mustn't turn its color DMD off. Tools › Color DMD shows which coloring each ROM table plays.
- **A table's script stops with an error after you changed it:** **Check it** in its script editor (the `<>` button on its page) lists the errors by line, and **Put back the table's own script** undoes your changes.
- **An achievement counts as softcore:** the table isn't playing as its author made it (physics of your own or a VPX physics set you've changed, an edited script, its options or difficulty changed, PinMAME's cheat mode). Its achievements on its page say why, and PLAY's **▾** › **Play Hardcore** plays a game as its author made it; see [Achievements](Achievements#hardcore-and-softcore).
- **An achievement pack doesn't show on a table:** a pack made for one table file, its parts named as that file names them, goes with that file alone. **Player hub** › **Achievements** lists every pack, with the tables it fits.
- **A table plays or looks different from VPX:** see below.

## Reporting a problem

The bug button at the top, or **Report a problem with it…** in a table's **…** menu, makes one file to send. Write what happened, untick anything you'd rather leave out, then **Make the report**:

- your note of what happened;
- the app's and the player's logs, from this time and the time before;
- your settings (never a PIN2DMD key);
- this PC: Windows, processor, memory, graphics card and screens;
- the table's details: what its page shows, and its own settings.

Your Windows user folder is taken out of every path in it. **Show it** to look through it first. Then **Send it to us**, which posts its text with your note to the team's private channel on Discord, where only the team sees it, or attach it to your post in the [Discord server](https://discord.gg/GyZTZtsJag)'s #bug-reports or a GitHub issue. Nothing is sent until you say so.

The file is kept in the data folder's `Logs\Reports`. The logs themselves are in `%APPDATA%\Visual Pinball Infinite\Logs`, and **Show the player's last log** on a table's **…** menu opens the last one.

## A table plays differently from VPX

Tell us which table, which version, and what's different, with the issue form for it on GitHub.
