# Fonts

## LiberationSans-Regular.ttf

Liberation Sans 2.00.1, Regular, by Red Hat (digitized data by Google), under the SIL Open Font
License 1.1 — its text, with the copyright lines it carries, in `LICENSE-LiberationSans.txt`
(from the Liberation fonts' own repository, release 2.00.3, the oldest tagged there; the font's
name table carries the same copyright lines and license). Unchanged; copied from VPX 5436's
source (`src/assets/LiberationSans-Regular.ttf`), where VPX's PinUP plugin uses it as the font of
last resort for labels (`PUPManager::LoadFonts`), as our player does (`pup/fonts.rs`). It's built
into the player.

## Inter-Latin-Regular.ttf, Inter-Latin-SemiBold.ttf, BarlowCondensed-Latin-Bold.ttf

The app's own fonts, for the player's pause menu (`pause_menu.rs`): [Inter](https://github.com/rsms/inter)
by Rasmus Andersson (The Inter Project Authors) and [Barlow Condensed](https://github.com/jpt/barlow)
by Jeremy Tribby (The Barlow Project Authors), both under the SIL Open Font License 1.1 with no
reserved font names — their texts, with their copyright lines, in `LICENSE-Inter.txt` and
`LICENSE-BarlowCondensed.txt`, as their packages carry them. Their Latin sets (Inter 400 and 600,
Barlow Condensed 700) from the app's font packages, Fontsource 5.3.0
(`app/ui/node_modules/@fontsource/inter`, `…/barlow-condensed`), written from those WOFF files as
the TrueType files they hold, which SDL_ttf reads, by `tools/menu-fonts/woff_to_ttf.py`: nothing in
the fonts changed but their wrapping. They're built into the player.
