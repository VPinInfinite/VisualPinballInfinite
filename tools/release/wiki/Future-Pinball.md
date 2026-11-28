# Future Pinball

Future Pinball's tables (`.fpt` files) sit in your library beside your VPX tables. They play in Future Pinball itself, through BAM where it's installed, and the app is their front end: it starts them on your screens, with your sound and controls, gives them the pause menu, and makes their media. Future Pinball and BAM aren't part of the app: they're their makers' own, free, and installed by you.

## Getting Future Pinball

Settings › Library › **Future Pinball** says where it is, **With BAM** or **Without BAM**, and how many tables it has. The app looks for it:

- where the app's settings name its folder (`[VPI]` `FuturePinballFolder`, in the app's `Settings.ini`);
- beside VPX's folder, as the Baller Installer lays them out (a `FuturePinball` folder beside `VisualPinball`);
- where its installer put it, as Windows' list of installed programs has it;
- in its usual folders.

Where it isn't found, **Get Future Pinball with BAM** opens BAM's download page in the app's own browser. Download the installer of Future Pinball with BAM there: it lands in `Downloads\Visual Pinball Infinite`. Run it from there, then **Look again**. **Future Pinball's site** opens its makers' site the same way.

## Its tables in your library

Its tables are the ones in its `Tables` folder, and one folder down. Each is matched to its game on the Virtual Pinball Spreadsheet among VPS's Future Pinball table files, for its pictures and facts, and shows in the library with the rest (see [Your library](Your-library)):

- Each tile has a tag for what plays it: **FP** for Future Pinball's tables, **VPX** for VPX's.
- Where your library holds both, the smart playlists start with **VPX** and **Future Pinball**.
- Favourites, ratings, playlists and your plays work as for VPX's tables.

## Playing

**PLAY** starts the table in Future Pinball, through BAM's loader where BAM is there. The app steps aside until Future Pinball has ended, then comes back on the same table, and the game counts as a play.

For each game, the app sets Future Pinball up as your settings pages have VPX, and puts Future Pinball's own settings back after it:

- **Screens:** the playfield goes where Settings › Screens puts VPX's playfield, at its size, filling its screen or in a window. Where the backglass shows in a window of its own, Future Pinball's backbox goes there, unless the table plays a PinUP pack, whose screens show instead.
- **Sound:** Future Pinball's sound plays through the playfield's output in Settings › Sound, and a PinUP pack's through the backglass's.
- **Keys:** the keys Settings › Controls gives the flippers and the upper flippers, the magna-saves, launching the ball, start, coins 1 to 3 and the three nudges. **Esc** ends the game.
- **A controller:** its buttons and triggers are passed to Future Pinball as those keys, from every controller connected, while Future Pinball's window is in front. Its menu button opens the pause menu (below).
- **A screen Windows scales** (above 100% in Windows' display settings): Future Pinball is drawn at the screen's own pixels, so it fills the screen rather than run past its edges.
- **Fullscreen optimizations:** **Turn off fullscreen optimizations**, in Settings › Library › Future Pinball, turns Windows' own setting off for Future Pinball and BAM's loader during its games. Future Pinball's guides set it against stutters in its full screen: leave it unticked unless you see them.

Where Future Pinball is installed, each settings page and part says which tables it sets: **VPX**, **FP** or **VPX + FP** (see [Settings](Settings)).

A table's own options (its physics, lights and camera) are in BAM's menu during a game: press **Q** or **~**.

## The pause menu

**F12**, or a controller's menu button, opens the app's pause menu over the table once it has loaded, and Future Pinball pauses. It holds **Resume**, **Rules**, **Guide** (where VPS links one for its game), **Controls**, **BAM's menu** (the table's physics, lights, camera and backbox, kept for it) and **Quit the game**. There's no volume, no levels and no achievements in it for Future Pinball's games. See [Playing](Playing#the-pause-menu).

Future Pinball's own full screen hides every other window, and gives way whenever another window takes the focus. So while Settings › Controls › **The pause menu** is on **The app's**, and Settings › Screens has the playfield fill its screen, Future Pinball plays in a borderless window over the whole of that screen, and the menu shows over it without taking the focus: the screen doesn't go black, and the sound doesn't cut out. With **The game's own**, Future Pinball keeps its own full screen, and the controller's menu button opens BAM's menu.

## Its page

A Future Pinball table's page shows what works for it: **PLAY**, its pictures and facts, its rating, its plays, **Edit its details**, **Show its files**, **Make media**, its PinUP pack's row with **Layout…**, and in its **…** menu **Add to a playlist…**, **Open on VPS's site**, **Play it in VR**, **Report a problem with it…**, **Copy where its file is** and **Remove from library…**. What the app does for VPX's tables alone isn't there: table settings, its script, Test it, high scores and achievements. Future Pinball's tables have no achievements.

## Its media

**Make media**, the film button beside PLAY, records a Future Pinball table's media from the game itself:

- **Recorded from the game:** the table plays on your screens in its attract mode for about a minute, then closes, with nothing to press. The app waits until the table plays, not its loading screen, and keeps only what Future Pinball and PinUP show: the playfield's picture and twenty seconds of its video, and the backglass's and score display's pictures where it shows them (its backbox, or a PinUP pack's screens).
- **Wheel:** its wheel, made from its pictures.

In Tools › Media, a Future Pinball table's row makes its wheel alone. See [Media](Media#making-media).

## Adding its files

Drop them onto the app, or onto the table's page, as for VPX's tables (see [Adding tables](Adding-tables)). Where Future Pinball is installed:

- a table (`.fpt`) goes into Future Pinball's `Tables` folder;
- a library (`.fpl`), which its tables load, into its `Libraries` folder;
- a table's own BAM settings, a `.cfg` in the table's download (its camera, lighting and BAM's own options, as its author set them), into BAM's `cfg` folder, named for the table file, which BAM finds them by;
- a PinUP pack into PinUP's `PUPVideos` folder.

Where Future Pinball isn't found, the drop window leaves its files out, and says why.

## In VR

Future Pinball's tables play in a VR headset through BAM's VR, as Settings › VR's **VR** says, or one game at a time with **Play it in VR**: see [VR](VR#future-pinball-in-vr).

## From a front end

A Future Pinball table started from PinUP Popper or another front end plays as that front end starts it, with Future Pinball's own settings: the app sets Future Pinball's screens and keys only for the games it starts.

## When something isn't right

- **Future Pinball isn't found:** Settings › Library › Future Pinball says where the app looks. Where yours is somewhere else, `[VPI]` `FuturePinballFolder`, in the app's `Settings.ini`, names its folder.
- **You leave a table while it's still loading:** Future Pinball stops answering when it's asked to end then. The app closes it once it has stopped answering for a few seconds, and comes back.
- **The pause menu doesn't open:** it opens once the table has loaded, while Settings › Controls › **The pause menu** is on **The app's**.
- **Something else:** **Report a problem with it…**, in the table's **…** menu, makes a report with the table's details in it (see [Troubleshooting](Troubleshooting#reporting-a-problem)).
