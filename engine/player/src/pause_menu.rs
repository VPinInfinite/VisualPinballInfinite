//! The pause menu, ours, following VPX 5436's in-game menu where it
//! does the same things (`src/ui/live/ingameui`, GPLv3+). P — VPX 5436's Pause — or a gamepad's
//! Back button — its InGameUI — pauses the game as VPX's pause does (`Player::SetPlayState`: the
//! physics and the script's timers held, the time shifted on when play goes on, the table's
//! `Paused` and `UnPaused` events, its music paused; ours holds the table's other sounds too) and
//! shows the menu over the table: Resume; a level for each kind of sound the table has played,
//! under VPX 5436's names (its playfield and backglass volumes, its audio sources' gains) and one
//! of ours for the table's music; the table's own options (`Table.Option`), each change told to
//! its script as VPX tells it (`OptionEvent` 1, and 3 when the menu closes); its rules (VPX's
//! table rules, when it holds them); its view (VPX 5436's view page, `PointOfViewSettingsPage`, in
//! plain words: the values its view mode has, the table redrawn as each changes, kept in the
//! table's own settings file as what differs from its own view, "Back to its own view" going
//! back); for a ROM machine with a dot-matrix display, recording its display for colouring it (ours:
//! every new frame from then on, as PPUC's colouring editor reads a frame dump, until it's stopped
//! there or play ends); and Quit. It's moved through as VPX 5436's menu is — the
//! magna-saves up and down (`UIUp`, `UIDown`), the flippers left and right (`UILeft`, `UIRight`),
//! held ones repeating as VPX's do — and on a keyboard with the arrow keys, Enter or Start to
//! choose, Esc or Backspace back. What changed is kept when the menu closes, with no Save to
//! press: the levels in the settings file, or the table's own where it sets
//! them; the options in the table's own, as VPX keeps them (`SaveMode::Table`). "Reset to
//! defaults" shows on a page once something there isn't at its default:
//! every level back to all of it, every option to the table's own default — told to the script as
//! a change, as VPX 5436 dispatches a reset. Test mode shows the menu without pausing, for pictures of it,
//! and keeps nothing.
//!
//! Esc opens the menu too, its Quit picked, so Esc then Enter ends play
//! and Esc again plays on (`[VPI]` `PauseMenuBeforeQuit`, on unless off; as VPX 10.8.0's Esc opened
//! its pause menu — VPX 5436's ends play at once — and, as VPX's, not when a front end starts the
//! table with `-minimized`); the mouse works in it — pointing picks a row, a click chooses it, a
//! level's bar is set where it's clicked or dragged, the wheel scrolls, the right or back button
//! steps back (`Pointer`); and a Controls page shows what the table, the game and the player are on
//! now, each with its keys and buttons (`ControlLine`, made by the window thread, which knows the
//! controllers).
//!
//! The menu lives on the game thread, which takes its presses and calls the script; each frame's
//! snapshot carries what it shows (`View`), and the window thread draws that into a picture
//! (`picture`) when it changes, laid over the frame by `render::menu`. The rules are VPX's
//! Markdown: their text, and the table's own pictures they name (`![Rules](rulescard)`, a
//! table's rules card), decoded when their page first shows.

use std::path::{Path, PathBuf};
use crate::camera::{self, Mode, ViewSetup, cm_to_vpu, vpu_to_cm};
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use vpi_settings::Change;

use crate::audio::Sounds;
use crate::pup::fonts::{PupFont, TextStyle};
use crate::pup::label::XAlign;
use crate::render::MenuPicture;
use crate::script::api::TableOptionDef;

/// The player's own actions on the game thread's input channel, as codes no key has (DirectInput's
/// stop at 0xED): the input thread sends them for VPX 5436's actions; test mode's scripted keys
/// may name them (`input::parse_key_script`).
pub mod code {
    /// VPX 5436's `Pause` (P): the menu opens, or closes.
    pub const PAUSE: u8 = 0xF0;
    /// VPX 5436's `InGameUI` (a gamepad's Back button): the same.
    pub const IN_GAME_UI: u8 = 0xF1;
    /// VPX 5436's menu actions: `UIUp` and `UIDown` (the magna-saves), `UILeft` and `UIRight`
    /// (the flippers).
    pub const UP: u8 = 0xF2;
    pub const DOWN: u8 = 0xF3;
    pub const LEFT: u8 = 0xF4;
    pub const RIGHT: u8 = 0xF5;
    /// Esc, VPX's `ExitGame`, when the settings have it open the menu first: the menu opens with
    /// Quit the table picked.
    pub const EXIT: u8 = 0xF6;
    /// The table's window lost the keyboard to another program's window, or has it again: play
    /// pauses meanwhile, as VPX's `IsPlaying` holds it while its window isn't focused.
    pub const FOCUS_LOST: u8 = 0xF7;
    pub const FOCUS_BACK: u8 = 0xF8;

    /// Whether a code is one of these.
    pub fn is_ours(dik: u8) -> bool {
        (PAUSE..=FOCUS_BACK).contains(&dik)
    }
}

/// The mouse in the menu, from the window thread, as the menu's own rows
/// and places in them (`Layout::at`), each with how many rows the picture it was aimed at had: a
/// row met or left out since (a Reset to defaults showing) moves those below it, and the press,
/// aimed at a picture gone, is left alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    /// Over a row: it's picked.
    Over(usize, usize),
    /// A click on a row, and where on it.
    Click(usize, Spot, usize),
    /// A level's or an option's bar dragged, to a place along it, 0 to 1.
    Drag(usize, f32, usize),
    /// The wheel turned, notches away from the user positive: the rules and controls scroll.
    Wheel(i32),
    /// The right button, or the mouse's back button: a step back.
    Back,
}

/// Where on a row a click was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spot {
    Row,
    /// On its bar, a place along it, 0 to 1.
    Bar(f32),
    /// On its value's left half, or its right: a step down, or up.
    Less,
    More,
}

/// The mouse's news waiting for the game thread, which takes it as the menu's held presses are
/// looked at (`Menu::pointed`).
static POINTER: Mutex<Vec<Pointer>> = Mutex::new(Vec::new());

/// The mouse did something in the menu (the window thread).
pub fn point(p: Pointer) {
    POINTER.lock().unwrap_or_else(|e| e.into_inner()).push(p);
}

/// A line of the Controls page: what it does, and the keys and buttons it's on now.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlLine {
    /// A heading above it, when it begins a group.
    pub section: Option<String>,
    pub label: String,
    pub keys: String,
}

/// The Controls page's lines, as the window thread last made them.
static CONTROLS: Mutex<Vec<ControlLine>> = Mutex::new(Vec::new());

/// The Controls page's lines from now on (the window thread, at the start and whenever a
/// controller comes or goes).
pub fn set_controls(lines: Vec<ControlLine>) {
    *CONTROLS.lock().unwrap_or_else(|e| e.into_inner()) = lines;
}

/// A table's own controls, beyond the game's usual ones: what its script reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extra {
    pub label: &'static str,
    /// VPX 5436's actions, by their names in the settings.
    pub actions: &'static [&'static str],
}

/// What a table's own script reads beyond the flippers, plunger, start, coins and nudges, by the
/// names VPX hands scripts for the keys (`LeftMagnaSave` and the rest), comments left out; a ROM
/// machine's coin door and service buttons where it starts one (`LoadVPM`), which VPX's shared
/// scripts give the machine.
pub fn table_extras(script: &str) -> Vec<Extra> {
    let code = without_comments(script).to_lowercase();
    let has = |names: &[&str]| names.iter().any(|n| code.contains(n));
    let mut out = Vec::new();
    for (names, extra) in [
        (&["leftmagnasave", "rightmagnasave"][..], Extra { label: "Magna-saves", actions: &["LeftMagna", "RightMagna"] }),
        (&["lockbarkey"][..], Extra { label: "Fire button", actions: &["Lockbar"] }),
        (&["stagedleftflipperkey", "stagedrightflipperkey"][..], Extra { label: "Upper flippers", actions: &["LeftStagedFlipper", "RightStagedFlipper"] }),
        (&["mechanicaltilt"][..], Extra { label: "Tilt", actions: &["Tilt"] }),
        (&["addcreditkey2"][..], Extra { label: "Second coin", actions: &["Credit2"] }),
        (&["loadvpm"][..], Extra { label: "Coin door", actions: &["CoinDoor"] }),
        (&["loadvpm"][..], Extra { label: "Service buttons", actions: &["Service1", "Service2", "Service3", "Service4"] }),
    ] {
        if has(names) {
            out.push(extra);
        }
    }
    out
}

/// A script's text with its comments taken out: from a `'` outside a string to the line's end,
/// and `Rem` lines.
fn without_comments(script: &str) -> String {
    let mut out = String::with_capacity(script.len());
    for line in script.lines() {
        let t = line.trim_start();
        if t.get(..3).is_some_and(|w| w.eq_ignore_ascii_case("rem")) && t[3..].chars().next().is_none_or(char::is_whitespace) {
            out.push('\n');
            continue;
        }
        let mut in_string = false;
        for c in line.chars() {
            match c {
                '"' => in_string = !in_string,
                '\'' if !in_string => break,
                _ => {}
            }
            out.push(c);
        }
        out.push('\n');
    }
    out
}

/// The Controls page's lines: the table's own controls, the game's, then the player's own, each
/// with the keys and buttons it's on now — `names` gives an action's, by its name in the settings,
/// as the keyboard, the mouse and the controllers connected name them; Esc says what it does.
pub fn control_lines(extras: &[Extra], names: &dyn Fn(&str) -> Vec<String>, esc_opens_menu: bool) -> Vec<ControlLine> {
    // each action's keys, the actions side by side (left, right)
    let keys = |actions: &[&str]| -> String {
        let each: Vec<String> = actions
            .iter()
            .map(|a| {
                let n = names(a);
                if n.is_empty() { "none".to_string() } else { n.join(" or ") }
            })
            .collect();
        each.join("  \u{b7}  ")
    };
    let mut lines = Vec::new();
    let group = |section: Option<&'static str>, items: &[(&str, &[&str])], lines: &mut Vec<ControlLine>| {
        for (k, (label, actions)) in items.iter().enumerate() {
            lines.push(ControlLine { section: section.filter(|_| k == 0).map(str::to_string), label: (*label).to_string(), keys: keys(actions) });
        }
    };
    let table: Vec<(&str, &[&str])> = extras.iter().map(|e| (e.label, e.actions)).collect();
    if !table.is_empty() {
        group(Some("This table"), &table, &mut lines);
    }
    group(
        Some("Playing"),
        &[
            ("Flippers", &["LeftFlipper", "RightFlipper"]),
            ("Plunger", &["LaunchBall"]),
            ("Start", &["Start"]),
            ("Coin", &["Credit1"]),
            ("Nudge", &["LeftNudge", "CenterNudge", "RightNudge"]),
        ],
        &mut lines,
    );
    // P and a gamepad's Back both open the menu: whichever are on something, as one list
    let pause: Vec<String> = ["Pause", "InGameUI"].iter().flat_map(|a| names(a)).collect();
    lines.push(ControlLine { section: Some("The player".into()), label: "Pause menu".into(), keys: if pause.is_empty() { "none".into() } else { pause.join(" or ") } });
    group(None, &[("Volume down, up", &["VolumeDown", "VolumeUp"]), ("Screenshot", &["Screenshot"])], &mut lines);
    lines.push(ControlLine { section: None, label: "Quit".into(), keys: if esc_opens_menu { "Esc, then Quit the table".into() } else { "Esc".into() } });
    lines
}

/// Whether the menu shows, and when it last closed (ms on the process's clock), for the window
/// thread: Esc steps back in the menu rather than ending play, including the press that closed it.
static SHOWN: AtomicBool = AtomicBool::new(false);
static CLOSED_AT_MS: AtomicU64 = AtomicU64::new(0);

/// The furthest the rules can scroll, in steps, as the last picture of them found it.
static RULES_MOST_TOP: AtomicUsize = AtomicUsize::new(usize::MAX);

fn clock_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

/// Whether the menu shows, or closed less than half a second ago.
pub fn shown_lately() -> bool {
    let closed = CLOSED_AT_MS.load(Ordering::Acquire);
    SHOWN.load(Ordering::Acquire) || (closed != 0 && clock_ms() < closed + 500)
}

fn set_shown(shown: bool) {
    SHOWN.store(shown, Ordering::Release);
    if !shown {
        CLOSED_AT_MS.store(clock_ms(), Ordering::Release);
    }
}

