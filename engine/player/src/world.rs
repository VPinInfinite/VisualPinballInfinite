//! What the table's script can see and change, beside the physics: each
//! part's script object and settings, the timers, the lights, what is
//! visible, the balls the script holds. The game thread owns it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use physics_compat::engine::{PhysicsEngine, Shape};
use physics_compat::env::BallId;
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;

use crate::script::idl::CLASSES;
use crate::script::idl::{I_COLLECTION, I_TABLE, I_V_P_DEBUG};
use crate::script::idl_types::Class;
use crate::script::object::{Obj, Target, global_interface};
use crate::script::value::Kept;

/// One part, as the script sees it.
pub struct Part {
    pub name: String,
    pub class: Option<&'static Class>,
    pub obj: *mut Obj,
    /// The physics collider that is this part, for parts that move or catch
    /// balls (flippers, plungers, kickers, gates, spinners).
    pub collider: Option<u32>,
    /// Its timer, as an index into `Timers::all`.
    pub timer: Option<usize>,
    /// The collections that fire events for it, with its index in each.
    pub event_collections: Vec<(u32, i32)>,
    /// Whether its own events fire too (no collection stops them).
    pub single_events: bool,
    pub user_value: Option<Kept>,
    pub visible: bool,
    /// A light's state, blinking and brightness.
    pub light: Option<crate::light::LightRt>,
    /// A score reel's digits and motor.
    pub reel: Option<crate::reel::ReelRt>,
    /// A text box's text and looks.
    pub text_box: Option<crate::textbox::TextBoxRt>,
    /// A light sequencer's shows.
    pub light_seq: Option<crate::lightseq::LightSeqRt>,
    /// A flasher or primitive a light sequencer has taken over (VPX's
    /// `m_lockedByLS`), and whether the show has it on (`m_inPlayState`).
    pub ls_locked: bool,
    pub ls_on: bool,
    /// A target's, trigger's or bumper's animation.
    pub target: Option<crate::anim::TargetAnim>,
    pub trigger: Option<crate::anim::TriggerAnim>,
    pub bumper: Option<crate::anim::BumperAnim>,
    /// The speed of the last hit that counted (`HitThreshold`).
    pub hit_threshold: f32,
    pub is_dropped: bool,
    /// The part's `Collidable` (or a kicker's or trigger's `Enabled`).
    pub collidable: bool,
    /// A wall's `Disabled`: its slingshot doesn't fire (VPX's `m_disabled`).
    pub disabled: bool,
    /// What a script changed about its images and materials.
    pub look: crate::look::PartLook,
    /// A flasher's place, colour and pictures.
    pub flasher: Option<crate::look::FlasherRt>,
    /// A primitive's colour, opacity and light map.
    pub primitive: Option<crate::look::PrimitiveRt>,
    /// A primitive's animation, when its mesh has frames.
    pub prim_anim: Option<crate::prim_anim::PrimAnim>,
    /// A ramp's heights and widths, bottom and top each, as a script set them (`HeightBottom`,
    /// `HeightTop`, `WidthBottom`, `WidthTop`), when it has.
    pub ramp_shape: Option<[f32; 4]>,
}

/// A collection, as the script sees it.
pub struct Collection {
    pub name: String,
    pub obj: *mut Obj,
    /// Its parts, in its order.
    pub parts: Vec<u32>,
}

// Ported from Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit
// af26b2d): the timers of src/physics/hittimer.cpp (HitTimer), src/core/player.cpp
// (TimerStateChange, FireTimers) and src/core/ieditable.cpp (put_TimerEnabled, put_TimerInterval).

/// VPX's `HitTimer`, with its part's `TimerEnabled` and `TimerInterval`.
pub struct Timer {
    pub part: u32,
    /// The interval it runs by, at least 1 ms (`HitTimer::m_interval`).
    pub interval: i32,
    /// The interval as the table stores it or the script last set it, which `TimerInterval` reads
    /// back unclamped (`m_timerInterval`): core.vbs's `vpmMapLights` takes a light's lamp number
    /// from it, 0 included (Tom and Jerry's).
    pub stored: i32,
    pub next_fire: u32,
    pub enabled: bool,
}

/// The timers, as VPX keeps them: the running ones in order, and the
/// switches made while timers fire, which wait until they have all fired.
#[derive(Default)]
pub struct Timers {
    pub all: Vec<Timer>,
    pub running: Vec<usize>,
    pub changes: Vec<(usize, bool)>,
    /// Timers are firing: switches wait (`m_deferTimerChanges`).
    pub deferring: bool,
}

