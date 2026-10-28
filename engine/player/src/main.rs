//! `vpi-player`: plays a table.
//!
//! ```text
//! vpi-player <table.vpx> [--vsync | --present immediate|mailbox|vsync|paced] [--max-fps 240] [--view desktop|cabinet|fss] [--size 1600x900] [--fullscreen]
//!            [--look classic|vpx] [--msaa off|4|8] [--aa-factor 0.5-2] [--ball-move vpx|intended] [--ao off|static|dynamic] [--motion-blur on|off] [--hdr on|off] [--hdr-dmd on|off] [--post-aa off|fxaa-fast|fxaa|fxaa-quality|nfaa|dlaa|smaa|faaa] [--sharpen off|cas|bilateral-cas] [--still-parts once|every-frame] [--shapes own|export] [--no-script] [--dmd-window X,Y,W,H] [--backglass-window X,Y,W,H] [--topper-window X,Y,W,H]
//!            [--score-panel auto|no-dmd|off|X,Y,W,H] [--backglass-dmd off|auto|X,Y,W,H] [--show-grill] [--prefer-pup] [--volume 0-100]
//!            [--shot picture.png [--at 1000] [--keys 500:1:d,600:1:u,...] [--video preview.mp4 [--seconds 10] [--video-sound]] [--crop playfield]] [--quit-after ms]
//! ```
//!
//! Keys (VPX's defaults): Shift flips, Enter pulls and releases the plunger,
//! 1 or B serves a ball, Esc or Q quits.
//!
//! The table's script runs as in VPX; `--no-script` leaves it out, and then
//! keys drive the table directly.
//!
//! Frames come at most `--max-fps` times a second (240 unless set; 0 for no
//! limit), whichever way they reach the screen, except that a key press gets
//! its frame at once.
//!
//! `--dmd-window` gives the score display a borderless window of its own,
//! placed on the desktop in pixels (VPX's score view window),
//! `--backglass-window` the table's backglass file and `--topper-window` a
//! PinUP pack's topper screen; `--displays` lists the screens and where they
//! sit. Like VPX's, those windows show once something first draws in them.
//! `--volume` sets the playfield and backglass volumes, in percent, over the
//! settings' (VPX's `[Player]` `SoundVolume` and `MusicVolume`); the `-` and `=`
//! keys turn them down and up while playing, as VPX 5436's do (`volume`). A table's PinUP pack shows
//! its screens in those windows too: before the backglass file's pictures
//! wherever it draws (the score Harry Potter's pack draws on its own
//! backglass was hidden behind the file, VPX's default order), unless the
//! settings give VPX's renderer priorities (`[Backglass]` or `[ScoreView]`
//! `Priority.PUP` and `Priority.B2S`, the higher first, globally or per
//! table); `--prefer-pup` puts the pack's first whatever they say.
//!
//! Without those options, the windows go where the settings file puts them
//! (`%APPDATA%\Visual Pinball Infinite\Settings.ini`): VPX 5436's
//! window settings under its own names — `[Player]` `PlayfieldDisplay`,
//! `PlayfieldFullScreen`, `PlayfieldWndX`, `PlayfieldWndY`,
//! `PlayfieldWidth`, `PlayfieldHeight` and `BGSet` (the view), and the
//! same in `[Backglass]`, `[ScoreView]` and `[Topper]`, with their `Output`
//! (0 none, 1 a window of its own); `VPI_SETTINGS` names another file. Test
//! mode reads one only when `VPI_SETTINGS` names it (the VPX picture tool's
//! cabinet view), its picture taking `--size` whatever the file says of the
//! playfield's window, and never the table's own (`<table>.ini`, `.pov`).
//!
//! The table is drawn in VPX's look as far as it's ported (the default): a high-range picture,
//! VPX's materials, blends, bloom, ball and reflections, and VPX's
//! tonemapping. `--look classic` draws the first look instead.
//!
//! The parts that never move are drawn once, as VPX draws them (its static
//! prepass): 128 times, each shifted by part of a texel, averaged,
//! then each frame draws only what moves over them; `--still-parts
//! every-frame` draws them every frame instead. Multisampling (`--msaa 4` or
//! `8`, VPX's MSAA levels) smooths the edges of what moves too, the still
//! parts kept as VPX's DirectX and OpenGL builds keep them; it's off unless
//! asked, VPX's default. VPX's
//! post-processed antialiasing (`--post-aa`) and sharpening (`--sharpen`)
//! follow the tonemapping, and its supersampling (`--aa-factor`) draws the
//! table bigger than the window and reads it down: all off unless asked, as
//! in VPX. VPX's ambient occlusion (`--ao`, dynamic for tables that ask for
//! it) and ball motion blur (`--motion-blur`) are on, as in VPX.
//!
//! Test mode: `--shot` plays without showing a window and saves a picture of
//! the table `--at` milliseconds after play starts, then quits; the game's
//! clock stops at that moment, so the picture shows it on every run. `--keys`
//! presses (`d`) and releases (`u`) keys at set times; a key is a DirectInput
//! code in hex (`2A` is left Shift) or one of `lshift`, `rshift`, `enter`,
//! `1`, `b`. `--quit-after` ends play by itself after that many milliseconds
//! and prints the frame rate and key-to-frame times it measured.

use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use sdl3_sys::everything::*;

use player::camera::{self, TableExtent, View, ViewSetup};
use player::dmd_overlay::Overlay;
use player::game::{self, GameSetup, Shared};
use player::input::{self, InputSink};
use player::math::{Mat4, Vec3};
use player::render::{AmbientOcclusion, EmbedKind, FrameInput, FrameResult, LookMode, PostAa, Presentation, Renderer, Sharpen, Text, sdl_error};
use player::scene::Scene;
use player::table_meshes;
use player::waker::Waker;

struct Options {
    table: PathBuf,
    /// How frames reach the screen: `--vsync` asks for tear-free.
    present: Presentation,
    view: View,
    size: (i32, i32),
    fullscreen: bool,
    shot: Option<PathBuf>,
    shot_at_ms: u64,
    /// Test mode: a video from the shot's moment on, `video_seconds` long (a table's preview).
    video: Option<PathBuf>,
    video_seconds: u64,
    /// Test mode: the picture and the video cut to the playfield, as VPS's pictures show tables
    /// (`--crop playfield`).
    crop_playfield: bool,
    /// Test mode: the video carries the table's sound, recorded as it plays with no output
    /// (`--video-sound`: a table's preview, for its media).
    video_sound: bool,
    /// Scripted key presses: (ms after start, DirectInput code, down).
    keys: Vec<(u64, u8, bool)>,
    /// Quit by itself after this long, printing what it measured.
    quit_after_ms: Option<u64>,
    /// Run the table's script.
    script: bool,
    /// Write a replay of the game here.
    record: Option<PathBuf>,
    /// Show the on-screen readout (frame rate, latency, physics).
    readout: bool,
    /// One screen (`--one-screen`): no backglass, score or topper window from the settings file,
    /// only those the options give; a ROM game's score display floats over the table.
    one_screen: bool,
    /// The score display's own window: left, top, width and height, in pixels.
    dmd_window: Option<[i32; 4]>,
    /// The backglass's own window, likewise.
    backglass_window: Option<[i32; 4]>,
    /// The topper's own window, likewise (a PinUP pack's topper screen shows there).
    topper_window: Option<[i32; 4]>,
    /// The playfield's window was given by the options (`--size`, `--fullscreen`), and the
    /// view (`--view`): the settings file's are left aside.
    playfield_given: bool,
    view_given: bool,
    /// The settings file; in test mode only the one `VPI_SETTINGS` names, if it names
    /// one, so its pictures never depend on anyone's settings unless asked.
    settings: player::settings::Settings,
    /// The backglass file's score panel in the score window, and where the
    /// DMD shows in it (None: the machine's displays alone).
    score_panel: Option<Overlay>,
    /// Where the DMD shows over the backglass.
    backglass_dmd: Overlay,
    /// The backglass picture's speaker grill shows (VPX's `ShowGrill`), else it's cut off.
    show_grill: bool,
    /// A PinUP pack's screens before the backglass file's pictures, whatever the settings'
    /// renderer priorities say.
    prefer_pup: bool,
    /// List the screens and where they sit, then quit.
    displays: bool,
    /// List the sound outputs, then quit.
    sound_outputs: bool,
    /// The app's Controls page's live test: the controllers and actions, as they come (`input::input_test`).
    input_test: bool,
    /// Those lists for a program to read (the app's settings pages): JSON.
    json: bool,
    /// The playfield and backglass volumes both, 0 to 1, over the settings' (VPX's
    /// `[Player]` `SoundVolume` and `MusicVolume`).
    volume: Option<f32>,
    /// At most this many frames a second (0: no limit), so the graphics card
    /// isn't drawing flat out; a key press still gets its frame at once.
    max_fps: f64,
    /// How the table is drawn: the first look, or VPX's as far as it's ported.
    look: LookMode,
    /// Samples per pixel for multisampling: 1 (none, VPX's default), 4 or 8.
    samples: u32,
    /// VPX's supersampling (`AAFactor`, 0.5 to 2; 1, VPX's default: none).
    aa_factor: f32,
    /// The balls' last-moment move: as VPX 5436 makes it, or as it means to.
    ball_move: BallMove,
    /// VPX's ambient occlusion (dynamic, its default, for tables that ask for it) and ball motion blur (on).
    ao: AmbientOcclusion,
    motion_blur: bool,
    /// VPX 5436's HDR10 output on a screen with HDR on (`--hdr`; None: as the settings say, on by default).
    hdr: Option<bool>,
    /// The score window on a screen with HDR on as VPX 5436 shows it there (`--hdr-dmd`; None: as
    /// the settings say, off by default).
    hdr_dmd: Option<bool>,
    /// VPX's post-processed antialiasing and sharpening (off, VPX's defaults).
    post_aa: PostAa,
    sharpen: Sharpen,
    /// The still parts drawn once (VPX's static prepass) or every frame.
    still_once: bool,
    /// Our own shapes for every part type, in place of the export's (`--shapes export` keeps
    /// the export's, the backup).
    own_shapes: bool,
    /// The options the command line gave, which count over the settings file's.
    given: std::collections::HashSet<String>,
    /// VPX's `MaxFramerate` from the settings (-1: the screen's refresh rate), worked out against
    /// the screen once the window is up; None: `max_fps` as it stands.
    frame_limit_setting: Option<f32>,
    /// VPX's `ForceBloomOff`.
    bloom_off: bool,
    /// Started as a front end starts VPX (`-Play <table>`): the table's window titled as VPX's,
    /// "Visual Pinball Player", which front ends' helpers look for (PinUP Popper's VPXSTARTER and
    /// PopperKeepFocus wait for it; its closer finds "Visual Pinball").
    front_end: bool,
    /// VPX's `-minimized` (PinUP Popper starts tables with it): as in VPX, Esc then ends play at
    /// once, no pause menu first (VPX's `m_disablePauseMenu`; `-ExtMinimized` keeps the menu).
    minimized: bool,
}

const USAGE: &str = "usage: vpi-player <table.vpx> [--vsync | --present immediate|mailbox|vsync|paced] [--max-fps 240] [--view desktop|cabinet|fss] [--size 1600x900] [--fullscreen] [--look classic|vpx] [--msaa off|4|8] [--aa-factor 0.5-2] [--ball-move vpx|intended] [--ao off|static|dynamic] [--motion-blur on|off] [--hdr on|off] [--hdr-dmd on|off] [--post-aa off|fxaa-fast|fxaa|fxaa-quality|nfaa|dlaa|smaa|faaa] [--sharpen off|cas|bilateral-cas] [--still-parts once|every-frame] [--shapes own|export] [--no-script] [--no-readout] [--one-screen] [--dmd-window X,Y,W,H] [--backglass-window X,Y,W,H] [--topper-window X,Y,W,H] [--score-panel auto|no-dmd|off|X,Y,W,H] [--backglass-dmd off|auto|X,Y,W,H] [--show-grill] [--prefer-pup] [--volume 0-100] [--shot picture.png [--at 1000] [--keys 500:1:d,600:1:u] [--video preview.mp4 [--seconds 10] [--video-sound]] [--crop playfield]] [--quit-after ms] [--record game.vpireplay]\n       vpi-player --displays [--json]\n       vpi-player --sound-outputs [--json]\n       vpi-player --input-test";

/// The frame-rate limit unless `--max-fps` says otherwise: above the 60–165 Hz of cabinets' and
/// desktops' screens.
const DEFAULT_MAX_FPS: f64 = 240.0;

/// The pause menu's mouse: the row it's over, and the bar it holds while a
/// level or an option is dragged; its news handed to the game thread (`pause_menu::point`).
#[derive(Default)]
struct MenuMouse {
    hover: Option<usize>,
    dragging: Option<usize>,
}

