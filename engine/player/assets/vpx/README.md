# Files from Visual Pinball X

Copied unchanged from Visual Pinball X build 5436 (10.8.1 pre-release, commit `af26b2d`), the build the player follows. The player builds them into itself.

- `segments/` — the pictures VPX draws segment display digits from (`src/assets`): for each kind of digit and maker, four tiles side by side, each colour channel of a tile one segment's distance field. Selected as VPX's renderer selects them (`Renderer::SetupSegmentRenderer`; `engine/player/src/segments.rs`). `segments/alphaseg/` holds their source, from the same build: the Inkscape drawing of the segments (`SegDisplays.svg`) and the script that makes the pictures from it with msdfgen (`extract-segs.py`).
- `ball/BallEnv.exr` — VPX's default ball picture (`src/assets/BallEnv.exr`): a tileable environment the ball reflects when the table names no ball picture of its own (`Ball::Render`, `Renderer::GetBallEnvironment`; drawn in VPX's look).
- `scoreview/` — the default layouts of VPX's score view (`plugins/scoreview/layouts`), which place a game's DMD or segment displays in the score display's window (`engine/player/src/scoreview.rs`), and the four glass pictures they name (`Glass-Dirty-1.webp`, `Glass-Dirty-2.webp`, `Glass-Cracked-1.webp`, `Glass-Cracked-2.webp`): dust, scratches and cracks laid over the display, their alpha the glass's roughness, which spreads the dots' light into a glow. **Their author is unknown:** they were added with the layouts by VPX's team (Vincent Bousquet, 10 February 2025, "Refactor DMD window", pull request #2163; moved into the score view plugin on 14 May 2025), and no commit, pull request, document or web page names a source. They're taken as the VPX team's own; if their author asks, they come out.

## License

Copyright (C) 2000-2026 Visual Pinball development team and contributors.

GPL-3.0-or-later, as the player is. VPX's `LICENSE` makes every contribution since October 2020 GPLv3+ (unless it points out third-party content), and marks code files so on their first line; files with no license mention stay under its older, MAME-derived license. Pictures and layouts can't carry that first-line mark, so for them the contribution's date decides: all of these were added by the VPX team (Vincent Bousquet) between February 2025 and August 2026 — the segment drawing and the first layouts on 10 February 2025, the segment pictures as they are now on 22–23 February 2025, the LCD and CRT layouts on 1 August 2026, the ball picture on 22 May 2023 ("Replace default ball", reworked 30 May 2023 as a tileable environment) — none naming an outside source.