/// VPX's `MAX_TIMER_MSEC_INTERVAL`: a timer fires at most once a step.
pub const MAX_TIMER_MSEC_INTERVAL: i32 = 1;

impl Timers {
    /// VPX's interval rule: at least 1 ms, or -1 (each frame) / -2 (each
    /// sync with the game logic).
    pub fn clamp_interval(interval: i32) -> i32 {
        if interval >= 0 { interval.max(MAX_TIMER_MSEC_INTERVAL) } else { interval.max(-2) }
    }

    /// `HitTimer::SetInterval` (`put_TimerInterval`): the timer next fires
    /// one interval from now.
    pub fn set_interval(&mut self, t: usize, interval: i32, now_msec: u32) {
        let timer = &mut self.all[t];
        timer.interval = Self::clamp_interval(interval);
        timer.next_fire = now_msec.wrapping_add(timer.interval as u32);
    }

    /// `put_TimerInterval`: the interval kept as given, the timer run by its clamped value.
    pub fn put_interval(&mut self, t: usize, interval: i32, now_msec: u32) {
        self.all[t].stored = interval;
        self.set_interval(t, interval, now_msec);
    }

    /// `put_TimerEnabled` (`Player::TimerStateChange`): a switch takes effect
    /// at once, except while timers fire, when it waits until they have all
    /// fired (a switched-off timer stops firing meanwhile).
    pub fn set_enabled(&mut self, t: usize, on: bool, now_msec: u32) {
        if on != self.all[t].enabled {
            if self.deferring {
                if on {
                    self.set_interval(t, self.all[t].interval, now_msec);
                } else {
                    self.all[t].next_fire = 0xFFFF_FFFF;
                }
                self.changes.push((t, on));
            } else if on {
                self.set_interval(t, self.all[t].interval, now_msec);
                self.running.push(t);
            } else if let Some(i) = self.running.iter().position(|&r| r == t) {
                self.running.remove(i);
            }
        }
        self.all[t].enabled = on;
    }

    /// The end of `Player::FireTimers`: the switches made while timers fired.
    pub fn apply_changes(&mut self) {
        for (t, on) in std::mem::take(&mut self.changes) {
            let at = self.running.iter().position(|&r| r == t);
            match (on, at) {
                (true, None) => self.running.push(t),
                (false, Some(i)) => {
                    self.running.remove(i);
                }
                _ => {}
            }
        }
    }
}

/// The table's gravity and slopes as its script sees them (VPX's
/// `m_Gravity`, `m_angletiltMin`, `m_angletiltMax`).
#[derive(Clone, Copy, Debug)]
pub struct TableGravity {
    pub gravity: f32,
    pub slope_min: f32,
    pub slope_max: f32,
    /// The physics set's slopes and gravity, when the table uses one.
    pub set: Option<(f32, f32, f32)>,
}

/// A ball's look, as VPX keeps it for each ball (`Ball::m_d`): its colour
/// (VPX's OLE colour), its pictures by name — the room it mirrors (none:
/// VPX's own) and its decal or scratches — and how strongly the bulbs and the
/// playfield show in it.
#[derive(Clone, Debug, PartialEq)]
pub struct BallLook {
    pub color: u32,
    pub image: String,
    pub decal: String,
    pub decal_mode: bool,
    pub bulb_scale: f32,
    pub reflection: f32,
    /// Its picture is a sphere map (else equirectangular).
    pub spherical: bool,
    /// Whether it's drawn (VPX's `Ball.Visible`).
    pub visible: bool,
    /// Whether it's drawn in the playfield's reflection (VPX's `ReflectionEnabled`), and drawn
    /// there even off the playfield (`ForceReflection`).
    pub reflected: bool,
    pub force_reflection: bool,
}

