//! The game thread: owns the physics, the table's script and what
//! the script sees. It wakes for input, for the next 1 ms physics step and
//! for frame requests, and runs them in VPX's order: input first (the
//! script's `KeyDown`/`KeyUp`), then the physics catching up with the clock
//! (timers at the start of each step, events where VPX fires them),
//! then — when the window thread wants a frame — VPX's frame preparation:
//! `Animate` events, the per-frame timers, and a snapshot of everything the
//! picture shows.
//!
//! Without a script (`--no-script`, or when it can't start), keys drive the
//! table directly: the flipper keys flip, Enter pulls the plunger, 1 or B
//! serves a ball from the kicker named `BallRelease`, and a ball that drops
//! into a kicker named like `Drain` is removed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use physics_compat::engine::{NoScript, PhysicsEngine, PhysicsHost, Shape, TableBox};
use physics_compat::env::{BallId, PhysicsEvent};
use physics_compat::fpmode::VpxFloatMode;
use physics_compat::hitball::HitBall;
use physics_compat::math::Vertex3Ds;
use physics_compat::physconst::DEFAULT_BALL_SIZE;
use table_physics::TablePhysics;
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;

use crate::dik::default_keys;
use crate::input::InputEvent;
use crate::math::{Mat4, Vec3};
use crate::render::BallDraw;
use crate::scene::PartState;
use crate::script::host::{Item, ScriptHost};
use crate::script::object::Obj;
use crate::script::value::Ret;
use crate::script::{self, log};
use crate::waker::{Waker, until};
use crate::world::World;

/// How one part looks right now, as the script left it.
#[derive(Clone, Copy, Debug)]
pub struct PartRender {
    pub visible: bool,
    /// A light's brightness now (VPX's intensity, 0 when off).
    pub light: f32,
    /// A light's colours, at its center and its edge (VPX's `COLORREF`).
    pub light_color: [u32; 2],
    /// What the light's colours are multiplied by: an incandescent bulb's
    /// tint as its filament heats and cools, else white.
    pub light_tint: [f32; 3],
    /// A light's place, halo height and blend as the script left them.
    pub light_live: Option<crate::scene::LightLive>,
    /// A bumper's ring offset (down is negative) and its skirt's tilt in
    /// degrees (x, y).
    pub bumper: Option<[f32; 3]>,
    /// Which of a bumper's base, skirt, ring and cap draw (`anim::BUMPER_PIECES`).
    pub bumper_shown: Option<[bool; 4]>,
    /// Where the script moved a primitive or a target, from where the table
    /// placed it.
    pub transform: Option<Mat4>,
    /// What a script changed about the part's images and materials, if
    /// anything.
    pub look: Option<crate::look::LookNow>,
    /// A flasher's place, colour and pictures now.
    pub flasher: Option<crate::look::FlasherNow>,
    /// A primitive's colour and opacity now, after its light map.
    pub primitive: Option<crate::look::PrimitiveNow>,
    /// A plunger's place along its frames' span (VPX's `Render`: the
    /// position less the span's start, over the span).
    pub plunger: Option<f32>,
    /// The animation frame a primitive shows, when a script has shown or
    /// played one (else its mesh as loaded).
    pub anim_frame: Option<f32>,
    /// A ramp's heights and widths (bottom and top each), when a script has set them (else as loaded).
    pub ramp_shape: Option<[f32; 4]>,
}

/// The digits a score reel unit shows.
#[derive(Clone, Debug)]
pub struct ReelValues {
    pub part: u32,
    /// One digit per reel, left to right.
    pub digits: Vec<u16>,
    /// The table picture a script set in place of the reel's own (`Some(None)`: none).
    pub image: Option<Option<u32>>,
}

/// What the window thread draws.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// Simulated time, in microseconds since play started.
    pub sim_time_us: u64,
    /// When the physics last stepped, on SDL's clock in nanoseconds (VPX's
    /// `m_timeUpdateTimeStamp`), for the balls' last-moment move.
    pub physics_at_ns: u64,
    pub balls: Vec<BallDraw>,
    /// Each moving part the renderer asked about, in its order.
    pub parts: Vec<Option<PartState>>,
    /// Every part, as `vpx.gameitems` lists them.
    pub part_render: Vec<PartRender>,
    /// The score reels' digits.
    pub reels: Vec<ReelValues>,
    /// The text boxes' text and looks.
    pub text_boxes: Vec<crate::textbox::TextBoxNow>,
    /// How far the cabinet has moved from a nudge, in metres, for the
    /// picture's shake.
    pub nudge_offset: [f32; 2],
    /// The newest input the game has taken in so far.
    pub newest_input: Option<InputEvent>,
    /// Physics steps run in the last second, and the slowest step's time.
    pub steps_per_second: u32,
    pub worst_step_us: f32,
    /// Whether the table's script is running.
    pub scripted: bool,
    /// The score display text boxes and flashers marked as DMDs show.
    pub dmd: Option<std::sync::Arc<crate::dmd::DmdFrame>>,
    /// The machine's segment displays, when it has them.
    pub segments: Option<std::sync::Arc<crate::segments::SegmentsNow>>,
    /// The script hid PinMAME's own window (`Controller.Hidden`), or the table has no ROM machine.
    pub display_hidden: bool,
    /// The game the script asked PinMAME for (`Controller.GameName`), once it has: the floating
    /// score window's place is kept for it.
    pub machine_game: Option<String>,
    /// The backglass, once its file is read.
    pub backglass: Option<std::sync::Arc<crate::b2s::BackglassNow>>,
    /// What the PinUP pack shows in the windows.
    pub pup: Option<std::sync::Arc<crate::pup::draw::PupNow>>,
    /// The colour grade picture the script chose, once it chose one (its
    /// name; empty for none).
    pub color_grade: Option<String>,
    /// The bloom's strength the script set, once it set one.
    pub bloom_strength: Option<f32>,
    /// The pause menu, while it shows (`pause_menu`).
    pub menu: Option<std::sync::Arc<crate::pause_menu::View>>,
}

/// Shared between the game thread and the window thread.
pub struct Shared {
    pub waker: Arc<Waker>,
    pub quit: AtomicBool,
    /// The game asks to end (the script's `QuitPlayer`).
    pub game_over: AtomicBool,
    frame_wanted: AtomicBool,
    snapshot: Mutex<Option<Snapshot>>,
    snapshot_ready: Condvar,
    /// The window took the snapshot waiting for it.
    snapshot_taken: Condvar,
    /// Test mode: a snapshot waiting is kept until the window takes it, so
    /// every frame of the game's clock reaches it.
    keep_unread: AtomicBool,
}

impl Shared {
    pub fn new(waker: Arc<Waker>) -> Shared {
        Shared {
            waker,
            quit: AtomicBool::new(false),
            game_over: AtomicBool::new(false),
            frame_wanted: AtomicBool::new(false),
            snapshot: Mutex::new(None),
            snapshot_ready: Condvar::new(),
            snapshot_taken: Condvar::new(),
            keep_unread: AtomicBool::new(false),
        }
    }

    /// The window thread asks for a frame's snapshot and waits for it, up to
    /// `timeout` (the game thread answers within one physics step).
    pub fn request_snapshot(&self, timeout: Duration) -> Option<Snapshot> {
        let mut slot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if !self.keep_unread.load(Ordering::Acquire) {
            *slot = None;
        }
        self.frame_wanted.store(true, Ordering::Release);
        self.waker.wake();
        let (mut slot, _) = self.snapshot_ready.wait_timeout_while(slot, timeout, |s| s.is_none()).unwrap_or_else(|e| e.into_inner());
        let taken = slot.take();
        if taken.is_some() {
            self.snapshot_taken.notify_one();
        }
        taken
    }

    pub fn stop(&self) {
        self.quit.store(true, Ordering::Release);
        self.waker.wake();
    }
}

