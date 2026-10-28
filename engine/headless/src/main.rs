//! `vpi-headless`: runs tables' physics without graphics, for automated tests
//! and to time the physics.
//!
//! ```text
//! vpi-headless <table.vpx>... [--ms 3000] [--balls 5] [--no-flippers] [--repeat 3]
//! vpi-headless <table.vpx>... --script [--ms 20000] [--keys 1000:5:d,1100:5:u,...] [--stick ms:x:y,...] [--axes ms:device:axis:value,...] [--fps 60] [--realtime] [--trace] [--dmd ms,...] [--pup] [--altsound] [--colour] [--dof <folder>] [--toys] [--options] [--dmd-panel <folder>] [--dmd-device <DmdDevice64.dll> [--dmd-device-config <DmdDevice.ini>]]
//! vpi-headless --replay game.vpireplay [--repeat 2]
//! ```
//!
//! `--replay` plays back a game the player recorded (`vpi-player --record`):
//! the same keys at the same physics steps, the same frames, in simulated
//! time. It prints the fingerprint of every ball state; repeats must agree,
//! unless the table's script reads the clock (`Randomize`, `Timer`, `Now`).
//!
//! With `--script`, each table runs as the player runs it, with its own
//! script, but in simulated time and without a window: scripted key presses
//! (by default a coin, start, the plunger, then both flippers each second),
//! a frame prepared `--fps` times a second, and every step timed with the
//! script inside it. It prints the time per step, the balls left, and how many
//! script errors and unsupported members the script met (each named above
//! the table's line). Scripts can read the clock, so the fingerprint may
//! vary between runs. `--realtime` holds each step to the wall clock, for
//! tables whose ROM runs in PinMAME, which runs at the real machine's pace;
//! `--trace` prints every switch the script sets in PinMAME and every
//! solenoid change it receives, and `--dmd` saves the ROM's DMD at those
//! moments (ms) in `results\headless`; `--altsound` plays a game's AltSound pack (off by
//! default: a pack without its settings file gets one written); `--colour` colours a game's display
//! with its colour file (off by default: checks start no more programs than they need), `--dmd`
//! then saving the coloured frame too; `--pup` prints the PinUP pack's state
//! at the end: each screen, what it holds and what it plays. `--stick` moves a
//! gamepad's left stick, `--axes` any device's axes, as the settings file
//! `VPI_SETTINGS` names maps them to nudge and plunger sensors (`[Input]`
//! `NudgeSensorCount`, `PlungerSensorCount`, `Mapping.Nudge<n>.…`, VPX
//! 5436's names): a cabinet's analog input, checked without its hardware.
//! `--menu` gives the run the player's pause menu, kept nowhere: the keys
//! `pause`, `ui-up`, `ui-down`, `ui-left`, `ui-right` and `esc` drive it, the
//! game paused while it shows as in play, and the physics steps done are
//! printed at the end (`menu: ...`).
//!
//! Each table is set up for play as VPX's player sets it up, the balls start
//! at rest along the top of the playfield, every flipper is pressed from 400
//! to 600 ms of each second, and the physics runs in 1 ms steps; no script
//! runs. Each table runs once to warm up, then `--repeat` times measured, on a
//! high-priority thread. It prints the physics time per step and a fingerprint
//! of every ball's state at every step: the same table and options must give
//! the same fingerprint on every run and every machine.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use physics_compat::engine::{NoScript, Shape};
use physics_compat::env::BallId;
use physics_compat::fpmode::VpxFloatMode;
use physics_compat::hitball::HitBall;
use physics_compat::math::Vertex3Ds;
use player::game::{Game, GameSetup};
use player::input::{InputEvent, parse_key_script};
use table_physics::build;
use table_physics::settings::PhysicsSettings;