/// What the menu knows of the table, and where it keeps what changes.
#[derive(Clone, Debug, Default)]
pub struct Setup {
    /// The table's name, and who made it.
    pub title: String,
    pub byline: String,
    /// The table's rules (VPX's `TableRules`), when it holds them.
    pub rules: Option<String>,
    /// The main settings file, and the table's own (`<table>.ini`), where what changes is kept.
    pub settings_file: Option<PathBuf>,
    pub table_file: Option<PathBuf>,
    /// Test mode: the menu shows without the game pausing, its changes kept nowhere.
    pub picture_only: bool,
    /// The table's view, for its page (none: no view page).
    pub view: Option<Viewing>,
}

/// The table's view for its page: which of its setups is drawn, as it plays now, and its own — the
/// table's with the main settings file's over it, neither its own file's nor its camera file's —
/// which "Back to its own view" goes back to and what's kept is kept against
/// (`ViewSetup::overrides_from`).
#[derive(Clone, Debug)]
pub struct Viewing {
    pub which: camera::View,
    pub now: ViewSetup,
    pub own: ViewSetup,
}

/// One of the view page's values: VPX 5436's view page's (`PointOfViewSettingsPage`), in plain words.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cam {
    /// VPX's view mode: legacy (shown as classic), camera or window.
    Mode,
    Fov,
    /// `LookAt`; in the legacy mode the view's tilt (VPX's page: "Actually inclination").
    LookAt,
    /// `Layback`, a stretch for depth.
    Layback,
    /// The eye's place: `PlayerZ` its height, `PlayerY` its distance, `PlayerX` left and right.
    Height,
    Distance,
    Sideways,
    /// The scene's scale, all of it together (VPX's "Lock XYZ Scale", on) or one way at a time.
    Size,
    Width,
    Length,
    Depth,
    /// `HOfs` and `VOfs`: the picture shifted.
    PictureSideways,
    PictureUpDown,
    /// `WindowTop`, `WindowBot`: in window mode, the screen's height above the playfield's top and
    /// its bottom.
    WindowTop,
    WindowBottom,
    /// `Rotation`, in quarter turns.
    Turn,
}

impl Cam {
    /// The values a mode has, the first few up front and the rest under More, as VPX 5436's view
    /// page lists them for each (`BuildPage`).
    fn of(mode: Mode) -> (&'static [Cam], &'static [Cam]) {
        match mode {
            Mode::Legacy => (&[Cam::Mode, Cam::Fov, Cam::LookAt, Cam::Layback, Cam::Height, Cam::Distance, Cam::Size], &[Cam::Width, Cam::Length, Cam::Depth, Cam::Sideways, Cam::Turn]),
            Mode::Camera => (&[Cam::Mode, Cam::Fov, Cam::LookAt, Cam::Height, Cam::Distance, Cam::Size], &[Cam::Width, Cam::Length, Cam::Depth, Cam::Sideways, Cam::PictureSideways, Cam::PictureUpDown, Cam::Turn]),
            Mode::Window => (&[Cam::Mode, Cam::PictureSideways, Cam::PictureUpDown, Cam::Size], &[Cam::Width, Cam::Length, Cam::WindowTop, Cam::WindowBottom, Cam::Turn]),
        }
    }

    fn label(self, mode: Mode) -> &'static str {
        match self {
            Cam::Mode => "View mode",
            Cam::Fov => "Field of view",
            Cam::LookAt if mode == Mode::Legacy => "Tilt",
            Cam::LookAt => "Look at",
            Cam::Layback => "Depth stretch",
            Cam::Height => "Height",
            Cam::Distance => "Distance",
            Cam::Sideways => "Left and right",
            Cam::Size => "Size",
            Cam::Width => "Width",
            Cam::Length => "Length",
            Cam::Depth => "Depth",
            Cam::PictureSideways => "Picture left and right",
            Cam::PictureUpDown => "Picture up and down",
            Cam::WindowTop => "Screen's top above the playfield",
            Cam::WindowBottom => "Screen's bottom above the playfield",
            Cam::Turn => "Turn",
        }
    }

    /// Its least, its most and a press's step, in the setup's own units: VPX 5436's ranges
    /// (`Settings_properties.inl`, `TableOverride` `ViewDT…`, `ViewCab…`).
    fn range(self, which: camera::View) -> (f32, f32, f32) {
        match self {
            Cam::Mode => (0.0, 2.0, 1.0),
            Cam::Fov => (25.0, 90.0, 0.5),
            Cam::LookAt => (0.0, 100.0, 0.5),
            Cam::Layback => (if which == camera::View::Cabinet { 0.0 } else { -90.0 }, 90.0, 0.5),
            Cam::Height | Cam::Distance => (cm_to_vpu(-100.0), cm_to_vpu(100.0), cm_to_vpu(0.5)),
            Cam::Sideways => (cm_to_vpu(-30.0), cm_to_vpu(30.0), cm_to_vpu(0.5)),
            Cam::Size | Cam::Width | Cam::Length | Cam::Depth => (0.5, 1.5, 0.01),
            Cam::PictureSideways => (-30.0, 30.0, 0.5),
            Cam::PictureUpDown => (-20.0, 50.0, 0.5),
            Cam::WindowTop | Cam::WindowBottom => (0.0, cm_to_vpu(50.0), cm_to_vpu(0.5)),
            Cam::Turn => (0.0, 270.0, 90.0),
        }
    }

    /// It goes round from its last value to its first, as a choice does, with no bar.
    fn goes_round(self) -> bool {
        matches!(self, Cam::Mode | Cam::Turn)
    }

    fn get(self, v: &ViewSetup) -> f32 {
        match self {
            Cam::Mode => match v.mode {
                Mode::Legacy => 0.0,
                Mode::Camera => 1.0,
                Mode::Window => 2.0,
            },
            Cam::Fov => v.fov,
            Cam::LookAt => v.look_at,
            Cam::Layback => v.layback,
            Cam::Height => v.view_z,
            Cam::Distance => v.view_y,
            Cam::Sideways => v.view_x,
            Cam::Size | Cam::Width => v.scene_scale.x,
            Cam::Length => v.scene_scale.y,
            Cam::Depth => v.scene_scale.z,
            Cam::PictureSideways => v.view_h_ofs,
            Cam::PictureUpDown => v.view_v_ofs,
            Cam::WindowTop => v.window_top_z_ofs,
            Cam::WindowBottom => v.window_bottom_z_ofs,
            Cam::Turn => v.viewport_rotation,
        }
    }

    fn set(self, v: &mut ViewSetup, x: f32) {
        match self {
            Cam::Mode => {
                v.mode = match x.round() as i32 {
                    1 => Mode::Camera,
                    2 => Mode::Window,
                    _ => Mode::Legacy,
                }
            }
            Cam::Fov => v.fov = x,
            Cam::LookAt => v.look_at = x,
            Cam::Layback => v.layback = x,
            Cam::Height => v.view_z = x,
            Cam::Distance => v.view_y = x,
            Cam::Sideways => v.view_x = x,
            Cam::Size => {
                // all of it together, as VPX's locked scale moves each by the same step, held to its
                // range (the depth not in window mode, whose page has none)
                let d = x - v.scene_scale.x;
                v.scene_scale.x = x;
                v.scene_scale.y = (v.scene_scale.y + d).clamp(0.5, 1.5);
                if v.mode != Mode::Window {
                    v.scene_scale.z = (v.scene_scale.z + d).clamp(0.5, 1.5);
                }
            }
            Cam::Width => v.scene_scale.x = x,
            Cam::Length => v.scene_scale.y = x,
            Cam::Depth => v.scene_scale.z = x,
            Cam::PictureSideways => v.view_h_ofs = x,
            Cam::PictureUpDown => v.view_v_ofs = x,
            Cam::WindowTop => v.window_top_z_ofs = x,
            Cam::WindowBottom => v.window_bottom_z_ofs = x,
            Cam::Turn => v.viewport_rotation = x,
        }
    }

    /// Its value as it shows, in VPX's page's units: degrees, percent, centimetres.
    fn text(self, x: f32) -> String {
        match self {
            Cam::Mode => ["Classic", "Camera", "Window"][(x.round() as usize).min(2)].to_string(),
            Cam::Fov | Cam::Turn => format!("{x:.1}\u{b0}").replace(".0\u{b0}", "\u{b0}"),
            Cam::LookAt => format!("{x:.1}%"),
            Cam::Layback => format!("{x:.1}"),
            Cam::Height | Cam::Distance | Cam::Sideways | Cam::WindowTop | Cam::WindowBottom => format!("{:.1} cm", vpu_to_cm(x)),
            Cam::PictureSideways | Cam::PictureUpDown => format!("{x:.1} cm"),
            Cam::Size | Cam::Width | Cam::Length | Cam::Depth => format!("{:.0}%", x * 100.0),
        }
    }
}

/// A press in the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Choose,
    Back,
}

impl Nav {
    /// What a key or action does in the menu: VPX 5436's menu actions; on a keyboard the arrow
    /// keys, Enter, Space, Esc and Backspace; the launch button and Start choose, as Enter does.
    pub fn of(dik: u8) -> Option<Nav> {
        use crate::dik::default_keys as k;
        Some(match dik {
            code::UP | 0xC8 => Nav::Up,
            code::DOWN | 0xD0 => Nav::Down,
            code::LEFT | 0xCB => Nav::Left,
            code::RIGHT | 0xCD => Nav::Right,
            k::PLUNGER | k::START_GAME | k::CENTER_TILT | 0x9C => Nav::Choose,
            k::ESCAPE | 0x0E => Nav::Back,
            _ => return None,
        })
    }

    /// Whether holding it repeats it.
    fn repeats(self) -> bool {
        matches!(self, Nav::Up | Nav::Down | Nav::Left | Nav::Right)
    }
}

/// A kind of sound with a level of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// VPX's `[Player]` `SoundVolume`: the table's sounds on the playfield's speakers.
    Playfield,
    /// VPX's `MusicVolume`: everything on the backglass's speakers.
    Backglass,
    /// The machine's sound, PinMAME's or its AltSound pack's (VPX 5436's `AudioSource.PinMAME.Gain`,
    /// `AudioSource.AltSound.Gain`).
    Machine,
    /// The table's music (`PlayMusic`): ours, `[VPI]` `MusicGain`.
    Music,
    /// A PinUP pack's sound (`AudioSource.PUP.Gain`).
    Pup,
    /// Windows Media Player's files, a script's own player (`AudioSource.WMP.Gain`).
    Media,
    /// The achievements' sound as each is announced: ours, `[VPI]` `AchievementGain`.
    Achievements,
}

/// How far a press moves a level.
const STEP: f32 = 0.05;

impl Level {
    const ALL: [Level; 7] = [Level::Playfield, Level::Backglass, Level::Machine, Level::Music, Level::Pup, Level::Media, Level::Achievements];

    /// Whether the table has played that kind of sound (the two volumes always show).
    fn shown(self, s: &Sounds) -> bool {
        match self {
            Level::Playfield | Level::Backglass => true,
            Level::Machine => s.machine.is_some(),
            Level::Music => s.music_played,
            Level::Pup => s.pup_played,
            Level::Media => s.media_played,
            Level::Achievements => crate::achievements::active(),
        }
    }