pub struct World {
    pub vpx: Arc<VPX>,
    /// Which of the table's views is shown (scripts ask: `ShowDT`).
    pub view: crate::camera::View,
    pub gravity: TableGravity,
    pub table_name: String,
    pub parts: Vec<Part>,
    pub collections: Vec<Collection>,
    pub global: *mut Obj,
    pub table: *mut Obj,
    pub debug: *mut Obj,
    /// The fence's `CreateObject`.
    pub fence: *mut Obj,
    pub balls: HashMap<BallId, *mut Obj>,
    pub active_ball: Option<BallId>,
    /// Balls destroyed while the script may still hold them: VPX's ball
    /// objects outlive their balls, reading back their last state.
    pub gone_balls: HashMap<BallId, physics_compat::hitball::HitBall>,
    /// Plungers' pull speeds the script set (VPX's `PullSpeed`, which `PullBack` reads), by part.
    pub plunger_pull_speeds: HashMap<u32, f32>,
    pub next_ball: u32,
    pub timers: Timers,
    /// Properties the script set that nothing else models yet, by part and
    /// member id: read back as written.
    pub kept: HashMap<(u64, i32), Kept>,
    /// Flipper lengths the script set, by part: VPX keeps them and nothing
    /// more (the flipper was made before the script started).
    pub lengths: HashMap<u32, f32>,
    /// The table's DirectInput key codes for scripts (`LeftFlipperKey`...).
    pub keys: crate::dik::Keys,
    /// The table file's folder and name, and the file itself.
    pub table_dir: Option<PathBuf>,
    pub table_file: String,
    pub table_path: Option<PathBuf>,
    /// The player shows a backglass window or a score window (for the
    /// backglass file's score panel): B2S.Server reads the table's backglass
    /// file for them.
    pub backglass_window: bool,
    /// The backglass picture's grill shows (VPX's `ShowGrill`), else it's cut off.
    pub show_grill: bool,
    /// The windows a PinUP pack can draw in, by their size in pixels: the topper's, the
    /// backglass's, the score display's.
    pub pup_windows: [Option<(u32, u32)>; 3],
    /// VPX 5436's PinUP settings (`[Plugin.PUP]`): packs on or off, their folder, the main volume and
    /// the windows' padding.
    pub pup_settings: crate::pup::PupSettings,
    /// VPinMAME's settings for each game from our file (`machine_settings`).
    pub machine_settings: Arc<crate::machine_settings::MachineSettings>,
    /// VPX 5436's `[Plugin.PinMAME]` settings: sound, cheat mode, PinMAME folder.
    pub pinmame_settings: crate::machine_settings::PinmameSettings,
    /// What wakes the game thread: PinMAME's helper wakes it too, when
    /// solenoids change.
    pub waker: Option<Arc<crate::waker::Waker>>,
    /// The table's backglass file, once B2S.Server asked for it, and what
    /// it showed last.
    pub backglass: Option<crate::b2s::Loading>,
    pub backglass_now: Option<Arc<crate::b2s::BackglassNow>>,
    /// The backglass as it plays: its lights and animations.
    pub backglass_show: Option<crate::b2s::Show>,
    pub started: Instant,
    /// `SaveValue`s, for this session.
    /// The values the script keeps between plays (`SaveValue`), as VPX keeps them.
    pub saved_values: crate::saved_values::SavedValues,
    /// The settings' table options by name, the last of a name counting (`Table.Option`).
    pub table_options: Vec<(String, String)>,
    /// The settings' `[Controller]` by name, the last of a name counting (`controller.vbs`'s).
    pub controller_settings: Vec<(String, String)>,
    /// The settings' `[Plugin.AltSound]` (VPX 5436's: `Enable`, `Folder`), by name.
    pub altsound_settings: Vec<(String, String)>,
    /// The table's day and night as its script set it (`Table.NightDay`, VPX's
    /// `m_globalEmissionScale`), 0 to 1; the table's own until then.
    pub table_night_day: Option<f32>,
    /// The scene's light level when the player chose one (VPX's `SceneLighting`): the global
    /// `NightDay` reads it, the table's own otherwise.
    pub light_level: Option<f32>,
    /// A plunger's `PullBack` eases forward again after a second (VPX's `PlungerRetract`).
    pub plunger_retract: bool,
    /// The table's playfield reflection strength as a script set it (`PlayfieldReflectionStrength`):
    /// VPX 5436 keeps it in the table and draws nothing differently for it during play.
    pub table_reflection_strength: Option<f32>,
    /// The backdrop pictures' names as a script set them (`BackdropImage_DT`, `_FS`, `_FSS`):
    /// VPX 5436 keeps them in the table, the backdrop drawn once at start, so nothing changes on screen.
    pub backdrop_images: [Option<String>; 3],
    /// VPX 10.8.0's counting of nudges toward a tilt, when the settings ask for it.
    pub nudge_count: Option<crate::nudge_count::NudgeCount>,
    /// The settings' `[Plugin.Serum]` (`Enable`, `SerumPath`) and `[Plugin.VNI]` (`Enable`,
    /// `VniPath`), by name.
    pub serum_settings: Vec<(String, String)>,
    pub vni_settings: Vec<(String, String)>,
    /// The settings' `[Plugin.AlphaDMD]` (`Enable`): a segment game's digits drawn for its
    /// colour file, as VPX 5436's AlphaDMD draws them.
    pub alphadmd_settings: Vec<(String, String)>,
    /// The score display's frames written down for a colouring, where `VPI_FRAME_DUMP` asks.
    pub frame_dump: Option<crate::frame_dump::FrameDump>,
    /// The game's event stream (VPX 5436's), for the PinUP pack and the toys.
    pub events: crate::events::EventStream,
    /// The cabinet's toys while the game runs (`vpi-dof`), the settings' `[Plugin.DOF]`,
    /// and a configuration folder given in place of the usual ones (the headless runner's `--dof`).
    pub dof: Option<crate::dof::Dof>,
    pub dof_settings: Vec<(String, String)>,
    pub dof_folder: Option<PathBuf>,
    /// A cabinet's real DMD panel while the game runs (`vpi-dmd`): the settings'
    /// `[Plugin.DMDUtil]`, whether it's on, and a folder for a check's pictures of libdmdutil's
    /// virtual panel (the headless runner's `--dmd-panel`).
    pub dmd_panel: Option<crate::dmd_panel::DmdPanel>,
    /// A real DMD panel through a VPinMAME display device (`dmd_device`): the device the settings
    /// name, and `vpi-dmddevice` driving it once the game shows something.
    pub dmd_device_setup: Option<crate::dmd_device::Setup>,
    pub dmd_device: Option<crate::dmd_device::DmdDevice>,
    /// AlphaDMD's picture of the segment displays for the panel, and the displays' frame it's of.
    pub alpha_render: Option<(u64, std::sync::Arc<crate::dmd::DmdFrame>)>,
    pub dmdutil_settings: Vec<(String, String)>,
    pub dmd_panel_on: bool,
    pub dmd_panel_check: Option<PathBuf>,
    /// What the toys are told, kept for a check (the headless runner's `--toys`).
    pub toys_record: Option<crate::dof::ToysRecord>,
    pub ball_user_values: HashMap<BallId, Kept>,
    /// Each ball's look (VPX's `Ball::m_d`), from the table's settings when
    /// it's first seen, as scripts change it.
    pub ball_looks: HashMap<BallId, BallLook>,
    /// The table's settings a new ball takes (VPX's `Ball::RenderSetup`).
    pub ball_table_look: BallLook,
    /// The script asked to quit (`QuitPlayer`).
    pub quit_requested: bool,
    /// Primitives' placements, once a script has changed one.
    pub placements: HashMap<u32, Placement>,
    /// The table's materials, as scripts read and change them.
    pub materials: crate::look::Materials,
    /// The colour grade picture the script chose (`Table.ColorGradeImage`),
    /// once it set one: its name, empty for none.
    pub color_grade: Option<String>,
    /// The bloom's strength the script set (`Table.BloomStrength`), once it
    /// set one.
    pub bloom_strength: Option<f32>,
    /// The table's images (`vpx.images`) by name in lower case.
    pub image_index: HashMap<String, u32>,
    /// The table's mirrors' names, in the scene's order (`Scene::mirrors`).
    pub mirror_names: Vec<String>,
    /// The table's refraction probes' names (`Scene::refractions`).
    pub refraction_names: Vec<String>,
    /// The table's sounds, and the line to the audio thread.
    pub sounds: crate::audio::Sounds,
    /// Every collider each part made, by part.
    pub part_colliders: HashMap<u32, Vec<u32>>,
    /// VPinMAME's controller and its machine, once the script asks for it
    ///.
    pub controller: Option<Box<crate::script::controller::ScriptController>>,
    /// PinUP Player's object and the table's pack, once the script asks for it.
    pub pup: Option<Box<crate::script::pup::ScriptPup>>,
    /// Every FlexDMD the script made (each UltraDMD has one), which the game steps.
    pub flexdmds: Vec<crate::flexdmd::FlexRef>,
    /// The score display's size as the script set it (VPX's `DMDWidth`, `DMDHeight`).
    pub dmd_size: [i32; 2],
    /// The newest display frame the script sent (`DMDPixels`, `DMDColoredPixels`).
    pub dmd_frame: Option<Arc<crate::dmd::DmdFrame>>,
    /// The running machine's own frame, with its number in the link, for a
    /// script that sends none.
    pub machine_dmd: Option<(u32, Arc<crate::dmd::DmdFrame>)>,
    /// Display frames made so far: the newest one's number.
    pub dmd_frames: u64,
    /// VPX 5436's DMD upscaler as the settings ask for it (`[Plugin.UpscaleDMD]`), and whether
    /// it scales each picture at once rather than a frame late on its own thread.
    pub upscale_mode: crate::upscale::Mode,
    pub upscale_at_once: bool,
    /// The upscaler, started with the first display frame.
    pub upscaler: Option<crate::upscale::Upscaler>,
    /// The running machine's segment displays, with the count of their frames
    /// in the link they were read at.
    pub machine_segments: Option<(u64, Arc<crate::segments::SegmentsNow>)>,
    /// A FlexDMD's segments as last shown: its mode, its values, and them made displays.
    pub flex_segments: Option<(i32, Vec<u16>, Arc<crate::segments::SegmentsNow>)>,
    /// Segment display states made so far: the newest one's number.
    pub segment_frames: u64,
    /// Frames prepared so far (VPX's `m_overall_frames`).
    pub frame_index: u32,
}