struct Options {
    tables: Vec<PathBuf>,
    ms: Option<u64>,
    balls: u32,
    flippers: bool,
    repeat: u32,
    script: bool,
    keys: Option<Vec<(u64, u8, bool)>>,
    fps: u64,
    replay: Option<PathBuf>,
    realtime: bool,
    trace: bool,
    dmd: Vec<u64>,
    /// `--pup`: the PinUP pack's state at the end.
    pup: bool,
    /// `--altsound`: a game's AltSound pack plays (off in checks: libaltsound writes a settings
    /// file into a pack that has none, and checks never write into an install).
    altsound: bool,
    /// `--colour`: a game's colour file colours its display.
    colour: bool,
    /// `--dof <folder>`: the cabinet's toys run (`vpi-dof`) with the configuration in the
    /// folder's `directoutputconfig`; off otherwise.
    dof: Option<PathBuf>,
    /// `--toys`: what the toys were told, counted and printed at the end (`toys: ...`), with no
    /// cabinet or configuration needed (the dashboard's toys check).
    toys: bool,
    /// `--menu`: the player's pause menu, its changes kept nowhere (`pause_menu`).
    menu: bool,
    /// `--options`: the options the table's script offers (`Table.Option`), printed at the end as
    /// one line of JSON (`options: [...]`), for the app's table settings; and, for a table driving
    /// PinMAME, its game and the settings its script set for it (`machine: {...}`).
    options: bool,
    /// `--dmd-panel <folder>`: a real DMD panel's frames handed to libdmdutil (`vpi-dmd`)
    /// with its virtual panel, whose pictures go to the folder at the `--dmd` times: what a
    /// panel would show, checked with no panel (the settings `VPI_SETTINGS` names may add
    /// `[Plugin.DMDUtil]`'s, a panel's colour among them).
    dmd_panel: Option<PathBuf>,
    /// `--dmd-device <DmdDevice64.dll>` and `--dmd-device-config <DmdDevice.ini>`: a VPinMAME
    /// display device driven as a real panel would be (`vpi-dmddevice`), its settings file
    /// named — for checks, one in a scratch folder (DMD Extensions' video output, say).
    dmd_device: Option<player::dmd_device::Setup>,
    /// `--stick ms:x:y,...`: a gamepad's left stick, as the player reads it, moved to `x`, `y`
    /// (-1 to 1; up negative) at each time: the stick's nudge, its tilt bob and the tilt it tells
    /// the table, checked without a controller.
    stick: Option<Vec<(u64, f32, f32)>>,
    /// `--axes ms:device:axis:value,...`: a device's axis (the device by its name in the
    /// settings, the axis as VPX numbers it, 512 for the first) read at `value` (-1 to 1) at each
    /// time, for the nudge and plunger sensors the settings `VPI_SETTINGS` names set up.
    axes: Option<Vec<(u64, String, u16, f32)>>,
}

const USAGE: &str = "usage: vpi-headless <table.vpx>... [--ms 3000] [--balls 5] [--no-flippers] [--repeat 3]\n       vpi-headless <table.vpx>... --script [--ms 20000] [--keys 1000:5:d,1100:5:u,...] [--stick ms:x:y,...] [--axes ms:device:axis:value,...] [--fps 60] [--realtime] [--trace] [--dmd ms,...] [--pup] [--altsound] [--colour] [--dof <folder>] [--toys] [--dmd-panel <folder>] [--dmd-device <DmdDevice64.dll> [--dmd-device-config <DmdDevice.ini>]]\n       vpi-headless --replay game.vpireplay [--repeat 2]";