    fn label(self, s: &Sounds) -> &'static str {
        match self {
            Level::Playfield => "Playfield",
            Level::Backglass => "Backglass",
            Level::Machine if s.machine == Some(true) => "Machine (AltSound)",
            Level::Machine => "Machine",
            Level::Music => "Music",
            Level::Pup => "PinUP pack",
            Level::Media => "Media Player",
            Level::Achievements => "Achievements",
        }
    }

    /// Whether it's at its default: the volumes all of it, a source as it comes (VPX's defaults).
    fn at_default(self, s: &Sounds) -> bool {
        (self.get(s) - 1.0).abs() < 0.001
    }

    /// The most it goes to: the volumes all of it, a source twice as loud as it comes (VPX's 0 to 2).
    fn most(self) -> f32 {
        match self {
            Level::Playfield | Level::Backglass => 1.0,
            _ => 2.0,
        }
    }

    fn get(self, s: &Sounds) -> f32 {
        match self {
            Level::Playfield => s.playfield_volume,
            Level::Backglass => s.backglass_volume,
            Level::Machine if s.machine == Some(true) => s.levels.altsound,
            Level::Machine => s.levels.pinmame,
            Level::Music => s.levels.music,
            Level::Pup => s.levels.pup,
            Level::Media => s.levels.wmp,
            Level::Achievements => s.levels.achievements,
        }
    }

    fn set(self, s: &mut Sounds, v: f32) {
        let v = ((v / STEP).round() * STEP).clamp(0.0, self.most());
        match self {
            Level::Playfield => s.set_volumes(v, s.backglass_volume),
            Level::Backglass => s.set_volumes(s.playfield_volume, v),
            _ => {
                let mut l = s.levels;
                match self {
                    Level::Machine if s.machine == Some(true) => l.altsound = v,
                    Level::Machine => l.pinmame = v,
                    Level::Music => l.music = v,
                    Level::Pup => l.pup = v,
                    Level::Achievements => l.achievements = v,
                    _ => l.wmp = v,
                }
                s.set_levels(l);
            }
        }
    }

    /// Its setting, section and name, and its value as VPX writes it (a percentage for the
    /// volumes, a float for the gains).
    fn setting(self, s: &Sounds) -> (&'static str, &'static str, String) {
        let v = self.get(s);
        let percent = || format!("{}", (v * 100.0).round() as i32);
        let gain = || format!("{v:.6}");
        match self {
            Level::Playfield => ("Player", "SoundVolume", percent()),
            Level::Backglass => ("Player", "MusicVolume", percent()),
            Level::Machine if s.machine == Some(true) => ("Player", "AudioSource.AltSound.Gain", gain()),
            Level::Machine => ("Player", "AudioSource.PinMAME.Gain", gain()),
            Level::Music => ("VPI", "MusicGain", gain()),
            Level::Pup => ("Player", "AudioSource.PUP.Gain", gain()),
            Level::Media => ("Player", "AudioSource.WMP.Gain", gain()),
            Level::Achievements => ("VPI", "AchievementGain", gain()),
        }
    }
}

/// A row of a page.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Row {
    Resume,
    Level(Level),
    /// Every level shown back to its default.
    ResetLevels,
    Options,
    Rules,
    Controls,
    /// The table's achievements, how far each has come.
    Achievements,
    /// Recording the machine's display for colouring, on or off.
    Record,
    Quit,
    Option(usize),
    /// Every option back to its default.
    ResetOptions,
    /// The view's page, and one of its values.
    View,
    Cam(Cam),
    /// The view back to the table's own.
    OwnView,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Main,
    Options,
    View,
    Rules,
    Controls,
    Achievements,
}

/// What a press in the menu asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    /// Play goes on.
    Close,
    /// Play ends.
    Quit,
    /// One of the table's options changed: its script is told (`OptionEvent` 1).
    OptionChanged,
    /// Recording the display for colouring started, or stopped.
    Record,
}

/// VPX 5436's repeats for a held button: after half a second, then a quarter, then every eighth.
fn repeat_after(count: u32) -> Duration {
    Duration::from_millis(match count {
        0 => 500,
        1 => 250,
        _ => 125,
    })
}

/// The menu while it shows.
pub struct Menu {
    setup: Setup,
    page: Page,
    selected: usize,
    /// The main page's row, kept while a page below it shows.
    main_selected: usize,
    /// The rules' first line shown, in steps.
    rules_top: usize,
    /// What changed since it opened, kept when it closes.
    levels_changed: Vec<Level>,
    options_changed: bool,
    /// A held press repeating: which, since when, how often so far.
    held: Option<(u8, Nav, Instant, u32)>,
    /// The table's pictures the rules name, once their page has shown.
    pictures: Option<Vec<(String, Picture)>>,
    /// The table's view as its page has set it (none: no view page).
    view_now: Option<ViewSetup>,
    /// Recording the machine's display: None where there's no display to record, else the frames
    /// recorded so far while it's on.
    recording: Option<Option<u64>>,
}

/// One of the table's pictures, decoded: shared, told apart by where it lives (a big picture
/// isn't compared pixel by pixel each frame).
#[derive(Clone)]
pub struct Picture(pub Arc<image::RgbaImage>);

impl PartialEq for Picture {
    fn eq(&self, other: &Picture) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picture({} x {})", self.0.width(), self.0.height())
    }
}

/// The table pictures a Markdown text names (`![…](name)`), in order.
fn picture_names(markdown: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in markdown.lines() {
        for (_, name, _) in images_in(line) {
            if !out.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                out.push(name);
            }
        }
    }
    out
}

/// A line's images, `![alt](name)` or `![alt](name "title")`: each one's place in the line (where
/// it starts and ends) and the picture's name.
fn images_in(line: &str) -> Vec<(usize, String, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(start) = line[from..].find("![").map(|k| from + k) {
        let Some(mid) = line[start..].find("](").map(|k| start + k) else { break };
        let Some(end) = line[mid..].find(')').map(|k| mid + k) else { break };
        let name = line[mid + 2..end].split_whitespace().next().unwrap_or("").trim_matches(['<', '>']).to_string();
        if !name.is_empty() {
            out.push((start, name, end + 1));
        }
        from = end + 1;
    }
    out
}

/// The table's pictures the rules name, decoded as the player reads them (a picture stored as
/// the table's screenshot's link resolved already); a very big one made smaller first.
pub fn decode_pictures(vpx: &vpin::vpx::VPX, markdown: &str) -> Vec<(String, Picture)> {
    picture_names(markdown)
        .into_iter()
        .filter_map(|name| {
            let image = vpx.images.iter().find(|i| i.name.eq_ignore_ascii_case(&name))?;
            let data = &image.jpeg.as_ref()?.data;
            let decoded = match crate::pictures::reader(data, &image.path) {
                Some(r) => r.decode().ok()?,
                None => crate::pictures::decode_psd(data)?,
            };
            let mut rgba = decoded.to_rgba8();
            if rgba.width() > 2048 {
                let h = (u64::from(rgba.height()) * 2048 / u64::from(rgba.width())).max(1) as u32;
                rgba = image::imageops::resize(&rgba, 2048, h, image::imageops::FilterType::Triangle);
            }
            Some((name, Picture(Arc::new(rgba))))
        })
        .collect()
}

impl Menu {
    pub fn new(setup: Setup) -> Menu {
        set_shown(true);
        RULES_MOST_TOP.store(usize::MAX, Ordering::Release);
        // the mouse's news from before it opened isn't for it
        POINTER.lock().unwrap_or_else(|e| e.into_inner()).clear();
        Menu {
            view_now: setup.view.as_ref().map(|v| v.now),
            recording: None,
            setup,
            page: Page::Main,
            selected: 0,
            main_selected: 0,
            rules_top: 0,
            levels_changed: Vec::new(),
            options_changed: false,
            held: None,
            pictures: None,
        }
    }

    /// Whether the machine's display can be recorded, and how many frames are while it is (the game
    /// thread tells it as the menu shows).
    pub fn set_recording(&mut self, recording: Option<Option<u64>>) {
        self.recording = recording;
    }

    /// The rules' page shows and the pictures they name aren't decoded yet (`decode_pictures`).
    pub fn wants_pictures(&self) -> Option<&str> {
        (self.page == Page::Rules && self.pictures.is_none()).then_some(self.setup.rules.as_deref()).flatten()
    }

    pub fn set_pictures(&mut self, pictures: Vec<(String, Picture)>) {
        self.pictures = Some(pictures);
    }

    /// Test mode: shown without pausing, keeping nothing.
    pub fn picture_only(&self) -> bool {
        self.setup.picture_only
    }

    /// The table's view as its page has set it.
    pub fn view_now(&self) -> Option<ViewSetup> {
        self.view_now
    }

    /// Which of the table's view setups is drawn.
    fn which_view(&self) -> camera::View {
        self.setup.view.as_ref().map_or(camera::View::Desktop, |v| v.which)
    }

    /// The view isn't the table's own.
    fn view_differs(&self) -> bool {
        matches!((&self.view_now, &self.setup.view), (Some(now), Some(v)) if *now != v.own)
    }

    /// One of the view's values moved a step (`dir`, 1 or -1), held to its range — the mode and
    /// the turn going round — the table drawn so at once.
    fn cam_step(&mut self, c: Cam, dir: f32) {
        let which = self.which_view();
        let Some(v) = self.view_now.as_mut() else { return };
        let (min, max, step) = c.range(which);
        let now = c.get(v);
        let next = if c.goes_round() {
            let count = ((max - min) / step).round() as i32 + 1;
            min + (((now - min) / step).round() as i32 + dir as i32).rem_euclid(count) as f32 * step
        } else {
            (now + dir * step).clamp(min, max)
        };
        c.set(v, next);
    }

    /// One of the view's values set to a place along its bar (0 to 1), to its step.
    fn cam_at(&mut self, c: Cam, at: f32) {
        let which = self.which_view();
        let Some(v) = self.view_now.as_mut() else { return };
        let (min, max, step) = c.range(which);
        c.set(v, (min + ((at * (max - min)) / step).round() * step).clamp(min, max));
    }

    /// The page shown scrolls: the rules', or the controls'.
    fn scrolls(&self) -> bool {
        matches!(self.page, Page::Rules | Page::Controls | Page::Achievements)
    }

    /// Quit the table picked, as Esc opens the menu.
    pub fn pick_quit(&mut self, sounds: &Sounds, options: &[TableOptionDef], stored: &[(String, String)]) {
        if let Some(k) = self.rows(sounds, options, stored).iter().position(|(_, r)| *r == Row::Quit) {
            self.selected = k;
        }
    }

    /// The mouse's news since last taken (`point`), each as the presses it stands for: what each
    /// asks of the game, in order, up to one that closes the menu.
    pub fn pointed(&mut self, sounds: &mut Sounds, options: &[TableOptionDef], stored: &mut Vec<(String, String)>) -> Vec<Outcome> {
        let news = std::mem::take(&mut *POINTER.lock().unwrap_or_else(|e| e.into_inner()));
        let mut out = Vec::new();
        for p in news {
            let o = self.point(p, sounds, options, stored);
            if o != Outcome::Nothing {
                out.push(o);
            }
            if matches!(o, Outcome::Close | Outcome::Quit) {
                break;
            }
        }
        out
    }

    fn point(&mut self, p: Pointer, sounds: &mut Sounds, options: &[TableOptionDef], stored: &mut Vec<(String, String)>) -> Outcome {
        let rows = self.rows(sounds, options, stored);
        match p {
            Pointer::Over(_, n) | Pointer::Click(_, _, n) | Pointer::Drag(_, _, n) if n != rows.len() => {}
            Pointer::Over(k, _) if k < rows.len() => self.selected = k,
            Pointer::Click(k, spot, _) if k < rows.len() => {
                self.selected = k;
                return match (rows[k].1, spot) {
                    (Row::Level(_) | Row::Option(_) | Row::Cam(_), Spot::Bar(at)) => self.set_at(rows[k].1, at, sounds, options, stored),
                    (Row::Level(_) | Row::Option(_) | Row::Cam(_), Spot::Less) => self.nav(Nav::Left, sounds, options, stored),
                    (Row::Level(_) | Row::Option(_) | Row::Cam(_), Spot::More) => self.nav(Nav::Right, sounds, options, stored),
                    _ => self.nav(Nav::Choose, sounds, options, stored),
                };
            }
            Pointer::Drag(k, at, _) if k < rows.len() => return self.set_at(rows[k].1, at, sounds, options, stored),
            Pointer::Wheel(notches) if self.scrolls() => {
                let most = RULES_MOST_TOP.load(Ordering::Acquire);
                self.rules_top = if notches > 0 { self.rules_top.saturating_sub(notches as usize) } else { (self.rules_top + notches.unsigned_abs() as usize).min(most) };
            }
            Pointer::Back => return self.back(),
            _ => {}
        }
        Outcome::Nothing
    }

    /// A level, or a numbered option, set to a place along its bar (0 to 1).
    fn set_at(&mut self, row: Row, at: f32, sounds: &mut Sounds, options: &[TableOptionDef], stored: &mut Vec<(String, String)>) -> Outcome {
        let at = at.clamp(0.0, 1.0);
        match row {
            Row::Level(l) => {
                let before = l.get(sounds);
                l.set(sounds, at * l.most());
                if l.get(sounds) != before && !self.levels_changed.contains(&l) {
                    self.levels_changed.push(l);
                }
                Outcome::Nothing
            }
            Row::Option(k) if options[k].names.is_none() && options[k].max > options[k].min => {
                let o = &options[k];
                let now = option_value(o, stored);
                let v = (o.min + ((at * (o.max - o.min)) / o.step).round() * o.step).clamp(o.min, o.max);
                if (v - now).abs() < o.step * 0.01 {
                    return Outcome::Nothing;
                }
                stored.push((o.id.clone(), number(v)));
                self.options_changed = true;
                Outcome::OptionChanged
            }
            Row::Cam(c) if !c.goes_round() => {
                self.cam_at(c, at);
                Outcome::Nothing
            }
            _ => Outcome::Nothing,
        }
    }

