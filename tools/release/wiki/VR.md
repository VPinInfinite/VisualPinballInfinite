# VR

VPX plays its tables in a VR headset, and the app sets it up: Settings › VR, and **Play it in VR** on a table's page. Future Pinball's tables play in VR too, through BAM.

## What VR needs

- **VPX's BGFX program, VPX 10.8.1 or newer.** VPX 10.9, as Settings › Library's Get VPX brings it, is that program (see [Which build of VPX](Installing-and-first-setup#which-build-of-vpx)). The app doesn't play VR games in VPX 10.8.0.
- **A VR program on the PC,** which talks to the headset: Meta Horizon Link, SteamVR, Virtual Desktop's Streamer, or another OpenXR program.
- **The headset,** connected to that program.

## Settings › VR

### Your VR program and headset

- **VPX plays VR with:** the VR program VPX plays with. Windows keeps one VR program for every VR app, shown as **Windows' own**; one chosen here is VPX's alone, and Windows' choice stays as it is. Below it, where that program sets its own resolution and refresh rate.
- **The headset:** **Check** asks the VR program for the headset, as VPX does before a game, and says what answers ("… answers, through …: ready to play"), or why nothing does. Until then, it shows VPX's last game in VR: its VR program, and the size it drew each eye at.

### Playing in VR

- **VR:** **Off**, **When the headset is on** or **Always**.
  - **When the headset is on:** the app asks for the headset as each game starts. The game plays in it when it answers, with VPX's own menu there, else on your screens with the app's pause menu.
  - **Always:** every table plays in the headset; for VPX's tables, the app checks first that it answers.
  - Future Pinball's tables follow it too, in BAM's VR.
- **The headset's refresh rate:** **The VR program's** (as it's set there), or **72 Hz**, **80 Hz**, **90 Hz** or **120 Hz**, asked of the headset as a game starts. A lower rate gives VPX more time to draw each picture, for a smoother game on a table that's heavy to draw.
- **Resolution in the headset:** **The VR program's**, or **× 1.2**, **× 1.4**, **× 1.6** or **× 2**. VPX draws each eye at the size the VR program recommends, times this: sharper, and heavier to draw. Each VR program recommends a size of its own, so the app keeps this for each one: one chosen for Link is never carried over to Virtual Desktop. Under it, the last game through the VR program VPX plays with: the size VPX drew each eye at, and the sizes recommended and at most.
- **Your screens meanwhile:** what the PC's screen shows while you play in the headset: **The playfield and displays**, **The playfield**, **What you see** (the headset's view, both eyes) or **Nothing**, which draws less if a game stutters. **The headset's view fitted to the screen** shrinks the headset's view to fit, rather than cut it.

### In the headset

- **A backglass for tables without their own:** a table made for screens shows no backglass in VR. On, its B2S or PinUP backglass stands above the playfield.
- **The backglass's sound from where it stands** and **The playfield's sounds from where they are:** each sound heard from where it comes from, as you turn your head.
- **Your room around the table:** mixed reality. VPX's room is drawn plain black, for the VR program to show your own room there, as Virtual Desktop does with its passthrough set to black.

### The cabinet you stand at

- **The lockbar's width** and **The lockbar's height,** with their **Width** and **Height** in centimetres: the virtual cabinet sized to a real lockbar under your hands, or to a cabinet's own. Off, the table's own size.
- **Tilted toward you:** for playing seated or leaning back. The view is tilted, and the physics stay as they are.

Where the table stands in your room, and its turn, are set in VPX's own menu in the headset, and the app keeps them from game to game.

**Save** keeps the page's changes in VPX's settings, and **Undo changes** puts them back. Sharpening (Settings › Picture) is left off in VR games, as VPX draws a line with it in the headset.

## Playing in VR

- **Play it in VR,** in a table's **…** menu, or **Play in VR** on its tile's right-click, plays one game in the headset, whatever Settings › VR says. It isn't offered for a table that plays with VPX 10.8.0.
- **PLAY** plays in the headset as Settings › VR's **VR** says.
- **The headset is asked for first** where a VPX table must play in VR (**Play it in VR**, or **VR** on **Always**). If it doesn't answer, the game isn't started, and the app says to put the headset on and connect it (Link or Air Link started on it, or Virtual Desktop connected), then play again.
- **In the headset,** F12 or the controller's menu button opens VPX's own menu, which VPX draws there: the app's pause menu isn't in VR.
- **The keyboard and a cabinet's buttons** reach VPX only while its window on the PC has the focus, so the app keeps it in front while a game plays in the headset.
- **The library's VR filter:** **Has** › **VR**, above the tables, shows the tables VPS lists as made for VR (VR, Hybrid or Mixed Reality).

## A Meta Quest 3

VPX reaches a Quest 3 through one of three VR programs: Meta Horizon Link, Virtual Desktop or SteamVR. Each is set up in its own program first, then chosen in Settings › VR › **VPX plays VR with**. Then **Check**, and play.

### Meta Horizon Link

1. On the PC: Meta Horizon Link, Meta's own program for the headset.
2. On the headset: start Link, or Air Link.
3. Settings › VR › **VPX plays VR with**: **Meta Horizon Link**, or **Windows' own: Meta Horizon Link** where Windows has it already.

Its refresh rate, up to 120 Hz on a Quest 3, and its render resolution, up to 5408 × 2736, are set on the PC: Meta Horizon Link › Devices › your headset › Graphics preferences. Frame interpolation (ASW) is in its Debug Tool.

### Virtual Desktop

1. On the PC: Virtual Desktop's Streamer, which brings Virtual Desktop's own VR program.
2. On the headset: Virtual Desktop, connected to the PC.
3. Settings › VR › **VPX plays VR with**: **Virtual Desktop**.

Its graphics quality, frame rate, codec and bitrate are set in the headset, on Virtual Desktop's Streaming tab. Leave its frame interpolation (SSW) off for pinball. **Resolution in the headset** is kept for Virtual Desktop apart from Link's: as each recommends a size of its own, a factor that suits one can be too heavy for the other.

### SteamVR

1. On the PC: SteamVR.
2. On the headset: connect it to the PC through Link or Virtual Desktop, as above. SteamVR reaches a Quest 3 through one of them.
3. Settings › VR › **VPX plays VR with**: **SteamVR**. The app finds SteamVR where it's installed, even while Windows has another VR program.

Its render resolution, for every app or for VPX alone, is set in SteamVR › Settings › Video.

## Future Pinball in VR

Future Pinball's tables play in VR through BAM's VR: as Settings › VR's **VR** says (**Always** plays them in the headset, **When the headset is on** asks for it first), or one game at a time with **Play it in VR**.

- **Straight to Link or Virtual Desktop:** where the VR program VPX plays with is **Meta Horizon Link** or **Virtual Desktop**, BAM's OpenComposite loader takes the game straight to it, without SteamVR. With another VR program, BAM's SteamVR loader plays it through SteamVR.
- **BAM's VR settings:** BAM's VR loaders, in Future Pinball's `BAM-OpenVR` and `BAM-OpenVR-OC` folders, need BAM's VR settings beside them. Where a folder has none, the app puts TerryRed's "BAM settings for VR" in place, from its zip in that folder or from his guide's folder in Future Pinball's folder, as his own steps put them: the table settings BAM had made there go to the Recycle Bin, for BAM to make again. Where neither is there, the game doesn't start, and the app says so.
- **Its picture:** Future Pinball's own settings are set for the game as TerryRed's VR start sets them (no backbox, no vertical sync), and put back after it.
- **In the headset,** BAM's menu works: the app's pause menu isn't in VR.

See [Future Pinball](Future-Pinball).

## When something isn't right

- **The headset didn't answer:** put it on and connect it to its VR program (Link or Air Link started on it, or Virtual Desktop connected), then play again. **Check**, on Settings › VR, tries it without a game.
- **"VPX's OpenXR library isn't beside it", or "VR plays in VPX's BGFX program … which isn't found for this library":** VR plays in VPX's BGFX program, VPX 10.8.1 or newer. VPX 10.9, as Settings › Library's **Get VPX 10.9.0** brings it, is that program.
- **VPX tried to play a table in VR, and couldn't:** after the game, the table's page says so, with **Turn VR off**, which sets Settings › VR's **VR** to **Off**. See [After a game](Playing#after-a-game).
- **Play it in VR isn't offered:** the table plays with VPX 10.8.0 (its page's **…** menu › **Play it with Settings › Library's VPX** takes it back), or VPX 10.8.0 plays every table (Settings › Library).
- **A game stutters:** a lower **Resolution in the headset**, a lower refresh rate, or **Nothing** on your screens meanwhile, each draws less. The VR program's own settings, above, matter too.
- **BAM's VR isn't set up:** see [Future Pinball in VR](#future-pinball-in-vr).