fn parse_args() -> Result<Options, String> {
    let mut o = Options {
        tables: Vec::new(),
        ms: None,
        balls: 5,
        flippers: true,
        repeat: 3,
        script: false,
        keys: None,
        fps: 60,
        replay: None,
        realtime: false,
        trace: false,
        dmd: Vec::new(),
        pup: false,
        altsound: false,
        colour: false,
        dof: None,
        toys: false,
        menu: false,
        options: false,
        dmd_panel: None,
        dmd_device: None,
        stick: None,
        axes: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut number = |what: &str| -> Result<u64, String> {
            args.next().and_then(|v| v.parse().ok()).ok_or_else(|| format!("{what} needs a number"))
        };
        match a.as_str() {
            "--ms" => o.ms = Some(number("--ms")?),
            "--fps" => o.fps = number("--fps")?.max(1),
            "--replay" => o.replay = Some(args.next().map(PathBuf::from).ok_or("--replay takes a file name")?),
            "--script" => o.script = true,
            "--realtime" => o.realtime = true,
            "--trace" => o.trace = true,
            "--pup" => o.pup = true,
            "--altsound" => o.altsound = true,
            "--colour" => o.colour = true,
            "--toys" => o.toys = true,
            "--menu" => o.menu = true,
            "--options" => o.options = true,
            "--dof" => o.dof = Some(PathBuf::from(args.next().ok_or("--dof takes the folder holding directoutputconfig")?)),
            "--dmd-panel" => o.dmd_panel = Some(PathBuf::from(args.next().ok_or("--dmd-panel takes the folder for the panel's pictures")?)),
            "--dmd-device" => {
                let library = PathBuf::from(args.next().ok_or("--dmd-device takes a display device's DLL")?);
                o.dmd_device = Some(player::dmd_device::Setup { library, config: o.dmd_device.take().and_then(|d| d.config) });
            }
            "--dmd-device-config" => {
                let config = PathBuf::from(args.next().ok_or("--dmd-device-config takes the device's settings file")?);
                match &mut o.dmd_device {
                    Some(d) => d.config = Some(config),
                    None => return Err("--dmd-device-config goes after --dmd-device".into()),
                }
            }
            // VPX 10.8.0's counting of nudges toward a tilt, in the bob's place (read where the game is set up)
            "--tilt-counts" => {}
            "--dmd" => o.dmd = args.next().map(|s| s.split(',').filter_map(|t| t.parse().ok()).collect()).ok_or("--dmd takes ms,ms,...")?,
            "--keys" => o.keys = Some(args.next().as_deref().and_then(parse_key_script).ok_or("--keys takes ms:key:d|u,...")?),
            "--stick" => o.stick = Some(args.next().as_deref().and_then(parse_stick_script).ok_or("--stick takes ms:x:y,...")?),
            "--axes" => o.axes = Some(args.next().as_deref().and_then(parse_axes_script).ok_or("--axes takes ms:device:axis:value,...")?),
            "--balls" => o.balls = number("--balls")? as u32,
            "--repeat" => o.repeat = number("--repeat")?.max(1) as u32,
            "--no-flippers" => o.flippers = false,
            "-h" | "--help" => return Err(USAGE.to_string()),
            s if s.starts_with("--") => return Err(format!("unknown option {s}\n{USAGE}")),
            s => o.tables.push(PathBuf::from(s)),
        }
    }
    if o.tables.is_empty() && o.replay.is_none() {
        return Err(USAGE.to_string());
    }
    Ok(o)
}

/// FNV-1a, over the bits of every state.
struct Fingerprint(u64);

impl Fingerprint {
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ u64::from(x)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn v3(&mut self, v: &Vertex3Ds) {
        for f in [v.x, v.y, v.z] {
            self.bytes(&f.to_bits().to_le_bytes());
        }
    }
}

/// One run: each step's physics time, and the fingerprint.
struct Run {
    steps: Vec<Duration>,
    fingerprint: u64,
}

fn run(vpx: &vpin::vpx::VPX, o: &Options) -> Run {
    let tp = build(vpx, &PhysicsSettings::default());
    let t = tp.table;
    let mut engine = tp.into_engine();
    engine.start_physics(0);

    // the balls, at rest along the top of the playfield
    for k in 0..o.balls {
        let mut hb = HitBall::new(BallId(k));
        let x = t.left + (t.right - t.left) * (k + 1) as f32 / (o.balls + 1) as f32;
        let y = t.top + (t.bottom - t.top) * 0.15;
        hb.d.pos = Vertex3Ds::new(x, y, 25.0);
        hb.d.radius = 25.0;
        hb.d.mass = 1.0;
        engine.add_ball(hb);
    }
    let flippers: Vec<u32> = (0..engine.colliders().len() as u32)
        .filter(|&i| matches!(engine.colliders()[i as usize].shape, Shape::Flipper(_)))
        .collect();

    let ms = o.ms.unwrap_or(3000);
    let mut steps = Vec::with_capacity(ms as usize);
    let mut fp = Fingerprint(0xcbf2_9ce4_8422_2325);
    for step in 1..=ms {
        if o.flippers && (step % 1000 == 400 || step % 1000 == 600) {
            for &k in &flippers {
                if let Shape::Flipper(f) = &mut engine.collider_mut(k).shape {
                    if step % 1000 == 400 { f.rotate_to_end() } else { f.rotate_to_start() }
                }
            }
        }
        let t0 = Instant::now();
        engine.update_physics(engine.start_time_usec() + step * 1000 + 1, &mut NoScript);
        steps.push(t0.elapsed());

        fp.bytes(&engine.env.rng.state.to_le_bytes());
        for b in engine.balls() {
            fp.v3(&b.d.pos);
            fp.v3(&b.d.vel);
            fp.v3(&b.angularmomentum);
        }
    }
    Run { steps, fingerprint: fp.0 }
}