/// A primitive's placement, as its script properties set it.
#[derive(Clone, Debug)]
pub struct Placement {
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub rot_and_tra: [f32; 9],
    /// Changed since the table was loaded.
    pub changed: bool,
}

/// The kind of object VPX makes for each part type.
fn class_of(item: &GameItemEnum) -> Option<&'static Class> {
    let name = match item {
        GameItemEnum::Wall(_) => "Wall",
        GameItemEnum::Flipper(_) => "Flipper",
        GameItemEnum::Timer(_) => "Timer",
        GameItemEnum::Plunger(_) => "Plunger",
        GameItemEnum::TextBox(_) => "Textbox",
        GameItemEnum::Bumper(_) => "Bumper",
        GameItemEnum::Trigger(_) => "Trigger",
        GameItemEnum::Light(_) => "Light",
        GameItemEnum::Kicker(_) => "Kicker",
        GameItemEnum::Decal(_) => "Decal",
        GameItemEnum::Gate(_) => "Gate",
        GameItemEnum::Spinner(_) => "Spinner",
        GameItemEnum::Ramp(_) => "Ramp",
        GameItemEnum::Reel(_) => "DispReel",
        GameItemEnum::LightSequencer(_) => "LightSeq",
        GameItemEnum::Primitive(_) => "Primitive",
        GameItemEnum::Flasher(_) => "Flasher",
        GameItemEnum::Rubber(_) => "Rubber",
        GameItemEnum::HitTarget(_) => "HitTarget",
        GameItemEnum::Ball(_) => "Ball",
        GameItemEnum::PartGroup(_) => "PartGroup",
        GameItemEnum::Generic(..) => return None,
    };
    CLASSES.iter().find(|c| c.name == name)
}

