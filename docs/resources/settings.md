# Settings

Every setting Visual Pinball Infinite has today: its name in the settings file, what it does, the values it takes and its default. The app's Settings pages set the everyday ones, and its Advanced page lists every one.

They're kept in `%APPDATA%\Visual Pinball Infinite\Settings.ini`, each in its `[section]`; a table's own settings, in a `<table>.ini` beside the table, count over them for that table. The ROM machines' are in `Machine settings.ini` beside it: its `[default]` section for every machine, a game's own section for one.

490 settings. This page is made from the program's own list of its settings (`tools/settings-map`), so it grows as settings arrive.

## Windows and screens

Where each of the player's windows goes, on which screen and how big; the cabinet's screens and where the player stands.

### Output (windows) settings

- **Display** · `[Player]` `PlayfieldDisplay`\
  Display used for the main Playfield window.\
  Values: text · Default: empty

- **Display Mode** · `[Player]` `PlayfieldFullScreen`\
  Select between windowed or borderless fullscreen mode.\
  Values: 0 Windowed · 1 Borderless Fullscreen · Default: 0 (Windowed)

- **X Position** · `[Player]` `PlayfieldWndX`\
  Horizontal position of the window on the selected display.\
  Values: 0 to 16384 · Default: 0

- **Y Position** · `[Player]` `PlayfieldWndY`\
  Vertical position of the window on the selected display.\
  Values: 0 to 16384 · Default: 0

- **Width** · `[Player]` `PlayfieldWidth`\
  Width of the window.\
  Values: 0 to 16384 · Default: 16384

- **Height** · `[Player]` `PlayfieldHeight`\
  Height of the window.\
  Values: 0 to 16384 · Default: 16384

### Backglass Window

- **Output Mode** · `[Backglass]` `BackglassOutput`\
  Select between disabled, floating, or embedded in another window mode.\
  Values: 0 Disabled · 1 Floating · 2 Embedded in playfield · Default: 0 (Disabled)

- **Display** · `[Backglass]` `BackglassDisplay`\
  Display used for the main Backglass window.\
  Values: text · Default: empty

- **Display Mode** · `[Backglass]` `BackglassFullScreen`\
  Select between windowed or borderless fullscreen mode.\
  Values: 0 Windowed · 1 Borderless Fullscreen · Default: 0 (Windowed)

- **X Position** · `[Backglass]` `BackglassWndX`\
  Horizontal position of the window on the selected display.\
  Values: 0 to 16384 · Default: 0

- **Y Position** · `[Backglass]` `BackglassWndY`\
  Vertical position of the window on the selected display.\
  Values: 0 to 16384 · Default: 0

- **Width** · `[Backglass]` `BackglassWidth`\
  Width of the window.\
  Values: 0 to 16384 · Default: 16384

- **Height** · `[Backglass]` `BackglassHeight`\
  Height of the window.\
  Values: 0 to 16384 · Default: 16384

### ScoreView Window

- **Output Mode** · `[ScoreView]` `ScoreViewOutput`\
  Select between disabled, floating, or embedded in another window mode.\
  Values: 0 Disabled · 1 Floating · 2 Embedded in playfield · Default: 0 (Disabled)

- **Display** · `[ScoreView]` `ScoreViewDisplay`\
  Display used for the main ScoreView window.\
  Values: text · Default: empty

- **Display Mode** · `[ScoreView]` `ScoreViewFullScreen`\
  Select between windowed or borderless fullscreen mode.\
  Values: 0 Windowed · 1 Borderless Fullscreen · Default: 0 (Windowed)

- **X Position** · `[ScoreView]` `ScoreViewWndX`\
  Horizontal position of the window on the selected display.\
  Values: 0 to 16384 · Default: 0

- **Y Position** · `[ScoreView]` `ScoreViewWndY`\
  Vertical position of the window on the selected display.\
  Values: 0 to 16384 · Default: 0

- **Width** · `[ScoreView]` `ScoreViewWidth`\
  Width of the window.\
  Values: 0 to 16384 · Default: 16384

- **Height** · `[ScoreView]` `ScoreViewHeight`\
  Height of the window.\
  Values: 0 to 16384 · Default: 16384

### Topper Window

- **Output Mode** · `[Topper]` `TopperOutput`\
  Select between disabled, floating, or embedded in another window mode.\
  Values: 0 Disabled · 1 Floating · 2 Embedded in playfield · Default: 0 (Disabled)

- **Display** · `[Topper]` `TopperDisplay`\
  Display used for the Topper window.\
  Values: text · Default: empty

- **Display Mode** · `[Topper]` `TopperFullScreen`\
  Select between windowed or borderless fullscreen mode.\
  Values: 0 Windowed · 1 Borderless Fullscreen · Default: 0 (Windowed)

- **X Position** · `[Topper]` `TopperWndX`\
  Horizontal position of the Topper window on the selected display.\
  Values: 0 to 16384 · Default: 0

- **Y Position** · `[Topper]` `TopperWndY`\
  Vertical position of the Topper window on the selected display.\
  Values: 0 to 16384 · Default: 0

- **Width** · `[Topper]` `TopperWidth`\
  Width of the Topper window.\
  Values: 0 to 16384 · Default: 16384

- **Height** · `[Topper]` `TopperHeight`\
  Height of the Topper window.\
  Values: 0 to 16384 · Default: 16384

### Real world cabinet & player settings

- **Screen Width** · `[Player]` `ScreenWidth`\
  Physical width (cm) of the display area of the playfield (main) screen (width > height).\
  Values: 5 to 200 · Default: 95.89

- **Screen Inclination** · `[Player]` `ScreenInclination`\
  Inclination (degree) of the playfield (main) screen, 0 is horizontal.\
  Values: -30 to 30 · Default: 0

- **Player X** · `[Player]` `ScreenPlayerX`\
  Player X position in real world, expressed from the bottom center of the playfield, in centimeters.\
  Values: -30 to 30 · Default: 0

- **Player Y** · `[Player]` `ScreenPlayerY`\
  Player Y position in real world, expressed from the bottom center of the playfield, in centimeters.\
  Values: -70 to 30 · Default: -10

- **Player Z** · `[Player]` `ScreenPlayerZ`\
  Player Z position in real world, expressed from the bottom center of the playfield, in centimeters.\
  Values: 30 to 100 · Default: 70

## Picture

How tables are drawn: quality, effects, the ball, light, smoothing and frame timing.

### Graphics synchronisation and latency reduction

- **Synchronization** · `[Player]` `SyncMode`\
  None: No synchronization. Vertical Sync: Synchronize on video sync, avoids video tearing at the price of high visual latency. Adaptive Sync: Synchronize on video sync, except for late frames (below target FPS), also features higher visual latency. Frame Pacing: pace the frame rendering to limit visual latency, comes with the risk of introducing more stutters if the computer is not powerful enough.\
  Values: 0 No Sync · 1 Vertical Sync · 2 Adaptive Sync · 3 Frame Pacing · Default: 3 (Frame Pacing)

- **Limit Framerate** · `[Player]` `MaxFramerate`\
  -1 will limit FPS to the display refresh rate 0 will not limit the display refresh rate Other values will limit the FPS to it (energy saving/less heat, framerate stability).\
  Values: -1 to 1000 · Default: -1

- **Max. Prerendered Frames** · `[Player]` `MaxPrerenderedFrames`\
  Maximum number of 'frames in flight' (frames pushed to the GPU queue waiting for rendering). Higher values may lead to higher FPS, but at higher input latency. Recommended to be adjusted to the lowest value that allows a stable framerate. (Not supported in OpenGL version).\
  Values: 1 to 3 · Default: 1

### Graphics settings

- **Show FPS** · `[Player]` `ShowFPS`\
  Performance overlay display mode.\
  Values: 0 Disable · 1 FPS · 2 Full · Default: 0 (Disable)

- **Disable tonemapping on HDR display** · `[Player]` `HDRDisableToneMapper`\
  Do not perform tonemapping when rendering on a high dynamic range (HDR) capable monitor/gfxboard/OS.\
  Values: 1 on, 0 off · Default: 1 (on)

- **HDR Display Global Exposure** · `[Player]` `HDRGlobalExposure`\
  Global exposure scale multiplier for high dynamic range (HDR) capable monitors.\
  Values: 0 to 5 · Default: 1

- **Disable Bloom** · `[Player]` `ForceBloomOff`\
  Disable postprocessed bloom filter.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Disable Motion Blur** · `[Player]` `ForceMotionBlurOff`\
  Disable ball motion blur. This feature is BGFX only (and not supported in VR or headtracking).\
  Values: 1 on, 0 off · Default: 0 (off)

- **Force Anisotropic Filtering** · `[Player]` `ForceAnisotropicFiltering`\
  Force anisotropic filtering for better rendering quality/texture clarity at the cost of a bit of performance.\
  Values: 1 on, 0 off · Default: 1 (on)

- **Disable Ambient Occlusion** · `[Player]` `DisableAO`\
  Values: 1 on, 0 off · Default: 0 (off)

- **Dynamic Ambient Occlusion** · `[Player]` `DynamicAO`\
  Values: 1 on, 0 off · Default: 1 (on)

- **Reflection Quality** · `[Player]` `PFReflection`\
  Limit the quality of reflections for better performance. 'Dynamic' is recommended and will give the best results, but may harm performance. 'Static Only' has no performance cost (except for VR rendering). Other options feature different trade-offs between quality and performance.\
  Values: 0 Disable Reflections · 1 Balls Only · 2 Static Only · 3 Static & Balls · 4 Static & Unsynced Dynamic · 5 Dynamic · Default: 5 (Dynamic)

- **Detail Level** · `[Player]` `AlphaRampAccuracy`\
  Level of detail for balls and ramps.\
  Values: 1 to 10 · Default: 10

- **View Mode** · `[Player]` `BGSet`\
  Select between desktop, cabinet or 'full single screen' viewing mode configurations (if a table has set them up correctly).\
  Values: 0 Desktop / Full Single Screen · 1 Cabinet · 2 Full Single Screen · Default: 0 (Desktop / Full Single Screen)

### Aliasing & sharpening

- **Full Scene Anti Aliasing** · `[Player]` `AAFactor`\
  Enables brute-force Up/Downsampling (similar to DSR). This delivers very good quality but has a significant impact on performance. 200% means twice the resolution to be handled while rendering.\
  Values: 0.5 to 2 · Default: 1