/// What the game thread needs from the table besides its physics.
pub struct GameSetup {
    pub physics: TablePhysics,
    pub vpx: Arc<VPX>,
    pub table_path: PathBuf,
    /// The parts the renderer moves, as `vpx.gameitems` indexes.
    pub render_parts: Vec<u32>,
    /// Run the table's script (else keys drive the table directly).
    pub script: bool,
    /// The table's sounds, connected to the audio thread.
    pub sounds: crate::audio::Sounds,
    /// The view shown (desktop, cabinet, single screen).
    pub view: crate::camera::View,
    /// Write a replay of the game here.
    pub record: Option<PathBuf>,
    /// Test mode: the game's clock stops here (ms after play starts), so
    /// a picture taken there shows the same moment on every run.
    pub stop_at_ms: Option<u64>,
    /// Test mode recording a video from here (ms after play starts): from then on each frame waits
    /// for the window to take it, so none is missed however long its picture takes.
    pub record_from_ms: Option<u64>,
    /// A backglass window, or a score window (for its score panel), shows
    /// the table's backglass file.
    pub backglass_window: bool,
    /// The backglass picture's grill shows (VPX's `ShowGrill`), else it's cut off.
    pub show_grill: bool,
    /// The windows a PinUP pack can draw in, by their size in pixels: the topper's, the
    /// backglass's, the score display's.
    pub pup_windows: [Option<(u32, u32)>; 3],
    /// VPX 5436's PinUP settings (`[Plugin.PUP]`: `Enable`, `PUPFolder`, `MainVol`, the windows' padding).
    pub pup: crate::pup::PupSettings,
    /// VPinMAME's settings for each game from our file, and VPX 5436's `[Plugin.PinMAME]`.
    pub machine_settings: Arc<crate::machine_settings::MachineSettings>,
    pub pinmame: crate::machine_settings::PinmameSettings,
    /// The file the script's saved values are kept in (`SaveValue`, `LoadValue`), as VPX keeps
    /// them in its `VPReg`; None: kept for the session only (test mode, the headless runner).
    pub saved_values: Option<std::path::PathBuf>,
    /// A cabinet's analog inputs reach the physics: a gamepad's right stick as the plunger
    ///; off in test mode and the headless runner, whose inputs are scripted.
    pub cabinet_input: bool,
    /// The cabinet's tilt bob, from VPX's `[Player]` settings: simulated, its damping and its
    /// ring's angle from the vertical in degrees (`SimulatedPlumb`, `PlumbDamping`,
    /// `PlumbThresholdAngle`: on, 1 and 1 by default).
    pub plumb: (bool, f32, f32),
    /// The cabinet's nudge and plunger sensors the settings name, and the default layouts' nudge
    /// strengths (VPX's `[Input]` `Mapping.Nudge<n>.Strength`, 0 to 2, 1 by default: how hard a
    /// full push shoves the cabinet's model; `sensors`).
    pub sensors: crate::sensors::Sensors,
    /// The machine tilts by VPX 10.8.0's counting of nudges instead of the tilt bob's touches
    /// (`[VPI]` `TiltCountsNudges`, off by default).
    pub tilt_counts_nudges: bool,
    /// The scene's light level when the player chose one over the table's day and night (VPX's
    /// `SceneLighting`): scripts' `NightDay` reads it.
    pub light_level: Option<f32>,
    /// A plunger pulled by a key or a script's `PullBack` eases forward again after a second (VPX's
    /// `[Player]` `PlungerRetract`).
    pub plunger_retract: bool,
    /// What wakes the game thread; PinMAME's helper wakes it too when
    /// solenoids change (set by [`run`]).
    pub waker: Option<Arc<crate::waker::Waker>>,
    /// The settings' table options (`[TableOption]`, the table's own settings file's over the
    /// main one's), which the script's `Table.Option` reads.
    pub table_options: Vec<(String, String)>,
    /// The settings' `[Controller]` (`controller.vbs`'s backglass server and toy settings).
    pub controller_settings: Vec<(String, String)>,
    /// The settings' `[Plugin.AltSound]` (whether the game's AltSound pack plays, and where else to look).
    pub altsound_settings: Vec<(String, String)>,
    /// The settings' `[Plugin.Serum]` and `[Plugin.VNI]` (whether a game's colour file colours its
    /// display, and where else to look).
    pub serum_settings: Vec<(String, String)>,
    pub vni_settings: Vec<(String, String)>,
    /// The settings' `[Plugin.AlphaDMD]` (whether a segment game's digits go to its colour file).
    pub alphadmd_settings: Vec<(String, String)>,
    /// The settings' `[Plugin.UpscaleDMD]` (VPX 5436's DMD upscaler: `UpscaleMode`, `Enable`).
    pub upscale_settings: Vec<(String, String)>,
    /// Each DMD picture upscaled at once rather than a frame late on the upscaler's thread (test
    /// mode and the headless runner, whose pictures repeat exactly).
    pub upscale_at_once: bool,
    /// The settings' `[Plugin.DOF]` (whether the cabinet's toys run), and a folder holding
    /// the configuration in place of the usual ones.
    pub dof_settings: Vec<(String, String)>,
    pub dof_folder: Option<PathBuf>,
    /// Keep what the toys are told, for a check (`Game::toys_summary`).
    pub record_toys: bool,
    /// The settings' `[Plugin.DMDUtil]` (a cabinet's real DMD panel), and a folder for a
    /// check's pictures of libdmdutil's virtual panel, which switches the panel on by itself.
    pub dmdutil_settings: Vec<(String, String)>,
    pub dmd_panel_check: Option<PathBuf>,
    /// A real DMD panel through a VPinMAME display device, when the settings name one
    /// (`dmd_device`, `[VPI]` `DmdDevice`).
    pub dmd_device: Option<crate::dmd_device::Setup>,
    /// The pause menu's knowledge of the table, when play has one (`pause_menu`; none in the
    /// headless runner).
    pub menu: Option<crate::pause_menu::Setup>,
}

/// The physics' host while the game runs: the script's timers and events.
struct Host<'a> {
    world: &'a mut World,
    script: Option<&'a ScriptHost>,
}

impl PhysicsHost for Host<'_> {
    fn event(&mut self, engine: &mut PhysicsEngine, event: PhysicsEvent) {
        let script = self.script;
        script::enter(engine, self.world, script, || physics_event(event));
    }

    fn step_begin(&mut self, engine: &mut PhysicsEngine) {
        let script = self.script;
        script::enter(engine, self.world, script, || fire_timers(0));
    }

    fn active_ball(&mut self, ball: BallId) {
        self.world.active_ball = Some(ball);
    }
}

/// A physics event, handed to the script as VPX fires it.
fn physics_event(event: PhysicsEvent) {
    // a part's event by its name, for achievements' signals (and where a check asks for them)
    if crate::game_state::wanted() {
        let named = match &event {
            PhysicsEvent::Hit { part } => Some((part.0, "Hit")),
            PhysicsEvent::Spin { part } => Some((part.0, "Spin")),
            PhysicsEvent::Slingshot { part } => Some((part.0, "Slingshot")),
            _ => None,
        };
        if let Some((part, what)) = named
            && let Some(name) = script::with(|_, w| w.parts.get(part as usize).map(|p| p.name.clone())).flatten()
        {
            crate::game_state::on_part(&name, what);
        }
    }
    match event {
        PhysicsEvent::Hit { part } => {
            // a trigger starts its animation (VPX's TriggerAnimationHit)
            script::with(|_, w| {
                if let Some(t) = w.parts.get_mut(part.0 as usize).and_then(|p| p.trigger.as_mut()) {
                    t.hit_event = true;
                }
            });
            script::fire_group(part.0, "Hit");
        }
        PhysicsEvent::Unhit { part } => {
            script::with(|_, w| {
                if let Some(t) = w.parts.get_mut(part.0 as usize).and_then(|p| p.trigger.as_mut()) {
                    t.unhit_event = true;
                }
            });
            script::fire_group(part.0, "Unhit");
        }
        PhysicsEvent::Slingshot { part } => script::fire_group(part.0, "Slingshot"),
        PhysicsEvent::Spin { part } => script::fire_group(part.0, "Spin"),
        PhysicsEvent::LimitEos { part, speed } => script::fire_part(part.0, "LimitEOS", vec![Ret::Float(speed)]),
        PhysicsEvent::LimitBos { part, speed } => script::fire_part(part.0, "LimitBOS", vec![Ret::Float(speed)]),
        PhysicsEvent::FlipperCollide { part, speed } => script::fire_part(part.0, "Collide", vec![Ret::Float(speed)]),
        PhysicsEvent::BallBallCollision { ball, other, speed } => {
            let objects = script::with(|_, w| (w.ball_object(ball), w.ball_object(other)));
            if let Some((a, b)) = objects {
                script::call("OnBallBallCollision", vec![Ret::Object(Obj::dispatch(a)), Ret::Object(Obj::dispatch(b)), Ret::Float(speed)]);
            }
        }
        PhysicsEvent::LaunchBallKey { pressed, .. } => {
            // an auto plunger's mechanical pull: VPX presses the launch key for the script
            let key = script::with(|_, w| w.keys.plunger).unwrap_or(default_keys::PLUNGER);
            script::fire_table(if pressed { "KeyDown" } else { "KeyUp" }, vec![Ret::Int(i32::from(key))]);
        }
        // part state the script can read, not events of their own
        PhysicsEvent::HitThreshold { part, value } => {
            script::with(|_, w| {
                if let Some(p) = w.parts.get_mut(part.0 as usize) {
                    p.hit_threshold = value;
                }
            });
        }
        PhysicsEvent::TargetHit { part } => {
            script::with(|_, w| {
                if let Some(t) = w.parts.get_mut(part.0 as usize).and_then(|p| p.target.as_mut()) {
                    t.hit_event = true;
                }
            });
        }
        // a part the table has its hits off for, watched for achievements: theirs alone
        PhysicsEvent::Watched { part } => {
            if let Some(name) = script::with(|_, w| w.parts.get(part.0 as usize).map(|p| p.name.clone())).flatten() {
                crate::game_state::on_part(&name, "Hit");
            }
        }
    }
}

/// VPX's `Player::FireTimers` (build 5436): `mode` 0 fires the timers that
/// are due (each physics step), -1 those that fire every frame, -2 those
/// that fire at each sync with the game logic. Switches made meanwhile wait
/// until all have fired.
fn fire_timers(mode: i32) {
    let Some((count, now)) = script::with(|e, w| {
        w.timers.deferring = true;
        (w.timers.running.len(), e.time_msec())
    }) else {
        return;
    };
    for i in 0..count {
        let due = script::with(|_, w| {
            let t = *w.timers.running.get(i)?;
            let timer = &w.timers.all[t];
            let due = match mode {
                0 => timer.interval >= 0 && timer.next_fire <= now,
                m => timer.interval == m,
            };
            due.then_some((t, timer.part, timer.next_fire))
        })
        .flatten();
        if let Some((t, part, next)) = due {
            script::fire_group(part, "Timer");
            if mode == 0 {
                script::with(|_, w| {
                    // only if the script didn't reschedule it while it ran
                    let timer = &mut w.timers.all[t];
                    if timer.next_fire == next && timer.interval > 0 {
                        while timer.next_fire <= now {
                            timer.next_fire = timer.next_fire.wrapping_add(timer.interval as u32);
                        }
                    }
                });
            }
        }
    }
    script::with(|_, w| {
        w.timers.deferring = false;
        w.timers.apply_changes();
    });
}