impl World {
    /// The script's view of a table, before play starts. `engine` gives the
    /// colliders that are parts; `objects` collects every object made, to be
    /// freed when play ends.
    pub fn new(vpx: Arc<VPX>, engine: &PhysicsEngine, sounds: crate::audio::Sounds, view: crate::camera::View, physics_set_gravity: Option<(f32, f32, f32)>) -> World {
        let mut colliders: HashMap<u32, u32> = HashMap::new();
        let mut part_colliders: HashMap<u32, Vec<u32>> = HashMap::new();
        for (i, c) in engine.colliders().iter().enumerate() {
            if (c.owner.id as usize) < vpx.gameitems.len() {
                part_colliders.entry(c.owner.id).or_default().push(i as u32);
            }
            let part = match &c.shape {
                Shape::Flipper(f) => f.part,
                Shape::Plunger(p) => p.part,
                Shape::Kicker(k) => k.part,
                Shape::Gate(g) => g.part,
                Shape::Spinner(s) => s.part,
                _ => continue,
            };
            colliders.entry(part.0).or_insert(i as u32);
        }

        let materials = crate::look::Materials::new(&vpx);
        let image_index = crate::look::images_by_name(&vpx);
        let find_image = |name: &str| if name.is_empty() { None } else { image_index.get(&name.to_ascii_lowercase()).copied() };
        let mirror_names = crate::scene::Mirror::of_table(&vpx).into_iter().map(|m| m.name).collect();
        let refraction_names = crate::scene::Refraction::of_table(&vpx).into_iter().map(|r| r.name).collect();

        let mut timers = Timers::default();
        let mut parts: Vec<Part> = Vec::with_capacity(vpx.gameitems.len());
        for (i, item) in vpx.gameitems.iter().enumerate() {
            let class = class_of(item);
            let obj = Obj::new(Target::Part(i as u32), class.and_then(|c| c.interface).unwrap_or(&I_V_P_DEBUG));
            let timer = item.timer().map(|t| {
                // VPX's HitTimer: next fire one interval after play starts
                let interval = Timers::clamp_interval(t.interval);
                timers.all.push(Timer { part: i as u32, interval, stored: t.interval, next_fire: interval as u32, enabled: t.is_enabled });
                if t.is_enabled {
                    timers.running.push(timers.all.len() - 1);
                }
                timers.all.len() - 1
            });
            let (visible, is_dropped) = match item {
                GameItemEnum::Light(l) => (l.visible.unwrap_or(true), false),
                GameItemEnum::Wall(w) => (w.is_top_bottom_visible, false),
                GameItemEnum::HitTarget(t) => (t.is_visible, t.is_dropped),
                // VPX's load (pintable.cpp, build 5436): before 10.8 the playfield mesh was always drawn
                GameItemEnum::Primitive(p) => (p.is_visible || (vpx.version.u32() < 1080 && p.is_playfield()), false),
                GameItemEnum::Rubber(r) => (r.is_visible, false),
                GameItemEnum::Ramp(r) => (r.is_visible, false),
                GameItemEnum::Flipper(f) => (f.is_visible, false),
                GameItemEnum::Flasher(f) => (f.is_visible, false),
                GameItemEnum::Reel(r) => (r.is_visible, false),
                GameItemEnum::Gate(g) => (g.is_visible, false),
                GameItemEnum::Spinner(s) => (s.is_visible, false),
                GameItemEnum::Plunger(p) => (p.is_visible, false),
                GameItemEnum::Trigger(t) => (t.is_visible, false),
                _ => (true, false),
            };
            let light = match item {
                GameItemEnum::Light(l) => {
                    let mut rt = crate::light::LightRt::new(l);
                    // VPX's load: before 10.8 only a bulb light shone on the ball
                    rt.on_ball &= l.is_bulb_light || vpx.version.u32() >= 1080;
                    Some(rt)
                }
                _ => None,
            };
            let reel = match item {
                GameItemEnum::Reel(r) => {
                    let image = crate::look::image_named(&vpx, &r.image).map(|i| &vpx.images[i]);
                    Some(crate::reel::ReelRt::new(r, image))
                }
                _ => None,
            };
            let text_box = match item {
                GameItemEnum::TextBox(t) => Some(crate::textbox::TextBoxRt::new(t)),
                _ => None,
            };
            let (target, trigger, bumper) = match item {
                GameItemEnum::HitTarget(t) => (Some(crate::anim::TargetAnim::new(t)), None, None),
                GameItemEnum::Trigger(t) => (None, Some(crate::anim::TriggerAnim::new(t)), None),
                GameItemEnum::Bumper(b) => (None, None, Some(crate::anim::BumperAnim::new(b))),
                _ => (None, None, None),
            };
            let (main_material, side_material) = file_materials(item);
            let look = crate::look::PartLook {
                file_material: main_material.and_then(|m| materials.find(m)),
                file_side_material: side_material.and_then(|m| materials.find(m)),
                ..Default::default()
            };
            // VPX's GetLight: the light of that name
            let light_named = |name: Option<&str>| {
                name.filter(|n| !n.is_empty()).and_then(|n| {
                    vpx.gameitems.iter().position(|g| matches!(g, GameItemEnum::Light(l) if l.name.eq_ignore_ascii_case(n))).map(|p| p as u32)
                })
            };
            let flasher = match item {
                GameItemEnum::Flasher(f) => {
                    let center = crate::look::bounds_center(&crate::look::flasher_outline(f));
                    Some(crate::look::FlasherRt::new(f, center, &find_image, light_named(f.light_map.as_deref())))
                }
                _ => None,
            };
            let primitive = match item {
                GameItemEnum::Primitive(pr) => Some(crate::look::PrimitiveRt::new(pr, light_named(pr.light_map.as_deref()))),
                _ => None,
            };
            parts.push(Part {
                name: item.name().to_string(),
                class,
                obj,
                collider: colliders.get(&(i as u32)).copied(),
                timer,
                event_collections: Vec::new(),
                single_events: true,
                user_value: None,
                visible,
                light,
                reel,
                text_box,
                light_seq: None,
                ls_locked: false,
                ls_on: visible,
                target,
                trigger,
                bumper,
                hit_threshold: 0.0,
                is_dropped,
                collidable: match item {
                    GameItemEnum::Wall(w) => w.is_collidable,
                    GameItemEnum::Ramp(r) => r.is_collidable,
                    GameItemEnum::Rubber(r) => r.is_collidable,
                    GameItemEnum::Kicker(k) => k.is_enabled,
                    GameItemEnum::Trigger(t) => t.is_enabled,
                    GameItemEnum::Bumper(b) => b.is_collidable.unwrap_or(true),
                    GameItemEnum::HitTarget(t) => t.is_collidable,
                    GameItemEnum::Gate(g) => g.is_collidable,
                    _ => true,
                },
                disabled: false,
                look,
                flasher,
                primitive,
                prim_anim: match item {
                    GameItemEnum::Primitive(pr) => crate::prim_anim::PrimAnim::new(pr),
                    _ => None,
                },
                ramp_shape: None,
            });
        }

        // collections: VPX's load (`Collection::LoadData`) and `TimerSetup`
        let mut collections = Vec::new();
        for (c, col) in vpx.collections.iter().enumerate() {
            let mut members = Vec::new();
            for item_name in &col.items {
                // the first part by this exact name; decals have no script object
                if let Some(p) = vpx.gameitems.iter().position(|g| g.name() == item_name && !matches!(g, GameItemEnum::Decal(_))) {
                    if col.fire_events {
                        parts[p].event_collections.push((c as u32, members.len() as i32));
                    }
                    if col.stop_single_events {
                        parts[p].single_events = false;
                    }
                    members.push(p as u32);
                }
            }
            collections.push(Collection { name: col.name.clone(), obj: Obj::new(Target::Collection(c as u32), &I_COLLECTION), parts: members });
        }

        // light sequencers find their collection when play starts (VPX's LightSeq::RenderSetup)
        let named: Vec<(&str, &[u32])> = collections.iter().map(|c| (c.name.as_str(), c.parts.as_slice())).collect();
        for (i, item) in vpx.gameitems.iter().enumerate() {
            if let GameItemEnum::LightSequencer(seq) = item {
                parts[i].light_seq = Some(crate::lightseq::LightSeqRt::new(seq, &vpx, &named));
            }
        }

        let g = &vpx.gamedata;
        let gravity = TableGravity { gravity: g.gravity, slope_min: g.angle_tilt_min, slope_max: g.angle_tilt_max, set: physics_set_gravity };
        // a new ball's look: white, and the table's pictures and strengths
        let ball_table_look = BallLook {
            color: 0xFF_FF_FF,
            image: g.ball_image.clone(),
            decal: g.ball_image_front.clone(),
            decal_mode: g.ball_decal_mode,
            bulb_scale: g.default_bulb_intensity_scale_on_ball.unwrap_or(1.0),
            reflection: 1.0,
            spherical: g.ball_spherical_mapping.unwrap_or(true),
            visible: true,
            reflected: true,
            force_reflection: false,
        };
        World {
            view,
            gravity,
            table_name: if vpx.gamedata.name.is_empty() { "Table1".into() } else { vpx.gamedata.name.clone() },
            vpx,
            parts,
            collections,
            global: Obj::new(Target::Global, global_interface()),
            table: Obj::new(Target::Table, &I_TABLE),
            debug: Obj::new(Target::Debug, &I_V_P_DEBUG),
            fence: Obj::new(Target::Fence, &crate::script::fence::I_CREATE_OBJECT),
            balls: HashMap::new(),
            active_ball: None,
            gone_balls: HashMap::new(),
            plunger_pull_speeds: HashMap::new(),
            next_ball: 0,
            timers,
            kept: HashMap::new(),
            lengths: HashMap::new(),
            keys: crate::dik::Keys::default(),
            table_dir: None,
            table_path: None,
            backglass_window: false,
            show_grill: false,
            pup_windows: [None; 3],
            pup_settings: crate::pup::PupSettings::default(),
            machine_settings: Arc::new(crate::machine_settings::MachineSettings::default()),
            pinmame_settings: crate::machine_settings::PinmameSettings::default(),
            waker: None,
            backglass: None,
            backglass_now: None,
            backglass_show: None,
            table_file: String::new(),
            started: Instant::now(),
            saved_values: crate::saved_values::SavedValues::default(),
            table_options: Vec::new(),
            controller_settings: Vec::new(),
            altsound_settings: Vec::new(),
            table_night_day: None,
            light_level: None,
            plunger_retract: false,
            table_reflection_strength: None,
            backdrop_images: [None, None, None],
            nudge_count: None,
            serum_settings: Vec::new(),
            vni_settings: Vec::new(),
            alphadmd_settings: Vec::new(),
            frame_dump: {
                // the table's achievements taken up as its world is made
                crate::achievements::start();
                crate::frame_dump::FrameDump::from_env()
            },
            events: crate::events::EventStream::default(),
            dof: None,
            dof_settings: Vec::new(),
            dof_folder: None,
            dmd_panel: None,
            dmd_device_setup: None,
            dmd_device: None,
            alpha_render: None,
            dmdutil_settings: Vec::new(),
            dmd_panel_on: false,
            dmd_panel_check: None,
            toys_record: None,
            ball_user_values: HashMap::new(),
            ball_looks: HashMap::new(),
            ball_table_look,
            quit_requested: false,
            placements: HashMap::new(),
            materials,
            color_grade: None,
            bloom_strength: None,
            image_index,
            mirror_names,
            refraction_names,
            sounds,
            part_colliders,
            controller: None,
            pup: None,
            flexdmds: Vec::new(),
            dmd_size: [0; 2],
            dmd_frame: None,
            machine_dmd: None,
            dmd_frames: 0,
            upscale_mode: crate::upscale::Mode::Disabled,
            upscale_at_once: false,
            upscaler: None,
            machine_segments: None,
            flex_segments: None,
            segment_frames: 0,
            frame_index: 0,
        }
    }