- **MSAA level** · `[Player]` `MSAASamples`\
  Set the amount of MSAA samples. MSAA can help reduce geometry aliasing at the cost of performance and GPU memory. This can improve image quality if not using supersampling.\
  Values: 0 Disabled · 1 4 Samples · 2 6 Samples · 3 8 Samples · Default: 0 (Disabled)

- **Post processed antialiasing** · `[Player]` `FXAA`\
  Select between different antialiasing techniques that offer different quality vs performance balances.\
  Values: 0 Disabled · 1 Fast FXAA · 2 Standard FXAA · 3 Quality FXAA · 4 Fast NFAA · 5 Standard DLAA · 6 Quality SMAA · 7 Quality FAAA · Default: 0 (Disabled)

- **Post processed sharpening** · `[Player]` `Sharpen`\
  Select between different sharpening techniques that offer different quality vs performance balances.\
  Values: 0 Disabled · 1 CAS · 2 Bilateral CAS · Default: 0 (Disabled)

### Ball rendering

- **Unstretch Ball** · `[Player]` `BallAntiStretch`\
  Compensate ball stretching.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Disable Ball Lighting** · `[Player]` `DisableLightingForBalls`\
  Disable lighting and reflection effects on balls, e.g. to help the visually handicapped.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Overwrite ball image** · `[Player]` `OverwriteBallImage`\
  Allow to define images that will be used instead of the table's provided one.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Ball image override** · `[Player]` `BallImage`\
  Image to use for the ball instead of the table's provide one.\
  Values: text · Default: empty

- **Decal image override** · `[Player]` `DecalImage`\
  Image to use for the ball's decal instead of the table's provide one.\
  Values: text · Default: empty

### Overall scene lighting settings

- **Override Table Light Level** · `[Player]` `OverrideTableEmissionScale`\
  Replace default table light level by a fixed/custom one.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Day/Night** · `[Player]` `EmissionScale`\
  Select a custom ambient lighting level, ranging from daylight to night time.\
  Values: 0 to 1 · Default: 1

- **Use Automatic Light Level** · `[Player]` `DynamicDayNight`\
  Automatically compute/match scene lighting based on geographic coordinates/localized sun position.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Latitude** · `[Player]` `Latitude`\
  Local geographic latitude used to derive the sun position for automatically computed/matching scene lighting.\
  Values: -90 to 90 · Default: 52.52

- **Longitude** · `[Player]` `Longitude`\
  Local geographic longitude used to derive the sun position for automatically computed/matching scene lighting.\
  Values: -180 to 180 · Default: 13.37

## Score displays

How dot-matrix and segment displays look.

### DMD Display profiles

- **Legacy Renderer** · `[DMD]` `Profile1Legacy`\
  Use legacy DMD renderer.\
  Values: 1 on, 0 off · Default: 1 (on)

- **Dot Tint** · `[DMD]` `Profile1DotTint`\
  Color of lit dots.\
  Values: 0 to 16777215 · Default: 2970367

- **Unlit Dot Tint** · `[DMD]` `Profile1UnlitDotColor`\
  Color of unlit dots.\
  Values: 0 to 16777215 · Default: 4210752

- **Dot Brightness** · `[DMD]` `Profile1DotBrightness`\
  Dot brightness level.\
  Values: 0.001 to 100 · Default: 25

- **Dot Size** · `[DMD]` `Profile1DotSize`\
  Dot size.\
  Values: 0.001 to 1 · Default: 0.75

- **Dot Sharpness** · `[DMD]` `Profile1DotSharpness`\
  Sharpness of the dots (from round to squares).\
  Values: 0 to 1 · Default: 0.9

- **DMD Diffuse Glow** · `[DMD]` `Profile1DiffuseGlow`\
  Glow of dots on the back of the DMD (between dots, against glass).\
  Values: 0 to 10 · Default: 0.1

### Classic Neon plasma DMD

- **Legacy Renderer** · `[DMD]` `Profile2Legacy`\
  Use legacy DMD renderer.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Dot Tint** · `[DMD]` `Profile2DotTint`\
  Color of lit dots.\
  Values: 0 to 16777215 · Default: 2970367

- **Unlit Dot Tint** · `[DMD]` `Profile2UnlitDotColor`\
  Color of unlit dots.\
  Values: 0 to 16777215 · Default: 4210752

- **Dot Brightness** · `[DMD]` `Profile2DotBrightness`\
  Dot brightness level.\
  Values: 0.001 to 100 · Default: 25

- **Dot Size** · `[DMD]` `Profile2DotSize`\
  Dot size.\
  Values: 0.001 to 1 · Default: 0.6

- **Dot Sharpness** · `[DMD]` `Profile2DotSharpness`\
  Sharpness of the dots (from round to squares).\
  Values: 0 to 1 · Default: 0.9

- **DMD Diffuse Glow** · `[DMD]` `Profile2DiffuseGlow`\
  Glow of dots on the back of the DMD (between dots, against glass).\
  Values: 0 to 10 · Default: 0.1

### Red Led DMD (used after RoHS regulation entry into force)

- **Legacy Renderer** · `[DMD]` `Profile3Legacy`\
  Use legacy DMD renderer.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Dot Tint** · `[DMD]` `Profile3DotTint`\
  Color of lit dots.\
  Values: 0 to 16777215 · Default: 1385471

- **Unlit Dot Tint** · `[DMD]` `Profile3UnlitDotColor`\
  Color of unlit dots.\
  Values: 0 to 16777215 · Default: 4210752

- **Dot Brightness** · `[DMD]` `Profile3DotBrightness`\
  Dot brightness level.\
  Values: 0.001 to 100 · Default: 25

- **Dot Size** · `[DMD]` `Profile3DotSize`\
  Dot size.\
  Values: 0.001 to 1 · Default: 0.75

- **Dot Sharpness** · `[DMD]` `Profile3DotSharpness`\
  Sharpness of the dots (from round to squares).\
  Values: 0 to 1 · Default: 0.9

- **DMD Diffuse Glow** · `[DMD]` `Profile3DiffuseGlow`\
  Glow of dots on the back of the DMD (between dots, against glass).\
  Values: 0 to 10 · Default: 0.1

### Green Led

- **Legacy Renderer** · `[DMD]` `Profile4Legacy`\
  Use legacy DMD renderer.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Dot Tint** · `[DMD]` `Profile4DotTint`\
  Color of lit dots.\
  Values: 0 to 16777215 · Default: 2359061

- **Unlit Dot Tint** · `[DMD]` `Profile4UnlitDotColor`\
  Color of unlit dots.\
  Values: 0 to 16777215 · Default: 4210752

- **Dot Brightness** · `[DMD]` `Profile4DotBrightness`\
  Dot brightness level.\
  Values: 0.001 to 100 · Default: 25

- **Dot Size** · `[DMD]` `Profile4DotSize`\
  Dot size.\
  Values: 0.001 to 1 · Default: 0.75

- **Dot Sharpness** · `[DMD]` `Profile4DotSharpness`\
  Sharpness of the dots (from round to squares).\
  Values: 0 to 1 · Default: 0.9

- **DMD Diffuse Glow** · `[DMD]` `Profile4DiffuseGlow`\
  Glow of dots on the back of the DMD (between dots, against glass).\
  Values: 0 to 10 · Default: 0.1

- **Color** · `[Alpha]` `Profile5Color`\
  Color of lit segments.\
  Values: 0 to 16777215 · Default: 2359061

- **Unlit Color** · `[Alpha]` `Profile5Unlit`\
  Color of unlit segments.\
  Values: 0 to 16777215 · Default: 4210752

- **Brightness** · `[Alpha]` `Profile5Brightness`\
  Brightness level.\
  Values: 0.001 to 100 · Default: 5

- **Diffuse Glow** · `[Alpha]` `Profile5DiffuseGlow`\
  Glow of segments on the back of the display (between segments, against glass).\
  Values: 0 to 10 · Default: 1

### Yellow Led

- **Legacy Renderer** · `[DMD]` `Profile5Legacy`\
  Use legacy DMD renderer.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Dot Tint** · `[DMD]` `Profile5DotTint`\
  Color of lit dots.\
  Values: 0 to 16777215 · Default: 2359295

- **Unlit Dot Tint** · `[DMD]` `Profile5UnlitDotColor`\
  Color of unlit dots.\
  Values: 0 to 16777215 · Default: 4210752

- **Dot Brightness** · `[DMD]` `Profile5DotBrightness`\
  Dot brightness level.\
  Values: 0.001 to 100 · Default: 25

- **Dot Size** · `[DMD]` `Profile5DotSize`\
  Dot size.\
  Values: 0.001 to 1 · Default: 0.75

- **Dot Sharpness** · `[DMD]` `Profile5DotSharpness`\
  Sharpness of the dots (from round to squares).\
  Values: 0 to 1 · Default: 0.9

- **DMD Diffuse Glow** · `[DMD]` `Profile5DiffuseGlow`\
  Glow of dots on the back of the DMD (between dots, against glass).\
  Values: 0 to 10 · Default: 0.1

- **Color** · `[Alpha]` `Profile6Color`\
  Color of lit segments.\
  Values: 0 to 16777215 · Default: 2359295

- **Unlit Color** · `[Alpha]` `Profile6Unlit`\
  Color of unlit segments.\
  Values: 0 to 16777215 · Default: 4210752

- **Brightness** · `[Alpha]` `Profile6Brightness`\
  Brightness level.\
  Values: 0.001 to 100 · Default: 5

- **Diffuse Glow** · `[Alpha]` `Profile6DiffuseGlow`\
  Glow of segments on the back of the display (between segments, against glass).\
  Values: 0 to 10 · Default: 1

### Generic Plasma

- **Legacy Renderer** · `[DMD]` `Profile6Legacy`\
  Use legacy DMD renderer.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Dot Tint** · `[DMD]` `Profile6DotTint`\
  Color of lit dots.\
  Values: 0 to 16777215 · Default: 16777215

- **Unlit Dot Tint** · `[DMD]` `Profile6UnlitDotColor`\
  Color of unlit dots.\
  Values: 0 to 16777215 · Default: 4210752