    fn rows(&self, sounds: &Sounds, options: &[TableOptionDef], stored: &[(String, String)]) -> Vec<(Option<&'static str>, Row)> {
        match self.page {
            Page::Main => {
                let mut rows = vec![(None, Row::Resume)];
                for (k, l) in Level::ALL.into_iter().filter(|l| l.shown(sounds)).enumerate() {
                    rows.push(((k == 0).then_some("Sound"), Row::Level(l)));
                }
                if Level::ALL.into_iter().any(|l| l.shown(sounds) && !l.at_default(sounds)) {
                    rows.push((None, Row::ResetLevels));
                }
                let table = [
                    (!options.is_empty()).then_some(Row::Options),
                    self.view_now.is_some().then_some(Row::View),
                    self.setup.rules.is_some().then_some(Row::Rules),
                    crate::achievements::active().then_some(Row::Achievements),
                    Some(Row::Controls),
                    self.recording.is_some().then_some(Row::Record),
                ];
                for (k, r) in table.into_iter().flatten().enumerate() {
                    rows.push(((k == 0).then_some("Table"), r));
                }
                rows.push((Some(""), Row::Quit));
                rows
            }
            Page::Options => {
                let mut rows: Vec<(Option<&'static str>, Row)> = (0..options.len()).map(|k| (None, Row::Option(k))).collect();
                if options.iter().any(|o| !option_at_default(o, stored)) {
                    rows.push((None, Row::ResetOptions));
                }
                rows.push((Some(""), Row::Back));
                rows
            }
            Page::View => {
                let Some(now) = self.view_now else { return vec![(None, Row::Back)] };
                let (first, more) = Cam::of(now.mode);
                let mut rows: Vec<(Option<&'static str>, Row)> = first.iter().map(|&c| (None, Row::Cam(c))).collect();
                rows.extend(more.iter().enumerate().map(|(k, &c)| ((k == 0).then_some("More"), Row::Cam(c))));
                if self.view_differs() {
                    rows.push((Some(""), Row::OwnView));
                    rows.push((None, Row::Back));
                } else {
                    rows.push((Some(""), Row::Back));
                }
                rows
            }
            Page::Rules | Page::Controls | Page::Achievements => vec![(None, Row::Back)],
        }
    }

    /// A press, or a release, on the game thread: what it asks of the game. `stored` is the
    /// settings' table options as the script reads them (`World::table_options`).
    pub fn press(&mut self, dik: u8, down: bool, sounds: &mut Sounds, options: &[TableOptionDef], stored: &mut Vec<(String, String)>) -> Outcome {
        let Some(nav) = Nav::of(dik) else { return Outcome::Nothing };
        if !down {
            if self.held.is_some_and(|(k, ..)| k == dik) {
                self.held = None;
            }
            return Outcome::Nothing;
        }
        if nav.repeats() {
            self.held = Some((dik, nav, Instant::now(), 0));
        }
        self.nav(nav, sounds, options, stored)
    }

    /// A held press's repeat, when one is due.
    pub fn tick(&mut self, sounds: &mut Sounds, options: &[TableOptionDef], stored: &mut Vec<(String, String)>) -> Outcome {
        let Some((dik, nav, since, count)) = self.held else { return Outcome::Nothing };
        if since.elapsed() < repeat_after(count) {
            return Outcome::Nothing;
        }
        self.held = Some((dik, nav, Instant::now(), count + 1));
        self.nav(nav, sounds, options, stored)
    }

    fn nav(&mut self, nav: Nav, sounds: &mut Sounds, options: &[TableOptionDef], stored: &mut Vec<(String, String)>) -> Outcome {
        let rows = self.rows(sounds, options, stored);
        if rows.is_empty() {
            return Outcome::Nothing;
        }
        self.selected = self.selected.min(rows.len() - 1);
        let row = rows[self.selected].1;
        match nav {
            Nav::Up if self.scrolls() => self.rules_top = self.rules_top.saturating_sub(1),
            Nav::Down if self.scrolls() => self.rules_top = (self.rules_top + 1).min(RULES_MOST_TOP.load(Ordering::Acquire)),
            Nav::Up => self.selected = (self.selected + rows.len() - 1) % rows.len(),
            Nav::Down => self.selected = (self.selected + 1) % rows.len(),
            Nav::Left | Nav::Right => {
                let dir = if nav == Nav::Right { 1.0 } else { -1.0 };
                match row {
                    Row::Level(l) => {
                        l.set(sounds, l.get(sounds) + dir * STEP);
                        if !self.levels_changed.contains(&l) {
                            self.levels_changed.push(l);
                        }
                    }
                    Row::Option(k) => {
                        if step_option(&options[k], stored, dir, false) {
                            self.options_changed = true;
                            return Outcome::OptionChanged;
                        }
                    }
                    Row::Cam(c) => self.cam_step(c, dir),
                    Row::ResetLevels | Row::ResetOptions | Row::OwnView if nav == Nav::Right => return self.reset(row, sounds, options, stored),
                    _ if nav == Nav::Right => return self.choose(row),
                    _ => return self.back(),
                }
            }
            Nav::Choose => match row {
                // a toggle or a choice goes to its next value, round from the last to the first
                Row::Option(k) if options[k].names.is_some() => {
                    if step_option(&options[k], stored, 1.0, true) {
                        self.options_changed = true;
                        return Outcome::OptionChanged;
                    }
                }
                Row::Cam(c) if c.goes_round() => self.cam_step(c, 1.0),
                Row::Option(_) | Row::Level(_) | Row::Cam(_) => {}
                Row::ResetLevels | Row::ResetOptions | Row::OwnView => return self.reset(row, sounds, options, stored),
                _ => return self.choose(row),
            },
            Nav::Back => return self.back(),
        }
        Outcome::Nothing
    }

    fn choose(&mut self, row: Row) -> Outcome {
        match row {
            Row::Resume => Outcome::Close,
            Row::Quit => Outcome::Quit,
            Row::Record => Outcome::Record,
            Row::Options | Row::View | Row::Rules | Row::Controls | Row::Achievements => {
                self.main_selected = self.selected;
                let page = match row {
                    Row::Options => Page::Options,
                    Row::View => Page::View,
                    Row::Rules => Page::Rules,
                    Row::Achievements => Page::Achievements,
                    _ => Page::Controls,
                };
                (self.page, self.selected, self.rules_top) = (page, 0, 0);
                Outcome::Nothing
            }
            Row::Back => self.back(),
            Row::Level(_) | Row::Option(_) | Row::ResetLevels | Row::ResetOptions | Row::Cam(_) | Row::OwnView => Outcome::Nothing,
        }
    }

    /// Every level shown, or every option, back to its default: the levels to all of it as they
    /// come, each option to the table's own default — told to the script as a change, as VPX 5436
    /// dispatches a reset (`PinTable::OptionEventType::Reseted` no longer used).
    fn reset(&mut self, row: Row, sounds: &mut Sounds, options: &[TableOptionDef], stored: &mut Vec<(String, String)>) -> Outcome {
        match row {
            Row::ResetLevels => {
                let due: Vec<Level> = Level::ALL.into_iter().filter(|l| l.shown(sounds) && !l.at_default(sounds)).collect();
                for l in due {
                    l.set(sounds, 1.0);
                    if !self.levels_changed.contains(&l) {
                        self.levels_changed.push(l);
                    }
                }
                Outcome::Nothing
            }
            Row::ResetOptions => {
                let mut changed = false;
                let due: Vec<&TableOptionDef> = options.iter().filter(|o| !option_at_default(o, stored)).collect();
                for o in due {
                    stored.push((o.id.clone(), number(o.default)));
                    changed = true;
                }
                if changed {
                    self.options_changed = true;
                    Outcome::OptionChanged
                } else {
                    Outcome::Nothing
                }
            }
            Row::OwnView => {
                if let (Some(now), Some(v)) = (self.view_now.as_mut(), self.setup.view.as_ref()) {
                    *now = v.own;
                }
                Outcome::Nothing
            }
            _ => Outcome::Nothing,
        }
    }

    fn back(&mut self) -> Outcome {
        if self.page == Page::Main {
            return Outcome::Close;
        }
        (self.page, self.selected) = (Page::Main, self.main_selected);
        Outcome::Nothing
    }

    /// What it shows now.
    pub fn view(&self, sounds: &Sounds, options: &[TableOptionDef], stored: &[(String, String)]) -> View {
        let rows = self.rows(sounds, options, stored);
        let link = |section: Option<&str>, label: &str, opens: bool| RowView { section: section.map(str::to_string), label: label.to_string(), value: None, bar: None, opens, changes: false };
        let selected = self.selected.min(rows.len().saturating_sub(1));
        let rows: Vec<RowView> = rows
            .iter()
            .map(|&(section, row)| match row {
                Row::Resume => link(section, "Resume", false),
                Row::Level(l) => {
                    let v = l.get(sounds);
                    RowView {
                        section: section.map(str::to_string),
                        label: l.label(sounds).to_string(),
                        value: Some(format!("{:.0}%", v * 100.0)),
                        bar: Some(v / l.most()),
                        opens: false,
                        changes: true,
                    }
                }
                Row::ResetLevels | Row::ResetOptions => link(section, "Reset to defaults", false),
                Row::Options => link(section, "Table options", true),
                Row::Rules => link(section, "Rules", true),
                Row::Controls => link(section, "Controls", true),
                Row::Achievements => RowView {
                    section: section.map(str::to_string),
                    label: "Achievements".to_string(),
                    value: crate::achievements::tally().map(|(earned, all)| format!("{earned} of {all}")),
                    bar: None,
                    opens: true,
                    changes: false,
                },
                Row::Record => match self.recording {
                    Some(Some(frames)) => RowView {
                        section: section.map(str::to_string),
                        label: "Recording the display: stop".to_string(),
                        value: Some(format!("{frames} frames")),
                        bar: None,
                        opens: false,
                        changes: false,
                    },
                    _ => link(section, "Record the display for coloring", false),
                },
                Row::Quit => link(section, "Quit the table", false),
                Row::Option(k) => {
                    let o = &options[k];
                    let v = option_value(o, stored);
                    RowView {
                        section: section.map(str::to_string),
                        label: o.name.clone(),
                        value: Some(option_text(o, v)),
                        bar: (o.names.is_none() && o.max > o.min).then(|| (v - o.min) / (o.max - o.min)),
                        opens: false,
                        changes: true,
                    }
                }
                Row::View => link(section, "Its view", true),
                Row::OwnView => link(section, "Back to its own view", false),
                Row::Cam(c) => {
                    let now = self.view_now.unwrap_or_default();
                    let (min, max, _) = c.range(self.which_view());
                    let x = c.get(&now);
                    RowView {
                        section: section.map(str::to_string),
                        label: c.label(now.mode).to_string(),
                        value: Some(c.text(x)),
                        bar: (!c.goes_round()).then(|| ((x - min) / (max - min)).clamp(0.0, 1.0)),
                        opens: false,
                        changes: true,
                    }
                }
                Row::Back => link(section, "Back", false),
            })
            .collect();
        // the achievements' page: each one's line, and how what's earned is kept
        let (achieved, kept) = if self.page == Page::Achievements { crate::achievements::page() } else { (Vec::new(), String::new()) };
        let scroll = "Magna-saves, \u{2191} \u{2193} or the wheel to scroll, Esc to go back";
        let (heading, hint) = match self.page {
            Page::Main => ("Paused", "Magna-saves, \u{2191} \u{2193} or the mouse to move, flippers or arrow keys to change\nStart, Enter or a click to choose, P or Esc to play on".to_string()),
            Page::Options => ("Table options", "Flippers or arrow keys to change, kept for this table\nEsc to go back".to_string()),
            Page::View => ("Its view", "Flippers or arrow keys to change, kept for this table\nEsc to go back".to_string()),
            Page::Rules => ("Rules", scroll.to_string()),
            Page::Controls => ("Controls", format!("Change them in the app: Settings \u{203a} Controls\n{scroll}")),
            Page::Achievements => ("Achievements", format!("{kept}\n{scroll}")),
        };
        View {
            heading: heading.to_string(),
            title: self.setup.title.clone(),
            byline: self.setup.byline.clone(),
            rows,
            selected,
            text: (self.page == Page::Rules).then(|| self.setup.rules.clone()).flatten(),
            controls: match self.page {
                Page::Controls => CONTROLS.lock().unwrap_or_else(|e| e.into_inner()).clone(),
                Page::Achievements => achieved,
                _ => Vec::new(),
            },
            text_top: self.rules_top,
            pictures: if self.page == Page::Rules { self.pictures.clone().unwrap_or_default() } else { Vec::new() },
            hint,
            camera: self.view_now,
            view_page: self.page == Page::View,
        }
    }

    /// The menu closes: what changed kept — each level in the table's own settings file where it
    /// sets that one, else in the main file; the options in the table's own, those that differ
    /// from it — as VPX keeps them; the view in the table's own, as what differs from its own view
    /// (`ViewSetup::overrides_from`). Nothing in test mode.
    pub fn close(self, sounds: &Sounds, options: &[TableOptionDef], stored: &[(String, String)]) {
        set_shown(false);
        let viewing = self.setup.view.as_ref().zip(self.view_now).filter(|(v, now)| *now != v.now);
        if self.setup.picture_only || (self.levels_changed.is_empty() && !self.options_changed && viewing.is_none()) {
            return;
        }
        let table_text = self.setup.table_file.as_deref().map(read).unwrap_or_default();
        let table_entries = vpi_settings::parse_ini(&table_text);
        let (mut main_changes, mut table_changes) = (Vec::new(), Vec::new());
        for l in &self.levels_changed {
            let (section, name, value) = l.setting(sounds);
            let change = Change::set(section, name, value);
            if vpi_settings::get(&table_entries, section, name).is_some() || self.setup.settings_file.is_none() {
                table_changes.push(change);
            } else {
                main_changes.push(change);
            }
        }
        if self.options_changed {
            for o in options {
                let v = option_value(o, stored);
                let held = vpi_settings::get(&table_entries, "TableOption", &o.id).map(|t| crate::settings::table_option_value(o.min, o.max, o.step, o.default, o.names.as_deref(), Some(t)));
                if held.unwrap_or(o.default) != v {
                    table_changes.push(Change::set("TableOption", &o.id, number(v)));
                }
            }
        }
        if let Some((v, now)) = viewing {
            table_changes.extend(now.overrides_from(&v.own, v.which));
        }
        // a table's file made for the first time takes in its POV file's views, which it ends the
        // reading of (as VPX saves a table's settings with the POV it imported)
        if !table_changes.is_empty()
            && let Some(file) = self.setup.table_file.as_deref()
        {
            table_changes.splice(0..0, vpi_settings::first_table_settings(file));
        }
        for (file, changes) in [(self.setup.settings_file.as_deref(), main_changes), (self.setup.table_file.as_deref(), table_changes)] {
            if let Some(file) = file
                && !changes.is_empty()
                && let Err(e) = write_changes(file, &changes)
            {
                eprintln!("vpi-player: the pause menu's changes weren't kept: {e}");
            }
        }
    }
}

/// A settings file's text, or none.
fn read(file: &Path) -> String {
    std::fs::read(file).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// Changes written into a settings file in place, only what changed, comments and all
/// (`vpi_settings::edit`), through a passing file.
fn write_changes(file: &Path, changes: &[Change]) -> Result<(), String> {
    let before = read(file);
    let after = vpi_settings::edit(&before, changes);
    if after == before {
        return Ok(());
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let new = file.with_extension("ini.new");
    std::fs::write(&new, after).and_then(|()| std::fs::rename(&new, file)).map_err(|e| format!("{}: {e}", file.display()))
}

/// A number as the settings keep a table option's: whole, or with six places.
fn number(v: f32) -> String {
    if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v:.6}") }
}

/// An option's value now: the one the settings or the menu set last, else its default, as
/// `Table.Option` reads it.
fn option_value(o: &TableOptionDef, stored: &[(String, String)]) -> f32 {
    let s = stored.iter().rev().find(|(k, v)| k.eq_ignore_ascii_case(&o.id) && !v.is_empty()).map(|(_, v)| v.as_str());
    crate::settings::table_option_value(o.min, o.max, o.step, o.default, o.names.as_deref(), s)
}

/// Whether an option is at the table's own default for it.
fn option_at_default(o: &TableOptionDef, stored: &[(String, String)]) -> bool {
    (option_value(o, stored) - option_value(o, &[])).abs() < 1e-6
}

/// An option moved a step (`dir`, 1 or -1), held to its range — or round from one end to the
/// other with `round` — set as the settings would set it: whether it changed.
fn step_option(o: &TableOptionDef, stored: &mut Vec<(String, String)>, dir: f32, round: bool) -> bool {
    let now = option_value(o, stored);
    let mut next = now + dir * o.step;
    let slack = o.step * 0.01;
    if next > o.max + slack {
        next = if round { o.min } else { o.max };
    } else if next < o.min - slack {
        next = if round { o.max } else { o.min };
    }
    let next = (o.min + ((next - o.min) / o.step).round() * o.step).clamp(o.min, o.max);
    if next == now {
        return false;
    }
    stored.push((o.id.clone(), number(next)));
    true
}

/// An option's value as it shows: its name, a percentage (VPX's unit 1), or the number.
fn option_text(o: &TableOptionDef, v: f32) -> String {
    if let Some(name) = o.names.as_ref().and_then(|n| n.get((v - o.min).round().max(0.0) as usize)) {
        return crate::settings::vpx_trim(name).to_string();
    }
    match o.unit {
        1 => format!("{:.0}%", v * 100.0),
        _ if v.fract() == 0.0 => format!("{}", v as i64),
        _ => format!("{v:.2}"),
    }
}

/// What the menu shows, for the window thread to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub heading: String,
    pub title: String,
    pub byline: String,
    pub rows: Vec<RowView>,
    pub selected: usize,
    /// The rules, on their page, the first part shown, in steps, and the table's pictures they name.
    pub text: Option<String>,
    /// The controls, on their page, scrolled as the rules are.
    pub controls: Vec<ControlLine>,
    pub text_top: usize,
    pub pictures: Vec<(String, Picture)>,
    pub hint: String,
    /// The table's view as its page has set it, for the window thread to draw the table by (none:
    /// no view page).
    pub camera: Option<ViewSetup>,
    /// The view's page shows: every part drawn every frame meanwhile
    /// (`Renderer::set_view_changing`).
    pub view_page: bool,
}

/// A row as it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct RowView {
    /// A heading above it (an empty one: a gap).
    pub section: Option<String>,
    pub label: String,
    pub value: Option<String>,
    /// A level's or a number's place between its least and its most, 0 to 1.
    pub bar: Option<f32>,
    /// It opens a page.
    pub opens: bool,
    /// The flippers change it.
    pub changes: bool,
}

/// Where the menu's rows are in its picture, for the mouse (`Pointer`): as the player sees the
/// screen once the picture is placed (`picture`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// The panel: left, top, width, height.
    pub panel: [f32; 4],
    pub rows: Vec<RowBox>,
}