impl MenuMouse {
    /// An event of the table's window's: pointing picks a row, a click chooses it (on a bar, sets
    /// it there, held to drag it), the wheel scrolls, the right or back button steps back. `turn`:
    /// the text's quarter turns and the window's size in pixels the layout was made for.
    ///
    /// # Safety
    /// On SDL's thread, with the event SDL filled in.
    unsafe fn event(&mut self, event: &SDL_Event, layout: &player::pause_menu::Layout, window: *mut SDL_Window, turn: (u32, (u32, u32))) {
        use player::pause_menu::{Pointer, Spot, point};
        // SAFETY: as the caller promises
        unsafe {
            let kind = event.r#type;
            let ours = |id: SDL_WindowID| id == SDL_GetWindowID(window);
            // how many rows the picture aimed at has
            let rows = layout.rows.len();
            // the point as the menu was laid out: in pixels, turned back with the text
            let at = |x: f32, y: f32| {
                let density = SDL_GetWindowPixelDensity(window).max(0.01);
                let (px, py) = (x * density, y * density);
                let (w, h) = (turn.1.0 as f32, turn.1.1 as f32);
                match turn.0 % 4 {
                    1 => (py, w - px),
                    2 => (w - px, h - py),
                    3 => (h - py, px),
                    _ => (px, py),
                }
            };
            if kind == SDL_EVENT_MOUSE_MOTION.0 && ours(event.motion.windowID) {
                let (x, y) = at(event.motion.x, event.motion.y);
                if let Some(row) = self.dragging {
                    if let Some(along) = layout.along(row, x) {
                        point(Pointer::Drag(row, along, rows));
                    }
                } else {
                    let over = layout.at(x, y).map(|(k, _)| k);
                    if over != self.hover {
                        self.hover = over;
                        if let Some(k) = over {
                            point(Pointer::Over(k, rows));
                        }
                    }
                }
            } else if kind == SDL_EVENT_MOUSE_BUTTON_DOWN.0 && ours(event.button.windowID) {
                let (x, y) = at(event.button.x, event.button.y);
                match i32::from(event.button.button) {
                    SDL_BUTTON_LEFT => {
                        if let Some((k, spot)) = layout.at(x, y) {
                            point(Pointer::Click(k, spot, rows));
                            if matches!(spot, Spot::Bar(_)) {
                                self.dragging = Some(k);
                            }
                        }
                    }
                    SDL_BUTTON_RIGHT | SDL_BUTTON_X1 => point(Pointer::Back),
                    _ => {}
                }
            } else if kind == SDL_EVENT_MOUSE_BUTTON_UP.0 && i32::from(event.button.button) == SDL_BUTTON_LEFT {
                self.dragging = None;
            } else if kind == SDL_EVENT_MOUSE_WHEEL.0 && ours(event.wheel.windowID) {
                let notches = event.wheel.integer_y;
                let notches = if event.wheel.direction == SDL_MOUSEWHEEL_FLIPPED { -notches } else { notches };
                if notches != 0 {
                    point(Pointer::Wheel(notches));
                }
            }
        }
    }
}

/// One of VPX's own switches, as VPX reads its command line: `-word` or `/word`, in any case
/// (`-Play`, `/play`); ours start `--`, and a table's path is neither.
fn vpx_switch(arg: &str) -> Option<String> {
    let word = arg.strip_prefix('-').filter(|w| !w.starts_with('-')).or_else(|| arg.strip_prefix('/'))?;
    (!word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric())).then(|| word.to_ascii_lowercase())
}