- **Dot Brightness** · `[DMD]` `Profile6DotBrightness`\
  Dot brightness level.\
  Values: 0.001 to 100 · Default: 25

- **Dot Size** · `[DMD]` `Profile6DotSize`\
  Dot size.\
  Values: 0.001 to 1 · Default: 0.6

- **Dot Sharpness** · `[DMD]` `Profile6DotSharpness`\
  Sharpness of the dots (from round to squares).\
  Values: 0 to 1 · Default: 0.9

- **DMD Diffuse Glow** · `[DMD]` `Profile6DiffuseGlow`\
  Glow of dots on the back of the DMD (between dots, against glass).\
  Values: 0 to 10 · Default: 0.1

- **Color** · `[Alpha]` `Profile7Color`\
  Color of lit segments.\
  Values: 0 to 16777215 · Default: 16777215

- **Unlit Color** · `[Alpha]` `Profile7Unlit`\
  Color of unlit segments.\
  Values: 0 to 16777215 · Default: 4210752

- **Brightness** · `[Alpha]` `Profile7Brightness`\
  Brightness level.\
  Values: 0.001 to 100 · Default: 5

- **Diffuse Glow** · `[Alpha]` `Profile7DiffuseGlow`\
  Glow of segments on the back of the display (between segments, against glass).\
  Values: 0 to 10 · Default: 1

### Generic Led

- **Legacy Renderer** · `[DMD]` `Profile7Legacy`\
  Use legacy DMD renderer.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Dot Tint** · `[DMD]` `Profile7DotTint`\
  Color of lit dots.\
  Values: 0 to 16777215 · Default: 16777215

- **Unlit Dot Tint** · `[DMD]` `Profile7UnlitDotColor`\
  Color of unlit dots.\
  Values: 0 to 16777215 · Default: 4210752

- **Dot Brightness** · `[DMD]` `Profile7DotBrightness`\
  Dot brightness level.\
  Values: 0.001 to 100 · Default: 25

- **Dot Size** · `[DMD]` `Profile7DotSize`\
  Dot size.\
  Values: 0.001 to 1 · Default: 0.75

- **Dot Sharpness** · `[DMD]` `Profile7DotSharpness`\
  Sharpness of the dots (from round to squares).\
  Values: 0 to 1 · Default: 0.9

- **DMD Diffuse Glow** · `[DMD]` `Profile7DiffuseGlow`\
  Glow of dots on the back of the DMD (between dots, against glass).\
  Values: 0 to 10 · Default: 0.1

- **Color** · `[Alpha]` `Profile8Color`\
  Color of lit segments.\
  Values: 0 to 16777215 · Default: 16777215

- **Unlit Color** · `[Alpha]` `Profile8Unlit`\
  Color of unlit segments.\
  Values: 0 to 16777215 · Default: 4210752

- **Brightness** · `[Alpha]` `Profile8Brightness`\
  Brightness level.\
  Values: 0.001 to 100 · Default: 5

- **Diffuse Glow** · `[Alpha]` `Profile8DiffuseGlow`\
  Glow of segments on the back of the display (between segments, against glass).\
  Values: 0 to 10 · Default: 1

### Alphanumeric Segment Display profiles

- **Color** · `[Alpha]` `Profile1Color`\
  Color of lit segments.\
  Values: 0 to 16777215 · Default: 2004735

- **Unlit Color** · `[Alpha]` `Profile1Unlit`\
  Color of unlit segments.\
  Values: 0 to 16777215 · Default: 4210752

- **Brightness** · `[Alpha]` `Profile1Brightness`\
  Brightness level.\
  Values: 0.001 to 100 · Default: 5

- **Diffuse Glow** · `[Alpha]` `Profile1DiffuseGlow`\
  Glow of segments on the back of the display (between segments, against glass).\
  Values: 0 to 10 · Default: 1

### VFD Blueish

- **Color** · `[Alpha]` `Profile2Color`\
  Color of lit segments.\
  Values: 0 to 16777215 · Default: 16772927

- **Unlit Color** · `[Alpha]` `Profile2Unlit`\
  Color of unlit segments.\
  Values: 0 to 16777215 · Default: 4210752

- **Brightness** · `[Alpha]` `Profile2Brightness`\
  Brightness level.\
  Values: 0.001 to 100 · Default: 5

- **Diffuse Glow** · `[Alpha]` `Profile2DiffuseGlow`\
  Glow of segments on the back of the display (between segments, against glass).\
  Values: 0 to 10 · Default: 1

### VFD Greenish

- **Color** · `[Alpha]` `Profile3Color`\
  Color of lit segments.\
  Values: 0 to 16777215 · Default: 15531848

- **Unlit Color** · `[Alpha]` `Profile3Unlit`\
  Color of unlit segments.\
  Values: 0 to 16777215 · Default: 4210752

- **Brightness** · `[Alpha]` `Profile3Brightness`\
  Brightness level.\
  Values: 0.001 to 100 · Default: 5

- **Diffuse Glow** · `[Alpha]` `Profile3DiffuseGlow`\
  Glow of segments on the back of the display (between segments, against glass).\
  Values: 0 to 10 · Default: 1

### Red Led

- **Color** · `[Alpha]` `Profile4Color`\
  Color of lit segments.\
  Values: 0 to 16777215 · Default: 1385471

- **Unlit Color** · `[Alpha]` `Profile4Unlit`\
  Color of unlit segments.\
  Values: 0 to 16777215 · Default: 4210752

- **Brightness** · `[Alpha]` `Profile4Brightness`\
  Brightness level.\
  Values: 0.001 to 100 · Default: 5

- **Diffuse Glow** · `[Alpha]` `Profile4DiffuseGlow`\
  Glow of segments on the back of the display (between segments, against glass).\
  Values: 0 to 10 · Default: 1

## Sound

The sound outputs, the speaker layout and the volumes.

- **Backglass Volume** · `[Player]` `MusicVolume`\
  Main volume for music and sound played from the backglass speakers.\
  Values: 0 to 100 · Default: 100

- **Playfield Volume** · `[Player]` `SoundVolume`\
  Main volume for mechanical sounds coming from the playfield.\
  Values: 0 to 100 · Default: 100

- **Backglass Sound Device** · `[Player]` `SoundDeviceBG`\
  Select backglass sound device.\
  Values: text · Default: empty

- **Playfield Sound Device** · `[Player]` `SoundDevice`\
  Select playfield sound device.\
  Values: text · Default: empty

- **Playfield Output Mode** · `[Player]` `Sound3D`\
  Select how playfield sound is output to a speaker configuration.\
  Values: 0 2 Front channels · 1 2 Rear channels · 2 Up to 6 channels. Rear at lockbar · 3 Up to 6 channels. Front at lockbar · 4 6ch Side & Rear at lockbar. Legacy mixing · 5 6ch Side & Rear at lockbar. New mixing · Default: 0 (2 Front channels)

## Controls, nudging and plunger

The buttons for each action, the tilt bob, the nudge and the plunger.

### UI & input settings

- **Devices** · `[Input]` `Devices`\
  List of known devices.\
  Values: text · Default: empty

### Plumb settings

- **Plumb Simulation** · `[Player]` `SimulatedPlumb`\
  Enable/Disable mechanical Tilt plumb simulation.\
  Values: 1 on, 0 off · Default: 1 (on)

- **Plumb Damping** · `[Player]` `PlumbDamping`\
  Adjust plumb damping (how fast the pumb gets back to rest).\
  Values: 0 to 2 · Default: 1

- **Plumb Threshold** · `[Player]` `PlumbThresholdAngle`\
  Define threshold angle at which a Tilt is triggered.\
  Values: 0.15 to 4 · Default: 1

### Nudge settings

- **Keyboard Nudge Mode** · `[Player]` `KeyboardNudgeMode`\
  Select how nudge is simulated when triggered from the keyboard.\
  Values: 0 VP9 (simple push/retract) · 1 VPX 10.8 (cabinet model) · 2 VPX 10.9 (improved cabinet) · Default: 2 (VPX 10.9 (improved cabinet))

- **Keyboard Nudge Strength** · `[Player]` `KeyboardNudgeStrength`\
  Strength of the keyboard nudging mode.\
  Values: 0 to 2 · Default: 1

- **Visual Nudge Strength** · `[Player]` `NudgeStrength`\
  Changes the visual effect/screen shaking when nudging the table.\
  Values: 0 to 2 · Default: 1

- **Nudge Sensor Count** · `[Input]` `NudgeSensorCount`\
  Number of hardware nudge sensors defined.\
  Values: 0 to 100 · Default: 0

### Plunger settings

- **One Second Retract** · `[Player]` `PlungerRetract`\
  Enable retracting the plunger after a 1 second press when using the digital plunger emulation through keyboard or joystick button.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Plunger Sensor Count** · `[Input]` `PlungerSensorCount`\
  Number of hardware plunger sensor defined.\
  Values: 0 to 100 · Default: 0

## Physics

Eight saved physics sets, 0 to 7, each of a flipper's and the table's physics values: a table whose file names one plays with it in place of its own physics.

- **FlipperPhysicsMass0** · `[Player]` `FlipperPhysicsMass0`\
  Values: a number · Default: 1

- **FlipperPhysicsStrength0** · `[Player]` `FlipperPhysicsStrength0`\
  Values: a number · Default: 2200

- **FlipperPhysicsElasticity0** · `[Player]` `FlipperPhysicsElasticity0`\
  Values: a number · Default: 0.8

- **FlipperPhysicsScatter0** · `[Player]` `FlipperPhysicsScatter0`\
  Values: a number · Default: 0

- **FlipperPhysicsReturnStrength0** · `[Player]` `FlipperPhysicsReturnStrength0`\
  Values: a number · Default: 0.058

- **FlipperPhysicsElasticityFalloff0** · `[Player]` `FlipperPhysicsElasticityFalloff0`\
  Values: a number · Default: 0.43

- **FlipperPhysicsFriction0** · `[Player]` `FlipperPhysicsFriction0`\
  Values: a number · Default: 0.6

- **FlipperPhysicsCoilRampUp0** · `[Player]` `FlipperPhysicsCoilRampUp0`\
  Values: a number · Default: 3

- **FlipperPhysicsEOSTorque0** · `[Player]` `FlipperPhysicsEOSTorque0`\
  Values: a number · Default: 0.75