/// A row's place: its box, its bar's left and width, and its value's left and right, when it has them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowBox {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
    pub bar: Option<(f32, f32)>,
    pub value: Option<(f32, f32)>,
}

impl Layout {
    fn moved(mut self, at: (f32, f32)) -> Layout {
        self.panel[0] += at.0;
        self.panel[1] += at.1;
        for r in &mut self.rows {
            (r.top, r.bottom, r.left, r.right) = (r.top + at.1, r.bottom + at.1, r.left + at.0, r.right + at.0);
            r.bar = r.bar.map(|(x, w)| (x + at.0, w));
            r.value = r.value.map(|(a, b)| (a + at.0, b + at.0));
        }
        self
    }

    /// The row at a point, and where on it: its bar (a little either side of it too), its value's
    /// halves, or the rest of it.
    pub fn at(&self, x: f32, y: f32) -> Option<(usize, Spot)> {
        let k = self.rows.iter().position(|r| y >= r.top && y < r.bottom && x >= r.left && x < r.right)?;
        let r = &self.rows[k];
        let slack = (r.bottom - r.top) * 0.2;
        if let Some((bx, bw)) = r.bar
            && x >= bx - slack
            && x <= bx + bw + slack
        {
            return Some((k, Spot::Bar(((x - bx) / bw.max(1.0)).clamp(0.0, 1.0))));
        }
        if let Some((a, b)) = r.value
            && x >= a
        {
            return Some((k, if x < (a + b) / 2.0 { Spot::Less } else { Spot::More }));
        }
        Some((k, Spot::Row))
    }

    /// A place along a row's bar, for dragging: anywhere left or right of it counts as its end.
    pub fn along(&self, row: usize, x: f32) -> Option<f32> {
        let (bx, bw) = self.rows.get(row)?.bar?;
        Some(((x - bx) / bw.max(1.0)).clamp(0.0, 1.0))
    }

    /// Whether a point is on the panel.
    pub fn on_panel(&self, x: f32, y: f32) -> bool {
        let [l, t, w, h] = self.panel;
        x >= l && x < l + w && y >= t && y < t + h
    }
}

// ---------------------------------------------------------------------------------- the picture

/// The menu's fonts: the app's own (Inter, Barlow Condensed; `assets/fonts`).
struct Fonts {
    heading: PupFont,
    body: PupFont,
    strong: PupFont,
}

fn fonts() -> Option<&'static Fonts> {
    static FONTS: OnceLock<Option<Fonts>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            Some(Fonts {
                heading: PupFont::built_in(include_bytes!("../assets/fonts/BarlowCondensed-Latin-Bold.ttf"))?,
                body: PupFont::built_in(include_bytes!("../assets/fonts/Inter-Latin-Regular.ttf"))?,
                strong: PupFont::built_in(include_bytes!("../assets/fonts/Inter-Latin-SemiBold.ttf"))?,
            })
        })
        .as_ref()
}

/// The app's colours (`app/ui/src/app.css`), as sRGB values.
fn colour(hex: u32) -> [f32; 3] {
    [((hex >> 16) & 0xFF) as f32 / 255.0, ((hex >> 8) & 0xFF) as f32 / 255.0, (hex & 0xFF) as f32 / 255.0]
}
const BG: u32 = 0x0d1016;
const LINE: u32 = 0x252d3c;
const FG: u32 = 0xe9edf4;
const SOFT: u32 = 0xc3cad6;
const MUTED: u32 = 0xaab3c2;
const FAINT: u32 = 0x8b94a6;
const ACCENT: u32 = 0x4cc2ff;
const ACCENT_BG: u32 = 0x24314a;
/// An achievement's points.
const GOOD: u32 = 0x9ff0c2;
const TRACK: u32 = 0x2c3445;

/// How dark the table goes behind the menu.
const DIM: f32 = 0.55;

/// A picture being drawn: premultiplied sRGB values.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

