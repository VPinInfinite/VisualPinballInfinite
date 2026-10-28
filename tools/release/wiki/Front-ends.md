# Starting tables from a front end

The player reads VPX's own command line, so a front end that starts VPX starts Visual Pinball Infinite with only the program changed. The installer puts the player at:

```
%LOCALAPPDATA%\Visual Pinball Infinite\vpi-player.exe
```

It reads `-Play "<table>"` and passes VPX's other switches over. Started that way, its window is titled "Visual Pinball Player", as VPX's is, so front ends' helpers find it. Started with VPX's `-minimized` switch, as front ends' scripts often start VPX, Esc quits the table at once, as in VPX.

## PinUP Popper

Popper Setup → Emulators → Visual Pinball X → its launch script. The script starts `%VPXEXE%`; set it to the player, in quotes:

```bat
SET VPXEXE="%LOCALAPPDATA%\Visual Pinball Infinite\vpi-player.exe"
```

The rest of the script stays as it is. To try it on a few tables first, leave the script alone and give those games the player as their alternative program.

The app works with Popper's folders too:

- It shows Popper's media for each game, and can put the ones you choose in Popper's folders (see [Media](Media#media-for-pinup-popper)).
- It shows the ratings you gave tables in Popper (VPin Studio keeps its ratings there too), and can put the ones you give in the app in Popper's list (see [Your library](Your-library#ratings)).
- A table renamed to VPS's name for all players changes Popper's game list with it.
- A table removed from the library comes off Popper's list.

## PinballX and PinballY

In the Visual Pinball X system's settings, set the program to the player, written out in full, and keep the parameters. These two haven't been checked yet: tell us how it goes. The media a table uses can go in their folders too: see [Media](Media). Ratings work as with Popper: theirs show in the app, and yours can go in their lists.

## Without a front end

The app's own [cabinet mode](Cabinet-mode) chooses and starts tables with a cabinet's buttons, on all its screens.