fn parse_args() -> Result<Options, String> {
    let mut table = None;
    let mut o = Options {
        table: PathBuf::new(),
        present: Presentation::Immediate,
        view: View::Desktop,
        size: (1600, 900),
        fullscreen: false,
        shot: None,
        shot_at_ms: 1000,
        video: None,
        video_seconds: 10,
        crop_playfield: false,
        video_sound: false,
        keys: Vec::new(),
        quit_after_ms: None,
        script: true,
        record: None,
        // VPX's default: its performance readout off until `[Player]` `ShowFPS` turns it on
        readout: false,
        one_screen: false,
        dmd_window: None,
        backglass_window: None,
        topper_window: None,
        playfield_given: false,
        view_given: false,
        settings: player::settings::Settings::default(),
        // the score panel with the DMD placed in it by itself (VPX shows neither unless asked)
        score_panel: Some(Overlay::Auto),
        backglass_dmd: Overlay::Off,
        show_grill: false,
        prefer_pup: false,
        displays: false,
        sound_outputs: false,
        input_test: false,
        json: false,
        volume: None,
        max_fps: DEFAULT_MAX_FPS,
        look: LookMode::Vpx,
        // VPX's default: no multisampling, in play as in test mode
        samples: 1,
        aa_factor: 1.0,
        // as VPX means it (VPX 5436's own, a hundredth of the way, with --ball-move vpx)
        ball_move: BallMove::Intended,
        ao: AmbientOcclusion::Dynamic,
        motion_blur: true,
        hdr: None,
        hdr_dmd: None,
        post_aa: PostAa::Off,
        sharpen: Sharpen::Off,
        still_once: true,
        // our own shapes, VPX's code; the export's as a backup
        own_shapes: true,
        given: Default::default(),
        frame_limit_setting: None,
        bloom_off: false,
        front_end: false,
        minimized: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a.starts_with("--") {
            o.given.insert(a.clone());
        }
        // VPX's own command line, as front ends start VPX (PinUP Popper, PinballX, PinballY):
        // `-Play <table>` (or `/play`), its window and editor switches passed over, a switch's value
        // read and set aside — so a front end needs only the program's name changed
        if let Some(word) = vpx_switch(&a) {
            match word.as_str() {
                "play" => {
                    table = Some(args.next().map(PathBuf::from).ok_or("-Play takes a table")?);
                    o.front_end = true;
                }
                "h" | "help" | "v" => return Err(USAGE.to_string()),
                "minimized" => o.minimized = true,
                "extminimized" => o.minimized = false,
                "primary" | "exit" | "disabletruefullscreen" | "enabletruefullscreen" | "lesscputhreads" | "nocodecheck" => {}
                "ini" | "tableini" | "pov" | "locale" | "displayid" | "c1" | "c2" | "c3" | "c4" | "c5" | "c6" | "c7" | "c8" | "c9" => {
                    args.next();
                }
                _ => return Err(format!("VPX's {a} isn't one the player takes\n{USAGE}")),
            }
            continue;
        }
        match a.as_str() {
            // tear-free: mailbox where the display has it, which doesn't queue frames
            "--vsync" => o.present = Presentation::TearFree,
            "--present" => {
                o.present = match args.next().as_deref() {
                    Some("immediate") => Presentation::Immediate,
                    Some("mailbox") => Presentation::TearFree,
                    Some("vsync") => Presentation::Queued,
                    Some("paced") => Presentation::Paced,
                    _ => return Err(format!("--present takes immediate, mailbox, vsync or paced\n{USAGE}")),
                }
            }
            "--fullscreen" => (o.fullscreen, o.playfield_given) = (true, true),
            "--look" => {
                o.look = match args.next().as_deref() {
                    Some("classic") => LookMode::Classic,
                    Some("vpx") => LookMode::Vpx,
                    _ => return Err(format!("--look takes classic or vpx\n{USAGE}")),
                }
            }
            "--view" => {
                o.view_given = true;
                o.view = match args.next().as_deref() {
                    Some("desktop") => View::Desktop,
                    Some("cabinet") => View::Cabinet,
                    Some("fss") => View::SingleScreen,
                    _ => return Err(format!("--view takes desktop, cabinet or fss\n{USAGE}")),
                }
            }
            "--size" => {
                let s = args.next().unwrap_or_default();
                let (w, h) = s.split_once('x').ok_or_else(|| format!("--size takes WIDTHxHEIGHT\n{USAGE}"))?;
                o.size = (w.parse().map_err(|_| USAGE.to_string())?, h.parse().map_err(|_| USAGE.to_string())?);
                o.playfield_given = true;
            }
            "--shot" => o.shot = Some(PathBuf::from(args.next().ok_or_else(|| USAGE.to_string())?)),
            "--video" => o.video = Some(PathBuf::from(args.next().ok_or_else(|| USAGE.to_string())?)),
            "--seconds" => o.video_seconds = args.next().and_then(|v| v.parse().ok()).filter(|s| (1..=120).contains(s)).ok_or_else(|| format!("--seconds takes 1 to 120\n{USAGE}"))?,
            "--video-sound" => o.video_sound = true,
            "--crop" => match args.next().as_deref() {
                Some("playfield") => o.crop_playfield = true,
                _ => return Err(format!("--crop takes playfield\n{USAGE}")),
            },
            "--at" => o.shot_at_ms = args.next().and_then(|v| v.parse().ok()).ok_or_else(|| format!("--at takes milliseconds
{USAGE}"))?,
            "--keys" => o.keys = args.next().as_deref().and_then(input::parse_key_script).ok_or_else(|| format!("--keys takes ms:key:d|u,...
{USAGE}"))?,
            "--quit-after" => o.quit_after_ms = Some(args.next().and_then(|v| v.parse().ok()).ok_or_else(|| format!("--quit-after takes milliseconds
{USAGE}"))?),
            "--msaa" => {
                o.samples = match args.next().as_deref() {
                    Some("off") => 1,
                    Some("4") => 4,
                    Some("8") => 8,
                    _ => return Err(format!("--msaa takes off, 4 or 8\n{USAGE}")),
                }
            }
            "--ao" => {
                o.ao = match args.next().as_deref() {
                    Some("off") => AmbientOcclusion::Off,
                    Some("static") => AmbientOcclusion::Static,
                    Some("dynamic") => AmbientOcclusion::Dynamic,
                    _ => return Err(format!("--ao takes off, static or dynamic
{USAGE}")),
                }
            }
            "--motion-blur" => {
                o.motion_blur = match args.next().as_deref() {
                    Some("on") => true,
                    Some("off") => false,
                    _ => return Err(format!("--motion-blur takes on or off
{USAGE}")),
                }
            }
            "--hdr" => {
                o.hdr = match args.next().as_deref() {
                    Some("on") => Some(true),
                    Some("off") => Some(false),
                    _ => return Err(format!("--hdr takes on or off\n{USAGE}")),
                }
            }
            "--hdr-dmd" => {
                o.hdr_dmd = match args.next().as_deref() {
                    Some("on") => Some(true),
                    Some("off") => Some(false),
                    _ => return Err(format!("--hdr-dmd takes on or off\n{USAGE}")),
                }
            }
            "--ball-move" => {
                o.ball_move = match args.next().as_deref() {
                    Some("vpx") => BallMove::Vpx,
                    Some("intended") => BallMove::Intended,
                    _ => return Err(format!("--ball-move takes vpx or intended\n{USAGE}")),
                }
            }
            "--aa-factor" => {
                o.aa_factor = args.next().and_then(|v| v.parse().ok()).filter(|v: &f32| (0.5..=2.0).contains(v)).ok_or_else(|| format!("--aa-factor takes 0.5 to 2
{USAGE}"))?;
            }
            "--post-aa" => {
                o.post_aa = match args.next().as_deref() {
                    Some("off") => PostAa::Off,
                    Some("fxaa-fast") => PostAa::FxaaFast,
                    Some("fxaa") => PostAa::Fxaa,
                    Some("fxaa-quality") => PostAa::FxaaQuality,
                    Some("nfaa") => PostAa::Nfaa,
                    Some("dlaa") => PostAa::Dlaa,
                    Some("smaa") => PostAa::Smaa,
                    Some("faaa") => PostAa::Faaa,
                    _ => return Err(format!("--post-aa takes off, fxaa-fast, fxaa, fxaa-quality, nfaa, dlaa, smaa or faaa
{USAGE}")),
                }
            }
            "--sharpen" => {
                o.sharpen = match args.next().as_deref() {
                    Some("off") => Sharpen::Off,
                    Some("cas") => Sharpen::Cas,
                    Some("bilateral-cas") => Sharpen::BilateralCas,
                    _ => return Err(format!("--sharpen takes off, cas or bilateral-cas
{USAGE}")),
                }
            }
            "--shapes" => {
                o.own_shapes = match args.next().as_deref() {
                    Some("own") => true,
                    Some("export") => false,
                    _ => return Err(format!("--shapes takes own or export
{USAGE}")),
                }
            }
            "--still-parts" => {
                o.still_once = match args.next().as_deref() {
                    Some("once") => true,
                    Some("every-frame") => false,
                    _ => return Err(format!("--still-parts takes once or every-frame\n{USAGE}")),
                }
            }
            "--no-script" => o.script = false,
            // pictures that compare equal from run to run (tools/render-check)
            "--no-readout" => o.readout = false,
            "--one-screen" => o.one_screen = true,
            "--record" => o.record = Some(args.next().map(PathBuf::from).ok_or("--record takes a file name")?),
            "--dmd-window" => o.dmd_window = Some(window_place("--dmd-window", args.next())?),
            "--backglass-window" => o.backglass_window = Some(window_place("--backglass-window", args.next())?),
            "--topper-window" => o.topper_window = Some(window_place("--topper-window", args.next())?),
            "--score-panel" => {
                o.score_panel = match args.next().as_deref() {
                    Some("off") => None,
                    Some("no-dmd") => Some(Overlay::Off),
                    Some(v) => Some(Overlay::parse(v).ok_or_else(|| format!("--score-panel takes auto, no-dmd, off or X,Y,WIDTH,HEIGHT\n{USAGE}"))?),
                    _ => return Err(format!("--score-panel takes auto, no-dmd, off or X,Y,WIDTH,HEIGHT\n{USAGE}")),
                }
            }
            "--show-grill" => o.show_grill = true,
            "--prefer-pup" => o.prefer_pup = true,
            "--backglass-dmd" => {
                o.backglass_dmd = args.next().as_deref().and_then(Overlay::parse).ok_or_else(|| format!("--backglass-dmd takes off, auto or X,Y,WIDTH,HEIGHT\n{USAGE}"))?;
            }
            "--displays" => o.displays = true,
            "--sound-outputs" => o.sound_outputs = true,
            "--input-test" => o.input_test = true,
            "--json" => o.json = true,
            "--max-fps" => {
                o.max_fps = args.next().and_then(|v| v.parse().ok()).filter(|v: &f64| *v == 0.0 || (10.0..=10_000.0).contains(v)).ok_or_else(|| format!("--max-fps takes 0 (no limit) or 10 to 10000\n{USAGE}"))?;
            }
            "--volume" => {
                let percent: f32 = args.next().and_then(|v| v.parse().ok()).filter(|v| (0.0..=100.0).contains(v)).ok_or_else(|| format!("--volume takes 0 to 100\n{USAGE}"))?;
                o.volume = Some(percent / 100.0);
            }
            "-h" | "--help" => return Err(USAGE.to_string()),
            s if s.starts_with("--") => return Err(format!("unknown option {s}\n{USAGE}")),
            s => table = Some(PathBuf::from(s)),
        }
    }
    if o.displays || o.sound_outputs {
        return Ok(o);
    }
    if o.input_test {
        o.settings = player::settings::Settings::load();
        return Ok(o);
    }
    o.table = table.ok_or_else(|| USAGE.to_string())?;
    // the settings file; test mode reads one only when `VPI_SETTINGS` names it (the VPX
    // picture tool's cabinet view), and never the table's own files beside it, so its pictures
    // depend on no one's settings, nor on what lies beside the table, unless asked
    if o.shot.is_none() {
        // the library's folders, as first setup keeps them
        if let Some(folders) = vpi_settings::library_folders_now() {
            player::script::api::set_library(folders);
        }
        // whether the user lets this table reach outside its own files, from the main settings
        // file alone, before the table's own is read over it
        let main = player::settings::Settings::load();
        if player::script::fence::open_for(&main, &o.table) {
            player::script::fence::set_open(true);
            println!("scripts: this table may reach outside its own files (your setting)");
        }
        // the table's own settings file over the main one, as VPX layers them
        o.settings = main.with_table(&o.table).with_pov(&o.table);
        if let Some(file) = &o.settings.table_path {
            println!("settings: the table's own as well, {} (VPX's per-table settings)", file.display());
        }
        if let Some(file) = &o.settings.pov_path {
            println!("settings: the table's views from its camera file, {} (as VPX imports one)", file.display());
        }
    } else if let Some(file) = std::env::var_os("VPI_SETTINGS").filter(|f| !f.is_empty()) {
        o.settings = player::settings::Settings::load();
        match &o.settings.path {
            Some(path) => println!("settings: {} (VPI_SETTINGS)", path.display()),
            // a file asked for and not there would leave the picture with VPX's defaults unnoticed
            None => return Err(format!("VPI_SETTINGS names {}, which can't be read", PathBuf::from(file).display())),
        }
    }
    if !o.view_given
        && let Some(view) = o.settings.view()
    {
        o.view = view;
    }
    apply_player_settings(&mut o);
    // VPX's frames the graphics card may queue, and its DMD and segment display profiles, as the
    // settings give them
    if let Some(frames) = o.settings.get("Player", "MaxPrerenderedFrames").and_then(player::settings::vpx_int) {
        player::render::set_frames_in_flight(frames.clamp(1, 3) as u32);
    }
    player::scoreview::set_profiles(player::vpx_settings::dmd_profiles(&o.settings));
    // a screenshot when the script saves a new high score, when that's on (`screenshot.rs`)
    player::screenshot::set_at_high_score(o.settings.get("VPI", "ScreenshotAtHighScore").and_then(player::settings::vpx_bool).unwrap_or(false));
    if let Some(path) = player::machine_settings::PinmameSettings::from_settings(&o.settings).path {
        player::pinmame::set_folder_setting(&path);
    }
    player::segments::set_profiles(player::vpx_settings::segment_profiles(&o.settings));
    Ok(o)
}

/// VPX 5436's display and picture settings, under its names, for whatever the command line didn't
/// give (`vpx_settings`): our own defaults hold where the settings say nothing.
fn apply_player_settings(o: &mut Options) {
    use player::vpx_settings as vs;
    let given = |names: &[&str]| names.iter().any(|n| o.given.contains(*n));
    let s = &o.settings;
    let present = (!given(&["--vsync", "--present"])).then(|| vs::presentation(s)).flatten();
    let frame_limit = (!given(&["--max-fps"])).then(|| vs::max_framerate(s)).flatten();
    let samples = (!given(&["--msaa"])).then(|| vs::samples(s)).flatten();
    let aa_factor = (!given(&["--aa-factor"])).then(|| vs::aa_factor(s)).flatten();
    let post_aa = (!given(&["--post-aa"])).then(|| vs::post_aa(s)).flatten();
    let sharpen = (!given(&["--sharpen"])).then(|| vs::sharpen(s)).flatten();
    let ao = (!given(&["--ao"])).then(|| vs::ambient_occlusion(s)).flatten();
    let motion_blur = (!given(&["--motion-blur"])).then(|| vs::motion_blur(s)).flatten();
    let readout = (!given(&["--no-readout"])).then(|| vs::readout(s)).flatten();
    let bloom_off = vs::bloom_off(s);
    // VPX 5436's B2S plugin settings, its legacy plugin's for what they share with B2S Server
    let show_grill = (!given(&["--show-grill"])).then(|| vs::show_grill(s)).flatten();
    let score_panel = (!given(&["--score-panel"])).then(|| if vs::hide_score_panel(s) { Some(None) } else { vs::b2s_dmd_overlay(s, "ScoreView").map(Some) }).flatten();
    let backglass_dmd = (!given(&["--backglass-dmd"])).then(|| vs::b2s_dmd_overlay(s, "Backglass")).flatten();
    if let Some(v) = show_grill {
        o.show_grill = v;
    }
    if let Some(v) = score_panel {
        o.score_panel = v;
    }
    if let Some(v) = backglass_dmd {
        o.backglass_dmd = v;
    }
    if let Some(v) = present {
        o.present = v;
    }
    o.frame_limit_setting = frame_limit;
    if let Some(v) = samples {
        o.samples = v;
    }
    if let Some(v) = aa_factor {
        o.aa_factor = v;
    }
    if let Some(v) = post_aa {
        o.post_aa = v;
    }
    if let Some(v) = sharpen {
        o.sharpen = v;
    }
    if let Some(v) = ao {
        o.ao = v;
    }
    if let Some(v) = motion_blur {
        o.motion_blur = v;
    }
    if let Some(v) = readout {
        o.readout = v;
    }
    o.bloom_off = bloom_off;
}

/// What VPX 5436's HDR10 output is asked to do on a screen with HDR on
/// (`render/hdr10.rs`): never in test mode, as VPX's own captures; `--hdr
/// off` or `[VPI]` `HDROutput = 0` keeps the ordinary colours; VPX's spline
/// and exposure from its own settings.
fn hdr_request(o: &Options) -> Option<player::render::HdrRequest> {
    if o.shot.is_some() {
        return None;
    }
    let on = o.hdr.unwrap_or_else(|| o.settings.get("VPI", "HDROutput").and_then(player::settings::vpx_bool).unwrap_or(true));
    on.then(|| player::render::HdrRequest {
        spline: o.settings.get("Player", "HDRDisableToneMapper").and_then(player::settings::vpx_bool).unwrap_or(true),
        exposure: o.settings.get("Player", "HDRGlobalExposure").and_then(player::settings::vpx_float).map_or(1.0, |e| e.clamp(0.0, 5.0)),
    })
}

/// The balls' last-moment move (VPX's `Ball::Render`, its update just
/// before the frame goes to the graphics card): each ball carried along its
/// velocity by the time since the physics stepped, the time until the frame
/// reaches the screen, and a millisecond for the screen itself.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BallMove {
    /// As VPX 5436 makes it: the time in seconds against a velocity per
    /// 10 ms (its physics' time unit), so a hundredth of the way — about a
    /// third of a unit for a fast ball, too little to see.
    Vpx,
    /// As VPX means it: the whole way, in its own time unit. The default
    ///.
    Intended,
}

impl BallMove {
    /// How far along its velocity a ball goes, in VPX's time units, for a
    /// delay in seconds.
    fn scale(self, delay_s: f32) -> f32 {
        match self {
            BallMove::Vpx => delay_s,
            BallMove::Intended => delay_s / 0.01,
        }
    }
}

/// VPX's guess at how long the frame being made takes to reach the screen
/// (`RenderDevice::GetPredictedDisplayDelay`, build 5436): the time to the
/// next frame the screen takes, counted from the last one shown, or a frame
/// later when making the last one took longer than that; none when frames
/// come as fast as they can. In seconds.
fn predicted_display_delay(now_ns: u64, last_present_ns: u64, frame_ns: Option<u64>, last_frame_ns: u64) -> f32 {
    let Some(frame) = frame_ns.filter(|&f| f > 0) else { return 0.0 };
    let mut delay = frame - now_ns.saturating_sub(last_present_ns) % frame;
    if delay < last_frame_ns {
        delay += frame;
    }
    delay as f32 * 1e-9
}

/// A window's place, `X,Y,WIDTH,HEIGHT` in the desktop's pixels.
fn window_place(option: &str, value: Option<String>) -> Result<[i32; 4], String> {
    let place: Vec<i32> = value.unwrap_or_default().split(',').filter_map(|v| v.trim().parse().ok()).collect();
    match place[..] {
        [x, y, w, h] if w > 0 && h > 0 => Ok([x, y, w, h]),
        _ => Err(format!("{option} takes X,Y,WIDTH,HEIGHT in pixels\n{USAGE}")),
    }
}

/// A window at `place`, `X,Y,WIDTH,HEIGHT` on the desktop, with SDL's
/// `flags` (VPX's `Window::Window` makes each window there, through SDL's
/// properties). Null, after saying why, when it can't be made.
fn placed_window(title: &std::ffi::CStr, [x, y, w, h]: [i32; 4], flags: SDL_WindowFlags) -> *mut SDL_Window {
    // SAFETY: SDL is initialised; the caller destroys the window
    let window = unsafe {
        let props = SDL_CreateProperties();
        SDL_SetStringProperty(props, SDL_PROP_WINDOW_CREATE_TITLE_STRING, title.as_ptr());
        SDL_SetNumberProperty(props, SDL_PROP_WINDOW_CREATE_X_NUMBER, i64::from(x));
        SDL_SetNumberProperty(props, SDL_PROP_WINDOW_CREATE_Y_NUMBER, i64::from(y));
        SDL_SetNumberProperty(props, SDL_PROP_WINDOW_CREATE_WIDTH_NUMBER, i64::from(w));
        SDL_SetNumberProperty(props, SDL_PROP_WINDOW_CREATE_HEIGHT_NUMBER, i64::from(h));
        SDL_SetNumberProperty(props, SDL_PROP_WINDOW_CREATE_FLAGS_NUMBER, flags.0 as i64);
        let window = SDL_CreateWindowWithProperties(props);
        SDL_DestroyProperties(props);
        window
    };
    if window.is_null() {
        eprintln!("{}: {}", title.to_string_lossy(), sdl_error());
    }
    window
}

/// A window of the player's own beside the playfield (the score display's,
/// the backglass's, the topper's): borderless, placed on the desktop, never
/// taking the keyboard from the playfield, and made hidden: the renderer
/// shows it once something draws in it, as VPX does (test mode never does).
/// Null when it has no place or can't be made.
fn extra_window(place: Option<[i32; 4]>, title: &std::ffi::CStr) -> *mut SDL_Window {
    let Some(place) = place else { return std::ptr::null_mut() };
    placed_window(title, place, SDL_WINDOW_HIGH_PIXEL_DENSITY | SDL_WINDOW_BORDERLESS | SDL_WINDOW_NOT_FOCUSABLE | SDL_WINDOW_HIDDEN)
}

/// One screen with no score window placed: VPX 10.8.0's way, PinMAME's own window floating over
/// the table, kept above it. Where VPinMAME puts its window unless told otherwise
/// (`dmd_pos_x`, `dmd_pos_y` at 0): the top-left corner, beside the table in a desktop view; a
/// quarter of the window wide, four times as wide as high. The settings' `[VPI]`
/// `FloatingScoreX`, `FloatingScoreY`, `FloatingScoreWidth` and `FloatingScoreHeight` place it
/// otherwise, in pixels from the table window's top-left corner (globally, or for one table in
/// its own settings file); where it was dragged to with the mouse for a game counts over them
/// (`player::score_window`). Shown with the display's first picture unless the script hides
/// PinMAME's window.
fn floating_window(playfield: *mut SDL_Window, settings: &player::settings::Settings) -> *mut SDL_Window {
    let (mut x, mut y, mut w, mut h) = (0, 0, 0, 0);
    // SAFETY: a live window of ours
    unsafe {
        SDL_GetWindowPosition(playfield, &mut x, &mut y);
        SDL_GetWindowSize(playfield, &mut w, &mut h);
    }
    let given = |name: &str| settings.get("VPI", name).and_then(|v| player::settings::vpx_trim(v).parse::<i32>().ok());
    let width = given("FloatingScoreWidth").filter(|&v| v > 0).unwrap_or((w / 4).max(128));
    let height = given("FloatingScoreHeight").filter(|&v| v > 0).unwrap_or((width / 4).max(32));
    let place = [x + given("FloatingScoreX").unwrap_or(0), y + given("FloatingScoreY").unwrap_or(0), width, height];
    placed_window(c"Visual Pinball Infinite - score display", place, SDL_WINDOW_HIGH_PIXEL_DENSITY | SDL_WINDOW_BORDERLESS | SDL_WINDOW_NOT_FOCUSABLE | SDL_WINDOW_HIDDEN | SDL_WINDOW_ALWAYS_ON_TOP)
}

/// Where each window goes: what an option gives first, else the
/// settings file (VPX 5436's window settings), else the player's defaults.
struct Layout {
    /// The playfield's window when the settings file places it: borderless,
    /// as VPX makes it, fullscreen when set; else the options' size, where
    /// SDL puts it.
    playfield: Option<player::settings::Placement>,
    /// The backglass's, the score view's and the topper's own windows: their
    /// places on the desktop. One set to borderless fullscreen covers its
    /// screen.
    backglass: Option<[i32; 4]>,
    score: Option<[i32; 4]>,
    topper: Option<[i32; 4]>,
    /// The backglass, score view and topper drawn inside the table's window instead (VPX 5436's
    /// embedded output, `…Output = 2`): their places in it, from its corner, in its units.
    embedded: [Option<[i32; 4]>; 3],
}

impl Layout {
    fn new(o: &Options, displays: &[player::settings::Display]) -> Layout {
        use player::settings::{Output, WindowId, place};
        let s = &o.settings;
        // test mode's window is never shown, and its picture takes `--size`: the file's place for it counts for nothing
        let playfield = if o.playfield_given || o.shot.is_some() || !s.has_window(WindowId::Playfield) { None } else { place(&s.window(WindowId::Playfield), displays) };
        let extra = |given: Option<[i32; 4]>, id: WindowId| {
            // one screen: only the windows the options give
            if given.is_some() || o.one_screen {
                return given;
            }
            let ws = s.window(id);
            match ws.output {
                Output::Disabled => None,
                Output::Floating => place(&ws, displays).map(|p| [p.x, p.y, p.width, p.height]),
                Output::Embedded => None,
            }
        };
        // drawn inside the table's window (VPX 5436's embedded output), unless an option gives it a window
        let embedded = |given: Option<[i32; 4]>, id: WindowId| {
            if given.is_some() || o.one_screen {
                return None;
            }
            let ws = s.window(id);
            (ws.output == Output::Embedded).then(|| [ws.x.unwrap_or(0), ws.y.unwrap_or(0), ws.width, ws.height])
        };
        Layout {
            playfield,
            backglass: extra(o.backglass_window, WindowId::Backglass),
            score: extra(o.dmd_window, WindowId::ScoreView),
            topper: extra(o.topper_window, WindowId::Topper),
            embedded: [
                embedded(o.backglass_window, WindowId::Backglass),
                embedded(o.dmd_window, WindowId::ScoreView),
                embedded(o.topper_window, WindowId::Topper),
            ],
        }
    }
}

/// Rolling input-to-frame times, in milliseconds.
struct LatencyLog {
    recent: Vec<f64>,
    last_reported: u64,
}

impl LatencyLog {
    fn add(&mut self, ms: f64) {
        if self.recent.len() == 20 {
            self.recent.remove(0);
        }
        self.recent.push(ms);
    }
    fn summary(&self) -> String {
        match self.recent.last() {
            None => "press a flipper key".into(),
            Some(last) => {
                let mean = self.recent.iter().sum::<f64>() / self.recent.len() as f64;
                format!("{last:.2} ms (mean of last {}: {mean:.2} ms)", self.recent.len())
            }
        }
    }
}

#[cfg(windows)]
mod crash;

fn main() -> ExitCode {
    #[cfg(windows)]
    crash::install();
    let o = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let run = if o.displays {
        list_displays(o.json)
    } else if o.sound_outputs {
        list_sound_outputs(o.json)
    } else if o.input_test {
        // SAFETY: the main thread, before anything else starts SDL
        unsafe { player::input::input_test(&o.settings) }
    } else {
        play(&o)
    };
    match run {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vpi-player: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The sound outputs, by the names the settings take (VPX's `[Player]` `SoundDevice` for the
/// playfield's sound, `SoundDeviceBG` for the backglass's); with `--json`, for the app's settings
/// pages: each output's name and channels, and the default output's channels.
fn list_sound_outputs(json: bool) -> Result<(), String> {
    // SAFETY: SDL is used from this thread only, and released before returning
    unsafe {
        if !SDL_Init(SDL_INIT_AUDIO) {
            return Err(format!("start SDL: {}", sdl_error()));
        }
        let names = player::audio::output_names();
        if json {
            let outputs: Vec<_> = names.iter().map(|n| serde_json::json!({ "name": n, "channels": player::audio::output_channels(Some(n)) })).collect();
            println!("{}", serde_json::json!({ "outputs": outputs, "defaultChannels": player::audio::output_channels(None) }));
            SDL_Quit();
            return Ok(());
        }
        if names.is_empty() {
            println!("no sound outputs found");
        }
        let channels = |n: Option<i32>| n.map_or(String::new(), |n| format!(", {n} channels"));
        for (i, name) in names.iter().enumerate() {
            println!("sound output {}: \"{name}\"{}", i + 1, channels(player::audio::output_channels(Some(name))));
        }
        println!("the default output{}", channels(player::audio::output_channels(None)));
        println!("(the settings file names an output by the name shown in quotes, without them: [Player] SoundDevice for the playfield's sound, SoundDeviceBG for the backglass's; unset, the default plays)");
        SDL_Quit();
    }
    Ok(())
}

/// Lists the screens, with where each sits on the desktop in pixels, for
/// placing the player's windows (`--dmd-window` and the like), and the
/// names the settings file knows them by; with `--json`, the same for the
/// app's Screens page, each with its pixel density as the player places
/// windows by it.
fn list_displays(json: bool) -> Result<(), String> {
    // SAFETY: SDL is used from this thread only, and released before returning
    unsafe {
        if !SDL_Init(SDL_INIT_VIDEO) {
            return Err(format!("start SDL: {}", sdl_error()));
        }
        let mut count = 0;
        let ids = SDL_GetDisplays(&mut count);
        if ids.is_null() {
            SDL_Quit();
            return Err(format!("list the screens: {}", sdl_error()));
        }
        let primary = SDL_GetPrimaryDisplay();
        let mut listed = Vec::new();
        for i in 0..count.max(0) as usize {
            let id = *ids.add(i);
            let mut r: SDL_Rect = std::mem::zeroed();
            SDL_GetDisplayBounds(id, &mut r);
            let name = SDL_GetDisplayName(id);
            let name = if name.is_null() { String::new() } else { std::ffi::CStr::from_ptr(name).to_string_lossy().into_owned() };
            let mode = SDL_GetCurrentDisplayMode(id);
            let refresh = if mode.is_null() { 0.0 } else { (*mode).refresh_rate };
            let scale = SDL_GetDisplayContentScale(id) * 100.0;
            let size_cm = player::settings::physical_size_cm(id);
            // Windows' HDR switched on for it: VPX 5436 then presents in HDR10 there
            let hdr = SDL_GetBooleanProperty(SDL_GetDisplayProperties(id), SDL_PROP_DISPLAY_HDR_ENABLED_BOOLEAN, false);
            if json {
                let desktop = SDL_GetDesktopDisplayMode(id);
                let density = if desktop.is_null() { 1.0 } else { (*desktop).pixel_density };
                listed.push(serde_json::json!({
                    "number": i + 1, "name": name, "x": r.x, "y": r.y, "width": r.w, "height": r.h,
                    "density": density, "scale": scale, "refresh": refresh,
                    "sizeCm": size_cm.map(|(w, h)| [w, h]), "hdr": hdr, "primary": id == primary,
                }));
                continue;
            }
            let main = if id == primary { ", the main screen" } else { "" };
            let size = size_cm.map(|(w, h)| format!(", {w:.0} x {h:.0} cm")).unwrap_or_default();
            let hdr = if hdr { ", HDR on" } else { "" };
            println!("screen {}: \"{name}\" — {}x{} at {},{} ({scale:.0}% scale, {refresh:.0} Hz{size}{hdr}{main})", i + 1, r.w, r.h, r.x, r.y);
        }
        if json {
            println!("{}", serde_json::Value::Array(listed));
        } else {
            println!("(the settings file names a screen by the name shown in quotes, without them: PlayfieldDisplay, BackglassDisplay, ScoreViewDisplay, TopperDisplay)");
        }
        SDL_free(ids.cast());
        SDL_Quit();
    }
    Ok(())
}

fn play(o: &Options) -> Result<(), String> {
    let t0 = Instant::now();
    // VPX's detail level for the rubbers, ramps and balls drawn, as the physics takes it
    player::shapes::rubber::set_detail_level(player::vpx_settings::detail_level(&o.settings).unwrap_or(10));
    let mut vpx = vpin::vpx::read(&o.table).map_err(|e| format!("could not read {}: {e}", o.table.display()))?;
    player::pictures::resolve_links(&mut vpx);
    // pictures kept beside the table in its own's place (`<table> pictures`); test mode reads
    // nothing beside the table
    if o.shot.is_none() {
        let replaced = player::pictures::replace_from_folder(&mut vpx, &o.table);
        if !replaced.is_empty() {
            println!("pictures: {} played from the table's pictures folder in place of its own", replaced.join(", "));
        }
    }
    // a physics settings file beside the table, as VPX loads one (`<table>.vpp`, else `autovpp.vpp`);
    // test mode reads nothing beside the table
    if o.shot.is_none()
        && !player::achievements::hardcore_play()
        && let Some(file) = table_physics::vpp::find(&o.table)
    {
        match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|text| table_physics::vpp::apply(&mut vpx, &text)) {
            Ok(missing) => {
                println!("physics: the table's and flippers' as {} sets them (VPX's physics settings file beside the table)", file.display());
                for m in missing {
                    println!("physics: {m} in {} (VPX says so too, and goes on)", file.display());
                }
            }
            Err(e) => println!("physics: {} left aside: {e}", file.display()),
        }
    }
    let t_read = t0.elapsed();
    // our own model of the table: our shapes, from VPX's code, and the table file alone; `--shapes export` keeps vpin's export and its shapes, the backup
    let (doc, pictures) = if o.own_shapes { player::model::load(&vpx)? } else { table_meshes::load(&mut vpx)? };
    let t_meshes = t0.elapsed();
    let mut scene = Scene::build(&vpx, &doc, pictures);
    if o.own_shapes {
        // VPX's plunger: a frame for each position
        scene.use_own_plungers(&vpx);
        // primitives scripts animate: their frames, shown as the script asks
        let animated = scene.use_own_animations(&vpx);
        if animated > 0 {
            println!("own shapes: {animated} animated primitive(s), each frame shown as its script asks");
        }
    }
    let t_scene = t0.elapsed();
    drop(doc);
    let g = &vpx.gamedata;
    let backdrop_image = match o.view {
        View::Desktop => g.backglass_image_full_desktop.clone(),
        View::Cabinet => g.backglass_image_full_fullscreen.clone(),
        View::SingleScreen => g.backglass_image_full_single_screen.clone().unwrap_or_else(|| g.backglass_image_full_desktop.clone()),
    };
    // the player's exposure, tone mapper and light level for the table, from the settings
    let light_level = player::vpx_settings::light_level(&o.settings, vpx.gamedata.global_emission_scale);
    scene.set_player_look(&vpx, player::vpx_settings::exposure(&o.settings), player::vpx_settings::tone_mapper(&o.settings), light_level);
    // VPX draws no backdrop items in the cabinet view (m_noBackdrop)
    scene.add_backdrop(&vpx, &backdrop_image, o.view != View::Cabinet);
    // VPX's detail level, the player's difficulty and the physics sets, from the settings
    let physics = table_physics::build(&vpx, &player::vpx_settings::physics(&o.settings));
    let legacy_bounds: Vec<Vec3> = table_physics::view_bounds::legacy_view_bounds(&vpx).iter().map(|v| Vec3::new(v.x, v.y, v.z)).collect();
    let g = &vpx.gamedata;
    let extent = TableExtent { left: g.left, top: g.top, right: g.right, bottom: g.bottom };
    // the table's view; what the settings file overrides comes once the screens are known
    let mut view_setup = ViewSetup::from_table(&vpx, o.view);
    println!(
        "{}: loaded in {:.0} ms (the file {:.0}, its meshes {:.0}, pictures and parts {:.0}, the rest {:.0}) — {} draws, {} images ({:.0} MB decoded), {} moving parts",
        o.table.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        t0.elapsed().as_secs_f64() * 1e3,
        t_read.as_secs_f64() * 1e3,
        (t_meshes - t_read).as_secs_f64() * 1e3,
        (t_scene - t_meshes).as_secs_f64() * 1e3,
        (t0.elapsed() - t_scene).as_secs_f64() * 1e3,
        scene.draws.len(),
        scene.images.len(),
        scene.images.iter().map(player::scene::Image::bytes).sum::<usize>() as f64 / 1e6,
        scene.groups.len()
    );
    if o.look == LookMode::Vpx {
        let mapper = ["Reinhard", "AgX", "Filmic", "Neutral", "AgX punchy"].get(scene.tone.mapper as usize).copied().unwrap_or("Reinhard");
        let grade = if scene.tone.color_grade.is_some() { g.image_color_grade.as_str() } else { "none" };
        let env = if scene.tone.env_default { "our default room (the table names none)".to_string() } else { g.env_image.clone().unwrap_or_default() };
        let size = match &scene.env_hdr {
            Some(h) => (h.width, h.height),
            None => scene.tone.env.and_then(|i| scene.images.get(i)).map_or((0, 0), |i| (i.width, i.height)),
        };
        let env = if scene.env_hdr.is_some() { format!("{env}, high range") } else { env };
        println!(
            "VPX's look: tone mapper {mapper}, exposure {}, colour grade {grade}; environment picture {env} ({}x{}), its emission {} (scale {} x day and night {})",
            scene.tone.exposure, size.0, size.1, scene.tone.env_emission, g.env_emission_scale, g.global_emission_scale
        );
        let ball = if scene.tone.ball_image.is_some() { format!("{} ({})", g.ball_image, if scene.tone.ball_spherical { "sphere map" } else { "equirectangular" }) } else { "VPX's default".to_string() };
        let scratches = if scene.tone.ball_decal.is_some() { g.ball_image_front.clone() } else { "none".to_string() };
        println!("VPX's look: ball picture {ball}, scratches {scratches}, mirrors the playfield {}", scene.tone.ball_reflection);
        // the mirrors parts show, with what they mirror and how strongly
        const MODES: [&str; 6] = ["nothing", "balls", "still parts", "still parts and balls", "everything (unsynced)", "everything"];
        let mirrors: Vec<String> = scene
            .mirrors
            .iter()
            .enumerate()
            .filter_map(|(k, m)| {
                let strengths: Vec<f32> = scene.draws.iter().filter_map(|d| d.mirror.filter(|&(i, _)| i == k).map(|(_, s)| s)).collect();
                let strongest = strengths.iter().copied().reduce(f32::max)?;
                let reflected = scene.draws.iter().filter(|d| d.reflected && d.mirror.is_none_or(|(i, _)| i != k)).count();
                let lights = if m.no_light_maps { "" } else { ", lights too" };
                Some(format!("{} ({}{lights}, strength up to {strongest:.2}, {reflected} meshes, roughness {})", m.name, MODES[m.mode.min(5) as usize], m.roughness))
            })
            .collect();
        println!("VPX's look: mirrors: {}", if mirrors.is_empty() { "none".to_string() } else { mirrors.join("; ") });
        let normal_mapped = scene.draws.iter().filter(|d| d.normal_map.is_some()).count();
        if normal_mapped > 0 {
            let object = scene.draws.iter().filter(|d| d.object_normal.is_some()).count();
            println!("VPX's look: {normal_mapped} meshes with normal maps ({object} in their part's own space)");
        }
    }

    let mut sounds = player::audio::Sounds::load(&vpx);
    (sounds.playfield_volume, sounds.backglass_volume) = o.volume.map_or_else(|| o.settings.volumes(), |v| (v, v));
    // each source's level on the backglass's speakers (VPX 5436's audio sources' gains)
    sounds.levels = player::audio::Levels::from_settings(&o.settings);
    // an achievement's sound as it's announced, on unless switched off
    player::achievements::set_sound(o.settings.int_or("VPI", "AchievementSound", 1, 0, 1) != 0);
    // what the pause menu shows of the table, and where it keeps what changes (outside test mode)
    let menu = {
        let i = &vpx.info;
        let given = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
        let title = given(&i.table_name).unwrap_or_else(|| o.table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
        let byline = match (given(&i.author_name), given(&i.table_version)) {
            (Some(a), Some(v)) => format!("by {a}  \u{b7}  version {v}"),
            (Some(a), None) => format!("by {a}"),
            (None, Some(v)) => format!("version {v}"),
            (None, None) => String::new(),
        };
        player::pause_menu::Setup {
            title,
            byline,
            rules: given(&i.table_rules),
            settings_file: if o.shot.is_some() { None } else { vpi_settings::settings_path() },
            table_file: (o.shot.is_none()).then(|| o.table.with_extension("ini")),
            picture_only: o.shot.is_some(),
            view: None,
        }
    };

    input::request_raw_keyboard();
    // SAFETY: SDL is used from this thread, the window's owner
    unsafe {
        // VPX's window hints (renderer/Window.cpp): a raise always brought to the front, and a
        // fullscreen window never minimized when another window takes the focus
        SDL_SetHint(SDL_HINT_FORCE_RAISEWINDOW, c"1".as_ptr());
        SDL_SetHint(SDL_HINT_VIDEO_MINIMIZE_ON_FOCUS_LOSS, c"0".as_ptr());
        // gamepads, but in test mode, whose keys are scripted
        let gamepads = if o.shot.is_none() { SDL_INIT_GAMEPAD } else { SDL_InitFlags(0) };
        if !SDL_Init(SDL_INIT_VIDEO | SDL_INIT_AUDIO | gamepads) {
            return Err(format!("start SDL: {}", sdl_error()));
        }
    }
    // SAFETY: SDL's video is started, on this thread
    let displays = unsafe { player::settings::displays() };
    let layout = Layout::new(o, &displays);
    // the view, with what the settings file overrides and, in window mode, where the player
    // stands before the cabinet's screen, whose size is by default the screen's own (VPX's
    // defaults without a file; in test mode the screen's size counts for nothing, so VPX's
    // default holds unless the file `VPI_SETTINGS` names gives one)
    let screen_cm = if o.shot.is_some() {
        None
    } else {
        let shown_on = layout.playfield.as_ref().map(|p| p.display.as_str());
        displays.iter().find(|d| shown_on.map_or(d.primary, |name| d.name == name)).and_then(|d| d.size_cm)
    };
    view_setup.apply_settings(&o.settings, o.view, &extent, screen_cm);
    // the table's own view — its own, with the main settings file's over it, neither its own
    // file's nor its camera file's: what the pause menu's view page goes back to, and keeps its
    // changes against (in test mode, as the test's settings have it)
    let own_view = {
        let mut own = ViewSetup::from_table(&vpx, o.view);
        let main = if o.shot.is_some() { o.settings.clone() } else { player::settings::Settings::load() };
        own.apply_settings(&main, o.view, &extent, screen_cm);
        own
    };
    // test mode: the table's sound recorded with no output, into its video (`--video-sound`)
    let recorder = (o.shot.is_some() && o.video.is_some() && o.video_sound).then(|| player::audio::Recorder::start(&mut sounds));
    // sound is welcome but not required: play on without it. Two outputs, as VPX 5436 opens
    // them (its AudioPlayer, the backglass's first): each the one the settings name, else the
    // default; one alone plays everything when the other won't open
    let audio = if o.shot.is_some() {
        None
    } else {
        let (playfield_output, backglass_output) = o.settings.sound_outputs();
        // the playfield's sound laid out among its output's speakers (VPX's `[Player]` `Sound3D`)
        let layout = player::audio::Layout::from_setting(o.settings.enum_or("Player", "Sound3D", 0, 6));
        let backglass = player::audio::Audio::open(&sounds, backglass_output.as_deref(), None);
        let playfield = player::audio::Audio::open(&sounds, playfield_output.as_deref(), Some(layout));
        match (playfield, backglass) {
            (Ok((pf, ptx)), Ok((bg, btx))) => {
                sounds.connect(ptx, btx);
                Some((Some(pf), Some(bg)))
            }
            (Ok((audio, tx)), Err(e)) | (Err(e), Ok((audio, tx))) => {
                eprintln!("vpi-player: one sound output only: {e}");
                sounds.connect(tx.clone(), tx);
                Some((Some(audio), None))
            }
            (Err(e), Err(_)) => {
                eprintln!("vpi-player: no sound: {e}");
                None
            }
        }
    };
    let setup = GameSetup {
        physics,
        vpx: Arc::new(vpx),
        table_path: o.table.clone(),
        render_parts: scene.groups.iter().map(|g| g.part).collect(),
        script: o.script,
        sounds,
        record: o.record.clone(),
        view: o.view,
        stop_at_ms: o.shot.as_ref().map(|_| o.shot_at_ms + o.video.as_ref().map_or(0, |_| o.video_seconds * 1000)),
        record_from_ms: o.shot.as_ref().and(o.video.as_ref()).map(|_| o.shot_at_ms),
        backglass_window: layout.backglass.is_some() || layout.score.is_some() || layout.embedded[0].is_some() || layout.embedded[1].is_some(),
        show_grill: o.show_grill,
        pup_windows: [layout.topper.or(layout.embedded[2]), layout.backglass.or(layout.embedded[0]), layout.score.or(layout.embedded[1])]
            .map(|p| p.map(|[_, _, w, h]| (w.max(1) as u32, h.max(1) as u32))),
        pup: player::pup::PupSettings::from_settings(&o.settings),
        // VPinMAME's settings for each game, beside the settings file; none read in test mode
        machine_settings: Arc::new(if o.shot.is_some() {
            player::machine_settings::MachineSettings::default()
        } else {
            player::machine_settings::MachineSettings::load(o.settings.path.as_deref())
        }),
        pinmame: player::machine_settings::PinmameSettings::from_settings(&o.settings),
        // the values scripts keep between plays, beside the settings file; for the session only in
        // test mode
        saved_values: if o.shot.is_some() { None } else { player::saved_values::file_for(o.settings.path.as_deref()) },
        cabinet_input: o.shot.is_none(),
        plumb: (
            o.settings.get("Player", "SimulatedPlumb").and_then(player::settings::vpx_bool).unwrap_or(true),
            o.settings.float_or("Player", "PlumbDamping", 1.0, 0.0, 2.0, 0.0),
            o.settings.float_or("Player", "PlumbThresholdAngle", 1.0, 0.15, 4.0, 0.0),
        ),
        // the settings' nudge and plunger sensors, read where the input map is made (`run_window`)
        sensors: player::sensors::Sensors::default(),
        tilt_counts_nudges: o.settings.get("VPI", "TiltCountsNudges").and_then(player::settings::vpx_bool).unwrap_or(false),
        light_level: Some(light_level),
        plunger_retract: player::vpx_settings::plunger_retract(&o.settings),
        waker: None,
        table_options: o.settings.section("TableOption"),
        controller_settings: o.settings.section("Controller"),
        // test mode writes into no install: a pack without its settings file would get one
        altsound_settings: if o.shot.is_some() { vec![("Enable".into(), "0".into())] } else { o.settings.section("Plugin.AltSound") },
        // test mode draws as VPX's reference pictures do, its plugins off: colour only when the
        // settings it reads (VPI_SETTINGS) switch it on
        serum_settings: colour_settings(o, "Plugin.Serum"),
        vni_settings: colour_settings(o, "Plugin.VNI"),
        alphadmd_settings: colour_settings(o, "Plugin.AlphaDMD"),
        upscale_settings: colour_settings(o, "Plugin.UpscaleDMD"),
        upscale_at_once: o.shot.is_some(),
        dof_settings: colour_settings(o, "Plugin.DOF"),
        dof_folder: None,
        record_toys: false,
        dmdutil_settings: colour_settings(o, "Plugin.DMDUtil"),
        dmd_panel_check: None,
        // a real DMD panel through a VPinMAME display device, when the settings name one
        dmd_device: player::dmd_device::setup(&o.settings),
        menu: Some(player::pause_menu::Setup { view: Some(player::pause_menu::Viewing { which: o.view, now: view_setup, own: own_view }), ..menu }),
    };

    let result = run_window(o, &layout, &mut scene, setup, &view_setup, &extent, &legacy_bounds, recorder);
    drop(audio);
    // SAFETY: everything SDL made is gone by now
    unsafe { SDL_Quit() };
    result
}

/// A colour plugin's settings (`Plugin.Serum`, `Plugin.VNI`): in play
/// as the settings give them, on unless switched off; in test mode off unless
/// the settings it reads (`VPI_SETTINGS`) switch it on, as VPX's reference
/// pictures are drawn with its plugins off.
/// Tells the game thread the table's window lost the keyboard (play pauses) or has it again (play
/// goes on), and says so in the log with the window that took it, as VPX's log says it.
fn tell_focus(game: &mpsc::Sender<input::InputEvent>, lost: bool) {
    use player::pause_menu::code;
    if lost {
        println!("the table's window lost the keyboard to {}: play paused until it's back", player::focus::foreground_window());
    } else {
        println!("the table's window has the keyboard again: play goes on");
    }
    // SAFETY: SDL's clock
    let time_ns = unsafe { SDL_GetTicksNS() };
    let _ = game.send(input::InputEvent { id: 0, time_ns, dik: if lost { code::FOCUS_LOST } else { code::FOCUS_BACK }, down: true });
}

fn colour_settings(o: &Options, section: &str) -> Vec<(String, String)> {
    let given = o.settings.section(section);
    if o.shot.is_some() { std::iter::once(("Enable".to_string(), "0".to_string())).chain(given).collect() } else { given }
}

/// VPX's nudge shake (build 5436, `Renderer::SetScreenOffset`, scaled by its
/// visual nudge strength, `[Player]` `NudgeStrength`, 1 by default): the
/// cabinet's movement from a nudge, in metres, as an offset of the whole
/// picture in screen units (-1 to 1), turned against the view's rotation.
fn nudge_shake(offset: [f32; 2], rotation_deg: f32, strength: f32) -> [f32; 2] {
    let (x, y) = (strength * offset[0] * 2.0, strength * offset[1] * 2.0);
    let (s, c) = (-rotation_deg.to_radians()).sin_cos();
    [x * c - y * s, x * s + y * c]
}

/// The playfield's box in a picture `w` by `h` drawn through `view_proj`: around its four corners
/// as they're seen, a few pixels to spare, held within the picture and even each way (the video's
/// encoder takes even sizes) — the playfield without the cabinet around it, as VPS's pictures
/// show tables.
fn playfield_box(view_proj: &Mat4, extent: &TableExtent, w: u32, h: u32) -> [u32; 4] {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in [(extent.left, extent.top), (extent.right, extent.top), (extent.left, extent.bottom), (extent.right, extent.bottom)] {
        let p = view_proj.transform(Vec3::new(x, y, 0.0));
        let (sx, sy) = ((p.x + 1.0) * 0.5 * w as f32, (1.0 - p.y) * 0.5 * h as f32);
        (x0, y0, x1, y1) = (x0.min(sx), y0.min(sy), x1.max(sx), y1.max(sy));
    }
    let spare = 4.0;
    let left = ((x0 - spare).floor().max(0.0) as u32).min(w.saturating_sub(2));
    let top = ((y0 - spare).floor().max(0.0) as u32).min(h.saturating_sub(2));
    let right = ((x1 + spare).ceil().max(0.0) as u32).clamp(left + 2, w);
    let bottom = ((y1 + spare).ceil().max(0.0) as u32).clamp(top + 2, h);
    [left, top, (right - left) & !1, (bottom - top) & !1]
}

/// A picture's pixels within box `b` (x, y, width, height) of a picture `w` wide.
fn cut(rgba: &[u8], w: u32, b: [u32; 4]) -> Vec<u8> {
    let [x, y, bw, bh] = b.map(|v| v as usize);
    let row = w as usize * 4;
    (y..y + bh).flat_map(|r| &rgba[r * row + x * 4..r * row + (x + bw) * 4]).copied().collect()
}

#[allow(clippy::too_many_arguments)]
fn run_window(o: &Options, layout: &Layout, scene: &mut Scene, setup: GameSetup, view_setup: &ViewSetup, extent: &TableExtent, legacy_bounds: &[Vec3], mut recorder: Option<player::audio::Recorder>) -> Result<(), String> {
    // the view as it plays: the pause menu's view page can change it
    let mut view_setup = *view_setup;
    let title = CString::new(if o.front_end {
        "Visual Pinball Player".to_string()
    } else {
        format!("Visual Pinball Infinite - {}", o.table.file_stem().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))
    })
    .unwrap_or_default();
    let mut flags = SDL_WINDOW_HIGH_PIXEL_DENSITY;
    if o.shot.is_some() {
        flags |= SDL_WINDOW_HIDDEN;
    }
    let window = match &layout.playfield {
        // where the settings file puts it: borderless, as VPX makes its windows, fullscreen when set
        Some(p) => {
            flags |= SDL_WINDOW_BORDERLESS;
            if p.fullscreen {
                flags |= SDL_WINDOW_FULLSCREEN;
            }
            placed_window(&title, [p.x, p.y, p.width, p.height], flags)
        }
        None => {
            flags |= SDL_WINDOW_RESIZABLE;
            if o.fullscreen {
                flags |= SDL_WINDOW_FULLSCREEN;
            }
            // SAFETY: SDL is initialised; the window is destroyed before returning
            unsafe { SDL_CreateWindow(title.as_ptr(), o.size.0, o.size.1, flags) }
        }
    };
    if window.is_null() {
        return Err(format!("create window: {}", sdl_error()));
    }
    // the backglass's, the score display's and the topper's own windows, these too destroyed before returning
    let backglass_window = extra_window(layout.backglass, c"Visual Pinball Infinite - backglass");
    // one screen, no score window placed: the score display floats over the table, as VPX 10.8.0
    // shows PinMAME's own window (`[VPI]` `FloatingScore = 0` turns it off)
    let floating = layout.score.is_none()
        && layout.embedded[1].is_none()
        && o.shot.is_none()
        && o.view != View::Cabinet
        && o.settings.get("VPI", "FloatingScore").and_then(player::settings::vpx_bool).unwrap_or(true);
    let dmd_window = if floating { floating_window(window, &o.settings) } else { extra_window(layout.score, c"Visual Pinball Infinite - score display") };
    // the floating one is dragged with the mouse, its place kept for each game, as VPinMAME's
    // SAFETY: both windows outlive it, and it's used on their thread
    let mut score_drag = (floating && !dmd_window.is_null())
        .then(|| unsafe { player::score_window::ScoreWindow::new(dmd_window, window, player::score_window::ScoreWindow::file_for(o.settings.path.as_deref())) });
    let topper_window = extra_window(layout.topper, c"Visual Pinball Infinite - topper");
    // shown from the start when the settings ask, else with their first picture
    if o.shot.is_none() && o.settings.windows_at_start() {
        for window in [backglass_window, dmd_window, topper_window].into_iter().filter(|w| !w.is_null()) {
            // SAFETY: a live window of ours; it never takes the keyboard
            unsafe { SDL_ShowWindow(window) };
        }
    }
    let result = (|| {
        // SAFETY: the window lives until after the renderer is dropped, below
        // test mode's pictures go without multisampling unless asked, so they repeat exactly
        let mut renderer = unsafe { Renderer::new(window, scene, &setup.vpx.images, o.present, o.samples, o.look, hdr_request(&o))? };
        if let Some(h) = renderer.hdr10() {
            println!(
                "screen: HDR, VPX 5436's HDR10 output: white {:.0} nits, peak {:.0}; {}, no bloom{}",
                h.sdr_white,
                h.peak,
                if h.request.spline { "VPX's spline in the tone mapper's place" } else { "the table's tone mapper" },
                if h.request.exposure != 1.0 { format!(", exposure x{}", h.request.exposure) } else { String::new() }
            );
        }
        renderer.draw_still_parts_once(o.still_once);
        renderer.set_post(o.post_aa, o.sharpen);
        renderer.set_supersampling(o.aa_factor);
        renderer.set_ambient_occlusion(o.ao);
        renderer.set_motion_blur(o.motion_blur);
        renderer.set_bloom_off(o.bloom_off);
        renderer.set_anisotropic(player::vpx_settings::anisotropic(&o.settings))?;
        renderer.set_max_reflection_mode(player::vpx_settings::reflection_mode(&o.settings).into());
        renderer.set_ball_settings(
            &player::vpx_settings::ball_override(&o.settings),
            player::vpx_settings::ball_lighting_off(&o.settings),
            player::vpx_settings::ball_anti_stretch(&o.settings),
        );
        if o.look == LookMode::Vpx {
            println!(
                "still parts: {}",
                match (renderer.still_parts_once(), renderer.multisampled()) {
                    (true, false) => "drawn once (VPX's static prepass)",
                    (true, true) => "drawn once (VPX's static prepass), what moves multisampled (as VPX's DirectX and OpenGL builds draw them)",
                    (false, true) => "drawn every frame, multisampled (as VPX 5436's BGFX build draws them then)",
                    (false, false) => "drawn every frame",
                }
            );
        }
        if !backglass_window.is_null() {
            // SAFETY: the backglass's window, too, outlives the renderer
            if let Err(e) = unsafe { renderer.add_backglass_window(backglass_window) } {
                eprintln!("backglass window: {e}");
            }
        }
        if !dmd_window.is_null() {
            // SAFETY: the display's window, too, outlives the renderer
            // the score window on a screen with HDR on as VPX 5436 shows it there, when asked (`[VPI]`
            // `HDRScoreView`); never in test mode
            let hdr_dmd = o.shot.is_none()
                && o.hdr_dmd.unwrap_or_else(|| o.settings.get("VPI", "HDRScoreView").and_then(player::settings::vpx_bool).unwrap_or(false));
            if let Err(e) = unsafe { renderer.add_display_window(dmd_window, hdr_dmd) } {
                eprintln!("score display window: {e}");
            }
            // the table's own score view layout, if it has one, else VPX's
            renderer.set_score_layouts(player::scoreview::for_table_with(&o.table, &player::scoreview::ScoreViewSettings::from_settings(&o.settings)));
            renderer.set_floating_display(floating);
        }
        if !topper_window.is_null() {
            // SAFETY: the topper's window, too, outlives the renderer
            if let Err(e) = unsafe { renderer.add_topper_window(topper_window) } {
                eprintln!("topper window: {e}");
            }
        }
        // the backglass, score view and topper drawn inside the table's window, where the settings
        // put them (VPX 5436's embedded output)
        for (kind, place) in [(EmbedKind::Backglass, layout.embedded[0]), (EmbedKind::ScoreView, layout.embedded[1]), (EmbedKind::Topper, layout.embedded[2])] {
            let Some(place) = place else { continue };
            // SAFETY: on the renderer's thread
            match unsafe { renderer.add_embedded(kind, place) } {
                Ok(()) => {
                    println!("{kind:?} drawn inside the table's window at {place:?} (VPX 5436's embedded output)");
                    if kind == EmbedKind::ScoreView {
                        renderer.set_score_layouts(player::scoreview::for_table_with(&o.table, &player::scoreview::ScoreViewSettings::from_settings(&o.settings)));
                    }
                }
                Err(e) => eprintln!("{kind:?} inside the table's window: {e}"),
            }
        }
        // the windows show once something draws in them, as VPX's do; test mode's stay hidden
        renderer.set_reveal_windows(o.shot.is_none());
        // the playfield's window brought to the front with the keyboard once the table is ready,
        // as VPX's player does (Window::RaiseAndFocus, fullscreen restored after it): Windows
        // keeps its taskbar behind a fullscreen window only while that window is in front
        if o.shot.is_none() {
            // SAFETY: the window is live, on SDL's thread
            unsafe {
                // raised the forceful way this once (SDL's thread-input trick): the player was just
                // started for this table, and a window that doesn't get the keyboard hears no keys
                SDL_SetHint(SDL_HINT_FORCE_RAISEWINDOW, c"1".as_ptr());
                SDL_RaiseWindow(window);
                SDL_SetHint(SDL_HINT_FORCE_RAISEWINDOW, c"0".as_ptr());
                if (SDL_GetWindowFlags(window) & SDL_WINDOW_FULLSCREEN).0 != 0 {
                    SDL_SetWindowFullscreen(window, true);
                }
            }
        }
        renderer.set_dmd_overlays(o.score_panel, o.backglass_dmd);
        // which comes first where both draw: `--prefer-pup` the pack; else VPX's renderer
        // priorities, when the settings give them; else the pack, wherever it draws
        let pack_first = |section: &str| o.prefer_pup || o.settings.pack_first(section).unwrap_or(true);
        renderer.set_pack_first(pack_first("Backglass"), pack_first("ScoreView"));
        // test mode: each frame waits for its PinUP pictures, so a picture repeats exactly
        player::pup::playback::set_exact(o.shot.is_some());
        // frame pacing follows the refreshes of the screen the window is on
        let vblank = if o.present == Presentation::Paced && o.shot.is_none() {
            // SAFETY: the window is live; its properties are SDL's
            let (hwnd, refresh) = unsafe {
                let hwnd = SDL_GetPointerProperty(SDL_GetWindowProperties(window), SDL_PROP_WINDOW_WIN32_HWND_POINTER, std::ptr::null_mut());
                let mode = SDL_GetCurrentDisplayMode(SDL_GetDisplayForWindow(window));
                (hwnd, if mode.is_null() { 0.0 } else { (*mode).refresh_rate })
            };
            let v = player::vblank::VBlank::start(hwnd, refresh);
            if v.is_none() {
                eprintln!("frame pacing: the screen's refreshes can't be followed here; plain vsync instead");
            }
            v
        } else {
            None
        };
        let mut pacer = player::vblank::Pacer::default();

        let waker = Arc::new(Waker::new());
        let (tx, rx) = mpsc::channel();
        let scripted = tx.clone();
        let mut pending_keys = o.keys.clone();
        pending_keys.sort_by_key(|k| k.0);
        let mut pending_keys = pending_keys.into_iter().peekable();
        // the window thread's own waker: the frame-rate limit waits on it, and keys wake it
        let frame_waker = Arc::new(Waker::new());
        // SAFETY: the window is live
        let refresh = unsafe {
            let mode = SDL_GetCurrentDisplayMode(SDL_GetDisplayForWindow(window));
            if mode.is_null() { 0.0 } else { (*mode).refresh_rate }
        };
        // the frame-rate limit: the option's or ours, else VPX's `MaxFramerate` worked out as VPX does
        // against the screen's refresh rate and the sync mode
        let max_fps = match o.frame_limit_setting {
            Some(setting) => player::vpx_settings::frame_limit(setting, refresh, o.present != Presentation::Immediate),
            None => o.max_fps,
        };
        let frame_period = (max_fps > 0.0).then(|| Duration::from_secs_f64(1.0 / max_fps));
        let nudge_strength = player::vpx_settings::visual_nudge_strength(&o.settings);
        let mut last_frame_began: Option<Instant> = None;
        // the balls' last-moment move: how long a frame the screen takes (VPX's target frame length: the
        // frame-rate limit without sync, else the screen's refresh), when the last frame went to it, and
        // how long making that frame took
        let frame_ns = match o.present {
            Presentation::Immediate => (max_fps > 0.0).then(|| (1e9 / max_fps) as u64),
            _ if refresh > 0.0 => Some((1e9 / f64::from(refresh).min(if max_fps > 0.0 { max_fps } else { f64::MAX })) as u64),
            _ => None,
        };
        let (mut last_present_ns, mut last_frame_ns) = (0u64, 0u64);
        // test mode: the frame before the picture is drawn
        let mut pre_shot_drawn = false;
        // a video being written, from the shot on (`--video`)
        let mut video: Option<player::pup::av::VideoWriter> = None;
        // the playfield's box in the picture, when cut to it: x, y, width, height
        let mut crop: Option<[u32; 4]> = None;
        // the video's next picture, µs after the shot
        let mut video_next_us: u64 = 0;
        // which button does what: the settings' `[Input]` mappings, else VPX 5436's defaults; the nudge and plunger sensors the settings name, the devices they read set up
        let mut input_map = player::input_map::InputMap::new(&o.settings);
        let mut setup = setup;
        // the table's own controls its script reads, for the pause menu's Controls page
        let table_extras = player::pause_menu::table_extras(&setup.vpx.gamedata.code.string);
        setup.sensors = player::sensors::Sensors::from_settings(&o.settings, &mut input_map);
        input_map.sensors_read(&setup.sensors.devices());
        let sink = Arc::new(InputSink::new(tx, waker.clone(), input_map).waking_frames(frame_waker.clone()));
        if !input::install(sink.clone()) {
            return Err(format!("watch input: {}", sdl_error()));
        }
        // a cabinet board sending Open Pinball Device reports, read as they come; not in
        // test mode, whose input is scripted
        if o.shot.is_none() {
            player::opd::start(sink.clone());
        }
        // a gamepad's rumble, on unless the settings turn it off (VPX's `RumbleMode`, on by default)
        player::rumble::set_mode(o.settings.enum_or("Player", "RumbleMode", 3, 4));
        // the 256 x 16 pictures a script may switch the colour grade to (VPX's
        // `ColorGradeImage`), by name in lower case, as the scene decoded them
        let grade_images: std::collections::HashMap<String, usize> = setup
            .vpx
            .images
            .iter()
            .enumerate()
            .filter(|(_, im)| im.width == 256 && im.height == 16)
            .filter_map(|(i, im)| Some((im.name.to_lowercase(), scene.table_images.get(i).copied().flatten()?)))
            .collect();
        // the pictures are on the graphics card now: their texels can go, but for those colour grades
        scene.release_pictures(&grade_images.values().copied().collect::<Vec<_>>());
        let shared = Arc::new(Shared::new(waker));
        let game_shared = shared.clone();
        let game = std::thread::Builder::new()
            .name("game".into())
            .spawn(move || game::run(setup, rx, game_shared))
            .map_err(|e| format!("start the game thread: {e}"))?;

        // the depth range covers each visible part's own box, as VPX's does, also as the
        // table's mirrors show them
        let bounds = &scene.depth_bounds;
        let mirror_planes: Vec<[f32; 4]> = scene.mirrors.iter().map(|m| m.plane).collect();

        let mut cam = None;
        let mut cam_size = (0, 0);
        let mut latency = LatencyLog { recent: Vec::new(), last_reported: 0 };
        let (mut frames, mut fps, mut fps_start) = (0u32, 0.0f64, Instant::now());
        let mut transforms = vec![Mat4::IDENTITY; scene.groups.len()];
        let mut lights: Vec<player::scene::LightNow> = scene.light_parts.iter().map(player::scene::LightNow::at_start).collect();
        let mut last_snapshot = game::Snapshot::default();
        let mut applied_grade: Option<String> = None;
        let mut applied_bloom: Option<f32> = None;
        // the pause menu as its picture last showed it, and the window's size then; where its rows
        // are, for the mouse, and the screen's turn and the window's size it was laid out for
        let mut menu_shown: Option<(player::pause_menu::View, (u32, u32))> = None;
        // the achievement's announcement shown on the backglass window now, and the window's size
        let mut toast_shown: Option<(String, (i32, i32))> = None;
        let mut menu_layout: Option<player::pause_menu::Layout> = None;
        // its pictures made on a thread of their own, but in test mode, whose picture has the
        // menu as it is at its moment
        let menu_maker = o.shot.is_none().then(player::pause_menu::Maker::new);
        let mut menu_turn: (u32, (u32, u32)) = (0, (1, 1));
        let mut menu_mouse = MenuMouse::default();
        // Esc opens the pause menu first unless the settings say not, or a front end started the
        // table with VPX's -minimized (as VPX's Esc then ends play at once)
        let esc_opens_menu = !o.minimized && o.settings.get("VPI", "PauseMenuBeforeQuit").and_then(player::settings::vpx_bool).unwrap_or(true);
        // the Controls page: the table's own controls, the game's and the player's, on what now
        // SAFETY: on SDL's thread
        player::pause_menu::set_controls(player::pause_menu::control_lines(&table_extras, &|a| unsafe { sink.names(a) }, esc_opens_menu));
        // the keyboard: play holds while another program's window has it, as VPX's does — but in
        // test mode, and with keys given on the command line, which play without it; a window not
        // given the keyboard as it showed is found out a moment after
        let follow_focus = o.shot.is_none() && o.keys.is_empty();
        // SAFETY: the window is live
        let window_id = unsafe { SDL_GetWindowID(window) };
        let mut focus_lost = false;
        let first_focus_look = Instant::now() + Duration::from_millis(1500);
        let mut focus_looked = false;
        let started = Instant::now();
        let mut total_frames = 0u64;
        'frames: loop {
            // the game asked to end: the script's `QuitPlayer`, or the pause menu's Quit (but in
            // test mode, whose picture is still to be taken)
            if o.shot.is_none() && shared.game_over.load(std::sync::atomic::Ordering::Acquire) {
                break 'frames;
            }
            if let Some(ms) = o.quit_after_ms
                && started.elapsed() >= Duration::from_millis(ms)
            {
                let secs = started.elapsed().as_secs_f64();
                println!("{total_frames} frames in {secs:.1} s: {:.0} fps ({})", total_frames as f64 / secs, renderer.present_mode);
                println!("key to frame: {}", latency.summary());
                if vblank.is_some() {
                    println!("paced: {} of {} frames late, starting {:.2} ms before each refresh", pacer.late, pacer.frames, pacer.lead_ms());
                }
                break 'frames;
            }
            // scripted keys, as if typed
            while let Some(&(at, dik, down)) = pending_keys.peek()
                && started.elapsed() >= Duration::from_millis(at)
            {
                pending_keys.next();
                // SAFETY: SDL's clock
                let time_ns = unsafe { SDL_GetTicksNS() };
                let _ = scripted.send(input::InputEvent { id: 1_000_000 + at, time_ns, dik, down });
                shared.waker.wake();
                frame_waker.wake();
            }

            // SAFETY: event polling on the window's thread
            unsafe {
                let mut event: SDL_Event = std::mem::zeroed();
                while SDL_PollEvent(&mut event) {
                    let kind = event.r#type;
                    if kind == SDL_EVENT_QUIT.0 || kind == SDL_EVENT_WINDOW_CLOSE_REQUESTED.0 {
                        break 'frames;
                    }
                    if follow_focus && (kind == SDL_EVENT_WINDOW_FOCUS_LOST.0 || kind == SDL_EVENT_WINDOW_FOCUS_GAINED.0) && event.window.windowID == window_id {
                        let lost = kind == SDL_EVENT_WINDOW_FOCUS_LOST.0;
                        if lost != focus_lost {
                            focus_lost = lost;
                            tell_focus(&scripted, lost);
                            shared.waker.wake();
                        }
                    }
                    // Esc (or Q) ends play — through the pause menu first, its Quit picked, when the
                    // settings say so — but while the menu shows (or just
                    // closed with it), where Esc steps back (the game thread hears it as a key,
                    // `pause_menu::Nav`) and Q ends play at once
                    if kind == SDL_EVENT_KEY_DOWN.0 && !event.key.repeat {
                        let (esc, q) = (event.key.scancode == SDL_SCANCODE_ESCAPE, event.key.scancode == SDL_SCANCODE_Q);
                        let menu_now = player::pause_menu::shown_lately();
                        if (esc || q) && !menu_now {
                            if esc_opens_menu {
                                let _ = scripted.send(input::InputEvent { id: 0, time_ns: SDL_GetTicksNS(), dik: player::pause_menu::code::EXIT, down: true });
                                shared.waker.wake();
                                frame_waker.wake();
                            } else {
                                break 'frames;
                            }
                        } else if q && menu_now {
                            break 'frames;
                        }
                    }
                    // the pause menu's mouse: on the table's window, while it shows
                    if let Some(layout) = &menu_layout {
                        menu_mouse.event(&event, layout, window, menu_turn);
                        shared.waker.wake();
                    } else {
                        menu_mouse.hover = None;
                    }
                    // a joystick or gamepad connected (or there at the start): opened, its buttons
                    // mapped; unplugged: whatever it held let go
                    if kind == SDL_EVENT_JOYSTICK_ADDED.0 {
                        sink.joystick_added(event.jdevice.which);
                        player::pause_menu::set_controls(player::pause_menu::control_lines(&table_extras, &|a| sink.names(a), esc_opens_menu));
                    } else if kind == SDL_EVENT_JOYSTICK_REMOVED.0 {
                        sink.joystick_removed(event.jdevice.which);
                        player::pause_menu::set_controls(player::pause_menu::control_lines(&table_extras, &|a| sink.names(a), esc_opens_menu));
                    }
                    if let Some(d) = &mut score_drag {
                        d.event(&event);
                    }
                }
                // the latest rumble asked for, on every gamepad
                player::rumble::play_pending();
            }
            // a window that never had the keyboard, found out once, a moment after it showed
            if follow_focus && !focus_looked && Instant::now() >= first_focus_look {
                focus_looked = true;
                // SAFETY: the window is live
                let has = unsafe { (SDL_GetWindowFlags(window) & SDL_WINDOW_INPUT_FOCUS).0 != 0 };
                if !has && !focus_lost {
                    focus_lost = true;
                    tell_focus(&scripted, true);
                    shared.waker.wake();
                }
            }

            // the frame-rate limit: no sooner than its period after the last frame began,
            // unless a key arrives meanwhile, which wakes the wait and gets its frame at once
            if o.shot.is_none()
                && let (Some(period), Some(last)) = (frame_period, last_frame_began)
                && let Some(left) = (last + period).checked_duration_since(Instant::now())
            {
                frame_waker.wait(Some(left));
            }
            let frame_began = Instant::now();

            // paced: sleep until just before the coming refresh, then take the newest state
            let paced = vblank.as_ref().and_then(|v| pacer.wait(v));
            // SAFETY: SDL's clock
            let frame_start = unsafe { SDL_GetTicksNS() };
            match shared.request_snapshot(Duration::from_millis(4)) {
                Some(s) => last_snapshot = s,
                // test mode draws only the game's own frames, each 10 ms of its clock, so the still
                // parts are drawn from its first, whatever the timing
                None if o.shot.is_some() => continue 'frames,
                None => {}
            }
            let s = &last_snapshot;
            // the floating score window goes where it was left for the game
            if let (Some(d), Some(game)) = (&mut score_drag, s.machine_game.as_deref()) {
                d.set_game(game);
            }
            // the colour grade, once the script switches it (VPX's `ColorGradeImage`):
            // a picture of the table's, or none for a name no picture has
            if s.color_grade != applied_grade {
                applied_grade.clone_from(&s.color_grade);
                if let Some(name) = &applied_grade {
                    renderer.set_color_grade(grade_images.get(&name.to_lowercase()).and_then(|&i| scene.images.get(i)))?;
                }
            }
            // the bloom's strength, once the script sets it (VPX's `BloomStrength`)
            if s.bloom_strength != applied_bloom {
                applied_bloom = s.bloom_strength;
                if let Some(strength) = applied_bloom {
                    renderer.set_bloom_strength(strength);
                }
            }
            for (i, group) in scene.groups.iter().enumerate() {
                transforms[i] = s.parts.get(i).copied().flatten().map_or(Mat4::IDENTITY, |state| group.motion.transform(state));
            }

            // a view set on the pause menu's view page: drawn at once and kept for the rest of the
            // play, every part drawn every frame while the page shows (the still parts once more after)
            if let Some(v) = s.menu.as_deref().and_then(|m| m.camera)
                && v != view_setup
            {
                view_setup = v;
                cam = None;
            }
            renderer.set_view_changing(s.menu.as_deref().is_some_and(|m| m.view_page));
            let size = renderer.window_size();
            if cam.is_none() || cam_size != size {
                cam = Some(camera::compute(&view_setup, extent, size.0 as f32 / size.1 as f32, legacy_bounds, bounds, &mirror_planes));
                cam_size = size;
            }
            let c = cam.expect("camera");

            // the text turned with the table on a turned screen, laid out as the player sees it
            let text_turn = (view_setup.rotation(size.0, size.1) / 90.0).round() as u32 % 4;
            let ui = if text_turn % 2 == 1 { (size.1, size.0) } else { size };
            let readout = if o.readout { 4 } else { 0 };
            let mut text = vec![
                Text {
                    x: 12.0,
                    y: 12.0,
                    scale: 2.0,
                    color: [1.0, 0.85, 0.3, 1.0],
                    text: match max_fps {
                        0.0 => format!("Visual Pinball Infinite  {fps:.0} fps  {}", renderer.present_mode),
                        limit => format!("Visual Pinball Infinite  {fps:.0} fps (limit {limit:.0})  {}", renderer.present_mode),
                    },
                },
                Text { x: 12.0, y: 34.0, scale: 2.0, color: [1.0, 1.0, 1.0, 1.0], text: format!("Key to frame: {}", latency.summary()) },
                Text {
                    x: 12.0,
                    y: 56.0,
                    scale: 2.0,
                    color: [0.7, 0.85, 1.0, 1.0],
                    text: format!("Physics: {} steps/s, slowest step {:.0} us, {} balls", s.steps_per_second, s.worst_step_us, s.balls.len()),
                },
                Text {
                    x: 12.0,
                    y: 78.0,
                    scale: 2.0,
                    color: [0.7, 0.7, 0.7, 1.0],
                    text: if s.scripted {
                        "The table's script is running. Esc: quit".into()
                    } else {
                        "No script: Shift flips, Enter plunger, 1 or B ball. Esc: quit".into()
                    },
                },
            ];
            text.truncate(readout);
            // an achievement's announcement: on the backglass window where it shows a backglass or
            // a PinUP pack, as a card; else the playfield's notice
            let announcement = player::achievements::notice();
            let on_backglass = !backglass_window.is_null() && (s.backglass.is_some() || s.pup.as_ref().is_some_and(|p| p.windows[1].is_some()));
            let toast_now = announcement.as_ref().filter(|_| on_backglass).map(|a| {
                let (mut w, mut h) = (0, 0);
                // SAFETY: the window is live
                unsafe { SDL_GetWindowSizeInPixels(backglass_window, &mut w, &mut h) };
                (a.clone(), (w, h))
            });
            if toast_now != toast_shown {
                let picture = toast_now.as_ref().and_then(|(a, (w, h))| player::pause_menu::toast_picture(a, ((*w).max(1) as u32, (*h).max(1) as u32)));
                if let Err(e) = renderer.set_toast(picture.as_ref()) {
                    eprintln!("vpi-player: an achievement's announcement: {e}");
                }
                toast_shown = toast_now;
            }
            // the volume keys' notice, low in the middle of the window, as VPX shows it; a
            // screenshot's and a machine's first start the same way
            if let Some(notice) = focus_lost
                .then(|| player::focus::NOTICE.to_string())
                .or_else(player::volume::notice)
                .or_else(player::screenshot::notice)
                .or_else(player::pinmame::notice)
                .or_else(|| announcement.filter(|_| !on_backglass).map(|a| a.replace(" · ", " - ")))
            {
                // as big as fits the window, up to three times the readout's letters
                let scale = (ui.0 as f32 * 0.92 / (notice.chars().count().max(1) as f32 * 8.0)).clamp(1.0, 3.0);
                let width = notice.chars().count() as f32 * 8.0 * scale;
                text.push(Text { x: ((ui.0 as f32 - width) / 2.0).max(0.0), y: ui.1 as f32 * 0.8, scale, color: [1.0, 1.0, 1.0, 1.0], text: notice });
            }
            // the pause menu's picture, made again when what it shows or the window's size changes
            let menu_same = match (&menu_shown, &s.menu) {
                (Some((was, size)), Some(now)) => **now == *was && *size == ui,
                (None, None) => true,
                _ => false,
            };
            let mut menu_made = None;
            if !menu_same {
                match (&menu_maker, s.menu.as_deref()) {
                    (Some(maker), Some(v)) => maker.make(v.clone(), ui),
                    (Some(maker), None) => {
                        maker.cancel();
                        menu_made = Some(None);
                    }
                    (None, v) => menu_made = Some(v.and_then(|v| player::pause_menu::picture(v, ui))),
                }
                menu_shown = s.menu.as_deref().map(|v| (v.clone(), ui));
            }
            // a picture made since: shown from this frame, the mouse finding its rows
            if let Some(made) = menu_made.or_else(|| menu_maker.as_ref().and_then(player::pause_menu::Maker::take)) {
                if let Err(e) = renderer.set_menu(made.as_ref().map(|(p, _)| p)) {
                    eprintln!("vpi-player: the pause menu: {e}");
                }
                menu_layout = made.map(|(_, layout)| layout);
            }
            menu_turn = (text_turn, size);
            // the lights as the script left them
            if !s.part_render.is_empty() {
                for (now, l) in lights.iter_mut().zip(&scene.light_parts) {
                    if let Some(r) = s.part_render.get(l.part as usize) {
                        let live = r.light_live.unwrap_or_else(|| player::scene::LightLive::of(l));
                        *now = player::scene::LightNow::from_game(r.light, r.light_color, r.light_tint, live);
                    }
                }
            }
            // VPX's nudge shake: the picture moves with the swinging table
            let shake = nudge_shake(s.nudge_offset, view_setup.rotation(size.0, size.1), nudge_strength);
            let shake_matrix = Mat4::translate(shake[0], shake[1], 0.0);
            // the balls' last-moment move, but in test mode, whose pictures repeat exactly
            let moved: Vec<player::render::BallDraw>;
            let balls: &[player::render::BallDraw] = if o.shot.is_none() {
                // SAFETY: SDL's clock
                let now = unsafe { SDL_GetTicksNS() };
                let delay_s = now.saturating_sub(s.physics_at_ns) as f32 * 1e-9 + predicted_display_delay(now, last_present_ns, frame_ns, last_frame_ns) + 0.001;
                let k = o.ball_move.scale(delay_s);
                moved = s
                    .balls
                    .iter()
                    .map(|b| {
                        let (p, v) = (b.position, b.velocity);
                        player::render::BallDraw { position: Vec3::new(p.x + v.x * k, p.y + v.y * k, p.z + v.z * k), ..*b }
                    })
                    .collect();
                &moved
            } else {
                &s.balls
            };
            let input = FrameInput {
                view_proj: c.view * c.projection * shake_matrix,
                view_rotation: view_setup.rotation(size.0, size.1),
                unshaken: c.view * c.projection,
                view: c.view,
                projection: c.projection,
                shake: shake_matrix,
                eye: c.eye,
                group_transforms: &transforms,
                balls,
                lights: &lights,
                part_render: &s.part_render,
                reels: &s.reels,
                text_boxes: &s.text_boxes,
                screen_offset: shake,
                text: &text,
                text_turn,
                dmd: s.dmd.as_deref(),
                segments: s.segments.as_deref(),
                display_hidden: s.display_hidden,
                backglass: s.backglass.as_ref(),
                pup: s.pup.as_ref(),
            };
            if let Some(path) = &o.shot {
                // the frame before the picture, drawn off screen, as VPX draws frames before its own
                // pictures: a moving ball's motion blur and its mirror of the frame before come from it
                if !pre_shot_drawn && s.sim_time_us > 0 && s.sim_time_us + 10_000 >= o.shot_at_ms * 1000 && s.sim_time_us < o.shot_at_ms * 1000 {
                    let (w, h) = (o.size.0 as u32, o.size.1 as u32);
                    let c = camera::compute(&view_setup, extent, w as f32 / h as f32, legacy_bounds, bounds, &mirror_planes);
                    let input = FrameInput { view_proj: c.view * c.projection * shake_matrix, unshaken: c.view * c.projection, view: c.view, projection: c.projection, eye: c.eye, ..input };
                    // numbered by the game's clock (a frame each 10 ms), started afresh, however many
                    // frames came before
                    renderer.start_afresh();
                    renderer.set_frame_number((o.shot_at_ms / 10).saturating_sub(1) as u32);
                    renderer.capture(&input, w, h)?;
                    pre_shot_drawn = true;
                }
                // the game's clock stops at the shot (GameSetup::stop_at_ms): the picture shows
                // that moment, the same on every run
                if s.sim_time_us >= o.shot_at_ms * 1000 {
                    let (w, h) = (o.size.0 as u32, o.size.1 as u32);
                    let c = camera::compute(&view_setup, extent, w as f32 / h as f32, legacy_bounds, bounds, &mirror_planes);
                    let input = FrameInput { view_proj: c.view * c.projection * shake_matrix, unshaken: c.view * c.projection, view: c.view, projection: c.projection, eye: c.eye, ..input };
                    // a video's pictures after the shot (`--video`): one every 40 ms of the game's
                    // clock, 25 a second, each frame waited for (GameSetup::record_from_ms)
                    if let Some(writer) = video.as_mut() {
                        const VIDEO_FRAME_US: u64 = 40_000;
                        let since = s.sim_time_us - o.shot_at_ms * 1000;
                        // the frame at or just past each picture's time (the physics' steps can
                        // leave a frame's clock a little off the grid)
                        if since >= video_next_us {
                            renderer.set_frame_number((s.sim_time_us / 10_000) as u32);
                            let rgba = renderer.capture(&input, w, h)?;
                            match crop {
                                Some(b) => writer.frame(&cut(&rgba, w, b))?,
                                None => writer.frame(&rgba)?,
                            }
                            while video_next_us <= since {
                                video_next_us += VIDEO_FRAME_US;
                            }
                        }
                        if since >= o.video_seconds * 1_000_000 {
                            let frames = video.as_ref().map_or(0, |w| w.frames);
                            let mut with_sound = String::new();
                            if let Some(mut writer) = video.take() {
                                // its sound (`--video-sound`), as long as its pictures (made up with
                                // silence if short), whole, so it's set to one loudness first:
                                // every table's alike
                                if let Some(rec) = recorder.take() {
                                    let mut sound = rec.finish();
                                    let due = u64::from(writer.frames) * u64::from(player::audio::OUTPUT_RATE as u32) * VIDEO_FRAME_US / 1_000_000;
                                    sound.resize(due as usize * 2, 0.0);
                                    with_sound = match player::audio::even_out(&mut sound) {
                                        Some((loudness, gain)) => format!(", with its sound ({loudness:.1} LUFS as recorded, {gain:+.1} dB)"),
                                        None => ", with its sound (silent)".into(),
                                    };
                                    writer.sound(&sound)?;
                                }
                                writer.finish()?;
                            }
                            println!("saved {}: {frames} pictures, {} a second{with_sound}", o.video.as_deref().unwrap_or(Path::new("")).display(), 1_000_000 / VIDEO_FRAME_US);
                            break 'frames;
                        }
                        continue;
                    }
                    renderer.set_frame_number((o.shot_at_ms / 10) as u32);
                    let rgba = renderer.capture(&input, w, h)?;
                    // cut to the playfield as it's seen (`--crop playfield`), the video's frames too
                    crop = o.crop_playfield.then(|| playfield_box(&input.view_proj, extent, w, h));
                    let (rgba, w, h) = match crop {
                        Some(b) => (cut(&rgba, w, b), b[2], b[3]),
                        None => (rgba, w, h),
                    };
                    image::save_buffer(path, &rgba, w, h, image::ExtendedColorType::Rgba8).map_err(|e| format!("save {}: {e}", path.display()))?;
                    println!("saved {} at {} ms: {} balls", path.display(), o.shot_at_ms, s.balls.len());
                    // the score display's window beside it, as <name>-dmd.png, when the table has
                    // a display to show there
                    if let Some([_, _, w, h]) = layout.score
                        && !dmd_window.is_null()
                        && (s.dmd.is_some() || s.segments.is_some() || s.pup.is_some())
                    {
                        let stem = path.file_stem().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        let dmd_path = path.with_file_name(format!("{stem}-dmd.png"));
                        let rgba = renderer.capture_display(s.dmd.as_deref(), s.segments.as_deref(), s.backglass.as_ref(), s.pup.as_ref(), w as u32, h as u32)?;
                        image::save_buffer(&dmd_path, &rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8).map_err(|e| format!("save {}: {e}", dmd_path.display()))?;
                        println!("saved {}", dmd_path.display());
                    }
                    // and the backglass's, as <name>-backglass.png, once its file is read or a
                    // PinUP pack shows there
                    if let Some([_, _, w, h]) = layout.backglass
                        && !backglass_window.is_null()
                        && (s.backglass.is_some() || s.pup.as_ref().is_some_and(|p| p.windows[1].is_some()))
                    {
                        let stem = path.file_stem().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        let bg_path = path.with_file_name(format!("{stem}-backglass.png"));
                        let rgba = renderer.capture_backglass_window(s.backglass.as_ref(), s.pup.as_ref(), s.dmd.as_deref(), w as u32, h as u32, true)?;
                        image::save_buffer(&bg_path, &rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8).map_err(|e| format!("save {}: {e}", bg_path.display()))?;
                        println!("saved {}", bg_path.display());
                    }
                    // and the topper's, as <name>-topper.png, once a PinUP pack shows there
                    if let Some([_, _, w, h]) = layout.topper
                        && !topper_window.is_null()
                        && let Some(rgba) = renderer.capture_topper_window(s.pup.as_ref(), w as u32, h as u32)?
                    {
                        let stem = path.file_stem().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        let topper_path = path.with_file_name(format!("{stem}-topper.png"));
                        image::save_buffer(&topper_path, &rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8).map_err(|e| format!("save {}: {e}", topper_path.display()))?;
                        println!("saved {}", topper_path.display());
                    }
                    // a video from here on, the picture its first
                    if let Some(video_path) = &o.video {
                        let mut writer = player::pup::av::VideoWriter::create(video_path, w, h, 25, recorder.as_ref().map(|_| player::audio::OUTPUT_RATE as u32))?;
                        writer.frame(&rgba)?;
                        // its sound from here on, kept as it plays (`--video-sound`)
                        if let Some(rec) = &recorder {
                            rec.record(o.video_seconds as u32 + 5);
                        }
                        video = Some(writer);
                        video_next_us = 40_000;
                        continue;
                    }
                    break 'frames;
                }
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            // a screenshot asked for (`screenshot.rs`): each window the settings name, as it shows
            // now, kept on a thread of its own
            if player::screenshot::take() {
                let wanted = player::screenshot::screens(&o.settings);
                let wants = |w: &str| wanted.as_ref().is_none_or(|l| l.iter().any(|x| x == w));
                let mut pictures: Vec<(&'static str, Vec<u8>, u32, u32)> = Vec::new();
                if wants("playfield") {
                    // the table as it shows, without the player's own readout and notices
                    match renderer.capture(&FrameInput { text: &[], ..input }, size.0, size.1) {
                        Ok(rgba) => pictures.push(("playfield", rgba, size.0, size.1)),
                        Err(e) => eprintln!("screenshot: {e}"),
                    }
                }
                if wants("backglass")
                    && let Some([_, _, w, h]) = layout.backglass
                    && !backglass_window.is_null()
                    && (s.backglass.is_some() || s.pup.as_ref().is_some_and(|p| p.windows[1].is_some()))
                    && let Ok(rgba) = renderer.capture_backglass_window(s.backglass.as_ref(), s.pup.as_ref(), s.dmd.as_deref(), w as u32, h as u32, false)
                {
                    pictures.push(("backglass", rgba, w as u32, h as u32));
                }
                if wants("dmd")
                    && let Some([_, _, w, h]) = layout.score
                    && !dmd_window.is_null()
                    && (s.dmd.is_some() || s.segments.is_some() || s.pup.is_some())
                    && let Ok(rgba) = renderer.capture_display(s.dmd.as_deref(), s.segments.as_deref(), s.backglass.as_ref(), s.pup.as_ref(), w as u32, h as u32)
                {
                    pictures.push(("score display", rgba, w as u32, h as u32));
                }
                if wants("topper")
                    && let Some([_, _, w, h]) = layout.topper
                    && !topper_window.is_null()
                    && let Ok(Some(rgba)) = renderer.capture_topper_window(s.pup.as_ref(), w as u32, h as u32)
                {
                    pictures.push(("topper", rgba, w as u32, h as u32));
                }
                player::screenshot::save(&o.table, pictures);
            }
            match renderer.frame(&input)? {
                FrameResult::Submitted { .. } => {
                    last_frame_began = Some(frame_began);
                    // SAFETY: SDL's clock
                    let submitted = unsafe { SDL_GetTicksNS() };
                    (last_present_ns, last_frame_ns) = (submitted, submitted.saturating_sub(frame_start));
                    if let Some(ev) = s.newest_input
                        && ev.id > latency.last_reported
                    {
                        latency.last_reported = ev.id;
                        if ev.down && matches!(ev.dik, 0x2A | 0x36) {
                            latency.add(submitted.saturating_sub(ev.time_ns) as f64 / 1e6);
                        }
                    }
                    frames += 1;
                    total_frames += 1;
                    if let Some((target, period)) = paced {
                        pacer.done(frame_start, submitted, target, period);
                    }
                }
                // the graphics card is busy: wait briefly, waking for any event
                FrameResult::Skipped => unsafe {
                    SDL_WaitEventTimeout(std::ptr::null_mut(), 1);
                },
            }
            let elapsed = fps_start.elapsed().as_secs_f64();
            if elapsed >= 0.5 {
                fps = f64::from(frames) / elapsed;
                (frames, fps_start) = (0, Instant::now());
            }
        }
        shared.stop();
        let _ = game.join();
        Ok(())
    })();
    // SAFETY: the renderer released the windows above
    unsafe {
        for w in [dmd_window, backglass_window, topper_window] {
            if !w.is_null() {
                SDL_DestroyWindow(w);
            }
        }
        SDL_DestroyWindow(window);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_display_delay_counts_to_the_next_frame_the_screen_takes() {
        // 4 ms frames, the last shown 1 ms ago: 3 ms to the next
        assert!((predicted_display_delay(11_000_000, 10_000_000, Some(4_000_000), 0) - 0.003).abs() < 1e-7);
        // a frame later when making the last one took longer than that
        assert!((predicted_display_delay(11_000_000, 10_000_000, Some(4_000_000), 3_500_000) - 0.007).abs() < 1e-7);
        // frames as fast as they can come: no guess
        assert_eq!(predicted_display_delay(11_000_000, 10_000_000, None, 0), 0.0);
    }

    #[test]
    fn vpx_moves_the_ball_a_hundredth_of_the_way_it_means_to() {
        // 5 ms against a velocity per 10 ms: VPX 5436 takes 0.005 of a step, as meant half a step
        assert_eq!(BallMove::Vpx.scale(0.005), 0.005);
        assert!((BallMove::Intended.scale(0.005) - 0.5).abs() < 1e-6);
    }
}