    /// A primitive's placement, from the table until a script changes it.
    pub fn placement(&mut self, p: u32) -> Option<&mut Placement> {
        if !self.placements.contains_key(&p) {
            let GameItemEnum::Primitive(prim) = &self.vpx.gameitems[p as usize] else { return None };
            self.placements.insert(
                p,
                Placement {
                    position: [prim.position.x, prim.position.y, prim.position.z],
                    size: [prim.size.x, prim.size.y, prim.size.z],
                    rot_and_tra: prim.rot_and_tra,
                    changed: false,
                },
            );
        }
        self.placements.get_mut(&p)
    }

    /// The script object of a ball, made the first time it is asked for.
    pub fn ball_object(&mut self, id: BallId) -> *mut Obj {
        *self.balls.entry(id).or_insert_with(|| Obj::new(Target::Ball(id), &crate::script::idl::I_BALL))
    }

    /// The part with this name, as VBScript matches names (ignoring case).
    pub fn part_named(&self, name: &str) -> Option<u32> {
        self.parts.iter().position(|p| p.name.eq_ignore_ascii_case(name)).map(|i| i as u32)
    }

    /// The table image with this name, ignoring case (VPX's `GetImage`;
    /// `look::images_by_name`).
    pub fn image_named(&self, name: &str) -> Option<u32> {
        if name.is_empty() {
            return None;
        }
        self.image_index.get(&name.to_ascii_lowercase()).copied()
    }

