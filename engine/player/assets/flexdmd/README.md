# FlexDMD's own resources

The fonts and pictures FlexDMD carries inside itself, which table scripts and UltraDMD's stand-in
name as `FlexDMD.Resources.<name>` (`FlexDMD.Resources.udmd-f5by7.fnt`, `FlexDMD.Resources.dmds.black.png`).
Copied unchanged from FlexDMD 1.9.1's source (`FlexDMD/Resources`, https://github.com/vbousquet/flexdmd,
tag `1.9.1`), the version installed with VPX; built into the player (`engine/player/src/flexdmd/resources.rs`).
Each font is a bitmap font in
AngelCode's BMFont format: a description (`.fnt`) and a picture of its characters (`.png`), drawn
from the font named by FlexDMD's author, Vincent Bousquet. The original font files themselves
aren't here.

| Files | Font, and its author | Its terms, as published with it |
|---|---|---|
| `teeny_tiny_pixls-5.*` | **Teeny Tiny Pixls**, by Chequered Ink Ltd. | "100% free for all use", except that the font file itself may not be redistributed without Chequered Ink's permission. What's here is FlexDMD's picture of its characters, not the font file. |
| `bm_army-12.*` | **BM army** (12 px), by BitmapMania (cooz), 2001–2002, http://bitmapmania.m78.com/ | Freeware, copyright kept by its author: personal use welcome; not to be sold, nor bundled with products made for profit, without the author's permission. Visual Pinball Infinite is free and not for sale. |
| `zx_spectrum-7.*` | **ZX Spectrum**, by Mark sensen, made with FontStruct (https://fontstruct.com/fontstructions/show/1662490; FontStruct © Rob Meek) | FontStruct's commercial font licence: free to use, the font file not to be redistributed; embedding the font as a rasterized picture is allowed (its section 2.4). What's here is such a picture. |
| `udmd-f4by5.*`, `udmd-f5by7.*`, `udmd-f6by12.*`, `udmd-f7by13.*`, `udmd-f7by5.*`, `udmd-f12by24.*`, `udmd-f14by26.*` | **UltraDMD's fonts**, from UltraDMD by Stephen Rakonza (2013), remade as bitmap fonts by Vincent Bousquet for FlexDMD's stand-in for UltraDMD | UltraDMD is given away free; no licence for its fonts is published. |
| `colors.png`, `dmds/black.png` | FlexDMD's own pictures (a colour test, a plain black screen), by Vincent Bousquet | FlexDMD's licence, Apache-2.0 (`engine/player/src/flexdmd/LICENSE-FlexDMD.txt`). |

These terms were read from the files FlexDMD keeps with the fonts' sources (`FlexDMD/Fonts`): for
BM army its Japanese "read me", for Teeny Tiny Pixls its `License.txt`, for ZX Spectrum its
`license.txt` and `readme.txt`. Should an author ask, their font comes out.