- **FlipperPhysicsEOSTorqueAngle0** · `[Player]` `FlipperPhysicsEOSTorqueAngle0`\
  Values: a number · Default: 6

- **TablePhysicsGravityConstant0** · `[Player]` `TablePhysicsGravityConstant0`\
  Values: a number

- **TablePhysicsContactFriction0** · `[Player]` `TablePhysicsContactFriction0`\
  Values: a number

- **TablePhysicsElasticity0** · `[Player]` `TablePhysicsElasticity0`\
  Values: a number

- **TablePhysicsElasticityFalloff0** · `[Player]` `TablePhysicsElasticityFalloff0`\
  Values: a number

- **TablePhysicsScatterAngle0** · `[Player]` `TablePhysicsScatterAngle0`\
  Values: a number

- **TablePhysicsContactScatterAngle0** · `[Player]` `TablePhysicsContactScatterAngle0`\
  Values: a number

- **TablePhysicsMinSlope0** · `[Player]` `TablePhysicsMinSlope0`\
  Values: a number

- **TablePhysicsMaxSlope0** · `[Player]` `TablePhysicsMaxSlope0`\
  Values: a number

- **PhysicsSetName0** · `[Player]` `PhysicsSetName0`\
  Values: text · Default: `Set 1`

- **FlipperPhysicsMass1** · `[Player]` `FlipperPhysicsMass1`\
  Values: a number · Default: 1

- **FlipperPhysicsStrength1** · `[Player]` `FlipperPhysicsStrength1`\
  Values: a number · Default: 2200

- **FlipperPhysicsElasticity1** · `[Player]` `FlipperPhysicsElasticity1`\
  Values: a number · Default: 0.8

- **FlipperPhysicsScatter1** · `[Player]` `FlipperPhysicsScatter1`\
  Values: a number · Default: 0

- **FlipperPhysicsReturnStrength1** · `[Player]` `FlipperPhysicsReturnStrength1`\
  Values: a number · Default: 0.058

- **FlipperPhysicsElasticityFalloff1** · `[Player]` `FlipperPhysicsElasticityFalloff1`\
  Values: a number · Default: 0.43

- **FlipperPhysicsFriction1** · `[Player]` `FlipperPhysicsFriction1`\
  Values: a number · Default: 0.6

- **FlipperPhysicsCoilRampUp1** · `[Player]` `FlipperPhysicsCoilRampUp1`\
  Values: a number · Default: 3

- **FlipperPhysicsEOSTorque1** · `[Player]` `FlipperPhysicsEOSTorque1`\
  Values: a number · Default: 0.75

- **FlipperPhysicsEOSTorqueAngle1** · `[Player]` `FlipperPhysicsEOSTorqueAngle1`\
  Values: a number · Default: 6

- **TablePhysicsGravityConstant1** · `[Player]` `TablePhysicsGravityConstant1`\
  Values: a number

- **TablePhysicsContactFriction1** · `[Player]` `TablePhysicsContactFriction1`\
  Values: a number

- **TablePhysicsElasticity1** · `[Player]` `TablePhysicsElasticity1`\
  Values: a number

- **TablePhysicsElasticityFalloff1** · `[Player]` `TablePhysicsElasticityFalloff1`\
  Values: a number

- **TablePhysicsScatterAngle1** · `[Player]` `TablePhysicsScatterAngle1`\
  Values: a number

- **TablePhysicsContactScatterAngle1** · `[Player]` `TablePhysicsContactScatterAngle1`\
  Values: a number

- **TablePhysicsMinSlope1** · `[Player]` `TablePhysicsMinSlope1`\
  Values: a number

- **TablePhysicsMaxSlope1** · `[Player]` `TablePhysicsMaxSlope1`\
  Values: a number

- **PhysicsSetName1** · `[Player]` `PhysicsSetName1`\
  Values: text · Default: `Set 2`

- **FlipperPhysicsMass2** · `[Player]` `FlipperPhysicsMass2`\
  Values: a number · Default: 1

- **FlipperPhysicsStrength2** · `[Player]` `FlipperPhysicsStrength2`\
  Values: a number · Default: 2200

- **FlipperPhysicsElasticity2** · `[Player]` `FlipperPhysicsElasticity2`\
  Values: a number · Default: 0.8

- **FlipperPhysicsScatter2** · `[Player]` `FlipperPhysicsScatter2`\
  Values: a number · Default: 0

- **FlipperPhysicsReturnStrength2** · `[Player]` `FlipperPhysicsReturnStrength2`\
  Values: a number · Default: 0.058

- **FlipperPhysicsElasticityFalloff2** · `[Player]` `FlipperPhysicsElasticityFalloff2`\
  Values: a number · Default: 0.43

- **FlipperPhysicsFriction2** · `[Player]` `FlipperPhysicsFriction2`\
  Values: a number · Default: 0.6

- **FlipperPhysicsCoilRampUp2** · `[Player]` `FlipperPhysicsCoilRampUp2`\
  Values: a number · Default: 3

- **FlipperPhysicsEOSTorque2** · `[Player]` `FlipperPhysicsEOSTorque2`\
  Values: a number · Default: 0.75

- **FlipperPhysicsEOSTorqueAngle2** · `[Player]` `FlipperPhysicsEOSTorqueAngle2`\
  Values: a number · Default: 6

- **TablePhysicsGravityConstant2** · `[Player]` `TablePhysicsGravityConstant2`\
  Values: a number

- **TablePhysicsContactFriction2** · `[Player]` `TablePhysicsContactFriction2`\
  Values: a number

- **TablePhysicsElasticity2** · `[Player]` `TablePhysicsElasticity2`\
  Values: a number

- **TablePhysicsElasticityFalloff2** · `[Player]` `TablePhysicsElasticityFalloff2`\
  Values: a number

- **TablePhysicsScatterAngle2** · `[Player]` `TablePhysicsScatterAngle2`\
  Values: a number

- **TablePhysicsContactScatterAngle2** · `[Player]` `TablePhysicsContactScatterAngle2`\
  Values: a number

- **TablePhysicsMinSlope2** · `[Player]` `TablePhysicsMinSlope2`\
  Values: a number

- **TablePhysicsMaxSlope2** · `[Player]` `TablePhysicsMaxSlope2`\
  Values: a number

- **PhysicsSetName2** · `[Player]` `PhysicsSetName2`\
  Values: text · Default: `Set 3`

- **FlipperPhysicsMass3** · `[Player]` `FlipperPhysicsMass3`\
  Values: a number · Default: 1

- **FlipperPhysicsStrength3** · `[Player]` `FlipperPhysicsStrength3`\
  Values: a number · Default: 2200

- **FlipperPhysicsElasticity3** · `[Player]` `FlipperPhysicsElasticity3`\
  Values: a number · Default: 0.8

- **FlipperPhysicsScatter3** · `[Player]` `FlipperPhysicsScatter3`\
  Values: a number · Default: 0

- **FlipperPhysicsReturnStrength3** · `[Player]` `FlipperPhysicsReturnStrength3`\
  Values: a number · Default: 0.058

- **FlipperPhysicsElasticityFalloff3** · `[Player]` `FlipperPhysicsElasticityFalloff3`\
  Values: a number · Default: 0.43

- **FlipperPhysicsFriction3** · `[Player]` `FlipperPhysicsFriction3`\
  Values: a number · Default: 0.6

- **FlipperPhysicsCoilRampUp3** · `[Player]` `FlipperPhysicsCoilRampUp3`\
  Values: a number · Default: 3

- **FlipperPhysicsEOSTorque3** · `[Player]` `FlipperPhysicsEOSTorque3`\
  Values: a number · Default: 0.75

- **FlipperPhysicsEOSTorqueAngle3** · `[Player]` `FlipperPhysicsEOSTorqueAngle3`\
  Values: a number · Default: 6

- **TablePhysicsGravityConstant3** · `[Player]` `TablePhysicsGravityConstant3`\
  Values: a number

- **TablePhysicsContactFriction3** · `[Player]` `TablePhysicsContactFriction3`\
  Values: a number

- **TablePhysicsElasticity3** · `[Player]` `TablePhysicsElasticity3`\
  Values: a number

- **TablePhysicsElasticityFalloff3** · `[Player]` `TablePhysicsElasticityFalloff3`\
  Values: a number

- **TablePhysicsScatterAngle3** · `[Player]` `TablePhysicsScatterAngle3`\
  Values: a number

- **TablePhysicsContactScatterAngle3** · `[Player]` `TablePhysicsContactScatterAngle3`\
  Values: a number

- **TablePhysicsMinSlope3** · `[Player]` `TablePhysicsMinSlope3`\
  Values: a number

- **TablePhysicsMaxSlope3** · `[Player]` `TablePhysicsMaxSlope3`\
  Values: a number

- **PhysicsSetName3** · `[Player]` `PhysicsSetName3`\
  Values: text · Default: `Set 4`

- **FlipperPhysicsMass4** · `[Player]` `FlipperPhysicsMass4`\
  Values: a number · Default: 1

- **FlipperPhysicsStrength4** · `[Player]` `FlipperPhysicsStrength4`\
  Values: a number · Default: 2200

- **FlipperPhysicsElasticity4** · `[Player]` `FlipperPhysicsElasticity4`\
  Values: a number · Default: 0.8

- **FlipperPhysicsScatter4** · `[Player]` `FlipperPhysicsScatter4`\
  Values: a number · Default: 0

- **FlipperPhysicsReturnStrength4** · `[Player]` `FlipperPhysicsReturnStrength4`\
  Values: a number · Default: 0.058

- **FlipperPhysicsElasticityFalloff4** · `[Player]` `FlipperPhysicsElasticityFalloff4`\
  Values: a number · Default: 0.43

- **FlipperPhysicsFriction4** · `[Player]` `FlipperPhysicsFriction4`\
  Values: a number · Default: 0.6

- **FlipperPhysicsCoilRampUp4** · `[Player]` `FlipperPhysicsCoilRampUp4`\
  Values: a number · Default: 3

- **FlipperPhysicsEOSTorque4** · `[Player]` `FlipperPhysicsEOSTorque4`\
  Values: a number · Default: 0.75

- **FlipperPhysicsEOSTorqueAngle4** · `[Player]` `FlipperPhysicsEOSTorqueAngle4`\
  Values: a number · Default: 6