/// One run with the table's script.
struct Scripted {
    steps: Vec<Duration>,
    fingerprint: u64,
    balls: usize,
    errors: u32,
    unsupported: u32,
}

/// A scripted stick's axes: a gamepad's left stick, as VPX's default layout for a gamepad maps
/// it, on a device of its own.
const STICK_X: player::sensors::Axis = player::sensors::Axis { device: 0xFFFE, element: player::input_map::AXIS, kind: player::sensors::Kind::Position, mapping: player::sensors::AxisMapping::VPX_DEFAULT };
const STICK_Y: player::sensors::Axis = player::sensors::Axis { element: player::input_map::AXIS | 1, ..STICK_X };

/// A scripted stick: `ms:x:y` items separated by commas, as (ms, x, y).
fn parse_stick_script(list: &str) -> Option<Vec<(u64, f32, f32)>> {
    list.split(',')
        .map(|item| {
            let mut f = item.split(':');
            Some((f.next()?.parse().ok()?, f.next()?.parse().ok()?, f.next()?.parse().ok()?))
        })
        .collect()
}

/// Scripted axes: `ms:device:axis:value` items separated by commas.
fn parse_axes_script(list: &str) -> Option<Vec<(u64, String, u16, f32)>> {
    list.split(',')
        .map(|item| {
            let f: Vec<&str> = item.split(':').collect();
            (f.len() == 4).then_some(())?;
            Some((f[0].parse().ok()?, f[1].to_string(), f[2].parse().ok()?, f[3].parse().ok()?))
        })
        .collect()
}

/// A game's worth of keys: a coin, start, the plunger pulled for a second,
/// then both flippers pressed from 400 to 600 ms of each second.
fn default_keys(ms: u64) -> Vec<(u64, u8, bool)> {
    const COIN: u8 = 0x06; // 5
    const START: u8 = 0x02; // 1
    const PLUNGER: u8 = 0x1C; // Enter
    const LEFT: u8 = 0x2A; // left Shift
    const RIGHT: u8 = 0x36; // right Shift
    let mut keys = vec![(1000, COIN, true), (1100, COIN, false), (2000, START, true), (2100, START, false), (4000, PLUNGER, true), (5000, PLUNGER, false)];
    for second in 6..ms.div_ceil(1000) {
        for (at, down) in [(400, true), (600, false)] {
            keys.push((second * 1000 + at, LEFT, down));
            keys.push((second * 1000 + at, RIGHT, down));
        }
    }
    keys
}