thread_local! {
    /// The last picture's memory, for the next: fresh memory a 4K window's size costs more to
    /// write the first time than the drawing itself.
    static SPARE: std::cell::RefCell<Vec<[f32; 4]>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Drop for Canvas {
    fn drop(&mut self) {
        let px = std::mem::take(&mut self.px);
        SPARE.with(|s| *s.borrow_mut() = px);
    }
}

impl Canvas {
    fn new(w: usize, h: usize) -> Canvas {
        let mut px = SPARE.with(|s| std::mem::take(&mut *s.borrow_mut()));
        px.clear();
        px.resize(w * h, [0.0; 4]);
        Canvas { w, h, px }
    }

    fn over(&mut self, x: usize, y: usize, c: [f32; 3], a: f32) {
        let d = &mut self.px[y * self.w + x];
        // opaque: what's below doesn't show
        if a >= 1.0 {
            *d = [c[0], c[1], c[2], 1.0];
            return;
        }
        for k in 0..3 {
            d[k] = c[k] * a + d[k] * (1.0 - a);
        }
        d[3] = a + d[3] * (1.0 - a);
    }

    /// A rounded rectangle, its edges smoothed.
    fn rounded(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: [f32; 3], alpha: f32) {
        let (x0, y0) = (x.floor().max(0.0) as usize, y.floor().max(0.0) as usize);
        let (x1, y1) = (((x + w).ceil().max(0.0) as usize).min(self.w), ((y + h).ceil().max(0.0) as usize).min(self.h));
        let (cx, cy, hw, hh) = (x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0);
        let r = r.min(hw).min(hh).max(0.0);
        for py in y0..y1 {
            let qy = ((py as f32 + 0.5) - cy).abs() - (hh - r);
            for px in x0..x1 {
                let qx = ((px as f32 + 0.5) - cx).abs() - (hw - r);
                // well inside its edges and corners it's wholly covered, as the distance below
                // would say; only the edges' pixels need it worked out
                let cover = if qx <= 0.0 && qy <= 0.0 && qx.max(qy) <= r - 0.5 {
                    1.0
                } else {
                    let outside = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
                    (0.5 - outside).clamp(0.0, 1.0)
                };
                if cover > 0.0 {
                    self.over(px, py, c, alpha * cover);
                }
            }
        }
    }

    /// Text drawn by SDL_ttf (its colour, its coverage in alpha), its top left at `x`, `y`; only
    /// its rows from `from` and `rows` of them.
    fn text(&mut self, t: &Text, x: f32, y: f32, from: usize, rows: usize) {
        let (x0, y0) = (x.round() as i64, y.round() as i64);
        for ty in from..(from + rows).min(t.h) {
            let py = y0 + (ty - from) as i64;
            if py < 0 || py >= self.h as i64 {
                continue;
            }
            for tx in 0..t.w {
                let px = x0 + tx as i64;
                if px < 0 || px >= self.w as i64 {
                    continue;
                }
                let p = &t.rgba[(ty * t.w + tx) * 4..(ty * t.w + tx) * 4 + 4];
                if p[3] > 0 {
                    self.over(px as usize, py as usize, [f32::from(p[0]) / 255.0, f32::from(p[1]) / 255.0, f32::from(p[2]) / 255.0], f32::from(p[3]) / 255.0);
                }
            }
        }
    }

    /// The picture: RGBA, straight alpha.
    fn rgba(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.px.len() * 4];
        for (p, o) in self.px.iter().zip(out.chunks_exact_mut(4)) {
            let a = p[3];
            // most of it is opaque or empty: no division for those; and rounded as round() rounds
            // these values, which are never below 0, without its call for each
            let byte = |v: f32| (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            if a >= 1.0 {
                o.copy_from_slice(&[byte(p[0]), byte(p[1]), byte(p[2]), 255]);
            } else if a > 0.0 {
                o.copy_from_slice(&[byte(p[0] / a), byte(p[1] / a), byte(p[2] / a), byte(a)]);
            }
        }
        out
    }
}

/// A piece of text drawn.
struct Text {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

fn text(font: &PupFont, s: &str, px: f32, c: u32, wrap: f32) -> Text {
    // each piece drawn once and kept: the menu's picture is made again whenever what it shows
    // changes (a row lit, a level dragged), and most of its text is as it was
    type Key = (usize, String, u32, u32, i32);
    static DRAWN: Mutex<Option<std::collections::HashMap<Key, Arc<Text>>>> = Mutex::new(None);
    let key: Key = (std::ptr::from_ref(font) as usize, s.to_string(), px.to_bits(), c, wrap.max(0.0) as i32);
    let mut drawn = DRAWN.lock().unwrap_or_else(|e| e.into_inner());
    let drawn = drawn.get_or_insert_with(std::collections::HashMap::new);
    if let Some(t) = drawn.get(&key) {
        return Text { w: t.w, h: t.h, rgba: t.rgba.clone() };
    }
    let style = TextStyle { italic: false, underline: false, align: XAlign::Left, outline: 0 };
    let hex = colour(c).map(|v| (v * 255.0).round() as u8);
    let t = match font.render_wrapped(s, px, hex, style, wrap.max(0.0) as i32) {
        Some((w, h, rgba)) if !s.is_empty() => Text { w: w as usize, h: h as usize, rgba },
        _ => Text { w: 0, h: (px * 1.2) as usize, rgba: Vec::new() },
    };
    // a few pages' worth: a window's new size draws them all again at its scale
    if drawn.len() >= 512 {
        drawn.clear();
    }
    drawn.insert(key, Arc::new(Text { w: t.w, h: t.h, rgba: t.rgba.clone() }));
    t
}

/// The rules drawn `width` pixels wide: their text, and the pictures they name each fitted to the
/// width, one after the other.
fn rules_picture(markdown: &str, pictures: &[(String, Picture)], width: f32, s: f32, f: &Fonts) -> Text {
    let w = width.max(1.0) as usize;
    let gap = (12.0 * s) as usize;
    let mut blocks: Vec<Text> = Vec::new();
    let mut words = String::new();
    let flush = |words: &mut String, blocks: &mut Vec<Text>| {
        let t = plain(words.trim_matches('\n'));
        if !t.trim().is_empty() {
            blocks.push(text(&f.body, &t, 17.0 * s, SOFT, width));
        }
        words.clear();
    };
    for line in markdown.lines() {
        let mut at = 0;
        for (start, name, end) in images_in(line) {
            words.push_str(&line[at..start]);
            flush(&mut words, &mut blocks);
            if let Some((_, p)) = pictures.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                blocks.push(fitted(p, w));
            }
            at = end;
        }
        words.push_str(&line[at..]);
        words.push('\n');
    }
    flush(&mut words, &mut blocks);
    let h = blocks.iter().map(|b| b.h).sum::<usize>() + gap * blocks.len().saturating_sub(1);
    let mut rgba = vec![0u8; w * h * 4];
    let mut y = 0;
    for b in &blocks {
        for row in 0..b.h {
            let n = b.w.min(w) * 4;
            let from = row * b.w * 4;
            rgba[(y + row) * w * 4..(y + row) * w * 4 + n].copy_from_slice(&b.rgba[from..from + n]);
        }
        y += b.h + gap;
    }
    Text { w, h, rgba }
}

/// The Controls page's lines drawn `width` pixels wide: each group's heading, then a line for each
/// control — what it does at the left, the keys and buttons it's on at the right.
fn controls_picture(lines: &[ControlLine], width: f32, s: f32, f: &Fonts) -> Text {
    let w = width.max(1.0) as usize;
    let mut pieces: Vec<(usize, usize, Text)> = Vec::new();
    let mut y = 0;
    for (k, l) in lines.iter().enumerate() {
        if let Some(section) = &l.section {
            if k > 0 {
                y += (16.0 * s) as usize;
            }
            let t = text(&f.strong, &section.to_uppercase(), 13.0 * s, FAINT, 0.0);
            let h = t.h;
            pieces.push((y, 0, t));
            y += h + (6.0 * s) as usize;
        }
        let label = text(&f.strong, &l.label, 17.0 * s, SOFT, width * 0.36);
        let keys = text(&f.body, &l.keys, 17.0 * s, FG, width * 0.6);
        let h = label.h.max(keys.h);
        let at = w.saturating_sub(keys.w);
        pieces.push((y, 0, label));
        pieces.push((y, at, keys));
        y += h + (8.0 * s) as usize;
    }
    let h = y.max(1);
    let mut rgba = vec![0u8; w * h * 4];
    for (py, px, t) in &pieces {
        for row in 0..t.h.min(h.saturating_sub(*py)) {
            let n = t.w.min(w.saturating_sub(*px)) * 4;
            let from = row * t.w * 4;
            let to = ((py + row) * w + px) * 4;
            rgba[to..to + n].copy_from_slice(&t.rgba[from..from + n]);
        }
    }
    Text { w, h, rgba }
}

/// A picture fitted to `w` pixels wide (made smaller only), kept for the next time it's drawn.
fn fitted(p: &Picture, w: usize) -> Text {
    static LAST: Mutex<Option<(usize, usize, Arc<Text>)>> = Mutex::new(None);
    let key = Arc::as_ptr(&p.0) as usize;
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, kw, t)) = last.as_ref()
        && *k == key
        && *kw == w
    {
        return Text { w: t.w, h: t.h, rgba: t.rgba.clone() };
    }
    let (pw, ph) = (p.0.width() as usize, p.0.height() as usize);
    let image = if pw > w {
        image::imageops::resize(p.0.as_ref(), w as u32, ((ph * w / pw.max(1)).max(1)) as u32, image::imageops::FilterType::Triangle)
    } else {
        p.0.as_ref().clone()
    };
    let t = Text { w: image.width() as usize, h: image.height() as usize, rgba: image.into_raw() };
    *last = Some((key, w, Arc::new(Text { w: t.w, h: t.h, rgba: t.rgba.clone() })));
    t
}

/// The rules' Markdown made plain text: headings, emphasis and list marks taken off, bullets kept.
fn plain(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let t = line.trim_start();
            let t = t.trim_start_matches('#').trim_start();
            let t = if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) { format!("\u{2022} {rest}") } else { t.to_string() };
            t.replace("**", "").replace("__", "")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The menu's picture for a window the player sees as `seen` pixels (wide, high), laid out
/// as the app's screens are and sized to the window: none if its fonts can't be read.
pub fn picture(view: &View, seen: (u32, u32)) -> Option<(MenuPicture, Layout)> {
    let f = fonts()?;
    let (sw, sh) = (seen.0.max(1) as f32, seen.1.max(1) as f32);
    let mut s = (sh / 1080.0).clamp(0.6, 2.4).min((sw - 32.0).max(240.0) / 720.0);
    for _ in 0..4 {
        if let Some((p, layout)) = draw(view, s, f, sh - 24.0) {
            let at = (((sw - p.width as f32) / 2.0).round(), ((sh - p.height as f32) / 2.0).round());
            return Some((MenuPicture { at, ..p }, layout.moved(at)));
        }
        s *= 0.8;
    }
    None
}

/// An achievement's announcement for the backglass, as a card at the foot of a window `seen`
/// big, sized to it: its words (`Achievement: Total Annihilation · 25 points`) as what it is over
/// the name, the points below.
pub fn toast_picture(words: &str, seen: (u32, u32)) -> Option<MenuPicture> {
    let f = fonts()?;
    let (sw, sh) = (seen.0.max(1) as f32, seen.1.max(1) as f32);
    let s = (sh / 720.0).clamp(0.5, 2.0).min(sw / 900.0);
    let (what, words) = words.split_once(": ").unwrap_or(("Achievement", words));
    let (name, rest) = words.split_once(" · ").unwrap_or((words, ""));
    let eyebrow = text(&f.heading, &what.to_uppercase(), 30.0 * s, ACCENT, 0.0);
    let title = text(&f.strong, name, 34.0 * s, FG, sw * 0.8);
    let detail = (!rest.is_empty()).then(|| text(&f.body, rest, 22.0 * s, GOOD, sw * 0.8));
    let pad = 24.0 * s;
    let inner = eyebrow.w.max(title.w).max(detail.as_ref().map_or(0, |d| d.w)) as f32;
    let w = (inner + 2.0 * pad).ceil();
    let h = (pad * 2.0 + eyebrow.h as f32 + 6.0 * s + title.h as f32 + detail.as_ref().map_or(0.0, |d| d.h as f32 + 4.0 * s)).ceil();
    let mut c = Canvas::new(w as usize, h as usize);
    c.rounded(0.0, 0.0, w, h, 16.0 * s, colour(BG), 0.92);
    c.rounded(0.0, 0.0, 6.0 * s, h, 3.0 * s, colour(ACCENT), 1.0);
    let mut y = pad;
    c.text(&eyebrow, pad, y, 0, eyebrow.h);
    y += eyebrow.h as f32 + 6.0 * s;
    c.text(&title, pad, y, 0, title.h);
    y += title.h as f32 + 4.0 * s;
    if let Some(d) = &detail {
        c.text(d, pad, y, 0, d.h);
    }
    let at = (((sw - w) / 2.0).round(), (sh - h - sh * 0.06).round().max(0.0));
    Some(MenuPicture { width: w as u32, height: h as u32, pixels: c.rgba(), at, dim: 0.0 })
}

/// The menu's pictures made on a thread of their own, so the window's frames never wait for one: a
/// 4K window's takes longer to make than a frame lasts, and a row lit or a level dragged asks for
/// another. The newest asked for is the one made; one asked for before it and not yet made is
/// skipped. One made while a newer was asked for is still handed over, the newer following once
/// it's made, so a PC slower to make them than the mouse moves sees the menu lag, never stop; one
/// made for a menu since closed (`cancel`) is dropped.
pub struct Maker {
    shared: Arc<(Mutex<Making>, std::sync::Condvar)>,
}

struct Making {
    /// The newest picture asked for, by number.
    asked: u64,
    /// The number the menu closed at: pictures up to it aren't wanted.
    closed: u64,
    job: Option<(u64, View, (u32, u32))>,
    /// The newest picture made and not yet taken.
    made: Option<(u64, Option<(MenuPicture, Layout)>)>,
    quit: bool,
}

/// A picture taking longer than this to make is said in the log, once: the menu follows the mouse
/// that far behind.
const SLOW_PICTURE: std::time::Duration = std::time::Duration::from_millis(100);