- **TablePhysicsGravityConstant4** · `[Player]` `TablePhysicsGravityConstant4`\
  Values: a number

- **TablePhysicsContactFriction4** · `[Player]` `TablePhysicsContactFriction4`\
  Values: a number

- **TablePhysicsElasticity4** · `[Player]` `TablePhysicsElasticity4`\
  Values: a number

- **TablePhysicsElasticityFalloff4** · `[Player]` `TablePhysicsElasticityFalloff4`\
  Values: a number

- **TablePhysicsScatterAngle4** · `[Player]` `TablePhysicsScatterAngle4`\
  Values: a number

- **TablePhysicsContactScatterAngle4** · `[Player]` `TablePhysicsContactScatterAngle4`\
  Values: a number

- **TablePhysicsMinSlope4** · `[Player]` `TablePhysicsMinSlope4`\
  Values: a number

- **TablePhysicsMaxSlope4** · `[Player]` `TablePhysicsMaxSlope4`\
  Values: a number

- **PhysicsSetName4** · `[Player]` `PhysicsSetName4`\
  Values: text · Default: `Set 5`

- **FlipperPhysicsMass5** · `[Player]` `FlipperPhysicsMass5`\
  Values: a number · Default: 1

- **FlipperPhysicsStrength5** · `[Player]` `FlipperPhysicsStrength5`\
  Values: a number · Default: 2200

- **FlipperPhysicsElasticity5** · `[Player]` `FlipperPhysicsElasticity5`\
  Values: a number · Default: 0.8

- **FlipperPhysicsScatter5** · `[Player]` `FlipperPhysicsScatter5`\
  Values: a number · Default: 0

- **FlipperPhysicsReturnStrength5** · `[Player]` `FlipperPhysicsReturnStrength5`\
  Values: a number · Default: 0.058

- **FlipperPhysicsElasticityFalloff5** · `[Player]` `FlipperPhysicsElasticityFalloff5`\
  Values: a number · Default: 0.43

- **FlipperPhysicsFriction5** · `[Player]` `FlipperPhysicsFriction5`\
  Values: a number · Default: 0.6

- **FlipperPhysicsCoilRampUp5** · `[Player]` `FlipperPhysicsCoilRampUp5`\
  Values: a number · Default: 3

- **FlipperPhysicsEOSTorque5** · `[Player]` `FlipperPhysicsEOSTorque5`\
  Values: a number · Default: 0.75

- **FlipperPhysicsEOSTorqueAngle5** · `[Player]` `FlipperPhysicsEOSTorqueAngle5`\
  Values: a number · Default: 6

- **TablePhysicsGravityConstant5** · `[Player]` `TablePhysicsGravityConstant5`\
  Values: a number

- **TablePhysicsContactFriction5** · `[Player]` `TablePhysicsContactFriction5`\
  Values: a number

- **TablePhysicsElasticity5** · `[Player]` `TablePhysicsElasticity5`\
  Values: a number

- **TablePhysicsElasticityFalloff5** · `[Player]` `TablePhysicsElasticityFalloff5`\
  Values: a number

- **TablePhysicsScatterAngle5** · `[Player]` `TablePhysicsScatterAngle5`\
  Values: a number

- **TablePhysicsContactScatterAngle5** · `[Player]` `TablePhysicsContactScatterAngle5`\
  Values: a number

- **TablePhysicsMinSlope5** · `[Player]` `TablePhysicsMinSlope5`\
  Values: a number

- **TablePhysicsMaxSlope5** · `[Player]` `TablePhysicsMaxSlope5`\
  Values: a number

- **PhysicsSetName5** · `[Player]` `PhysicsSetName5`\
  Values: text · Default: `Set 6`

- **FlipperPhysicsMass6** · `[Player]` `FlipperPhysicsMass6`\
  Values: a number · Default: 1

- **FlipperPhysicsStrength6** · `[Player]` `FlipperPhysicsStrength6`\
  Values: a number · Default: 2200

- **FlipperPhysicsElasticity6** · `[Player]` `FlipperPhysicsElasticity6`\
  Values: a number · Default: 0.8

- **FlipperPhysicsScatter6** · `[Player]` `FlipperPhysicsScatter6`\
  Values: a number · Default: 0

- **FlipperPhysicsReturnStrength6** · `[Player]` `FlipperPhysicsReturnStrength6`\
  Values: a number · Default: 0.058

- **FlipperPhysicsElasticityFalloff6** · `[Player]` `FlipperPhysicsElasticityFalloff6`\
  Values: a number · Default: 0.43

- **FlipperPhysicsFriction6** · `[Player]` `FlipperPhysicsFriction6`\
  Values: a number · Default: 0.6

- **FlipperPhysicsCoilRampUp6** · `[Player]` `FlipperPhysicsCoilRampUp6`\
  Values: a number · Default: 3

- **FlipperPhysicsEOSTorque6** · `[Player]` `FlipperPhysicsEOSTorque6`\
  Values: a number · Default: 0.75

- **FlipperPhysicsEOSTorqueAngle6** · `[Player]` `FlipperPhysicsEOSTorqueAngle6`\
  Values: a number · Default: 6

- **TablePhysicsGravityConstant6** · `[Player]` `TablePhysicsGravityConstant6`\
  Values: a number

- **TablePhysicsContactFriction6** · `[Player]` `TablePhysicsContactFriction6`\
  Values: a number

- **TablePhysicsElasticity6** · `[Player]` `TablePhysicsElasticity6`\
  Values: a number

- **TablePhysicsElasticityFalloff6** · `[Player]` `TablePhysicsElasticityFalloff6`\
  Values: a number

- **TablePhysicsScatterAngle6** · `[Player]` `TablePhysicsScatterAngle6`\
  Values: a number

- **TablePhysicsContactScatterAngle6** · `[Player]` `TablePhysicsContactScatterAngle6`\
  Values: a number

- **TablePhysicsMinSlope6** · `[Player]` `TablePhysicsMinSlope6`\
  Values: a number

- **TablePhysicsMaxSlope6** · `[Player]` `TablePhysicsMaxSlope6`\
  Values: a number

- **PhysicsSetName6** · `[Player]` `PhysicsSetName6`\
  Values: text · Default: `Set 7`

- **FlipperPhysicsMass7** · `[Player]` `FlipperPhysicsMass7`\
  Values: a number · Default: 1

- **FlipperPhysicsStrength7** · `[Player]` `FlipperPhysicsStrength7`\
  Values: a number · Default: 2200

- **FlipperPhysicsElasticity7** · `[Player]` `FlipperPhysicsElasticity7`\
  Values: a number · Default: 0.8

- **FlipperPhysicsScatter7** · `[Player]` `FlipperPhysicsScatter7`\
  Values: a number · Default: 0

- **FlipperPhysicsReturnStrength7** · `[Player]` `FlipperPhysicsReturnStrength7`\
  Values: a number · Default: 0.058

- **FlipperPhysicsElasticityFalloff7** · `[Player]` `FlipperPhysicsElasticityFalloff7`\
  Values: a number · Default: 0.43

- **FlipperPhysicsFriction7** · `[Player]` `FlipperPhysicsFriction7`\
  Values: a number · Default: 0.6

- **FlipperPhysicsCoilRampUp7** · `[Player]` `FlipperPhysicsCoilRampUp7`\
  Values: a number · Default: 3

- **FlipperPhysicsEOSTorque7** · `[Player]` `FlipperPhysicsEOSTorque7`\
  Values: a number · Default: 0.75

- **FlipperPhysicsEOSTorqueAngle7** · `[Player]` `FlipperPhysicsEOSTorqueAngle7`\
  Values: a number · Default: 6

- **TablePhysicsGravityConstant7** · `[Player]` `TablePhysicsGravityConstant7`\
  Values: a number

- **TablePhysicsContactFriction7** · `[Player]` `TablePhysicsContactFriction7`\
  Values: a number

- **TablePhysicsElasticity7** · `[Player]` `TablePhysicsElasticity7`\
  Values: a number

- **TablePhysicsElasticityFalloff7** · `[Player]` `TablePhysicsElasticityFalloff7`\
  Values: a number

- **TablePhysicsScatterAngle7** · `[Player]` `TablePhysicsScatterAngle7`\
  Values: a number

- **TablePhysicsContactScatterAngle7** · `[Player]` `TablePhysicsContactScatterAngle7`\
  Values: a number

- **TablePhysicsMinSlope7** · `[Player]` `TablePhysicsMinSlope7`\
  Values: a number

- **TablePhysicsMaxSlope7** · `[Player]` `TablePhysicsMaxSlope7`\
  Values: a number

- **PhysicsSetName7** · `[Player]` `PhysicsSetName7`\
  Values: text · Default: `Set 8`

## Table overrides and camera

Settings that count over every table's own, and the camera tables open with.

- **View mode** · `[TableOverride]` `ViewDTMode`\
  Select between 'Legacy' (old rendering mode with visually incorrect stretchs), 'Camera' (classic camera, for desktop) and 'Window' (custom projection designed for cabinet users) rendering mode.\
  Values: 0 Legacy · 1 Camera · 2 Window · Default: 1 (Camera)

- **Look at** · `[TableOverride]` `ViewDTLookAt`\
  Relative point of playfield where the camera is looking at.\
  Values: 0 to 100 · Default: 25

- **Field Of View (overall scale)** · `[TableOverride]` `ViewDTFOV`\
  Global view scale (same as XYZ scale).\
  Values: 25 to 90 · Default: 50

- **Layback** · `[TableOverride]` `ViewDTLayback`\
  Fake visual stretch of the table to give more depth.\
  Values: -90 to 90 · Default: 0

- **Table X Scale** · `[TableOverride]` `ViewDTScaleX`\
  Stretch the scene along the playfield width axis.\
  Values: 0.5 to 1.5 · Default: 1

- **Table Y Scale** · `[TableOverride]` `ViewDTScaleY`\
  Stretch the scene along the playfield height axis.\
  Values: 0.5 to 1.5 · Default: 1

- **Table Z Scale** · `[TableOverride]` `ViewDTScaleZ`\
  Stretch the scene along the vertical axis (perpendicular to playfield).\
  Values: 0.5 to 1.5 · Default: 1