/// VPX's frame preparation (`Player::PrepareFrame`, build 5436): each part's
/// animation, then the timers that fire every frame (interval -1).
fn prepare_frame(last_angles: &mut HashMap<u32, f32>, last_frame_msec: &mut u32) {
    let Some((now, count)) = script::with(|e, w| {
        w.frame_index = w.frame_index.wrapping_add(1);
        (e.time_msec(), w.parts.len() as u32)
    }) else {
        return;
    };
    // each part's UpdateAnimation, in table order, when time has moved on
    let diff_ms = now.wrapping_sub(*last_frame_msec) as f32;
    *last_frame_msec = now;
    if diff_ms > 0.0 {
        for p in 0..count {
            let (animate, event) = script::with(|e, w| part_animation(e, w, p, diff_ms, now, last_angles)).unwrap_or((0, None));
            // a drop target's Dropped or Raised comes before its Animate (VPX); a light
            // sequencer's PlayDone is its own event only (FireVoidEvent)
            match event {
                Some("PlayDone") => script::fire_part(p, "PlayDone", Vec::new()),
                Some(event) => script::fire_group(p, event),
                None => {}
            }
            for _ in 0..animate {
                script::fire_group(p, "Animate");
            }
        }
    }
    fire_timers(-1);
}

/// One part's `UpdateAnimation`: how many `Animate` events it fires, and a
/// drop target's `Dropped` or `Raised`.
fn part_animation(e: &mut PhysicsEngine, w: &mut World, p: u32, diff_ms: f32, now: u32, last_angles: &mut HashMap<u32, f32>) -> (u32, Option<&'static str>) {
    let World { parts, sounds, vpx, part_colliders, .. } = w;
    // a light sequencer changes other parts: take it out while it plays
    if let Some(mut seq) = parts[p as usize].light_seq.take() {
        let done = seq.update(now, parts, &mut e.env.rng);
        parts[p as usize].light_seq = Some(seq);
        return (0, done.then_some("PlayDone"));
    }
    let part = &mut parts[p as usize];
    if let Some(light) = &mut part.light {
        return (u32::from(light.update(diff_ms, now)), None);
    }
    if let Some(reel) = &mut part.reel {
        // each step of a reel clicks its sound, as VPX's PlaySound(sound, 0, 1)
        let rng = &mut e.env.rng; // VPX's shared generator (rand_mt_01)
        let moved = reel.update(now, |sound| sounds.play(sound, 0, 1.0, 0.0, 0.0, 0.0, 0, false, true, || rng.rand_01()));
        return (u32::from(moved), None);
    }
    if let Some(target) = &mut part.target {
        let was = part.is_dropped;
        let (animate, event) = target.update(diff_ms, now, part.visible, &mut part.is_dropped);
        // the physics sees the target down only once its fall has ended
        if part.is_dropped != was {
            let id = physics_compat::env::PartId(p);
            if part.is_dropped { e.env.dropped_targets.insert(id) } else { e.env.dropped_targets.remove(id) }
        }
        let has_hit_event = matches!(&vpx.gameitems[p as usize], GameItemEnum::HitTarget(t) if t.use_hit_event);
        return (u32::from(animate), event.filter(|_| has_hit_event));
    }
    if let Some(trigger) = &mut part.trigger {
        return (u32::from(trigger.update(diff_ms)), None);
    }
    if let Some(anim) = &mut part.prim_anim {
        anim.update(diff_ms);
        return (0, None);
    }
    let Some(k) = part.collider.or_else(|| part_colliders.get(&p).and_then(|c| c.first().copied())) else { return (0, None) };
    if let Some(bumper) = &mut part.bumper {
        let Shape::Bumper(b) = &mut e.collider_mut(k).shape else { return (0, None) };
        let hit_ball = [b.anim_hit_ball_position.x, b.anim_hit_ball_position.y];
        return (bumper.update(diff_ms, &mut b.anim_hit_event, &mut b.anim_ring_anim_offset, hit_ball), None);
    }
    // flippers, gates and spinners: Animate once a frame when they moved
    let angle = match &e.colliders()[k as usize].shape {
        Shape::Flipper(f) => f.mover.angle_cur,
        Shape::Gate(g) => g.mover.angle,
        Shape::Spinner(s) => s.mover.angle,
        _ => return (0, None),
    };
    (u32::from(last_angles.insert(p, angle) != Some(angle)), None)
}

/// VPX's `RecalculateMatrices` for a primitive's placement.
pub(crate) fn primitive_matrix(position: [f32; 3], size: [f32; 3], r: &[f32; 9]) -> Mat4 {
    let rt = Mat4::translate(r[3], r[4], r[5]) * Mat4::rotate_z(r[2].to_radians()) * Mat4::rotate_y(r[1].to_radians()) * Mat4::rotate_x(r[0].to_radians());
    let rt = rt * Mat4::rotate_z(r[8].to_radians()) * Mat4::rotate_y(r[7].to_radians()) * Mat4::rotate_x(r[6].to_radians());
    Mat4::scale(size[0], size[1], size[2]) * rt * Mat4::translate(position[0], position[1], position[2])
}

/// How every part looks now.
fn part_render(engine: &PhysicsEngine, world: &World) -> Vec<PartRender> {
    world
        .parts
        .iter()
        .enumerate()
        .map(|(p, part)| {
            let item = &world.vpx.gameitems[p];
            let (light, light_color) = part.light.as_ref().map_or((0.0, [0; 2]), |l| (l.current_intensity, [l.color, l.color2]));
            // VPX's Light::Render: an incandescent light's colours follow its filament
            let light_tint = match &part.light {
                Some(l) if l.fader == crate::light::FADER_INCANDESCENT => crate::bulb::incandescent_tint(l.filament_temperature as f32),
                _ => [1.0; 3],
            };
            let light_live = part.light.as_ref().map(|l| crate::scene::LightLive {
                center: l.center,
                halo_height: l.halo_height,
                modulate_vs_add: l.modulate_vs_add,
                transmission: l.transmission,
                falloff: l.falloff,
                falloff_power: l.falloff_power,
                depth_bias: l.depth_bias,
                on_ball: l.on_ball,
                ball_shadows: l.ball_shadows,
                bulb_mesh: l.show_bulb_mesh,
            });
            let transform = match item {
                GameItemEnum::Primitive(prim) => world.placements.get(&(p as u32)).filter(|pl| pl.changed).map(|pl| {
                    let base = primitive_matrix([prim.position.x, prim.position.y, prim.position.z], [prim.size.x, prim.size.y, prim.size.z], &prim.rot_and_tra);
                    base.inverse() * primitive_matrix(pl.position, pl.size, &pl.rot_and_tra)
                }),
                // VPX's UpdateTarget: a drop target sinks; a standing target leans back
                GameItemEnum::HitTarget(t) => part.target.as_ref().filter(|a| a.offset != 0.0).map(|a| {
                    if a.is_drop {
                        Mat4::translate(0.0, 0.0, a.offset)
                    } else {
                        let place = |lean: f32| {
                            Mat4::scale(t.size.x, t.size.y, t.size.z)
                                * Mat4::rotate_x(lean.to_radians())
                                * Mat4::rotate_z(t.rot_z.to_radians())
                                * Mat4::translate(t.position.x, t.position.y, t.position.z)
                        };
                        place(0.0).inverse() * place(a.offset)
                    }
                }),
                // a trigger's wire, star or button pressed down
                GameItemEnum::Trigger(_) => part.trigger.as_ref().filter(|a| a.offset != 0.0).map(|a| Mat4::translate(0.0, 0.0, a.offset)),
                _ => None,
            };
            // a dropped wall hides; a flasher or primitive a light sequencer holds off hides
            let visible = part.visible
                && !(matches!(item, GameItemEnum::Wall(_)) && part.is_dropped)
                && !(matches!(item, GameItemEnum::Flasher(_) | GameItemEnum::Primitive(_)) && part.ls_locked && !part.ls_on);
            // the bumper's ring offset lives in its collider, as in VPX
            let bumper = part.bumper.as_ref().map(|anim| {
                let collider = part.collider.or_else(|| world.part_colliders.get(&(p as u32)).and_then(|c| c.first().copied()));
                let ring = collider
                    .and_then(|k| match &engine.colliders()[k as usize].shape {
                        Shape::Bumper(b) => Some(b.anim_ring_anim_offset),
                        _ => None,
                    })
                    .unwrap_or(0.0);
                [ring, anim.rot_x, anim.rot_y]
            });
            let look = part.look.touched(&world.materials).then(|| part.look.now(&world.materials));
            // VPX's light map: the light's brightness now, of its full brightness, scales the opacity
            let light_factor = |lightmap: Option<u32>| {
                let light = lightmap.and_then(|l| world.parts[l as usize].light.as_ref());
                light.map_or(1.0, |l| {
                    if l.intensity != 0.0 && l.intensity_scale != 0.0 { l.current_intensity / (l.intensity * l.intensity_scale) } else { 0.0 }
                })
            };
            let flasher = part.flasher.as_ref().map(|f| f.now(light_factor(f.lightmap)));
            let primitive = part.primitive.as_ref().map(|pr| pr.now(light_factor(pr.lightmap)));
            let plunger = part.collider.and_then(|k| match &engine.colliders()[k as usize].shape {
                Shape::Plunger(pl) => Some((pl.mover.pos - pl.mover.frame_start) / (pl.mover.frame_end - pl.mover.frame_start)),
                _ => None,
            });
            let anim_frame = part.prim_anim.as_ref().and_then(crate::prim_anim::PrimAnim::frame);
            let bumper_shown = part.bumper.as_ref().map(|anim| anim.shown);
            PartRender { visible, light, light_color, light_tint, light_live, bumper, bumper_shown, transform, look, flasher, primitive, plunger, anim_frame, ramp_shape: part.ramp_shape }
        })
        .collect()
}

/// Collider indexes of the parts keys drive when there's no script.
struct DirectParts {
    flippers: Vec<(u32, f32)>, // (collider, x)
    plungers: Vec<(u32, f32)>, // (collider, pull speed)
    ball_release: Option<u32>,
    drains: HashMap<u32, u32>,
}