impl Maker {
    pub fn new() -> Maker {
        let shared = Arc::new((Mutex::new(Making { asked: 0, closed: 0, job: None, made: None, quit: false }), std::sync::Condvar::new()));
        let worker = Arc::clone(&shared);
        let started = std::thread::Builder::new().name("pause menu pictures".into()).spawn(move || {
            let (lock, wake) = &*worker;
            let mut told_slow = false;
            loop {
                let (n, view, seen) = {
                    let mut m = lock.lock().unwrap_or_else(|e| e.into_inner());
                    loop {
                        if m.quit {
                            return;
                        }
                        if let Some(job) = m.job.take() {
                            break job;
                        }
                        m = wake.wait(m).unwrap_or_else(|e| e.into_inner());
                    }
                };
                let started = std::time::Instant::now();
                let made = picture(&view, seen);
                let took = started.elapsed();
                if took > SLOW_PICTURE && !told_slow {
                    told_slow = true;
                    println!("the pause menu: its pictures take {} ms each to make here ({}x{}), so it follows the mouse that far behind", took.as_millis(), seen.0, seen.1);
                }
                let mut m = lock.lock().unwrap_or_else(|e| e.into_inner());
                if n > m.closed && m.made.as_ref().is_none_or(|(k, _)| n > *k) {
                    m.made = Some((n, made));
                }
            }
        });
        if let Err(e) = started {
            eprintln!("vpi-player: the pause menu's pictures: {e}");
        }
        Maker { shared }
    }

    /// The picture of `view` for a window seen as `seen` pixels, asked for.
    pub fn make(&self, view: View, seen: (u32, u32)) {
        let (lock, wake) = &*self.shared;
        let mut m = lock.lock().unwrap_or_else(|e| e.into_inner());
        m.asked += 1;
        m.job = Some((m.asked, view, seen));
        wake.notify_one();
    }

    /// No picture wanted any more: the menu closed.
    pub fn cancel(&self) {
        let mut m = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        m.asked += 1;
        m.closed = m.asked;
        m.job = None;
        m.made = None;
    }

    /// The newest picture made since the last taken, the newest asked for once it's made (`None`
    /// inside when the fonts can't be read).
    pub fn take(&self) -> Option<Option<(MenuPicture, Layout)>> {
        let mut m = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        match m.made.take() {
            Some((n, made)) if n > m.closed => Some(made),
            _ => None,
        }
    }
}

impl Default for Maker {
    fn default() -> Maker {
        Maker::new()
    }
}

impl Drop for Maker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        lock.lock().unwrap_or_else(|e| e.into_inner()).quit = true;
        wake.notify_one();
    }
}