/// `--dmd`: the ROM's DMD at this moment, four pixels a dot, in
/// `results\headless` (git-ignored).
fn save_dmd(game: &mut Game, table: &Path, ms: u64) {
    const SCALE: u32 = 4;
    // the machine's own DMD; a segment game's digits as AlphaDMD draws them for its colour file
    let own = game.dmd().or_else(|| game.alpha_dmd());
    if own.is_none() && game.shown_dmd().is_none_or(|f| f.luminance) {
        eprintln!("no DMD to save at {ms} ms");
        return;
    }
    let stem = table.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let dir = PathBuf::from("results").join("headless");
    let save = |path: PathBuf, saved: Result<(), image::ImageError>| {
        if let Err(e) = saved {
            eprintln!("could not save {}: {e}", path.display());
        }
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("could not make {}: {e}", dir.display());
        return;
    }
    if let Some((w, h, dots)) = &own {
        let (w, h) = (*w as u32, *h as u32);
        let picture = image::GrayImage::from_fn(w * SCALE, h * SCALE, |x, y| image::Luma([dots.get(((y / SCALE) * w + x / SCALE) as usize).copied().unwrap_or(0)]));
        let path = dir.join(format!("{stem} at {ms} ms.png"));
        save(path.clone(), picture.save(&path));
    }
    // what the display shows, when a colour file coloured it (`--colour`)
    if let Some(f) = game.shown_dmd().filter(|f| !f.luminance) {
        let srgb = |c: f32| (255.0 * if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }).round().clamp(0.0, 255.0) as u8;
        let picture = image::RgbImage::from_fn(f.width * SCALE, f.height * SCALE, |x, y| {
            let d = f.dots.get(((y / SCALE) * f.width + x / SCALE) as usize).copied().unwrap_or([0.0; 4]);
            image::Rgb([srgb(d[0]), srgb(d[1]), srgb(d[2])])
        });
        let path = dir.join(format!("{stem} at {ms} ms, coloured.png"));
        save(path.clone(), picture.save(&path));
    } else if let Some(f) = game.shown_dmd().filter(|f| own.as_ref().is_some_and(|(w, h, _)| (f.width, f.height) != (*w as u32, *h as u32))) {
        // the display upscaled (the settings file `VPI_SETTINGS` names asking for VPX's DMD
        // upscaler), its brightness as the machine's picture shows it
        let picture = image::GrayImage::from_fn(f.width * SCALE, f.height * SCALE, |x, y| {
            let d = f.dots.get(((y / SCALE) * f.width + x / SCALE) as usize).copied().unwrap_or([0.0; 4]);
            image::Luma([(d[0] * 255.0).round().clamp(0.0, 255.0) as u8])
        });
        let path = dir.join(format!("{stem} at {ms} ms, upscaled.png"));
        save(path.clone(), picture.save(&path));
    }
}