/// The table's behaviour without a script: drains remove balls.
struct DirectHost<'a> {
    drains: &'a HashMap<u32, u32>,
}

impl PhysicsHost for DirectHost<'_> {
    fn event(&mut self, engine: &mut PhysicsEngine, event: PhysicsEvent) {
        if let PhysicsEvent::Hit { part } = event
            && let Some(&k) = self.drains.get(&part.0)
        {
            engine.kicker_destroy_ball(k);
        }
    }
}

fn now_ns() -> u64 {
    // SAFETY: SDL's clock, valid once SDL is initialised
    unsafe { sdl3_sys::everything::SDL_GetTicksNS() }
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

/// A script left beside the table to take its own one's place, as VPX's loading has it
/// (`PinTable::LoadGameFromFilename`, `FileLocator::SearchScript`): `<table>.vbs` beside the
/// table, in its `user` or `scripts` folder, or in the folders VPX's own install and
/// preferences stand for — the player's folder (and its `user`, `scripts` and `tables`) and its
/// data folder (and its `user` and `scripts`); else a script named after the table's folder,
/// beside it. Read as UTF-8, else as Latin-1 (`string_from_utf8_or_iso8859_1`). Before them all,
/// ours: the script `VPI_SCRIPT_FILE` names, for checking an edit before it's kept.
fn script_override(world: &World) -> Option<(PathBuf, String)> {
    let read = |found: PathBuf| {
        let bytes = std::fs::read(&found).ok()?;
        let text = String::from_utf8(bytes).unwrap_or_else(|e| e.into_bytes().iter().map(|&b| char::from(b)).collect());
        Some((found, text))
    };
    if let Some(named) = std::env::var_os("VPI_SCRIPT_FILE").map(PathBuf::from).filter(|p| p.is_file()) {
        return read(named);
    }
    // hardcore: the table's own script, as its author made it
    if crate::achievements::hardcore_play() {
        return None;
    }
    let table = world.table_path.as_ref()?;
    let dir = table.parent()?;
    let name = table.with_extension("vbs").file_name()?.to_os_string();
    let mut places = vec![dir.to_path_buf(), dir.join("user"), dir.join("scripts")];
    if let Some(app) = std::env::current_exe().ok().as_deref().and_then(std::path::Path::parent) {
        places.extend([app.to_path_buf(), app.join("user"), app.join("scripts"), app.join("tables")]);
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let data = PathBuf::from(appdata).join("Visual Pinball Infinite");
        places.extend([data.clone(), data.join("user"), data.join("scripts")]);
    }
    let named_after_folder = || dir.file_name().map(|f| dir.join(format!("{}.vbs", f.to_string_lossy()))).filter(|p| p.is_file());
    let found = places.iter().map(|p| p.join(&name)).find(|p| p.is_file()).or_else(named_after_folder)?;
    read(found)
}

/// Starts the table's script as VPX's player does: the objects by name, the
/// script's own code (or a script left beside the table in its place), then
/// `Init` for the table and each part, `Animate` for the parts that have it,
/// and the options event.
fn start_script(engine: &mut PhysicsEngine, world: &mut World) -> Option<ScriptHost> {
    let mut items = vec![
        Item { name: world.table_name.clone(), obj: world.table, global: false },
        Item { name: "Global".into(), obj: world.global, global: true },
        Item { name: "Debug".into(), obj: world.debug, global: false },
        // found before VBScript's own CreateObject: every call reaches the fence
        Item { name: "CreateObject".into(), obj: world.fence, global: false },
    ];
    let mut names: std::collections::HashSet<String> = items.iter().map(|i| i.name.to_lowercase()).collect();
    for c in &world.collections {
        if names.insert(c.name.to_lowercase()) {
            items.push(Item { name: c.name.clone(), obj: c.obj, global: false });
        }
    }
    for (p, part) in world.parts.iter().enumerate() {
        if part.class.is_none() || matches!(world.vpx.gameitems[p], GameItemEnum::Decal(_)) {
            continue;
        }
        // VPX skips a second item with a name already used
        if names.insert(part.name.to_lowercase()) {
            items.push(Item { name: part.name.clone(), obj: part.obj, global: false });
        }
    }
    let mut host = match ScriptHost::new(&items) {
        Ok(h) => h,
        Err(e) => {
            log::once(&format!("the table's script can't run: {e}"));
            return None;
        }
    };
    // the fence's rule for files: written only in the player's data folder for
    // tables, read there, beside the table and in the shared scripts' folders
    if let Some(data) = script::api::user_directory() {
        script::files::set(&data, &script::api::script_folders(world));
    }
    let code = match script_override(world) {
        Some((path, text)) => {
            log::once(&format!("the table's script is replaced by {}, a script left beside it (as VPX loads it)", path.display()));
            text
        }
        None => world.vpx.gamedata.code.string.clone(),
    };
    script::enter(engine, world, None, || {
        if let Err(e) = host.load(&code) {
            log::once(&e);
        }
    });
    let host_ref = &host;
    script::enter(engine, world, Some(host_ref), || {
        script::fire_table("Init", Vec::new());
        let parts: Vec<(u32, bool)> = script::with(|_, w| {
            w.vpx
                .gameitems
                .iter()
                .enumerate()
                .filter(|(_, g)| !matches!(g, GameItemEnum::Decal(_) | GameItemEnum::Generic(..)))
                .map(|(p, g)| {
                    let animate = matches!(
                        g,
                        GameItemEnum::Bumper(_)
                            | GameItemEnum::Reel(_)
                            | GameItemEnum::Flipper(_)
                            | GameItemEnum::Gate(_)
                            | GameItemEnum::HitTarget(_)
                            | GameItemEnum::Light(_)
                            | GameItemEnum::Spinner(_)
                            | GameItemEnum::Trigger(_)
                    );
                    (p as u32, animate)
                })
                .collect()
        })
        .unwrap_or_default();
        for (p, animate) in parts {
            script::fire_part(p, "Init", Vec::new());
            if animate {
                script::fire_part(p, "Animate", Vec::new());
            }
        }
        script::fire_table("OptionEvent", vec![Ret::Int(0)]);
    });
    Some(host)
}

/// The game: the physics, the table's script and every part's state, run in
/// VPX's order. The game thread drives it against the clock; the
/// headless runner drives it in simulated time. Physics and script run with
/// VPX's processor mode set ([`VpxFloatMode`]): the caller enters it before
/// making a game and keeps it while driving it.
pub struct Game {
    // the script engine closes before the objects it holds are freed: fields
    // drop in this order
    script: Option<ScriptHost>,
    world: World,
    engine: PhysicsEngine,
    direct: DirectParts,
    table: TableBox,
    plunger_rest: HashMap<u32, f32>,
    render_parts: Vec<u32>,
    last_angles: HashMap<u32, f32>,
    last_frame_msec: u32,
    /// The score display shown with the newest frame.
    dmd: Option<std::sync::Arc<crate::dmd::DmdFrame>>,
    /// The segment displays shown with the newest frame.
    segments: Option<std::sync::Arc<crate::segments::SegmentsNow>>,
    /// The backglass shown with the newest frame.
    backglass: Option<std::sync::Arc<crate::b2s::BackglassNow>>,
    /// What the PinUP pack shows in the windows with the newest frame.
    pup: Option<std::sync::Arc<crate::pup::draw::PupNow>>,
    /// When play started, on the clock `start` was given.
    start_us: u64,
    /// The newest input taken in so far.
    pub newest_input: Option<InputEvent>,
    /// The tilt bob's touches of its ring handed to the script so far, and whether the
    /// centre-nudge key its last touch pressed is still to be let go.
    tilt_touches: i32,
    tilt_key_down: bool,
    /// The pause menu: what it knows of the table, and the menu while it shows.
    menu_setup: Option<crate::pause_menu::Setup>,
    menu: Option<crate::pause_menu::Menu>,
    /// The game is paused (the menu shows, outside test mode): the clock holds; play is to go on
    /// at the game thread's next look.
    paused: bool,
    resume_due: bool,
    /// The table's window has lost the keyboard to another program's: play is held until it's back.
    focus_lost: bool,
    /// Achievements' sounds playing.
    chimes: Vec<std::sync::Arc<crate::audio::MediaStream>>,
    /// The keys the script heard pressed and not let go, and those let go while the menu showed,
    /// which reach the script when play goes on.
    keys_down: Vec<u8>,
    let_go_meanwhile: Vec<u8>,
}

impl Game {
    /// Sets the table up for play as VPX's player does, and starts its
    /// script (its `Init` events run at play time 0).
    pub fn new(setup: GameSetup) -> Game {
        let table = setup.physics.table;
        let set_gravity = setup.physics.physics_set_gravity;
        let mut engine = setup.physics.into_engine();
        // keyboard nudging: the settings' model and strength, VPX's "10.8" box model at VPX's
        // default strength unless they say otherwise
        engine.nudge = Some(physics_compat::nudge::NudgeHandler::new(setup.sensors.keyboard_mode, setup.sensors.keyboard_strength, setup.vpx.gamedata.nudge_time));
        // the tilt bob, swung by the nudge (VPX's `PlumbHandler`)
        engine.plumb = Some(physics_compat::plumb::PlumbHandler::new(setup.plumb.0, setup.plumb.1, setup.plumb.2));
        // a cabinet's analog plunger and nudging: VPX's plunger and nudge handlers, their sensors
        // the settings' and those of the default layouts as devices connect (a gamepad's sticks, a
        // Pinscape board's accelerometer and plunger)
        if setup.cabinet_input {
            engine.env.plunger_sensor = Some(std::rc::Rc::new(crate::plunger_input::PlungerHandler::new(&setup.sensors)));
            if let Some(n) = &mut engine.nudge {
                crate::sensors::add_nudge_sensors(n, &setup.sensors);
            }
        }
        let mut world = World::new(setup.vpx.clone(), &engine, setup.sounds, setup.view, set_gravity);
        // the parts the table's achievements count, watched where the table has their hits off
        let counted = crate::achievements::watched_parts();
        if !counted.is_empty() {
            engine.env.watching = world.parts.iter().enumerate().filter(|(_, p)| counted.iter().any(|n| n.eq_ignore_ascii_case(&p.name))).map(|(i, _)| physics_compat::env::PartId(i as u32)).collect();
        }
        if setup.tilt_counts_nudges {
            world.nudge_count = Some(crate::nudge_count::NudgeCount::new(crate::nudge_count::NudgeCount::sensitivity_in(&world.vpx.gamedata.code.string)));
        }
        world.light_level = setup.light_level;
        world.plunger_retract = setup.plunger_retract;
        world.table_dir = setup.table_path.parent().map(|d| d.to_path_buf());
        world.table_path = Some(setup.table_path.clone());
        world.table_options = setup.table_options.clone();
        world.controller_settings = setup.controller_settings.clone();
        world.altsound_settings = setup.altsound_settings.clone();
        world.serum_settings = setup.serum_settings.clone();
        world.vni_settings = setup.vni_settings.clone();
        world.alphadmd_settings = setup.alphadmd_settings.clone();
        world.upscale_mode = crate::upscale::Mode::from_settings(&setup.upscale_settings);
        world.upscale_at_once = setup.upscale_at_once;
        world.dof_settings = setup.dof_settings.clone();
        world.dof_folder = setup.dof_folder.clone();
        world.dmd_panel_on = crate::dmd_panel::enabled(&setup.dmdutil_settings) || setup.dmd_panel_check.is_some();
        world.dmdutil_settings = setup.dmdutil_settings.clone();
        world.dmd_panel_check = setup.dmd_panel_check.clone();
        world.dmd_device_setup = setup.dmd_device.clone();
        world.toys_record = setup.record_toys.then(crate::dof::ToysRecord::default);
        world.backglass_window = setup.backglass_window;
        world.show_grill = setup.show_grill;
        world.pup_windows = setup.pup_windows;
        world.pup_settings = setup.pup.clone();
        world.machine_settings = setup.machine_settings.clone();
        world.pinmame_settings = setup.pinmame.clone();
        world.saved_values = crate::saved_values::SavedValues::load(setup.saved_values.clone());
        world.waker = setup.waker.clone();
        world.table_file = setup.table_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();

        // without a script: what keys drive directly
        let mut direct = DirectParts { flippers: Vec::new(), plungers: Vec::new(), ball_release: None, drains: HashMap::new() };
        for (p, part) in world.parts.iter().enumerate() {
            let Some(k) = part.collider else { continue };
            match &setup.vpx.gameitems[p] {
                GameItemEnum::Flipper(f) => direct.flippers.push((k, f.center.x)),
                GameItemEnum::Plunger(pl) => direct.plungers.push((k, pl.speed_pull)),
                GameItemEnum::Kicker(_) if part.name.eq_ignore_ascii_case("BallRelease") => direct.ball_release = Some(k),
                GameItemEnum::Kicker(_) if part.name.to_lowercase().contains("drain") => {
                    direct.drains.insert(p as u32, k);
                }
                _ => {}
            }
        }
        let plunger_rest: HashMap<u32, f32> = world
            .parts
            .iter()
            .enumerate()
            .filter_map(|(p, part)| match &engine.colliders()[part.collider? as usize].shape {
                Shape::Plunger(pl) => Some((p as u32, pl.mover.pos)),
                _ => None,
            })
            .collect();

        let script = if setup.script { start_script(&mut engine, &mut world) } else { None };
        Game {
            script,
            world,
            engine,
            direct,
            table,
            plunger_rest,
            render_parts: setup.render_parts,
            last_angles: HashMap::new(),
            last_frame_msec: 0,
            dmd: None,
            segments: None,
            backglass: None,
            pup: None,
            start_us: 0,
            newest_input: None,
            tilt_touches: 0,
            tilt_key_down: false,
            menu_setup: setup.menu.clone(),
            menu: None,
            paused: false,
            focus_lost: false,
            chimes: Vec::new(),
            resume_due: false,
            keys_down: Vec::new(),
            let_go_meanwhile: Vec::new(),
        }
    }

    /// Starts the physics clock at `now_us` (any clock, in microseconds).
    pub fn start(&mut self, now_us: u64) {
        self.start_us = now_us;
        self.engine.start_physics(now_us);
    }

    /// Whether the table's script is running.
    pub fn scripted(&self) -> bool {
        self.script.is_some()
    }

    /// When the next 1 ms step ends, on the clock `start` was given.
    pub fn next_step_time_usec(&self) -> u64 {
        self.engine.next_step_time_usec()
    }

    /// Physics steps done since play started (its clock shifted on by any pause).
    pub fn steps_done(&self) -> u64 {
        (self.engine.next_step_time_usec() - self.engine.start_time_usec()) / 1000 - 1
    }

    /// The physics, as it is now.
    pub fn engine(&self) -> &PhysicsEngine {
        &self.engine
    }

    /// The PinUP pack's state, if the table has one ([`crate::pup::Pup::summary`]).
    pub fn pup_summary(&self) -> Option<String> {
        self.world.pup.as_ref().map(|p| p.pup.summary())
    }

    /// Whether the script asked to end play (`QuitPlayer`).
    pub fn quit_requested(&self) -> bool {
        self.world.quit_requested
    }

    /// The score display the last frame showed (the script's, the machine's
    /// own or coloured, a FlexDMD's).
    pub fn shown_dmd(&self) -> Option<Arc<crate::dmd::DmdFrame>> {
        self.dmd.clone()
    }

    /// What the toys were told so far, when the set-up asked to keep it
    /// (`GameSetup::record_toys`): a line, `ToysRecord::summary`.
    pub fn toys_summary(&self) -> Option<String> {
        self.world.toys_record.as_ref().map(crate::dof::ToysRecord::summary)
    }

    /// The machine the script drives, as JSON for the app's table settings: the game name it gave,
    /// the ROM set that stands for it, and the settings it set for that game
    /// (`Controller.Games(name).Settings.Value`), which count over the machine settings file's
    /// while it plays; None when it drives none.
    pub fn machine_json(&self) -> Option<String> {
        let c = &self.world.controller.as_ref()?.state;
        if c.game_name.is_empty() {
            return None;
        }
        let set: std::collections::BTreeMap<&String, &i32> = c.game_settings.get(&c.game_name.to_lowercase()).map(|s| s.iter().collect()).unwrap_or_default();
        Some(serde_json::json!({ "game": c.game_name, "rom": c.rom_name, "set": set }).to_string())
    }

    /// A segment game's digits as VPX 5436's AlphaDMD draws them for colour
    /// files (`segments::alpha_identify`), as `dmd` gives a DMD: 128 × 32,
    /// a byte a dot.
    pub fn alpha_dmd(&self) -> Option<(i32, i32, Vec<u8>)> {
        let (_, now) = self.world.machine_segments.as_ref()?;
        let frame = crate::segments::alpha_identify(now)?;
        Some((128, 32, frame.dots.iter().map(|&level| level * 85).collect()))
    }

    /// The running ROM's main DMD, if the script started one: its width,
    /// height, and a byte a dot from 0 (off) to 255.
    pub fn dmd(&mut self) -> Option<(i32, i32, Vec<u8>)> {
        let machine = self.world.controller.as_mut()?.state.machine.as_mut()?;
        machine.dmd().map(|(w, h, dots)| (w, h, dots.to_vec()))
    }

    /// A key pressed or released: the script's `KeyDown` or `KeyUp`, as VPX
    /// fires them before the physics catches up. Without a script, the keys
    /// drive the table directly.
    pub fn input(&mut self, ev: InputEvent) {
        self.newest_input = Some(ev);
        if self.menu_input(ev) {
            return;
        }
        let Game { script, world, engine, direct, table, .. } = self;
        if let Some(host) = script.as_ref() {
            // a nudge key: VPX 5436 (InputManager's nudge actions) fires the script's KeyDown, then,
            // when the script pushed nothing of its own (older tables pushed through their scripts),
            // pushes the table itself: base force 2, its angle and force randomised by the physics'
            // own random numbers, towards 75° (left), 285° (right) or straight ahead
            let nudge = ev.down.then(|| [world.keys.left_tilt, world.keys.right_tilt, world.keys.center_tilt].iter().position(|&k| k == ev.dik)).flatten();
            let before = engine.nudge.as_ref().map(physics_compat::nudge::NudgeHandler::keyboard_index);
            script::enter(engine, world, Some(host), || {
                script::fire_table(if ev.down { "KeyDown" } else { "KeyUp" }, vec![Ret::Int(i32::from(ev.dik))]);
            });
            if let Some(which) = nudge {
                const BASE_FORCE: f32 = 2.0;
                let script_pushed = engine.nudge.as_ref().map(physics_compat::nudge::NudgeHandler::keyboard_index) != before;
                if !script_pushed && let Some(n) = engine.nudge.as_mut() {
                    let angle = (engine.env.rng.rand_01() - 0.5) * 15.0 * BASE_FORCE;
                    let force = (0.6 + engine.env.rng.rand_01() * 0.8) * BASE_FORCE;
                    n.apply_keyboard_impulse([75.0, 285.0, 0.0][which] + angle, force);
                }
                // VPX 10.8.0's counting, when that's the setting: its scripts pushed every nudge key
                // through cvpmNudge.DoNudge at force 2, randomised as above, whatever the table did
                if let Some(count) = world.nudge_count.as_mut() {
                    count.nudge((0.6 + engine.env.rng.rand_01() * 0.8) * BASE_FORCE);
                }
            }
            return;
        }
        let table_mid_x = 0.5 * (table.left + table.right);
        match ev.dik {
            default_keys::LEFT_FLIPPER | default_keys::RIGHT_FLIPPER => {
                let left = ev.dik == default_keys::LEFT_FLIPPER;
                for &(k, x) in &direct.flippers {
                    if (x < table_mid_x) == left
                        && let Shape::Flipper(f) = &mut engine.collider_mut(k).shape
                    {
                        if ev.down { f.rotate_to_end() } else { f.rotate_to_start() }
                    }
                }
            }
            default_keys::PLUNGER => {
                for &(k, speed) in &direct.plungers {
                    if let Shape::Plunger(pl) = &mut engine.collider_mut(k).shape {
                        match (ev.down, world.plunger_retract) {
                            (true, false) => pl.mover.pull_back(speed),
                            (true, true) => pl.mover.pull_back_and_retract(speed),
                            (false, _) => pl.mover.fire(),
                        }
                    }
                }
            }
            default_keys::START_GAME | 0x30 /* B */ if ev.down => {
                let id = BallId(world.next_ball);
                world.next_ball += 1;
                match direct.ball_release {
                    // what the example table's script does: CreateBall, then Kick 90, 7
                    Some(k) => {
                        engine.kicker_create_ball(k, id, DEFAULT_BALL_SIZE, 1.0, &mut NoScript);
                        engine.kicker_kick(k, 90.0, 7.0, 0.0, Vertex3Ds::default(), 0.0);
                    }
                    None => {
                        let mut hb = HitBall::new(id);
                        hb.d.pos = Vertex3Ds::new(table_mid_x, table.top + 100.0, 25.0);
                        hb.d.radius = DEFAULT_BALL_SIZE;
                        hb.d.mass = 1.0;
                        engine.add_ball(hb);
                    }
                }
            }
            _ => {}
        }
    }

    /// The pause menu's share of the input (`pause_menu`): P or a gamepad's Back opens it, or
    /// closes it; Esc, when the settings have it open the menu first, opens it with Quit the table
    /// picked; while it shows it takes every press, and a key the script
    /// holds let go meanwhile reaches the script when play goes on. Whether the menu took it.
    fn menu_input(&mut self, ev: InputEvent) -> bool {
        use crate::pause_menu::code;
        if ev.dik == code::FOCUS_LOST || ev.dik == code::FOCUS_BACK {
            self.focus(ev.dik == code::FOCUS_BACK);
            return true;
        }
        let toggle = matches!(ev.dik, code::PAUSE | code::IN_GAME_UI);
        if self.menu.is_none() {
            if (toggle || ev.dik == code::EXIT) && ev.down && self.menu_setup.is_some() && !self.resume_due {
                self.open_menu();
                if ev.dik == code::EXIT
                    && let Some(m) = self.menu.as_mut()
                {
                    m.pick_quit(&self.world.sounds, &script::api::table_options(), &self.world.table_options);
                }
            }
            if code::is_ours(ev.dik) {
                return true;
            }
            if ev.down {
                if !self.keys_down.contains(&ev.dik) {
                    self.keys_down.push(ev.dik);
                }
            } else {
                self.keys_down.retain(|&k| k != ev.dik);
            }
            return false;
        }
        if !ev.down && self.keys_down.contains(&ev.dik) {
            self.keys_down.retain(|&k| k != ev.dik);
            self.let_go_meanwhile.push(ev.dik);
        }
        if toggle {
            if ev.down {
                self.close_menu(false);
            }
            return true;
        }
        if ev.dik == code::EXIT {
            return true;
        }
        let options = script::api::table_options();
        let outcome = match self.menu.as_mut() {
            Some(m) => m.press(ev.dik, ev.down, &mut self.world.sounds, &options, &mut self.world.table_options),
            None => crate::pause_menu::Outcome::Nothing,
        };
        self.menu_outcome(outcome);
        self.menu_pictures();
        true
    }

    /// The rules' page shows: the table's pictures they name, decoded once.
    fn menu_pictures(&mut self) {
        if let Some(m) = self.menu.as_mut()
            && let Some(rules) = m.wants_pictures()
        {
            let pictures = crate::pause_menu::decode_pictures(&self.world.vpx, rules);
            m.set_pictures(pictures);
        }
    }

    /// The menu told whether the machine's display can be recorded for colouring — a ROM machine
    /// with a dot-matrix display — and how many frames are while it is.
    fn menu_recording(&mut self) {
        let can = self.world.controller.as_ref().and_then(|c| c.state.machine.as_ref()).and_then(|m| m.dmd_identify_count()).is_some();
        let state = can.then(|| self.world.frame_dump.as_ref().map(|d| d.frames));
        if let Some(m) = self.menu.as_mut() {
            m.set_recording(state);
        }
    }

    /// Recording the display for colouring, from the pause menu: stopped where it's on, else begun
    /// into the data folder's `Colouring\<rom>\<rom> <when>.txt.gz`, where the app finds it.
    fn toggle_recording(&mut self) {
        if let Some(mut dump) = self.world.frame_dump.take() {
            dump.finish();
            return;
        }
        let rom = self.world.controller.as_ref().map(|c| c.state.game_name.clone()).unwrap_or_default();
        let Some(data) = std::env::var_os("APPDATA").filter(|_| !rom.is_empty()) else { return };
        let path = std::path::PathBuf::from(data).join("Visual Pinball Infinite").join("Coloring").join(&rom).join(format!("{rom} {}.txt.gz", crate::screenshot::stamp()));
        match crate::frame_dump::FrameDump::create(&path) {
            Ok(d) => self.world.frame_dump = Some(d),
            Err(e) => eprintln!("frame dump: {}: {e}", path.display()),
        }
    }

    /// The menu's held presses repeated, as VPX's menu repeats them; and the mouse's news.
    pub fn menu_tick(&mut self) {
        self.menu_recording();
        let options = script::api::table_options();
        let outcome = match self.menu.as_mut() {
            Some(m) => m.tick(&mut self.world.sounds, &options, &mut self.world.table_options),
            None => return,
        };
        self.menu_outcome(outcome);
        let pointed = match self.menu.as_mut() {
            Some(m) => m.pointed(&mut self.world.sounds, &options, &mut self.world.table_options),
            None => return,
        };
        for outcome in pointed {
            self.menu_outcome(outcome);
        }
        self.menu_pictures();
    }

    fn menu_outcome(&mut self, outcome: crate::pause_menu::Outcome) {
        use crate::pause_menu::Outcome;
        match outcome {
            Outcome::Nothing => {}
            Outcome::Close => self.close_menu(false),
            Outcome::Quit => self.close_menu(true),
            Outcome::Record => {
                self.toggle_recording();
                self.menu_recording();
            }
            Outcome::OptionChanged => {
                // achievements: an option changed while playing makes the rest of the play softcore
                crate::achievements::to_softcore("an option was changed while playing");
                // VPX's TableOptionsPage: the new value live, the script told (OptionEvent 1)
                let Game { script, world, engine, .. } = self;
                script::enter(engine, world, script.as_ref(), || script::fire_table("OptionEvent", vec![Ret::Int(1)]));
            }
        }
    }

    /// The menu opens; outside test mode the game pauses, as VPX's `SetPlayState(false)`: the
    /// table told (`Paused`, which VPX 5436's shared scripts turn into `Controller.Pause`), its
    /// music paused — and, ours, its other sounds held where they are.
    fn open_menu(&mut self) {
        let Some(setup) = self.menu_setup.clone() else { return };
        let picture_only = setup.picture_only;
        self.menu = Some(crate::pause_menu::Menu::new(setup));
        self.menu_recording();
        if picture_only {
            return;
        }
        self.paused = true;
        self.world.sounds.hold(true);
        let Game { script, world, engine, .. } = self;
        script::enter(engine, world, script.as_ref(), || script::fire_table("Paused", Vec::new()));
    }

    /// The menu closes: the end of editing told to the script as VPX's `InGameUI::Close` tells it
    /// (`OptionEvent` 3), what changed kept; play goes on at the game thread's next look
    /// (`resume`), or ends with `quit`.
    fn close_menu(&mut self, quit: bool) {
        let Some(menu) = self.menu.take() else { return };
        let options = script::api::table_options();
        // a view set on its page plays on, and the menu opens on it next time
        if let Some(now) = menu.view_now()
            && let Some(viewing) = self.menu_setup.as_mut().and_then(|s| s.view.as_mut())
        {
            viewing.now = now;
        }
        menu.close(&self.world.sounds, &options, &self.world.table_options);
        {
            let Game { script, world, engine, .. } = self;
            script::enter(engine, world, script.as_ref(), || script::fire_table("OptionEvent", vec![Ret::Int(3)]));
        }
        if quit {
            self.world.quit_requested = true;
        }
        if self.paused && !self.focus_lost {
            self.resume_due = true;
        }
    }

    /// The table's window lost the keyboard (`back` false) or has it again: VPX's `OnFocusChanged`,
    /// its `SetPlayState` holding play while its window isn't focused — the table told (`Paused`,
    /// `UnPaused`), its sounds held. A game the menu paused stays paused until the menu closes.
    fn focus(&mut self, back: bool) {
        if !back {
            if self.focus_lost {
                return;
            }
            self.focus_lost = true;
            if self.paused {
                // paused already (the menu shows): nothing more to hold
                return;
            }
            self.paused = true;
            self.resume_due = false;
            self.world.sounds.hold(true);
            let Game { script, world, engine, .. } = self;
            script::enter(engine, world, script.as_ref(), || script::fire_table("Paused", Vec::new()));
            return;
        }
        if !std::mem::take(&mut self.focus_lost) {
            return;
        }
        if self.paused && self.menu.is_none() {
            self.resume_due = true;
        }
    }

    /// An achievement's announcement just began: its sound — its pack's, else ours — played on the
    /// backglass speakers at their volume and the achievements' level, unless the settings switch it
    /// off; those played to their end let go.
    pub fn play_chime(&mut self) {
        self.chimes.retain(|c| {
            let playing = c.is_playing();
            if !playing {
                c.close();
            }
            playing
        });
        let file = match crate::achievements::take_sound() {
            Some(crate::achievements::Chime::File(f)) => Some(f),
            Some(crate::achievements::Chime::Ours) => crate::achievements::our_chime(),
            _ => None,
        };
        let Some(file) = file.filter(|_| crate::achievements::sound_on()) else { return };
        let stream = crate::audio::MediaStream::load(file, 0.8);
        self.world.sounds.play_chime(&stream);
        stream.play();
        self.chimes.push(stream);
    }

    /// The pause menu's knowledge of the table (the headless runner's `--menu`).
    pub fn set_menu(&mut self, setup: Option<crate::pause_menu::Setup>) {
        self.menu_setup = setup;
    }

    /// Whether the game is paused: the game thread runs no steps and holds the clock.
    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Play goes on after a pause, if it's due, at `now_us` on the clock `start` was given: the
    /// whole game shifted on in time by the pause, as VPX's `UpdatePhysics` shifts it, the table
    /// told (`UnPaused`), its sounds going on, and the keys let go meanwhile handed to the script.
    pub fn resume_if_due(&mut self, now_us: u64) {
        if !std::mem::take(&mut self.resume_due) {
            return;
        }
        self.paused = false;
        self.engine.shift_clock_to(now_us);
        self.world.sounds.hold(false);
        {
            let Game { script, world, engine, .. } = self;
            script::enter(engine, world, script.as_ref(), || script::fire_table("UnPaused", Vec::new()));
        }
        for dik in std::mem::take(&mut self.let_go_meanwhile) {
            self.input(InputEvent { id: 0, time_ns: now_ns(), dik, down: false });
        }
    }

    /// Runs every whole 1 ms step that ends before `now_us`, with the
    /// script's timers and events inside them, then the timers that fire at
    /// each sync with the game logic (interval -2), as VPX's logic loop does;
    /// returns how many steps ran.
    pub fn advance(&mut self, now_us: u64) -> u32 {
        let before = self.engine.next_step_time_usec();
        let Game { script, world, engine, direct, .. } = self;
        match script.as_ref() {
            Some(host) => {
                engine.update_physics(now_us, &mut Host { world, script: Some(host) });
                script::enter(engine, world, Some(host), || fire_timers(-2));
            }
            None => engine.update_physics(now_us, &mut DirectHost { drains: &direct.drains }),
        }
        // the physics' rumble (a slingshot's kick, the plunger's bounce) to the controller
        if let Some((low, high, ms)) = self.engine.env.rumble.take() {
            crate::rumble::request(low, high, ms);
        }
        self.tilt_to_script();
        ((self.engine.next_step_time_usec() - before) / 1000) as u32
    }

    /// The tilt bob touched its ring: VPX 5436 presses its tilt action for the table, as a
    /// player pressing the tilt key would (`PlumbHandler`'s tilt event), let go when the bob
    /// leaves the ring; ours presses the mechanical tilt's key (T, VPX 10.8.0's code for it, as
    /// scripts get every key), let go at the next look. VPX 5436's shared scripts, which
    /// the player runs, turn it into a pulse of the machine's tilt switch
    /// (`cvpmNudge.DoMechTilt`); their nudges no longer count toward a tilt themselves
    ///.
    fn tilt_to_script(&mut self) {
        // VPX 10.8.0's counting, when the setting asks for it, in the bob's place
        let touches = match self.world.nudge_count.as_mut() {
            Some(count) => {
                count.step(self.engine.time_msec());
                count.presses as i32
            }
            None => self.engine.plumb.as_ref().map_or(0, physics_compat::plumb::PlumbHandler::tilt_index),
        };
        let Game { script, world, engine, .. } = self;
        let Some(host) = script.as_ref() else { return };
        let key = i32::from(world.keys.mech_tilt);
        if self.tilt_key_down {
            self.tilt_key_down = false;
            script::enter(engine, world, Some(host), || script::fire_table("KeyUp", vec![Ret::Int(key)]));
        }
        if touches != self.tilt_touches {
            self.tilt_touches = touches;
            self.tilt_key_down = true;
            script::log::once("the tilt bob touched its ring: the tilt key pressed for the table, as VPX 5436 presses it");
            script::enter(engine, world, Some(host), || script::fire_table("KeyDown", vec![Ret::Int(key)]));
        }
    }

    /// Play ends, as VPX 5436's player closes (`Player::~Player`): the table's `Exit` event, where
    /// scripts save their high scores and stop what they started (`Controller.Stop`), before the
    /// script closes and lets go of its objects — the machine stopping then, its memory kept.
    pub fn end(&mut self) {
        let Game { script, world, engine, .. } = self;
        // the frame dump closed, whatever happens to the script
        if let Some(dump) = world.frame_dump.as_mut() {
            dump.finish();
        }
        crate::game_state::print_parts();
        let Some(host) = script.as_ref() else { return };
        script::enter(engine, world, Some(host), || script::fire_table("Exit", Vec::new()));
        // the scores the script last showed on the backglass, each player's, for the app's play
        // log (it reads them from the player's output)
        if let Some(server) = world.controller.as_ref().and_then(|c| c.b2s.as_ref()) {
            let scores = server.player_scores();
            if scores.iter().any(|&(_, s)| s > 0) {
                println!("scores at the end: {}", scores.iter().map(|(p, s)| format!("{p}={s}")).collect::<Vec<_>>().join(", "));
            }
        }
    }

    /// VPX's frame preparation: animations, then the timers that fire every
    /// frame.
    pub fn prepare_frame(&mut self) {
        let Game { script, world, engine, last_angles, last_frame_msec, .. } = self;
        script::enter(engine, world, script.as_ref(), || prepare_frame(last_angles, last_frame_msec));
        // the event stream to the PinUP pack and the toys; the pack's triggers fired, its screens'
        // timers and what they play, and what it shows
        crate::events::poll(&mut self.world, u64::from(self.engine.time_msec()));
        if let Some(p) = self.world.pup.as_mut() {
            p.pup.update(u64::from(self.engine.time_msec()));
            self.pup = Some(std::sync::Arc::new(p.pup.now(u64::from(self.engine.time_msec()), crate::pup::playback::exact())));
        }
        // each FlexDMD draws its steps up to now
        for f in &self.world.flexdmds {
            crate::flexdmd::advance(f, f64::from(self.engine.time_msec()));
        }
        // the score display this frame shows: the script's, else the machine's own
        // a ball seen for the first time takes the table's look (VPX's Ball::RenderSetup)
        for b in self.engine.balls() {
            if !self.world.ball_looks.contains_key(&b.id) {
                self.world.ball_looks.insert(b.id, self.world.ball_table_look.clone());
            }
        }
        // the machine's newest frame to its colour file's program, its triggers to the PinUP pack
        crate::colour::poll(&mut self.world);
        // and to the frame dump, where one's being written
        if let (Some(dump), Some(machine)) = (self.world.frame_dump.as_mut(), self.world.controller.as_ref().and_then(|c| c.state.machine.as_ref())) {
            dump.poll(machine, u64::from(self.engine.time_msec()));
        }
        // the balls in play, for achievements' signals: those that travelled away from where they
        // rested, not those resting in a trough or held in a kicker
        if crate::game_state::wanted() {
            let now = self.engine.time_msec();
            let balls = self.engine.balls().iter().map(|b| {
                let v = b.d.vel;
                (b.id.0, (v.x * v.x + v.y * v.y + v.z * v.z).sqrt() > 0.5, b.d.locked_in_kicker, b.d.pos.x, b.d.pos.y)
            });
            crate::game_state::balls_in_play(crate::game_state::in_play(balls, u64::from(now)));
        }
        self.dmd = crate::dmd::shown(&mut self.world);
        self.segments = crate::segments::shown(&mut self.world);
        // a cabinet's real DMD panel takes its display as VPX's DMDUtil plugin picks it, or as
        // VPinMAME hands a display device its frames
        crate::dmd_panel::update(&mut self.world);
        crate::dmd_device::update(&mut self.world);
        self.backglass = crate::b2s::shown(&mut self.world, self.engine.time_sec() * 1e3);
    }

    /// Drives a VPinMAME display device as a real panel (the headless runner's `--dmd-device`).
    pub fn set_dmd_device(&mut self, setup: crate::dmd_device::Setup) {
        self.world.dmd_device_setup = Some(setup);
    }

    /// A real DMD panel's frames handed to libdmdutil's virtual panel for a check, its pictures
    /// going to `folder` (the headless runner's `--dmd-panel`), with the panel settings given.
    pub fn set_dmd_panel_check(&mut self, folder: PathBuf, settings: Vec<(String, String)>) {
        self.world.dmd_panel_on = true;
        self.world.dmdutil_settings = settings;
        self.world.dmd_panel_check = Some(folder);
    }

    /// A picture of what libdmdutil's virtual panel shows now, for a check (`--dmd-panel`).
    pub fn panel_picture(&mut self, ms: u32) {
        crate::dmd_panel::ask_picture(&mut self.world, ms);
    }

    /// What the picture shows now; `steps_per_second` and `worst_step_us`
    /// are the caller's timing of the physics.
    pub fn snapshot(&self, steps_per_second: u32, worst_step_us: f32, physics_at_ns: u64) -> Snapshot {
        let (engine, world) = (&self.engine, &self.world);
        let parts = self
            .render_parts
            .iter()
            .map(|&p| {
                let k = world.parts[p as usize].collider?;
                match &engine.colliders()[k as usize].shape {
                    Shape::Flipper(f) => Some(PartState::Flipper(f.mover.angle_cur)),
                    Shape::Gate(g) => Some(PartState::Axle(g.mover.angle)),
                    Shape::Spinner(s) => Some(PartState::Axle(s.mover.angle)),
                    Shape::Plunger(pl) => Some(PartState::Plunger(pl.mover.pos - self.plunger_rest.get(&p).copied().unwrap_or(pl.mover.pos))),
                    _ => None,
                }
            })
            .collect();
        Snapshot {
            sim_time_us: (engine.time_sec() * 1e6) as u64,
            physics_at_ns,
            // a ball its script hides isn't drawn, nor casts a shadow, nor blurs (VPX's
            // Ball::Render, and Renderer's ball shadows and motion blur, skip it)
            balls: engine
                .balls()
                .iter()
                .filter(|b| world.ball_looks.get(&b.id).is_none_or(|l| l.visible))
                .map(|b| {
                    let look = world.ball_looks.get(&b.id).unwrap_or(&world.ball_table_look);
                    let named = |n: &str| world.image_named(n);
                    let c = |k: u32| ((look.color >> (8 * k)) & 0xFF) as f32 / 255.0;
                    // VPX draws a ball held in a kicker sunk by its radius
                    let sunk = if b.d.locked_in_kicker { b.d.radius } else { 0.0 };
                    BallDraw {
                        id: b.id.0,
                        position: Vec3::new(b.d.pos.x, b.d.pos.y, b.d.pos.z - sunk),
                        velocity: Vec3::new(b.d.vel.x, b.d.vel.y, b.d.vel.z),
                        locked: b.d.locked_in_kicker,
                        radius: b.d.radius,
                        orientation: b.orientation.m,
                        color: [c(0), c(1), c(2)],
                        image: named(&look.image),
                        decal: named(&look.decal),
                        decal_mode: look.decal_mode,
                        bulb_scale: look.bulb_scale,
                        reflection: look.reflection,
                        spherical: look.spherical,
                        reflected: look.reflected,
                        force_reflection: look.force_reflection,
                    }
                })
                .collect(),
            parts,
            part_render: part_render(engine, world),
            reels: world
                .parts
                .iter()
                .enumerate()
                .filter_map(|(p, part)| {
                    let reel = part.reel.as_ref()?;
                    Some(ReelValues {
                        part: p as u32,
                        digits: reel.values().map(|d| d.clamp(0, i32::from(u16::MAX)) as u16).collect(),
                        image: reel.image.as_ref().map(|(_, i)| *i),
                    })
                })
                .collect(),
            text_boxes: world.parts.iter().enumerate().filter_map(|(p, part)| Some(part.text_box.as_ref()?.now(p as u32, part.visible))).collect(),
            nudge_offset: engine.nudge.as_ref().map_or([0.0; 2], |n| [n.cabinet_offset().x, n.cabinet_offset().y]),
            newest_input: self.newest_input,
            steps_per_second,
            worst_step_us,
            scripted: self.script.is_some(),
            dmd: self.dmd.clone(),
            segments: self.segments.clone(),
            // (a floating score window is PinMAME's: without a ROM machine, none floats)
            // hidden by the script, or by the game's VPinMAME setting (`showwindmd` = 0)
            display_hidden: world.controller.is_none()
                || crate::script::controller::hidden(world)
                || world.controller.as_ref().is_some_and(|c| c.state.setting("showwindmd") == Some(0)),
            machine_game: world.controller.as_ref().map(|c| c.state.game_name.clone()).filter(|g| !g.is_empty()),
            backglass: self.backglass.clone(),
            pup: self.pup.clone(),
            color_grade: world.color_grade.clone(),
            bloom_strength: world.bloom_strength,
            menu: self.menu.as_ref().map(|m| std::sync::Arc::new(m.view(&world.sounds, &script::api::table_options(), &world.table_options))),
        }
    }
}

/// Runs the game thread until `shared.quit`.
pub fn run(setup: GameSetup, inputs: Receiver<InputEvent>, shared: Arc<Shared>) {
    raise_priority();
    let _mode = VpxFloatMode::enter(); // VPX's processor mode for physics, script and input
    let mut recorder = setup.record.as_ref().and_then(|path| match crate::replay::Recorder::create(path, &setup.table_path) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("vpi-player: no replay: {}: {e}", path.display());
            None
        }
    });
    let (stop_at_ms, record_from_ms) = (setup.stop_at_ms, setup.record_from_ms);
    let mut setup = setup;
    setup.waker = Some(shared.waker.clone());
    let mut game = Game::new(setup);

    let start_us = now_ns() / 1000;
    game.start(start_us);
    // test mode: the clock stops at the shot, and frames are prepared at fixed game times
    // (every 10 ms, and at the stop), so animations and bulbs' filaments (integrated frame
    // by frame, as in VPX) come out the same on every run
    const TEST_FRAME_US: u64 = 10_000;
    let stop_us = stop_at_ms.map(|ms| start_us + ms * 1000);
    let mut next_frame_us = stop_us.map(|stop| (start_us + TEST_FRAME_US).min(stop));
    shared.keep_unread.store(stop_us.is_some(), Ordering::Release);
    let mut last_frame: Option<Snapshot> = None;
    let (mut steps_this_second, mut steps_per_second, mut second_start) = (0u32, 0u32, start_us);
    let (mut worst_step_us, mut worst_this_second) = (0f32, 0f32);
    // when the physics last stepped (VPX's m_timeUpdateTimeStamp: each step's start)
    let mut physics_at_ns = now_ns();
    let mut volume_keys = crate::volume::Keys::default();

    while !shared.quit.load(Ordering::Acquire) {
        let stopped = stop_us.is_some_and(|stop| game.next_step_time_usec() > stop);
        // a stopped clock only answers the window's requests for pictures; a paused game its
        // menu's presses too, looking every 10 ms for a held one's repeat
        let next_ns = if game.paused() {
            now_ns() + 10_000_000
        } else if stopped {
            now_ns() + 1_000_000
        } else {
            game.next_step_time_usec() * 1000 + 1000 // just after the step's end
        };
        shared.waker.wait(Some(until(now_ns(), next_ns)));

        // input first, as VPX does: the script's KeyDown/KeyUp
        while let Ok(ev) = inputs.try_recv() {
            game.input(ev);
            if let Some(r) = &mut recorder {
                r.record(crate::replay::Event::Input { dik: ev.dik, down: ev.down });
            }
        }
        volume_keys.poll(&mut game.world.sounds);
        game.menu_tick();
        game.play_chime();
        // play goes on after a pause, the time it lasted skipped
        game.resume_if_due(now_ns() / 1000);

        // the physics, up to the present (in test mode, up to the next frame's time); none while paused
        let t0 = std::time::Instant::now();
        let stepping_from_ns = now_ns();
        let target_us = match next_frame_us {
            Some(frame) => (now_ns() / 1000).min(frame),
            None => now_ns() / 1000,
        };
        let steps = if game.paused() { 0 } else { game.advance(target_us) };
        if let Some(r) = &mut recorder {
            r.record(crate::replay::Event::Advance { steps: game.steps_done() });
        }
        if steps > 0 {
            worst_this_second = worst_this_second.max(t0.elapsed().as_secs_f32() * 1e6 / steps as f32);
            // the last step's start: the batch's, plus the steps before it
            physics_at_ns = stepping_from_ns + t0.elapsed().as_nanos() as u64 * u64::from(steps - 1) / u64::from(steps);
        }
        steps_this_second += steps;
        let now_us = now_ns() / 1000;
        if now_us - second_start >= 1_000_000 {
            (steps_per_second, worst_step_us) = (steps_this_second, worst_this_second);
            (steps_this_second, worst_this_second, second_start) = (0, 0.0, now_us);
        }
        if game.quit_requested() {
            shared.game_over.store(true, Ordering::Release);
        }

        // VPX's frame preparation, then what the picture shows
        let frame_due = match next_frame_us {
            Some(frame) => target_us == frame,
            None => shared.frame_wanted.swap(false, Ordering::AcqRel),
        };
        if frame_due {
            game.prepare_frame();
            if let Some(r) = &mut recorder {
                r.record(crate::replay::Event::Frame);
            }
            let snapshot = game.snapshot(steps_per_second, worst_step_us, physics_at_ns);
            // test mode: the picture's frame waits until the window has taken the frame before it,
            // which it draws first (a slow table's window could otherwise miss it), and the second
            // frame until it has taken the first, which its still parts are drawn from; while a
            // video records, every frame does, so none of its pictures is missed
            let recording = record_from_ms.is_some_and(|ms| target_us >= start_us + ms * 1000);
            if recording || stop_us.is_some_and(|stop| target_us >= stop || target_us == start_us + 2 * TEST_FRAME_US) {
                let slot = shared.snapshot.lock().unwrap_or_else(|e| e.into_inner());
                drop(shared.snapshot_taken.wait_timeout_while(slot, Duration::from_secs(5), |s| s.is_some()));
            }
            if let (Some(stop), Some(frame)) = (stop_us, next_frame_us.as_mut()) {
                *frame = if *frame >= stop { u64::MAX } else { (*frame + TEST_FRAME_US).min(stop) };
                last_frame = Some(snapshot.clone());
            }
            *shared.snapshot.lock().unwrap_or_else(|e| e.into_inner()) = Some(snapshot);
            shared.snapshot_ready.notify_one();
        } else if next_frame_us == Some(u64::MAX) && shared.frame_wanted.swap(false, Ordering::AcqRel) {
            // stopped: the last frame again, without preparing another
            *shared.snapshot.lock().unwrap_or_else(|e| e.into_inner()) = last_frame.clone();
            shared.snapshot_ready.notify_one();
        }
    }
    // the table's own goodbye: its scores saved, its machine stopped
    game.end();
    if let Some(r) = recorder
        && let Err(e) = r.finish()
    {
        eprintln!("vpi-player: the replay is incomplete: {e}");
    }
}