- **Camera X** · `[TableOverride]` `ViewDTPlayerX`\
  View point width offset.\
  Values: a number

- **Camera Y** · `[TableOverride]` `ViewDTPlayerY`\
  View point height offset.\
  Values: a number

- **Camera Z** · `[TableOverride]` `ViewDTPlayerZ`\
  View point vertical offset.\
  Values: a number

- **Horizontal Offset** · `[TableOverride]` `ViewDTHOfs`\
  Horizontal offset of the virtual table behind the screen 'window'.\
  Values: -30 to 30 · Default: 0

- **Vertical Offset** · `[TableOverride]` `ViewDTVOfs`\
  Vertical offset of the virtual table behind the screen 'window'.\
  Values: -20 to 50 · Default: 0

- **Window Top Z Ofs.** · `[TableOverride]` `ViewDTWindowTop`\
  Distance between the 'window' (i.e. the screen) at the top of the playfield.\
  Values: a number

- **Window Bottom Z Ofs.** · `[TableOverride]` `ViewDTWindowBot`\
  Distance between the 'window' (i.e. the screen) at the bottom of the playfield.\
  Values: a number

- **Viewport Rotation** · `[TableOverride]` `ViewDTRotation`\
  Values: 0 to 360, in steps of 90 · Default: 0

- **View mode** · `[TableOverride]` `ViewFSSMode`\
  Select between 'Legacy' (old rendering mode with visually incorrect stretchs), 'Camera' (classic camera, for desktop) and 'Window' (custom projection designed for cabinet users) rendering mode.\
  Values: 0 Legacy · 1 Camera · 2 Window · Default: 1 (Camera)

- **Look at** · `[TableOverride]` `ViewFSSLookAt`\
  Relative point of playfield where the camera is looking at.\
  Values: 0 to 100 · Default: 0

- **Field Of View (overall scale)** · `[TableOverride]` `ViewFSSFOV`\
  Global view scale (same as XYZ scale).\
  Values: 25 to 90 · Default: 25

- **Layback** · `[TableOverride]` `ViewFSSLayback`\
  Fake visual stretch of the table to give more depth.\
  Values: -90 to 90 · Default: 0

- **Table X Scale** · `[TableOverride]` `ViewFSSScaleX`\
  Stretch the scene along the playfield width axis.\
  Values: 0.5 to 1.5 · Default: 1

- **Table Y Scale** · `[TableOverride]` `ViewFSSScaleY`\
  Stretch the scene along the playfield height axis.\
  Values: 0.5 to 1.5 · Default: 1

- **Table Z Scale** · `[TableOverride]` `ViewFSSScaleZ`\
  Stretch the scene along the vertical axis (perpendicular to playfield).\
  Values: 0.5 to 1.5 · Default: 1

- **Camera X** · `[TableOverride]` `ViewFSSPlayerX`\
  View point width offset.\
  Values: a number

- **Camera Y** · `[TableOverride]` `ViewFSSPlayerY`\
  View point height offset.\
  Values: a number

- **Camera Z** · `[TableOverride]` `ViewFSSPlayerZ`\
  View point vertical offsets.\
  Values: a number

- **Horizontal Offset** · `[TableOverride]` `ViewFSSHOfs`\
  Horizontal offset of the virtual table behind the screen 'window'.\
  Values: -30 to 30 · Default: 0

- **Vertical Offset** · `[TableOverride]` `ViewFSSVOfs`\
  Vertical offset of the virtual table behind the screen 'window'.\
  Values: -20 to 50 · Default: 0

- **Window Top Z Ofs.** · `[TableOverride]` `ViewFSSWindowTop`\
  Distance between the 'window' (i.e. the screen) at the top of the playfield.\
  Values: a number

- **Window Bottom Z Ofs.** · `[TableOverride]` `ViewFSSWindowBot`\
  Distance between the 'window' (i.e. the screen) at the bottom of the playfield.\
  Values: a number

- **Viewport Rotation** · `[TableOverride]` `ViewFSSRotation`\
  Values: 0 to 360, in steps of 90 · Default: 0

- **View mode** · `[TableOverride]` `ViewCabMode`\
  Select between 'Legacy' (old rendering mode with visually incorrect stretchs), 'Camera' (classic camera, for desktop) and 'Window' (custom projection designed for cabinet users) rendering mode.\
  Values: 0 Legacy · 1 Camera · 2 Window · Default: 2 (Window)

- **Look at** · `[TableOverride]` `ViewCabLookAt`\
  Relative point of playfield where the camera is looking at.\
  Values: 0 to 100 · Default: 0

- **Field Of View (overall scale)** · `[TableOverride]` `ViewCabFOV`\
  Global view scale (same as XYZ scale).\
  Values: 25 to 90 · Default: 25

- **Layback** · `[TableOverride]` `ViewCabLayback`\
  Fake visual stretch of the table to give more depth.\
  Values: 0 to 90 · Default: 0

- **Table X Scale** · `[TableOverride]` `ViewCabScaleX`\
  Stretch the scene along the playfield width axis.\
  Values: 0.5 to 1.5 · Default: 1

- **Table Y Scale** · `[TableOverride]` `ViewCabScaleY`\
  Stretch the scene along the playfield height axis.\
  Values: 0.5 to 1.5 · Default: 1

- **Table Z Scale** · `[TableOverride]` `ViewCabScaleZ`\
  Stretch the scene along the vertical axis (perpendicular to playfield).\
  Values: 0.5 to 1.5 · Default: 1

- **Camera X** · `[TableOverride]` `ViewCabPlayerX`\
  View point width offset.\
  Values: a number

- **Camera Y** · `[TableOverride]` `ViewCabPlayerY`\
  View point height offset.\
  Values: a number

- **Camera Z** · `[TableOverride]` `ViewCabPlayerZ`\
  View point vertical offset.\
  Values: a number

- **Horizontal Offset** · `[TableOverride]` `ViewCabHOfs`\
  Horizontal offset of the virtual table behind the screen 'window'.\
  Values: -30 to 30 · Default: 0

- **Vertical Offset** · `[TableOverride]` `ViewCabVOfs`\
  Vertical offset of the virtual table behind the screen 'window'.\
  Values: -20 to 50 · Default: 0

- **Window Top Z Ofs.** · `[TableOverride]` `ViewCabWindowTop`\
  Distance between the 'window' (i.e. the screen) at the top of the playfield.\
  Values: a number

- **Window Bottom Z Ofs.** · `[TableOverride]` `ViewCabWindowBot`\
  Distance between the 'window' (i.e. the screen) at the bottom of the playfield.\
  Values: a number

- **Viewport Rotation** · `[TableOverride]` `ViewCabRotation`\
  Values: 0 to 360, in steps of 90 · Default: 0

- **Difficulty** · `[TableOverride]` `Difficulty`\
  Overall difficulty (affects slope, flipper size, ball trajectories scattering,...).\
  Values: 0 to 1 · Default: 1

- **Camera Exposure** · `[TableOverride]` `Exposure`\
  Overall brightness scale for the rendered scene/table.\
  Values: 0 to 2 · Default: 1

- **Tonemapper** · `[TableOverride]` `ToneMapper`\
  Colors too bright to be displayed by low dynamic range monitors need to be mapped into a normalized range. Different mappers come at different tradeoffs, depending on each tables setup/lighting.\
  Values: 0 Reinhard · 1 AgX · 2 Filmic · 3 Neutral · 4 AgX Punchy · Default: 0 (Reinhard)

## Machines, packs, displays and toys

ROM machines, PinUP packs, backglasses, color files, real DMD panels, the DMD upscaler and cabinet toys.

### AltSound

- **AltSound Folder** · `[Plugin.AltSound]` `Folder`\
  Values: text · Default: empty

### B2S

- **ScoreView DMD Overlay** · `[Plugin.B2S]` `ScoreViewDMDOverlay`\
  Enable a DMD overlay on the Score View.\
  Values: 1 on, 0 off · Default: 0 (off)

- **ScoreView DMD Automatic position** · `[Plugin.B2S]` `ScoreViewDMDAutoPos`\
  Enable automatic DMD bounds detection.\
  Values: 1 on, 0 off · Default: 0 (off)

- **ScoreView DMD X position** · `[Plugin.B2S]` `ScoreViewDMDX`\
  DMD overlay X position.\
  Values: 0 to 65535 · Default: 0

- **ScoreView DMD Y position** · `[Plugin.B2S]` `ScoreViewDMDY`\
  DMD overlay Y position.\
  Values: 0 to 65535 · Default: 0

- **ScoreView DMD width** · `[Plugin.B2S]` `ScoreViewDMDW`\
  DMD overlay width.\
  Values: 0 to 65535 · Default: 0

- **ScoreView DMD height** · `[Plugin.B2S]` `ScoreViewDMDH`\
  DMD overlay height.\
  Values: 0 to 65535 · Default: 0

- **Backglass DMD Overlay** · `[Plugin.B2S]` `BackglassDMDOverlay`\
  Enable a DMD overlay on the Backglass.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Backglass DMD Automatic position** · `[Plugin.B2S]` `BackglassDMDAutoPos`\
  Enable automatic DMD bounds detection.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Backglass DMD X position** · `[Plugin.B2S]` `BackglassDMDX`\
  DMD overlay X position.\
  Values: 0 to 65535 · Default: 0

- **Backglass DMD Y position** · `[Plugin.B2S]` `BackglassDMDY`\
  DMD overlay Y position.\
  Values: 0 to 65535 · Default: 0

- **Backglass DMD width** · `[Plugin.B2S]` `BackglassDMDW`\
  DMD overlay width.\
  Values: 0 to 65535 · Default: 0

- **Backglass DMD height** · `[Plugin.B2S]` `BackglassDMDH`\
  DMD overlay height.\
  Values: 0 to 65535 · Default: 0

- **Show Grill** · `[Plugin.B2S]` `ShowGrill`\
  Values: 1 on, 0 off · Default: 0 (off)

### B2SLegacy

- **B2SHideGrill** · `[Plugin.B2SLegacy]` `B2SHideGrill`\
  Values: 1 on, 0 off · Default: 0 (off)

- **B2SHideB2SDMD** · `[Plugin.B2SLegacy]` `B2SHideB2SDMD`\
  Values: 1 on, 0 off · Default: 0 (off)

