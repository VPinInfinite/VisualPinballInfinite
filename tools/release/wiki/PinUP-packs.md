# PinUP packs

A PinUP pack is a set of videos, pictures and sounds made for one table. It plays on your other screens (the topper, the backglass, the score display, and sometimes the playfield) while the table plays, started by what happens in the game. VPX 10.8.1's own PinUP add-on plays packs, so PinUP Player isn't needed. Settings › Library › its add-ons can use the classic PinUP Player instead where it's installed, and VPX 10.8.0 always does.

## Getting one in

Drop the pack's download onto the app, with its table or onto its page. It goes into PinUP's `PUPVideos` folder (or into the table's own `pupvideos` folder, for a table kept in a folder of its own that has one), under the name the table's script asks for, which isn't always the pack's own. A pack that's there already can be replaced, or added to with **Append**, for packs that come in parts.

## Choosing its layout

Packs come with a few layouts for different setups (two screens, three with a full DMD, four with a topper), as option files their authors made. Until one is chosen, many packs show nothing.

1. **Layout…** on the table's page, or on the pack's row in Tools › PinUP packs, lists its layouts by their authors' names, with the one **In place** marked.
2. **Try it** starts the table in VPX with that layout on your screens, without changing the pack: close the table to come back. It isn't counted as a play.
3. Choose one and **Use this layout**: the app makes the copies its option files describe, inside the pack's folder. It never runs the files themselves; anything an option file asks for beyond copying inside the pack is shown, for you to do by hand if you want it. A file of yours it replaces goes to the Recycle Bin.

## Every pack: Tools › PinUP packs

The list shows every pack, with the table that plays it, its layout and its **Status**:

- **Complete**
- **No layout chosen:** choose one with **Layout…**
- **Files missing:** it may be an add-on for another pack.
- **Empty folder**
- **Still zipped:** **Unpack…** opens it in the drop window.
- **A folder too deep:** its files are packed one folder down.
- **Set by its script:** the table's script chooses what plays.

**Which packs show** picks **All**, **Used** or **Unused**. **Files** opens a pack's folder, **VPS** opens its page on VPS, and the bin moves a pack to the Recycle Bin, after asking.

**No table here plays these:** packs whose table isn't here, or whose table asks for another name. Where a table of yours asks for a name like the pack's, or VPS has a game by its name, a **Possible table match** is named, and **Rename it …** makes the pack that table's. A pack made for another version may not fit, so a renamed pack says "renamed from …", with **Undo**.

## On or off for one table

The **Enable** switch on the pack's row, on the table's page, turns it off for that table alone, as its settings' **PinUP pack** does. It's kept in the table's own settings.

Settings › PinUP packs › **PinUP packs** turns VPX's own PinUP add-on off for every table: it's the same switch as Settings › Library's for PinUP packs, so where the classic PinUP Player is installed, it plays the packs instead.

## Where it draws

Where a table has both a PinUP pack and a backglass file, Settings › PinUP packs chooses which comes first on the backglass and on the score display: **The pack** or **The backglass file**. **Room around the packs' screens** draws a pack's screens this many pixels in from each side of their window, on the backglass, the score display and the topper. Their volume is on Settings › Sound (**PinUP packs**), and the pause menu has a **PinUP pack** level while one plays.