    /// The table's mirror with this name, ignoring case (VPX's `GetRenderProbe`).
    pub fn mirror_named(&self, name: &str) -> Option<u32> {
        if name.is_empty() {
            return None;
        }
        self.mirror_names.iter().position(|m| m.eq_ignore_ascii_case(name)).map(|i| i as u32)
    }

    /// The table's refraction probe with this name, ignoring case.
    pub fn refraction_named(&self, name: &str) -> Option<u32> {
        if name.is_empty() {
            return None;
        }
        self.refraction_names.iter().position(|r| r.eq_ignore_ascii_case(name)).map(|i| i as u32)
    }
}

/// The materials a part's two looks use in the table file: its main one
/// (a wall's top, a flipper's body) and its second (a wall's sides, a
/// flipper's rubber).
pub(crate) fn file_materials(item: &GameItemEnum) -> (Option<&str>, Option<&str>) {
    match item {
        GameItemEnum::Primitive(p) => (Some(&p.material), None),
        GameItemEnum::Wall(w) => (Some(&w.top_material), Some(&w.side_material)),
        GameItemEnum::Ramp(r) => (Some(&r.material), None),
        GameItemEnum::Rubber(r) => (Some(&r.material), None),
        GameItemEnum::HitTarget(t) => (Some(&t.material), None),
        GameItemEnum::Flipper(f) => (Some(&f.material), Some(&f.rubber_material)),
        GameItemEnum::Spinner(s) => (Some(&s.material), None),
        GameItemEnum::Trigger(t) => (Some(&t.material), None),
        GameItemEnum::Kicker(k) => (Some(&k.material), None),
        GameItemEnum::Gate(g) => (Some(&g.material), None),
        GameItemEnum::Plunger(p) => (Some(&p.material), None),
        _ => (None, None),
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // the game ends: PinMAME saves its NVRAM, and its program ends
        let controller = self.controller.take();
        // SAFETY: the script engine is closed before the world goes, and
        // these objects were made by `Obj::new`
        unsafe {
            if let Some(mut c) = controller {
                c.state.stop();
                for o in c.objects() {
                    Obj::free(o);
                }
            }
            if let Some(p) = self.pup.take() {
                Obj::free(p.obj);
            }
            for p in &self.parts {
                Obj::free(p.obj);
            }
            for c in &self.collections {
                Obj::free(c.obj);
            }
            for &o in self.balls.values() {
                Obj::free(o);
            }
            for o in [self.global, self.table, self.debug, self.fence] {
                Obj::free(o);
            }
        }
    }
}