- **ScoreView DMD Overlay** · `[Plugin.B2SLegacy]` `ScoreViewDMDOverlay`\
  Enable a DMD overlay on the Score View.\
  Values: 1 on, 0 off · Default: 0 (off)

- **ScoreView DMD Automatic position** · `[Plugin.B2SLegacy]` `ScoreViewDMDAutoPos`\
  Enable automatic DMD bounds detection.\
  Values: 1 on, 0 off · Default: 0 (off)

- **ScoreView DMD X position** · `[Plugin.B2SLegacy]` `ScoreViewDMDX`\
  DMD overlay X position.\
  Values: 0 to 65535 · Default: 0

- **ScoreView DMD Y position** · `[Plugin.B2SLegacy]` `ScoreViewDMDY`\
  DMD overlay Y position.\
  Values: 0 to 65535 · Default: 0

- **ScoreView DMD width** · `[Plugin.B2SLegacy]` `ScoreViewDMDW`\
  DMD overlay width.\
  Values: 0 to 65535 · Default: 0

- **ScoreView DMD height** · `[Plugin.B2SLegacy]` `ScoreViewDMDH`\
  DMD overlay height.\
  Values: 0 to 65535 · Default: 0

- **Backglass DMD Overlay** · `[Plugin.B2SLegacy]` `BackglassDMDOverlay`\
  Enable a DMD overlay on the Backglass.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Backglass DMD Automatic position** · `[Plugin.B2SLegacy]` `BackglassDMDAutoPos`\
  Enable automatic DMD bounds detection.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Backglass DMD X position** · `[Plugin.B2SLegacy]` `BackglassDMDX`\
  DMD overlay X position.\
  Values: 0 to 65535 · Default: 0

- **Backglass DMD Y position** · `[Plugin.B2SLegacy]` `BackglassDMDY`\
  DMD overlay Y position.\
  Values: 0 to 65535 · Default: 0

- **Backglass DMD width** · `[Plugin.B2SLegacy]` `BackglassDMDW`\
  DMD overlay width.\
  Values: 0 to 65535 · Default: 0

- **Backglass DMD height** · `[Plugin.B2SLegacy]` `BackglassDMDH`\
  DMD overlay height.\
  Values: 0 to 65535 · Default: 0

### DMDUtil

- **ZeDMD** · `[Plugin.DMDUtil]` `ZeDMD`\
  Values: 1 on, 0 off · Default: 0 (off)

- **ZeDMDDevice** · `[Plugin.DMDUtil]` `ZeDMDDevice`\
  Values: text · Default: empty

- **ZeDMDDebug** · `[Plugin.DMDUtil]` `ZeDMDDebug`\
  Values: 1 on, 0 off · Default: 0 (off)

- **ZeDMDBrightness** · `[Plugin.DMDUtil]` `ZeDMDBrightness`\
  Values: -1 to 1000 · Default: -1

- **ZeDMDWiFiEnabled** · `[Plugin.DMDUtil]` `ZeDMDWiFiEnabled`\
  Values: 1 on, 0 off · Default: 0 (off)

- **ZeDMDWiFiAddr** · `[Plugin.DMDUtil]` `ZeDMDWiFiAddr`\
  Values: text · Default: `zedmd-wifi.local`

- **ZeDMDSPIEnabled** · `[Plugin.DMDUtil]` `ZeDMDSPIEnabled`\
  Values: 1 on, 0 off · Default: 0 (off)

- **ZeDMDSPISpeed** · `[Plugin.DMDUtil]` `ZeDMDSPISpeed`\
  Values: 0 to 100000000 · Default: 72000000

- **ZeDMDSPIFramePause** · `[Plugin.DMDUtil]` `ZeDMDSPIFramePause`\
  Values: 0 to 1000 · Default: 2

- **ZeDMDSPIWidth** · `[Plugin.DMDUtil]` `ZeDMDSPIWidth`\
  Values: 0 to 1000 · Default: 128

- **ZeDMDSPIHeight** · `[Plugin.DMDUtil]` `ZeDMDSPIHeight`\
  Values: 0 to 1000 · Default: 32

- **Pixelcade** · `[Plugin.DMDUtil]` `Pixelcade`\
  Values: 1 on, 0 off · Default: 0 (off)

- **PixelcadeDevice** · `[Plugin.DMDUtil]` `PixelcadeDevice`\
  Values: text · Default: empty

- **PIN2DMD** · `[Plugin.DMDUtil]` `PIN2DMD`\
  Values: 1 on, 0 off · Default: 0 (off)

- **DMDServer** · `[Plugin.DMDUtil]` `DMDServer`\
  Values: 1 on, 0 off · Default: 0 (off)

- **DMDServerAddr** · `[Plugin.DMDUtil]` `DMDServerAddr`\
  Values: text · Default: `localhost`

- **DMDServerPort** · `[Plugin.DMDUtil]` `DMDServerPort`\
  Values: 0 to 65535 · Default: 6789

- **FindDisplays** · `[Plugin.DMDUtil]` `FindDisplays`\
  Values: 1 on, 0 off · Default: 1 (on)

- **DumpDMDTxt** · `[Plugin.DMDUtil]` `DumpDMDTxt`\
  Values: 1 on, 0 off · Default: 0 (off)

- **DumpDMDRaw** · `[Plugin.DMDUtil]` `DumpDMDRaw`\
  Values: 1 on, 0 off · Default: 0 (off)

- **LumTintR** · `[Plugin.DMDUtil]` `LumTintR`\
  Values: 0 to 255

- **LumTintG** · `[Plugin.DMDUtil]` `LumTintG`\
  Values: 0 to 255

- **LumTintB** · `[Plugin.DMDUtil]` `LumTintB`\
  Values: 0 to 255

### PinMAME

- **Enable Sound** · `[Plugin.PinMAME]` `Sound`\
  Enable sound emulation.\
  Values: 1 on, 0 off · Default: 1 (on)

- **PinMAME Path** · `[Plugin.PinMAME]` `PinMAMEPath`\
  Folder that contains PinMAME subfolders (roms, nvram, ...).\
  Values: text · Default: empty

- **Cheat Mode** · `[Plugin.PinMAME]` `Cheat`\
  Values: 1 on, 0 off · Default: 0 (off)

### PUP

- **Main Volume** · `[Plugin.PUP]` `MainVol`\
  Overall volume.\
  Values: 0 to 1, in steps of 0.01 · Default: 1

- **Backglass Left Pad** · `[Plugin.PUP]` `BGPadLeft`\
  Left Padding of backglass.\
  Values: 0 to 4096 · Default: 0

- **Backglass Right Pad** · `[Plugin.PUP]` `BGPadRight`\
  Right Padding of backglass.\
  Values: 0 to 4096 · Default: 0

- **Backglass Top Pad** · `[Plugin.PUP]` `BGPadTop`\
  Top Padding of backglass.\
  Values: 0 to 4096 · Default: 0

- **Backglass Bottom Pad** · `[Plugin.PUP]` `BGPadBottom`\
  Bottom Padding of backglass.\
  Values: 0 to 4096 · Default: 0

- **Score View Left Pad** · `[Plugin.PUP]` `SVPadLeft`\
  Left Padding of Score View.\
  Values: 0 to 4096 · Default: 0

- **Score View Right Pad** · `[Plugin.PUP]` `SVPadRight`\
  Right Padding of Score View.\
  Values: 0 to 4096 · Default: 0

- **Score View Top Pad** · `[Plugin.PUP]` `SVPadTop`\
  Top Padding of Score View.\
  Values: 0 to 4096 · Default: 0

- **Score View Bottom Pad** · `[Plugin.PUP]` `SVPadBottom`\
  Bottom Padding of Score View.\
  Values: 0 to 4096 · Default: 0

- **Topper Left Pad** · `[Plugin.PUP]` `TopperPadLeft`\
  Left Padding of topper.\
  Values: 0 to 4096 · Default: 0

- **Topper Right Pad** · `[Plugin.PUP]` `TopperPadRight`\
  Right Padding of topper.\
  Values: 0 to 4096 · Default: 0

- **Topper Top Pad** · `[Plugin.PUP]` `TopperPadTop`\
  Top Padding of topper.\
  Values: 0 to 4096 · Default: 0

- **Topper Bottom Pad** · `[Plugin.PUP]` `TopperPadBottom`\
  Bottom Padding of topper.\
  Values: 0 to 4096 · Default: 0

- **PinUp Player Folder** · `[Plugin.PUP]` `PUPFolder`\
  Values: text · Default: empty

### ScoreView

- **Layout Folder** · `[Plugin.ScoreView]` `LayoutFolder`\
  Folder where custom ScoreView layouts are stored.\
  Values: text · Default: empty

### Serum

- **Serum Path** · `[Plugin.Serum]` `SerumPath`\
  Folder that cotains Serum colorization files (cROMc, cRZ).\
  Values: text · Default: empty

### UpscaleDMD

- **Mode** · `[Plugin.UpscaleDMD]` `UpscaleMode`\
  Select upscaler.\
  Values: 0 Disabled · 1 ScaleFX AA 1x · 2 ScaleFX 3x · 3 MMPX 2x · 4 xBRZ 2x · 5 xBRZ 3x · 6 xBRZ 4x · 7 xBRZ 5x · 8 xBRZ 6x · 9 Super-XBR 2x · Default: 0 (Disabled)

### VNI

- **VNI Path** · `[Plugin.VNI]` `VniPath`\
  Folder that contains VNI colorization files (PAL, VNI).\
  Values: text · Default: empty

## Every ROM machine

PinMAME's settings for every machine, in `Machine settings.ini`'s `[default]` section; a game's own section there counts over them for that game.

- **Its sound** · `[default]` `sound` in `Machine settings.ini`\
  Values: 1 on, 0 off · Default: 1 (on)

- **Its volume (dB)** · `[default]` `volume` in `Machine settings.ini`\
  Values: -32 to 32 · Default: 0

- **Its sound comes from** · `[default]` `sound_mode` in `Machine settings.ini`\
  Values: 0 PinMAME's own · 1 Its AltSound pack · 2 PinSound · 3 PinSound with recordings

- **Older machines' recorded sounds** · `[default]` `samples` in `Machine settings.ini`\
  Values: 1 on, 0 off · Default: 0 (off)