/// The menu drawn at scale `s`; none if it would be taller than `most` pixels.
fn draw(view: &View, s: f32, f: &Fonts, most: f32) -> Option<(MenuPicture, Layout)> {
    let w = (720.0 * s).round();
    let pad = 40.0 * s;
    let inner = w - 2.0 * pad;
    // the pieces first, for the height
    let heading = text(&f.heading, &view.heading.to_uppercase(), 50.0 * s, ACCENT, 0.0);
    let title = text(&f.strong, &view.title, 24.0 * s, FG, inner);
    let byline = (!view.byline.is_empty()).then(|| text(&f.body, &view.byline, 16.0 * s, MUTED, inner));
    let hint = text(&f.body, &view.hint, 15.0 * s, FAINT, inner);
    let row_h = 46.0 * s;
    let rows: Vec<(Option<Text>, Text, Option<Text>)> = view
        .rows
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let chosen = k == view.selected;
            let section = r.section.as_deref().filter(|t| !t.is_empty()).map(|t| text(&f.strong, &t.to_uppercase(), 13.0 * s, FAINT, 0.0));
            let label = text(&f.strong, &r.label, 20.0 * s, if chosen { FG } else { SOFT }, inner * 0.55);
            let value = match (&r.value, r.opens) {
                (Some(v), _) if chosen && r.changes => Some(text(&f.body, &format!("\u{2039}  {v}  \u{203a}"), 18.0 * s, FG, 0.0)),
                (Some(v), _) => Some(text(&f.body, v, 18.0 * s, SOFT, 0.0)),
                (None, true) => Some(text(&f.strong, "\u{203a}", 24.0 * s, if chosen { ACCENT } else { FAINT }, 0.0)),
                (None, false) => None,
            };
            (section, label, value)
        })
        .collect();
    let section_gap = |r: &RowView| match r.section.as_deref() {
        None => 0.0,
        Some("") => 14.0 * s,
        Some(_) => 18.0 * s + 13.0 * s * 1.6,
    };
    // the rules — their text and pictures — and how much of them shows
    let rules = match &view.text {
        Some(t) => Some(rules_picture(t, &view.pictures, inner, s, f)),
        None if !view.controls.is_empty() => Some(controls_picture(&view.controls, inner, s, f)),
        None => None,
    };
    let fixed = pad + heading.h as f32 + 6.0 * s + title.h as f32 + byline.as_ref().map_or(0.0, |b| b.h as f32 + 2.0 * s) + 22.0 * s;
    let rows_h: f32 = view.rows.iter().map(|r| section_gap(r) + row_h + 2.0 * s).sum();
    let footer = 16.0 * s + 1.0 + 14.0 * s + hint.h as f32 + pad * 0.8;
    let rules_room = rules.as_ref().map(|r| (most - fixed - rows_h - footer - 16.0 * s).max(0.0).min(r.h as f32));
    if rules_room == Some(0.0) || fixed + rows_h + footer + rules_room.map_or(0.0, |v| v + 16.0 * s) > most {
        return None;
    }
    let h = (fixed + rules_room.map_or(0.0, |v| v + 16.0 * s) + rows_h + footer).round();
    let mut c = Canvas::new(w as usize, h as usize);
    // the panel: a hairline round it, as the app's cards
    c.rounded(0.0, 0.0, w, h, 18.0 * s, colour(LINE), 1.0);
    let edge = (1.5 * s).max(1.0);
    c.rounded(edge, edge, w - 2.0 * edge, h - 2.0 * edge, 18.0 * s - edge, colour(BG), 1.0);
    let mut y = pad;
    c.text(&heading, pad, y, 0, heading.h);
    y += heading.h as f32 + 6.0 * s;
    c.text(&title, pad, y, 0, title.h);
    y += title.h as f32;
    if let Some(b) = &byline {
        y += 2.0 * s;
        c.text(b, pad, y, 0, b.h);
        y += b.h as f32;
    }
    y += 22.0 * s;
    if let (Some(r), Some(room)) = (&rules, rules_room) {
        // a press scrolls a quarter of what shows
        let step = (room / 4.0).max(1.0);
        let most_top = ((r.h as f32 - room) / step).ceil().max(0.0) as usize;
        RULES_MOST_TOP.store(most_top, Ordering::Release);
        let from = ((view.text_top.min(most_top) as f32 * step) as usize).min(r.h.saturating_sub(room as usize));
        c.text(r, pad, y, from, room as usize);
        y += room + 16.0 * s;
    }
    let mut boxes = Vec::with_capacity(view.rows.len());
    for (k, (r, (section, label, value))) in view.rows.iter().zip(&rows).enumerate() {
        y += section_gap(r);
        if let Some(t) = section {
            c.text(t, pad, y - 13.0 * s * 1.6, 0, t.h);
        }
        let chosen = k == view.selected;
        if chosen {
            c.rounded(pad - 14.0 * s, y, inner + 28.0 * s, row_h, 10.0 * s, colour(ACCENT_BG), 1.0);
            c.rounded(pad - 14.0 * s, y + 10.0 * s, 4.0 * s, row_h - 20.0 * s, 2.0 * s, colour(ACCENT), 1.0);
        }
        c.text(label, pad, y + (row_h - label.h as f32) / 2.0, 0, label.h);
        let right = w - pad;
        let value_w = 92.0 * s;
        if let Some(v) = value {
            c.text(v, right - v.w as f32, y + (row_h - v.h as f32) / 2.0, 0, v.h);
        }
        // where the mouse finds it: the row, its bar, the value whose halves step it down and up
        let track_w = 190.0 * s;
        let track_x = right - value_w - 12.0 * s - track_w;
        boxes.push(RowBox {
            top: y,
            bottom: y + row_h,
            left: pad - 14.0 * s,
            right: pad + inner + 14.0 * s,
            bar: r.bar.map(|_| (track_x, track_w)),
            value: (r.changes && value.is_some()).then(|| (right - value.as_ref().map_or(0.0, |v| v.w as f32).max(value_w), right)),
        });
        if let Some(fill) = r.bar {
            // a track between the label and the value, filled to the level
            let (tx, ty, th) = (track_x, y + row_h / 2.0 - 3.0 * s, 6.0 * s);
            c.rounded(tx, ty, track_w, th, th / 2.0, colour(TRACK), 1.0);
            let fill = fill.clamp(0.0, 1.0);
            if fill > 0.0 {
                c.rounded(tx, ty, (track_w * fill).max(th), th, th / 2.0, colour(ACCENT), if chosen { 1.0 } else { 0.7 });
            }
            if chosen {
                let knob = 8.0 * s;
                c.rounded(tx + track_w * fill - knob, y + row_h / 2.0 - knob, 2.0 * knob, 2.0 * knob, knob, colour(FG), 1.0);
            }
        }
        y += row_h + 2.0 * s;
    }
    y += 16.0 * s;
    c.rounded(pad, y, inner, 1.0, 0.0, colour(LINE), 1.0);
    y += 1.0 + 14.0 * s;
    c.text(&hint, pad, y, 0, hint.h);
    Some((MenuPicture { width: c.w as u32, height: c.h as u32, pixels: c.rgba(), at: (0.0, 0.0), dim: DIM }, Layout { panel: [0.0, 0.0, w, h], rows: boxes }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests that make a menu share its statics (shown, the mouse's news): one at a time.
    static ONE_MENU: Mutex<()> = Mutex::new(());

    /// How long the menu's picture takes to make for a 4K window, as the window thread makes it
    /// when what the menu shows changes: `cargo test --release -p player menu_picture_time --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn menu_picture_time() {
        let _one = ONE_MENU.lock().unwrap_or_else(|e| e.into_inner());
        let menu = Menu::new(Setup { picture_only: true, ..Setup::default() });
        let sounds = crate::audio::Sounds::none_played_for_tests();
        let view = menu.view(&sounds, &[], &[]);
        for size in [(3840, 2160), (1920, 1080)] {
            let times: Vec<f64> = (0..5)
                .map(|_| {
                    let t = std::time::Instant::now();
                    assert!(picture(&view, size).is_some());
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            println!("menu picture at {}x{}: {:.1?} ms", size.0, size.1, times);
        }
        set_shown(false);
    }

    /// The pictures made on their own thread arrive, the newest asked for last.
    #[test]
    fn the_maker_hands_over_the_newest_picture_asked_for() {
        let _one = ONE_MENU.lock().unwrap_or_else(|e| e.into_inner());
        let menu = Menu::new(Setup { picture_only: true, ..Setup::default() });
        let sounds = crate::audio::Sounds::none_played_for_tests();
        let view = menu.view(&sounds, &[], &[]);
        let size_of = |size| picture(&view, size).map(|(q, _)| (q.width, q.height));
        let (small, big) = (size_of((1280, 720)), size_of((1920, 1080)));
        let maker = Maker::new();
        maker.make(view.clone(), (1280, 720));
        maker.make(view.clone(), (1920, 1080));
        // the first, if it was being made as the second was asked for, then the second
        let mut sizes = Vec::new();
        for _ in 0..500 {
            match maker.take() {
                Some(made) => {
                    let (p, _) = made.expect("its fonts read");
                    sizes.push(Some((p.width, p.height)));
                    if sizes.last() == Some(&big) {
                        break;
                    }
                }
                None => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
        assert_eq!(sizes.last(), Some(&big), "the newest made in time");
        assert!(sizes.iter().all(|s| *s == small || *s == big));
        // nothing more till another is asked for; none once the menu closes
        assert!(maker.take().is_none());
        maker.make(view, (1280, 720));
        maker.cancel();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(maker.take().is_none());
        set_shown(false);
    }

    /// Asked for again and again, faster than they're made (a slow PC, the mouse moving down the
    /// rows), the menu's pictures still arrive: each one made is handed over, none waited for
    /// forever.
    #[test]
    fn the_maker_keeps_up_with_a_moving_mouse() {
        let _one = ONE_MENU.lock().unwrap_or_else(|e| e.into_inner());
        let menu = Menu::new(Setup { picture_only: true, ..Setup::default() });
        let sounds = crate::audio::Sounds::none_played_for_tests();
        let mut view = menu.view(&sounds, &[], &[]);
        let rows = view.rows.len().max(1);
        let maker = Maker::new();
        let start = std::time::Instant::now();
        let mut handed = 0;
        while handed < 2 && start.elapsed() < std::time::Duration::from_secs(20) {
            view.selected = (view.selected + 1) % rows;
            maker.make(view.clone(), (1920, 1080));
            if maker.take().is_some() {
                handed += 1;
            }
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
        assert!(handed >= 2, "pictures handed over while still asked for: {handed}");
        set_shown(false);
    }

    fn option(id: &str, min: f32, max: f32, step: f32, default: f32, names: Option<&[&str]>) -> TableOptionDef {
        TableOptionDef { id: id.into(), name: id.into(), min, max, step, default, unit: 0, names: names.map(|n| n.iter().map(|s| s.to_string()).collect()) }
    }

    #[test]
    fn options_step_within_their_range_and_choices_go_round() {
        let mut stored = Vec::new();
        let volume = option("Volume", 0.0, 1.0, 0.1, 0.5, None);
        assert!(step_option(&volume, &mut stored, 1.0, false));
        assert_eq!(option_value(&volume, &stored), 0.6);
        let colours = option("Colour", 0.0, 2.0, 1.0, 2.0, Some(&["Red", "Green", "Blue"]));
        // at its last, a step on holds it; Choose goes round to the first
        assert!(!step_option(&colours, &mut stored, 1.0, false));
        assert!(step_option(&colours, &mut stored, 1.0, true));
        assert_eq!(option_text(&colours, option_value(&colours, &stored)), "Red");
        let rails = option("Rails", 0.0, 1.0, 1.0, 1.0, Some(&["Hide", "Show"]));
        assert!(step_option(&rails, &mut stored, -1.0, false));
        assert_eq!(option_value(&rails, &stored), 0.0);
    }

    #[test]
    fn the_view_page_changes_the_view_goes_back_to_its_own_and_keeps_what_differs() {
        let own = ViewSetup { mode: Mode::Camera, fov: 50.0, ..ViewSetup::default() };
        let now = ViewSetup { fov: 55.0, ..own };
        let file = std::env::temp_dir().join(format!("vpi-view-page-{}.ini", std::process::id()));
        std::fs::write(&file, "[TableOverride]\r\nViewDTFOV = 55\r\n").unwrap();
        let _one = ONE_MENU.lock().unwrap_or_else(|e| e.into_inner());
        let setup = Setup { table_file: Some(file.clone()), view: Some(Viewing { which: camera::View::Desktop, now, own }), ..Setup::default() };
        let mut menu = Menu::new(setup);
        let mut sounds = crate::audio::Sounds::none_played_for_tests();
        let mut stored = Vec::new();
        // its page, from the main page's Table rows
        let k = menu.rows(&sounds, &[], &stored).iter().position(|(_, r)| *r == Row::View).expect("a view row");
        menu.selected = k;
        menu.nav(Nav::Choose, &mut sounds, &[], &mut stored);
        assert_eq!(menu.page, Page::View);
        let rows = menu.rows(&sounds, &[], &stored);
        let labels: Vec<String> = menu.view(&sounds, &[], &stored).rows.iter().map(|r| r.label.clone()).collect();
        assert_eq!(&labels[..6], ["View mode", "Field of view", "Look at", "Height", "Distance", "Size"]);
        assert!(rows.iter().any(|(s, r)| *s == Some("More") && *r == Row::Cam(Cam::Width)));
        // not its own view yet: "Back to its own view" shows
        assert!(rows.iter().any(|(_, r)| *r == Row::OwnView));
        // the field of view a step up: the table drawn so
        menu.selected = rows.iter().position(|(_, r)| *r == Row::Cam(Cam::Fov)).unwrap();
        menu.nav(Nav::Right, &mut sounds, &[], &mut stored);
        let view = menu.view(&sounds, &[], &stored);
        assert_eq!((view.camera.map(|c| c.fov), view.view_page), (Some(55.5), true));
        // the turn goes round
        for _ in 0..4 {
            menu.cam_step(Cam::Turn, 1.0);
        }
        assert_eq!(menu.view_now.unwrap().viewport_rotation, 0.0);
        menu.cam_step(Cam::Turn, -1.0);
        assert_eq!(menu.view_now.unwrap().viewport_rotation, 270.0);
        menu.cam_step(Cam::Turn, 1.0);
        // back to its own view, then a size step: kept as what differs from its own
        let back = menu.rows(&sounds, &[], &stored).iter().position(|(_, r)| *r == Row::OwnView).unwrap();
        menu.selected = back;
        menu.nav(Nav::Choose, &mut sounds, &[], &mut stored);
        assert_eq!(menu.view_now, Some(own));
        assert!(!menu.rows(&sounds, &[], &stored).iter().any(|(_, r)| *r == Row::OwnView));
        menu.cam_step(Cam::Size, 1.0);
        assert_eq!(menu.view_now.unwrap().scene_scale, crate::math::Vec3::new(1.01, 1.01, 1.01));
        menu.close(&sounds, &[], &stored);
        let kept = std::fs::read_to_string(&file).unwrap();
        let _ = std::fs::remove_file(&file);
        assert!(!kept.contains("ViewDTFOV") && kept.contains("ViewDTScaleX = 1.01") && kept.contains("ViewDTScaleZ = 1.01"), "{kept}");
    }

    #[test]
    fn options_reset_to_the_tables_own_defaults() {
        let mut stored = Vec::new();
        let rails = option("Rails", 0.0, 1.0, 1.0, 1.0, Some(&["Hide", "Show"]));
        let volume = option("Volume", 0.0, 1.0, 0.1, 0.5, None);
        assert!(option_at_default(&rails, &stored) && option_at_default(&volume, &stored));
        assert!(step_option(&rails, &mut stored, -1.0, false) && step_option(&volume, &mut stored, 1.0, false));
        assert!(!option_at_default(&rails, &stored) && !option_at_default(&volume, &stored));
        let _one = ONE_MENU.lock().unwrap_or_else(|e| e.into_inner());
        let mut menu = Menu::new(Setup { picture_only: true, ..Setup::default() });
        menu.page = Page::Options;
        let mut sounds = crate::audio::Sounds::none_played_for_tests();
        let options = [rails.clone(), volume.clone()];
        assert_eq!(menu.reset(Row::ResetOptions, &mut sounds, &options, &mut stored), Outcome::OptionChanged);
        assert!(option_at_default(&rails, &stored) && option_at_default(&volume, &stored));
        // nothing left to reset: no row for it
        assert!(!menu.rows(&sounds, &options, &stored).iter().any(|(_, r)| *r == Row::ResetOptions));
        set_shown(false);
    }

    #[test]
    fn the_menus_keys_are_vpxs_actions_and_a_keyboards_own() {
        use crate::dik::default_keys as k;
        assert_eq!(Nav::of(code::UP), Some(Nav::Up));
        assert_eq!(Nav::of(0xC8), Some(Nav::Up));
        assert_eq!(Nav::of(k::PLUNGER), Some(Nav::Choose));
        assert_eq!(Nav::of(k::START_GAME), Some(Nav::Choose));
        assert_eq!(Nav::of(k::ESCAPE), Some(Nav::Back));
        // the flippers reach the menu as VPX 5436's UILeft and UIRight, never as themselves
        assert_eq!(Nav::of(k::LEFT_FLIPPER), None);
        assert!(!code::is_ours(0xED) && code::is_ours(code::PAUSE) && code::is_ours(code::RIGHT));
    }

    #[test]
    fn the_rules_name_the_tables_pictures() {
        assert_eq!(picture_names("![Rules](rulescard)\nAnd ![a](Card2 \"its title\") here ![b](rulesCARD)"), ["rulescard", "Card2"]);
        assert_eq!(images_in("x ![Rules](rulescard) y"), [(2, "rulescard".to_string(), 21)]);
    }

    #[test]
    fn a_tables_own_controls_are_the_keys_its_script_reads() {
        let script = "Sub Table1_KeyDown(ByVal keycode)\n  If keycode = LeftMagnaSave Then Magna\n  ' If keycode = LockbarKey Then (a comment)\n  Rem MechanicalTilt too\n  s = \"it's LockbarKey in a string\"\nEnd Sub";
        let labels: Vec<&str> = table_extras(script).iter().map(|e| e.label).collect();
        assert_eq!(labels, ["Magna-saves", "Fire button"]);
        let rom = table_extras("LoadVPM \"01560000\", \"WPC.VBS\", 3.26");
        assert_eq!(rom.iter().map(|e| e.label).collect::<Vec<_>>(), ["Coin door", "Service buttons"]);
        let lines = control_lines(&rom, &|a| if a == "CoinDoor" { vec!["End".into()] } else { Vec::new() }, true);
        assert_eq!((lines[0].section.as_deref(), lines[0].label.as_str(), lines[0].keys.as_str()), (Some("This table"), "Coin door", "End"));
        assert_eq!(lines.last().map(|l| l.keys.as_str()), Some("Esc, then Quit the table"));
    }

    /// A line whose third byte falls inside a letter, as a comment's `' ©` puts it, is read as any
    /// other line.
    #[test]
    fn lines_starting_with_letters_of_two_bytes_are_read() {
        let script = "' \u{a9} 2023 the table's authors\n'\u{a9}\n\u{a9}\u{a9}\nRem LeftMagnaSave\nRemark = LockbarKey\n";
        let labels: Vec<&str> = table_extras(script).iter().map(|e| e.label).collect();
        assert_eq!(labels, ["Fire button"]);
    }

    #[test]
    fn the_mouse_picks_a_row_sets_a_level_on_its_bar_and_chooses() {
        let _one = ONE_MENU.lock().unwrap_or_else(|e| e.into_inner());
        let mut menu = Menu::new(Setup { picture_only: true, ..Setup::default() });
        let mut sounds = crate::audio::Sounds::none_played_for_tests();
        let mut stored = Vec::new();
        let rows = menu.rows(&sounds, &[], &stored);
        let playfield = rows.iter().position(|(_, r)| *r == Row::Level(Level::Playfield)).unwrap();
        let quit_before = rows.iter().position(|(_, r)| *r == Row::Quit).unwrap();
        point(Pointer::Over(playfield, rows.len()));
        point(Pointer::Click(playfield, Spot::Bar(0.5), rows.len()));
        assert_eq!(menu.pointed(&mut sounds, &[], &mut stored), []);
        assert_eq!((menu.selected, sounds.playfield_volume), (playfield, 0.5));
        // Reset to defaults shows now: a click aimed at the picture before is left alone
        point(Pointer::Click(quit_before, Spot::Row, rows.len()));
        assert_eq!(menu.pointed(&mut sounds, &[], &mut stored), []);
        let rows = menu.rows(&sounds, &[], &stored);
        let quit = rows.iter().position(|(_, r)| *r == Row::Quit).unwrap();
        assert_eq!(quit, quit_before + 1);
        point(Pointer::Click(quit, Spot::Row, rows.len()));
        point(Pointer::Over(0, rows.len()));
        // a click that closes the menu ends what's taken: the rest isn't for it
        assert_eq!(menu.pointed(&mut sounds, &[], &mut stored), [Outcome::Quit]);
        menu.selected = 0;
        menu.pick_quit(&sounds, &[], &stored);
        assert_eq!(menu.selected, quit);
        set_shown(false);
    }

    #[test]
    fn a_layout_finds_the_row_its_bar_and_its_values_halves() {
        let row = RowBox { top: 10.0, bottom: 50.0, left: 0.0, right: 400.0, bar: Some((100.0, 200.0)), value: Some((320.0, 400.0)) };
        let layout = Layout { panel: [0.0, 0.0, 400.0, 60.0], rows: vec![row] }.moved((20.0, 0.0));
        assert_eq!(layout.at(220.0, 30.0), Some((0, Spot::Bar(0.5))));
        assert_eq!(layout.at(350.0, 30.0), Some((0, Spot::Less)));
        assert_eq!(layout.at(410.0, 30.0), Some((0, Spot::More)));
        assert_eq!(layout.at(40.0, 30.0), Some((0, Spot::Row)));
        assert_eq!(layout.at(40.0, 55.0), None);
        assert_eq!(layout.along(0, 0.0), Some(0.0));
    }

    #[test]
    fn rules_lose_their_markdown() {
        assert_eq!(plain("# Rules\n- **Spell** it\nok"), "Rules\n\u{2022} Spell it\nok");
    }
}