/// The table as the player runs it, script and all, in simulated time: one
/// 1 ms step at a time, keys reaching the script before the step they fall
/// in, and a frame prepared `fps` times a second.
fn run_scripted(path: &Path, vpx: vpin::vpx::VPX, o: &Options) -> Scripted {
    player::script::log::reset();
    let ms = o.ms.unwrap_or(20_000);
    // a scripted stick is a gamepad's: the player's cabinet input, its left stick mapped
    let mut stick = o.stick.clone().unwrap_or_default();
    stick.sort_by_key(|s| s.0);
    if o.stick.is_some() {
        player::sensors::add_layout(Some([STICK_X, STICK_Y]), None);
    }
    // the settings' nudge and plunger sensors, and the scripted axes on their devices
    let settings = std::env::var_os("VPI_SETTINGS").and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(player::settings::Settings::default, |t| player::settings::Settings::from_text(&t));
    // a ROM machine's high scores watched as in play, when the settings take a screenshot then
    player::screenshot::set_at_high_score(settings.get("VPI", "ScreenshotAtHighScore").and_then(player::settings::vpx_bool).unwrap_or(false));
    let mut input_map = player::input_map::InputMap::new(&settings);
    let sensors = player::sensors::Sensors::from_settings(&settings, &mut input_map);
    let mut axes: Vec<(u64, u16, u16, f32)> = o.axes.iter().flatten().map(|(at, device, axis, value)| (*at, input_map.device(device), *axis, *value)).collect();
    axes.sort_by_key(|a| a.0);
    let cabinet_input = o.stick.is_some() || o.axes.is_some();
    let mut game = game_for(path, vpx, o.altsound, o.colour, cabinet_input, sensors, o.dof.as_deref(), o.toys, o.dmd_panel.is_some());
    if let Some(device) = &o.dmd_device {
        game.set_dmd_device(device.clone());
    }
    if let Some(folder) = &o.dmd_panel {
        let _ = std::fs::create_dir_all(folder);
        game.set_dmd_panel_check(folder.clone(), vpi_settings_section("Plugin.DMDUtil"));
    }
    if o.menu {
        game.set_menu(Some(player::pause_menu::Setup { title: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), ..Default::default() }));
    }
    game.start(0);

    let mut keys = o.keys.clone().unwrap_or_else(|| default_keys(ms));
    keys.sort_by_key(|k| k.0);
    let (mut next_stick, mut next_axis) = (0, 0);
    let frame_us = 1_000_000 / o.fps;
    let (mut next_frame, mut next_key) = (frame_us, 0);
    let mut steps = Vec::with_capacity(ms as usize);
    let mut fp = Fingerprint(0xcbf2_9ce4_8422_2325);
    let began = Instant::now();
    for step in 1..=ms {
        let t = step * 1000;
        if o.realtime {
            let due = Duration::from_millis(step);
            if let Some(wait) = due.checked_sub(began.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        while let Some(&(at, dik, down)) = keys.get(next_key)
            && at * 1000 < t
        {
            game.input(InputEvent { id: next_key as u64 + 1, time_ns: at * 1_000_000, dik, down });
            next_key += 1;
        }
        while let Some(&(at, x, y)) = stick.get(next_stick)
            && at * 1000 < t
        {
            player::sensors::set_axis(STICK_X.device, STICK_X.element, at * 1_000_000, x);
            player::sensors::set_axis(STICK_Y.device, STICK_Y.element, at * 1_000_000, y);
            next_stick += 1;
        }
        while let Some(&(at, device, axis, value)) = axes.get(next_axis)
            && at * 1000 < t
        {
            player::sensors::set_axis(device, axis, at * 1_000_000, value);
            next_axis += 1;
        }
        let t0 = Instant::now();
        // the pause menu's held presses, and play going on after a pause; no steps while paused
        game.menu_tick();
        game.resume_if_due(t);
        if !game.paused() {
            game.advance(t + 1);
        }
        if t >= next_frame {
            game.prepare_frame();
            next_frame += frame_us;
        }
        steps.push(t0.elapsed());
        for b in game.engine().balls() {
            fp.v3(&b.d.pos);
            fp.v3(&b.d.vel);
        }
        if o.dmd.contains(&step) {
            save_dmd(&mut game, path, step);
            if o.dmd_panel.is_some() {
                game.panel_picture(step as u32);
            }
        }
        if game.quit_requested() {
            break;
        }
    }
    if o.pup {
        print!("{}", game.pup_summary().unwrap_or_else(|| "PinUP: the script didn't ask for it
".to_string()));
    }
    if o.toys {
        eprintln!("toys: {}", game.toys_summary().unwrap_or_default());
    }
    if o.menu {
        println!("menu: {} physics steps in {ms} ms{}", game.steps_done(), if game.paused() { ", paused at the end" } else { "" });
    }
    if o.options {
        println!("options: {}", player::script::api::table_options_json());
        if let Some(machine) = game.machine_json() {
            println!("machine: {machine}");
        }
    }
    // the table's Exit event, as when play ends: what its script does then counted too
    game.end();
    let (errors, unsupported) = player::script::log::counts();
    Scripted { steps, fingerprint: fp.0, balls: game.engine().balls().len(), errors, unsupported }
}

#[cfg(windows)]
fn raise_priority() {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
    }
    const THREAD_PRIORITY_HIGHEST: i32 = 2;
    // SAFETY: plain Windows calls on this thread's own pseudo-handle
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
    }
}

#[cfg(not(windows))]
fn raise_priority() {}

fn micros(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

fn main() -> ExitCode {
    let o = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    raise_priority();
    let _mode = VpxFloatMode::enter(); // VPX's processor mode for physics
    if let Some(path) = &o.replay {
        return replayed(path, o.repeat);
    }
    if o.script {
        return scripted(&o);
    }

    println!(
        "{} ms, {} balls, flippers {}, best of {} runs after a warm-up",
        o.ms.unwrap_or(3000),
        o.balls,
        if o.flippers { "pressed 400-600 ms of each second" } else { "at rest" },
        o.repeat
    );
    println!("{:<44} {:>9} {:>9} {:>9} {:>9}  fingerprint", "table", "total ms", "mean us", "p99 us", "worst us");
    let mut ok = true;
    for path in &o.tables {
        let name = path.file_stem().map_or_else(|| path.display().to_string(), |s| s.to_string_lossy().into_owned());
        let vpx = match vpin::vpx::read(path) {
            Ok(mut v) => {
                player::pictures::resolve_links(&mut v);
                v
            }
            Err(e) => {
                println!("{name:<44} could not read the table: {e}");
                ok = false;
                continue;
            }
        };
        run(&vpx, &o); // warm-up: caches, and the processor up to speed
        let runs: Vec<Run> = (0..o.repeat).map(|_| run(&vpx, &o)).collect();
        let repeatable = runs.iter().all(|r| r.fingerprint == runs[0].fingerprint);
        // the fastest run: the one least disturbed by the rest of the machine
        let best = runs.iter().min_by_key(|r| r.steps.iter().sum::<Duration>()).expect("a run");
        let mut sorted = best.steps.clone();
        sorted.sort();
        let total: Duration = best.steps.iter().sum();
        let p99 = sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)];
        let worst = *sorted.last().expect("a step");
        println!(
            "{:<44} {:>9.1} {:>9.1} {:>9.0} {:>9.0}  {:016x}{}",
            name.chars().take(44).collect::<String>(),
            total.as_secs_f64() * 1e3,
            micros(total) / best.steps.len() as f64,
            micros(p99),
            micros(worst),
            best.fingerprint,
            if repeatable { "" } else { "  NOT REPEATABLE" }
        );
        ok &= repeatable;
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// The table as the player sets it up, script and all.
#[allow(clippy::too_many_arguments)] // each a check's own switch
fn game_for(path: &Path, vpx: vpin::vpx::VPX, altsound: bool, colour: bool, cabinet_input: bool, sensors: player::sensors::Sensors, dof: Option<&Path>, toys: bool, panel: bool) -> Game {
    let physics = build(&vpx, &PhysicsSettings::default());
    let sounds = player::audio::Sounds::load(&vpx);
    Game::new(GameSetup {
        menu: None,
        physics,
        vpx: Arc::new(vpx),
        table_path: path.to_path_buf(),
        render_parts: Vec::new(),
        script: true,
        sounds,
        view: player::camera::View::Desktop,
        record: None,
        stop_at_ms: None,
        record_from_ms: None,
        backglass_window: false,
        show_grill: false,
        pup_windows: [None; 3],
        pup: player::pup::PupSettings::default(),
        // VPinMAME's settings for each game from the file `VPI_MACHINE_SETTINGS` names, for checks
        // the values scripts keep between plays: for the run only, unless `VPI_SAVED_VALUES` names
        // a file to keep them in, as the player's `VPReg.ini`
        saved_values: std::env::var_os("VPI_SAVED_VALUES").map(PathBuf::from),
        machine_settings: std::sync::Arc::new(
            std::env::var_os("VPI_MACHINE_SETTINGS")
                .and_then(|f| std::fs::read_to_string(f).ok())
                .map_or_else(Default::default, |t| player::machine_settings::MachineSettings::from_text(&t)),
        ),
        pinmame: Default::default(),
        cabinet_input,
        plumb: (true, 1.0, 1.0),
        sensors,
        tilt_counts_nudges: std::env::args().any(|a| a == "--tilt-counts"),
        light_level: None,
        plunger_retract: false,
        waker: None,
        table_options: Vec::new(),
        controller_settings: Vec::new(),
        altsound_settings: if altsound { Vec::new() } else { vec![("Enable".into(), "0".into())] },
        serum_settings: if colour { Vec::new() } else { vec![("Enable".into(), "0".into())] },
        // with `--colour`, the settings file `VPI_SETTINGS` names may add PIN2DMD's key (`PacKey`)
        vni_settings: if colour { vpi_settings_section("Plugin.VNI") } else { vec![("Enable".into(), "0".into())] },
        // AlphaDMD's pictures of a segment game's digits: for its colour file, and for a panel
        alphadmd_settings: if colour || panel { Vec::new() } else { vec![("Enable".into(), "0".into())] },
        // the settings file `VPI_SETTINGS` names may ask for VPX's DMD upscaler
        upscale_settings: vpi_settings_section("Plugin.UpscaleDMD"),
        upscale_at_once: true,
        // the toys only when asked for, with the configuration given
        dof_settings: if dof.is_some() { Vec::new() } else { vec![("Enable".into(), "0".into())] },
        dof_folder: dof.map(Path::to_path_buf),
        record_toys: toys,
        // a real DMD panel only for a check (`--dmd-panel`, set on the game once made)
        dmdutil_settings: Vec::new(),
        dmd_panel_check: None,
        // a display device only for a check (`--dmd-device`, set on the game once made)
        dmd_device: None,
    })
}

/// `--replay`: a recorded game played back `repeat` times; the fingerprints
/// must agree.
fn replayed(path: &Path, repeat: u32) -> ExitCode {
    let replay = match player::replay::read(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let inputs = replay.events.iter().filter(|e| matches!(e, player::replay::Event::Input { .. })).count();
    let frames = replay.events.iter().filter(|e| matches!(e, player::replay::Event::Frame)).count();
    println!("{}: {} keys, {} frames", replay.table.display(), inputs, frames);
    let mut prints = Vec::new();
    for run in 1..=repeat.max(1) {
        player::script::log::reset();
        let vpx = match vpin::vpx::read(&replay.table) {
            Ok(mut v) => {
                player::pictures::resolve_links(&mut v);
                v
            }
            Err(e) => {
                eprintln!("could not read {}: {e}", replay.table.display());
                return ExitCode::FAILURE;
            }
        };
        let mut game = game_for(&replay.table, vpx, false, false, false, player::sensors::Sensors::default(), None, false, false);
        let mut fp = Fingerprint(0xcbf2_9ce4_8422_2325);
        let t0 = Instant::now();
        player::replay::play(&mut game, &replay.events, |g, e| {
            if matches!(e, player::replay::Event::Advance { .. }) {
                for b in g.engine().balls() {
                    fp.v3(&b.d.pos);
                    fp.v3(&b.d.vel);
                }
            }
        });
        println!(
            "run {run}: {} steps in {:.1} ms, {} balls at the end, fingerprint {:016x}",
            game.steps_done(),
            t0.elapsed().as_secs_f64() * 1e3,
            game.engine().balls().len(),
            fp.0
        );
        prints.push(fp.0);
    }
    if prints.iter().all(|&p| p == prints[0]) {
        ExitCode::SUCCESS
    } else {
        println!("the runs differ: the table's script reads the clock, or something isn't deterministic yet");
        ExitCode::FAILURE
    }
}

/// `--script`: each table once, with its script.
fn scripted(o: &Options) -> ExitCode {
    player::pinmame::TRACE.store(o.trace, std::sync::atomic::Ordering::Relaxed);
    println!(
        "{} ms with the table's script, frames at {} a second, {}",
        o.ms.unwrap_or(20_000),
        o.fps,
        if o.keys.is_some() { "keys as given" } else { "a coin, start, the plunger, then the flippers each second" }
    );
    let mut lines = Vec::new();
    let mut ok = true;
    for path in &o.tables {
        let name = path.file_stem().map_or_else(|| path.display().to_string(), |s| s.to_string_lossy().into_owned());
        let vpx = match vpin::vpx::read(path) {
            Ok(mut v) => {
                player::pictures::resolve_links(&mut v);
                v
            }
            Err(e) => {
                lines.push(format!("{name:<44} could not read the table: {e}"));
                ok = false;
                continue;
            }
        };
        eprintln!("== {name}");
        let r = run_scripted(path, vpx, o);
        let mut sorted = r.steps.clone();
        sorted.sort();
        let total: Duration = r.steps.iter().sum();
        let p99 = sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)];
        let worst = *sorted.last().expect("a step");
        lines.push(format!(
            "{:<44} {:>9.1} {:>9.1} {:>9.0} {:>9.0} {:>6} {:>7} {:>8}  {:016x}",
            name.chars().take(44).collect::<String>(),
            total.as_secs_f64() * 1e3,
            micros(total) / r.steps.len() as f64,
            micros(p99),
            micros(worst),
            r.balls,
            r.errors,
            r.unsupported,
            r.fingerprint
        ));
    }
    println!("{:<44} {:>9} {:>9} {:>9} {:>9} {:>6} {:>7} {:>8}  fingerprint", "table", "total ms", "mean us", "p99 us", "worst us", "balls", "errors", "missing");
    for l in lines {
        println!("{l}");
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// A section of the settings file `VPI_SETTINGS` names, if it names one (the player's test mode
/// reads settings the same way).
fn vpi_settings_section(section: &str) -> Vec<(String, String)> {
    std::env::var_os("VPI_SETTINGS").and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(Vec::new, |t| player::settings::Settings::from_text(&t).section(section))
}
