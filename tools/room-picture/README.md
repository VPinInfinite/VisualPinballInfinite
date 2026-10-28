# Room picture

Makes the player's default room picture, `engine/player/assets/vpi/room/rostock-laage-airport.png`: the room a table is drawn in, and its parts reflect, when it names no room picture (environment) of its own, or one it doesn't hold.

VPX draws such a table with its built-in `EnvMap.webp`, an airport hall, which dates from 2014, before VPX's GPL rule, and can't be taken. Ours is a hall like it:

- **Source:** Poly Haven's [Rostock-Laage Airport](https://polyhaven.com/a/rostock_laage_airport) by Greg Zaal, **CC0** (no rights reserved: [Poly Haven's licence](https://polyhaven.com/license)). The file used is the 1k Radiance picture, `rostock_laage_airport_1k.hdr` (1,666,919 bytes, MD5 `ca1f40ff9dd4ca2c2b74b31c7fe7f6f6`), from `https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/1k/rostock_laage_airport_1k.hdr`. It isn't kept in the repository.
- **Made like VPX's:** 512 × 256, an ordinary 8-bit picture, its highlights rolled off softly, graded above and below the horizon so the light it casts on parts facing up and on parts facing down is what VPX's room casts, and its average colour VPX's room's (near neutral). VPX's picture is only measured, never copied.

To remake it (Python with NumPy and Pillow; the downloaded file in a folder of its own):

```
python -I tools/room-picture/make_room.py <folder>\rostock_laage_airport_1k.hdr vpx-build\vpinball-5436\build\Release\assets\EnvMap.webp engine\player\assets\vpi\room\rostock-laage-airport.png
```

It prints the light up and down and the average colour, VPX's room's beside ours. Then run the picture check and the VPX picture tool (`--ours-only`) on the tables that use it.

**Measured** against VPX's own pictures of the seven tables that draw the default room (share of texels differing, the grey gradient it replaced first): Andromeda 3.7% (4.3%), strippedTable 2.1% (3.6%), lightSeqTable 1.3% (0.5%), Nudge Test and Calibration 1.3% (0.8%), Jurassic Park 0.1% (0.1%), Gorgar and Screen Size Calibration 0.0% (0.0%). Andromeda's rails reflect the room, so they show the two rooms' differences; it counts as working (`tools/compatibility`'s accepted list). Matching VPX's room band by band, eight bands down the picture, measured worse (Andromeda 4.0%) and was dropped.