- **Cheat mode (skips some games' start-up tests)** · `[default]` `cheat` in `Machine settings.ini`\
  Values: 1 on, 0 off · Default: 0 (off)

- **A real DMD panel for it** · `[default]` `showpindmd` in `Machine settings.ini`\
  Values: 1 on, 0 off · Default: 0 (off)

- **Its score window** · `[default]` `showwindmd` in `Machine settings.ini`\
  Values: 1 on, 0 off · Default: 1 (on)

- **DMD color: red** · `[default]` `dmd_red` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 255

- **DMD color: green** · `[default]` `dmd_green` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 88

- **DMD color: blue** · `[default]` `dmd_blue` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 32

- **DMD off brightness (%)** · `[default]` `dmd_perc0` in `Machine settings.ini`\
  Values: 0 to 100 · Default: 20

- **DMD low brightness (%)** · `[default]` `dmd_perc33` in `Machine settings.ini`\
  Values: 0 to 100 · Default: 33

- **DMD medium brightness (%)** · `[default]` `dmd_perc66` in `Machine settings.ini`\
  Values: 0 to 100 · Default: 67

- **DMD shades as colors of their own** · `[default]` `dmd_colorize` in `Machine settings.ini`\
  Values: 1 on, 0 off · Default: 0 (off)

- **Colored DMD: red at 66%** · `[default]` `dmd_red66` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 225

- **Colored DMD: green at 66%** · `[default]` `dmd_green66` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 15

- **Colored DMD: blue at 66%** · `[default]` `dmd_blue66` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 193

- **Colored DMD: red at 33%** · `[default]` `dmd_red33` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 6

- **Colored DMD: green at 33%** · `[default]` `dmd_green33` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 0

- **Colored DMD: blue at 33%** · `[default]` `dmd_blue33` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 214

- **Colored DMD: red at 0%** · `[default]` `dmd_red0` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 0

- **Colored DMD: green at 0%** · `[default]` `dmd_green0` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 0

- **Colored DMD: blue at 0%** · `[default]` `dmd_blue0` in `Machine settings.ini`\
  Values: 0 to 255 · Default: 0

## Visual Pinball Infinite's own

In the `[VPI]` section.

- **A sound for each achievement** · `[VPI]` `AchievementSound`\
  Whether a sound plays as each achievement is announced: the pack's own, or the player's chime (Settings › Sound).\
  Values: 1 on, 0 off · Default: 1 (on)

- **The achievements' sound level** · `[VPI]` `AchievementGain`\
  The level of the sound played as an achievement is announced, over the backglass volume, from 0 to 2: one of the pause menu's sound levels (Settings › Sound too).\
  Values: 0 to 2 · Default: 1

- **The table's music level** · `[VPI]` `MusicGain`\
  The level of a table's own music, what its script plays with PlayMusic, over the backglass volume, from 0 to 2: one of the pause menu's sound levels, beside the machine's, AltSound's, PinUP's and Media Player's ([Player] AudioSource.….Gain).\
  Values: 0 to 2 · Default: 1

- **How frames reach the screen** · `[VPI]` `Present`\
  How frames reach the screen: as soon as each is drawn (immediate, which can tear), tear-free without waiting (mailbox), in the display's queue (vsync), or each drawn just before the refresh that shows it (paced); empty, as [Player] SyncMode says.\
  Values: empty: As [Player] SyncMode says · `immediate` As soon as drawn · `mailbox` Tear-free, without waiting (mailbox) · `vsync` The display's queue (vsync) · `paced` Drawn just before the refresh that shows them (paced)

- **The score display floating over the table** · `[VPI]` `FloatingScore`\
  On one screen, a ROM table's score display floats over the table while its script leaves PinMAME's window shown.\
  Values: 1 on, 0 off · Default: 1 (on)

- **The other windows shown from the start** · `[VPI]` `ExtraWindowsAtStart`\
  The backglass, score and topper windows show as the table starts; off, each shows with its first picture.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Tilt by counting nudges** · `[VPI]` `TiltCountsNudges`\
  A tilt comes from counting presses of the nudge keys, in place of the tilt bob's swing.\
  Values: 1 on, 0 off · Default: 0 (off)

- **HDR on screens with HDR on** · `[VPI]` `HDROutput`\
  An HDR picture (HDR10) on a screen with Windows' HDR on; off, ordinary colors there too.\
  Values: 1 on, 0 off · Default: 1 (on)

- **The score window in the HDR look** · `[VPI]` `HDRScoreView`\
  On an HDR screen, the score window in the HDR look; off, in its true colors.\
  Values: 1 on, 0 off · Default: 0 (off)

- **A display device for a real DMD** · `[VPI]` `DmdDevice`\
  A display device for a real DMD panel: the path of its DmdDevice64.dll (DMD Extensions', or a panel maker's), or 1 for the one in VPinMAME's folder; empty, none.\
  Values: text · Default: empty

- **The display device's settings file** · `[VPI]` `DmdDeviceConfig`\
  The display device's own settings file (its DmdDevice.ini); empty, the one beside it.\
  Values: text · Default: empty

- **Esc opens the pause menu first** · `[VPI]` `PauseMenuBeforeQuit`\
  Esc (or Q) opens the pause menu with Quit the table picked; off, Esc ends play at once. A front end that starts a table with -minimized always gets the at-once end.\
  Values: 1 on, 0 off · Default: 1 (on)

- **The screens layout tables play with** · `[VPI]` `ScreenPreset`\
  One of the Screens page's saved layouts, by its name; a table's own settings can name another.\
  Values: text · Default: empty

- **A screenshot at a high score** · `[VPI]` `ScreenshotAtHighScore`\
  A screenshot when a game makes the high scores — a ROM machine's in its memory, or a table script's own when it saves a new one — into the table's screenshots.\
  Values: 1 on, 0 off · Default: 0 (off)

- **The windows a screenshot takes** · `[VPI]` `ScreenshotScreens`\
  The windows a screenshot takes: playfield, backglass, dmd and topper, separated by commas; empty, every window that shows.\
  Values: text · Default: empty

- **Tables may reach outside their files** · `[VPI]` `ScriptsReachOutside`\
  Tables' scripts may reach outside their own files — the internet, other programs, every file — as some tables' online features need; off, each script keeps to its table's own files. A table's own line in a [ScriptsReachOutside] section, its file's name = 1 or 0, counts over this for that table.\
  Values: 1 on, 0 off · Default: 0 (off)

- **The library's tables folder** · `[VPI]` `TablesFolder`\
  The folder holding the library's tables, which first setup sets; the app and the player read the library from it.\
  Values: text · Default: empty

- **The library's scripts folder** · `[VPI]` `ScriptsFolder`\
  The folder of the scripts tables load (core.vbs, controller.vbs and the machines' own); empty, the Scripts folder beside the tables folder.\
  Values: text · Default: empty

- **The library's PinUP packs folder** · `[VPI]` `PinUPFolder`\
  The folder holding the library's PinUP packs (PinUP's PUPVideos); empty, the one found above the tables.\
  Values: text · Default: empty

- **How the library keeps its tables** · `[VPI]` `LibraryLayout`\
  game: each game in a folder of its own, its versions sharing it, as Visual Pinball X 10.8.1 keeps tables; empty, VPX's shared folders. The app's Library settings switch between them, moving the files.\
  Values: text · Default: empty

- **PinballX's folder** · `[VPI]` `PinballXFolder`\
  Where PinballX is installed, for putting a table's media where it shows them; empty, its usual folders are looked in.\
  Values: text · Default: empty

- **PinballY's folder** · `[VPI]` `PinballYFolder`\
  Where PinballY is installed, for putting a table's media where it shows them; empty, its usual folders are looked in.\
  Values: text · Default: empty

## The player, other

The player's other settings.

### Other

- **RumbleMode** · `[Player]` `RumbleMode`\
  Use rumble motor(s) in attached input devices.\
  Values: 0 Off · 1 Table only (N/A yet) · 2 Generic only (N/A yet) · 3 Table with generic fallback · Default: 3 (Table with generic fallback)

### Older tables' sound and toy hints (their controller.vbs reads these)

- **DOF Contactors** · `[Controller]` `DOFContactors`\
  Hints the table script on how to handle contactor events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **DOF Knocker** · `[Controller]` `DOFKnocker`\
  Hints the table script on how to handle knocker events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **DOF Chimes** · `[Controller]` `DOFChimes`\
  Hints the table script on how to handle chime events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **DOF Bell** · `[Controller]` `DOFBell`\
  Hints the table script on how to handle bell events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **DOF Gear** · `[Controller]` `DOFGear`\
  Hints the table script on how to handle gear events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **DOF Shaker** · `[Controller]` `DOFShaker`\
  Hints the table script on how to handle shaker events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **DOF Flippers** · `[Controller]` `DOFFlippers`\
  Hints the table script on how to handle flipper events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **DOF Targets** · `[Controller]` `DOFTargets`\
  Hints the table script on how to handle target events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **DOF Drop Targets** · `[Controller]` `DOFDropTargets`\
  Hints the table script on how to handle drop target events, between sounds and direct output framework.\
  Values: 0 Sound FX · 1 DOF · 2 Both · Default: 2 (Both)

- **Disable B2S** · `[Controller]` `ForceDisableB2S`\
  Legacy 'Disable B2S' hints script to avoid using B2S controller. Only works with tables using 'controller.vbs' and not using plugins for backglass rendering.\
  Values: 1 on, 0 off · Default: 0 (off)

### What's switched on

- **Enable** · `[Plugin.AltSound]` `Enable`\
  Enable legacy AltSound plugin.\
  Values: 1 on, 0 off

- **Enable** · `[Plugin.DMDUtil]` `Enable`\
  Enable DMDUtil plugin.\
  Values: 1 on, 0 off · Default: 0 (off)

- **Enable** · `[Plugin.PUP]` `Enable`\
  Enable PinUp player plugin.\
  Values: 1 on, 0 off

- **Enable** · `[Plugin.ScoreView]` `Enable`\
  Enable ScoreView player plugin.\
  Values: 1 on, 0 off

- **Enable** · `[Plugin.Serum]` `Enable`\
  Enable Serum plugin.\
  Values: 1 on, 0 off

- **Enable** · `[Plugin.VNI]` `Enable`\
  Enable VNI plugin.\
  Values: 1 on, 0 off
