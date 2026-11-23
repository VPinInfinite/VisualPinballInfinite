Achievements for Visual Pinball X
=================================

An add-on for Visual Pinball X 10.8.1 that follows a table's achievement packs while you play:
a card and a sound when you earn one, an Achievements page in VPX's own menu (F12), and what
you've earned kept between games. It works on its own, in VPX alone, and inside Visual Pinball
Infinite, which manages packs for you.

What you need
-------------
- Visual Pinball X 10.8.1, build 5575 or later (a table's lights need build 6148 or later).
  Build 5436, on VPX's releases page, is too old for it: newer builds come from VPX's own
  automatic builds, and Visual Pinball Infinite's Get VPX 10.8.1 brings one.
- VPX's own add-ons for the tables you play: its PinMAME add-on for ROM machines, and its B2S
  add-on for a table whose backglass tells its scores.

Putting it in
-------------
VPX keeps a settings folder for each version: %APPDATA%\VPinballX\10.8 for VPX 10.8.1, and
%APPDATA%\VPinballX\10.9 for a 10.9 build. The steps below name 10.8's.
1. Copy the vpi-achievements folder into the plugins folder of VPX's settings folder:
   %APPDATA%\VPinballX\10.8\plugins\vpi-achievements
   (VPX's own plugins folder, beside VPinballX_BGFX64.exe, works too.)
2. Switch it on: in VPX's settings file, %APPDATA%\VPinballX\10.8\VPinballX.ini, add
     [Plugin.VPIAchievements]
     Enable = 1
   or switch it on where VPX lists its add-ons.

Where packs go
--------------
A pack is a folder holding achievements.json and its pictures and sounds. Put packs in an
"achievements" folder, either beside the table or in VPX's settings folder:
  <the table's folder>\achievements\<pack>
  %APPDATA%\VPinballX\10.8\achievements\<pack>
A pack plays with a table when one of the ROM sets it names is the one the table runs, or else
when the game's name it gives is in the table file's name (and its year too, where both have
one). Where several versions of one pack are kept, the latest plays.

Hardcore and softcore
---------------------
A game counts as hardcore while the table plays as its author made it. It's softcore, kept for
you but not counted toward mastering a pack, when a script or a physics file sits beside the
table, when the table's options or difficulty are changed in its own settings, or when PinMAME's
cheat mode is on. Changing one of those during a game turns the rest of that game softcore.

Settings
--------
In VPX's menu (F12, the add-on's settings), kept by VPX under [Plugin.VPIAchievements]:
- Achievement sound: on or off. Its level is in VPX's sound settings, as "Achievements".
- Achievement card: on the backglass, on the score display, or none.
- Achievement card size: small, medium or large.

What's earned
-------------
Kept in %APPDATA%\VPinballX\10.8\achievements (earned.sealed).

A ROM machine's score, ball and game
------------------------------------
For these, VPX's PinMAME add-on needs the machine's memory map from the Pinball Memory Maps
project in its memmaps folder. Newer VPX 10.8.1 builds carry these maps for most machines, and
Visual Pinball Infinite adds any that are missing. Without one, a ROM table's lamps, switches
and coils, its lights and its balls in play are still heard.

About
-----
Part of Visual Pinball Infinite, free and open source under the GNU General Public License,
version 3 or later (LICENSE.txt). Written with Claude, Anthropic's AI model.
What's built into it from others, with their licences: THIRD-PARTY-NOTICES.txt.
Its source: https://github.com/VPinInfinite/VisualPinballInfinite
