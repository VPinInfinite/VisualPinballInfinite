// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the B2S plugin's data
// model (plugins/b2s/B2SDataModel.cpp and B2SDataModel.h), how its server finds a table's backglass file
// (plugins/b2s/B2SServer.cpp), and what its renderer shows on the backglass and the score view
// (plugins/b2s/B2SRenderer.cpp), with its later rules for a light's channel (2026-10: ecbbe7275, 1ea259b74).
// The animations' playing is our own, from B2S Backglass Designer's own description of them: VPX 5436's
// plugin leaves them unplayed.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A table's backglass file (`.directb2s`, made with B2S Backglass
//! Designer), read as VPX 5436 reads it: the backglass
//! picture, its lights — "bulbs", pictures lit by the machine's lamps, coils
//! and lighting strings, or by the table's script — its score reels, the
//! score panel for a third screen with lights and scores of its own, and its
//! animations, light shows the backglass plays by itself.
//!
//! The file is XML holding every picture as base64 text, often tens of
//! megabytes, so it's read away from the game (on a thread of its own), and
//! its pictures are decoded side by side.

use std::path::{Path, PathBuf};

use base64::Engine as _;

/// A picture from the file: RGBA, 8 bits a channel, rows from the top.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picture({}x{})", self.width, self.height)
    }
}

/// What lights a picture from the machine (B2S's `RomIDType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RomKind {
    None,
    Lamp,
    Solenoid,
    GiString,
    Mech,
}

impl RomKind {
    fn from(n: i32) -> RomKind {
        match n {
            1 => RomKind::Lamp,
            2 => RomKind::Solenoid,
            3 => RomKind::GiString,
            4 => RomKind::Mech,
            _ => RomKind::None,
        }
    }
}

/// How a light's picture moves (B2S's `SnippitType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Snippit {
    Standard,
    /// Turns by itself.
    SelfRotating,
    /// Turned by one of the machine's mechanisms.
    MechRotating,
}

/// One light on the backglass (B2S's `Bulb`).
#[derive(Debug)]
pub struct Bulb {
    pub name: String,
    /// Lit by the script (`B2SSetData id`), or −1.
    pub b2s_id: i32,
    /// Lit fully when the script's value is this one; 0: the value is its brightness.
    pub b2s_value: i32,
    /// The machine's lamp, coil, lighting string or mechanism that lights it.
    pub rom_id: i32,
    pub rom_kind: RomKind,
    pub rom_inverted: bool,
    /// Lit before anything lights it.
    pub initial_state: bool,
    pub z_order: i32,
    /// The tint of its picture.
    pub light_color: [f32; 3],
    pub visible: bool,
    /// Where it sits on the backglass picture, in its pixels from the top left.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub snippit: Snippit,
    /// A turning picture's steps around a full turn.
    pub rotating_steps: i32,
    /// Its picture lit, and unlit (drawn under it while it isn't fully lit).
    pub image: Option<Picture>,
    pub off_image: Option<Picture>,
}

/// How a score shows (B2S's `ReelType`, by its start).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreKind {
    Reel,
    Dream7,
    Led,
    RenderedLed,
    ImportedLed,
}

/// One score on the backglass: digits drawn from reel pictures, or segments
/// (B2S's `Score`).
#[derive(Debug)]
pub struct Score {
    /// The first digit's number, for scores set digit by digit (0: counted in order).
    pub start_digit: i32,
    /// The player whose score it shows (0: set digit by digit).
    pub player: i32,
    /// The reel pictures' name, ending in `_0` or `_00` (the digit's place).
    pub reel_type: String,
    pub digits: i32,
    pub spacing: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub kind: ScoreKind,
}

/// One reel picture: a digit (or `Empty`) of a reel type.
#[derive(Debug)]
pub struct ReelImage {
    pub name: String,
    pub image: Option<Picture>,
}

/// What starts and stops an animation besides the script (B2S's `IDJoin`):
/// one of the machine's outputs — inverted, it plays while that is off — or
/// a light the script sets by its B2S id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Join {
    Machine { kind: RomKind, number: i32, inverted: bool },
    Script { id: i32 },
}

/// The lights an animation switches as it starts (B2S's
/// `LightsStateAtAnimationStart`): its own off or on, every light off, or
/// none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartLights {
    InvolvedOff,
    InvolvedOn,
    AllOff,
    NoChange,
}

/// The lights an animation switches as it ends (B2S's
/// `LightsStateAtAnimationEnd`): `Reset` puts every light of its screen back
/// as it was when the animation started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndLights {
    InvolvedOff,
    InvolvedOn,
    Reset,
    NoChange,
}

/// What stopping an animation does (B2S's `AnimationStopBehaviour`): end
/// it at once, let it finish its loops, or let it run back to its first
/// step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopBehaviour {
    Immediately,
    TillEnd,
    ToFirstStep,
}

/// One step of an animation (B2S's `AnimationStep`): lights switched on by
/// name, a wait, lights switched off, a wait, the waits counted in the
/// animation's intervals. Lights of the same name switch together.
#[derive(Debug)]
pub struct AnimationStep {
    pub on: Vec<String>,
    pub wait_on: i32,
    pub off: Vec<String>,
    pub wait_off: i32,
}

/// A light show the backglass plays by itself (B2S's `Animation`).
#[derive(Debug)]
pub struct Animation {
    pub name: String,
    /// On the third screen's score panel; else on the backglass.
    pub on_display: bool,
    /// Its unit of time, in milliseconds.
    pub interval_ms: i32,
    /// Times through its steps; 0 for endless.
    pub loops: i32,
    pub join: Option<Join>,
    /// Starts as the backglass starts.
    pub at_startup: bool,
    pub start_lights: StartLights,
    pub end_lights: EndLights,
    pub stop: StopBehaviour,
    /// While it plays, its lights ignore the machine and the script.
    pub lock: bool,
    pub steps: Vec<AnimationStep>,
}

/// Where a light, score or animation of a backglass file shows: on the
/// backglass, or on the score panel for a third screen (B2S's `DMD`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Backglass,
    Panel,
}

impl Page {
    pub const BOTH: [Page; 2] = [Page::Backglass, Page::Panel];

    pub fn index(self) -> usize {
        self as usize
    }
}

/// A backglass file, as VPX 5436 keeps it (B2S's `B2STable`).
#[derive(Debug)]
pub struct B2sFile {
    pub name: String,
    /// The speaker grill's height at the bottom of the backglass picture
    /// (VPX cuts it off unless asked to show it).
    pub grill_height: i32,
    pub backglass: Option<Picture>,
    /// The backglass lit, drawn over it as bright as what lights it.
    pub backglass_on: Option<Picture>,
    pub backglass_on_rom: (RomKind, i32),
    pub backglass_off: Option<Picture>,
    /// The picture for a third screen's score display, with its own lights and scores.
    pub dmd_image: Option<Picture>,
    pub bulbs: Vec<Bulb>,
    pub dmd_bulbs: Vec<Bulb>,
    pub scores: Vec<Score>,
    pub dmd_scores: Vec<Score>,
    pub reels: Vec<ReelImage>,
    /// Its light shows, the first of each name (B2S Designer keeps them unique).
    pub animations: Vec<Animation>,
}

impl B2sFile {
    pub fn bulbs_of(&self, page: Page) -> &[Bulb] {
        match page {
            Page::Backglass => &self.bulbs,
            Page::Panel => &self.dmd_bulbs,
        }
    }

    pub fn scores_of(&self, page: Page) -> &[Score] {
        match page {
            Page::Backglass => &self.scores,
            Page::Panel => &self.dmd_scores,
        }
    }

    /// Whether the file has a score panel for a third screen: its picture
    /// or its lights (VPX's `RenderScoreView` draws nothing without either).
    pub fn has_panel(&self) -> bool {
        self.dmd_image.is_some() || !self.dmd_bulbs.is_empty()
    }

    /// The score panel's size in its own pixels: its picture's, else
    /// VPX's 1024 × 768.
    pub fn panel_size(&self) -> (f32, f32) {
        self.dmd_image.as_ref().map_or((1024.0, 768.0), |p| (p.width as f32, p.height as f32))
    }

    /// The reel picture for a digit of a reel type (VPX's `B2SReel::GetImage`):
    /// a type ending in `_00` takes two-figure digits, `_0` one; a negative
    /// digit is the empty reel.
    pub fn reel_image(&self, reel_type: &str, digit: i32) -> Option<&Picture> {
        let name = if let Some(stem) = reel_type.strip_suffix("_00") {
            if digit < 0 { format!("{stem}_Empty") } else { format!("{stem}_{digit:02}") }
        } else {
            let stem = reel_type.strip_suffix("_0")?;
            if digit < 0 { format!("{stem}_Empty") } else { format!("{stem}_{digit}") }
        };
        self.reels.iter().find(|r| r.name == name)?.image.as_ref()
    }
}

/// What the table's script told `B2S.Server` (VPX 5436's `B2SServer`):
/// lights by B2S id, scores by player, digits by number, and orders for the
/// animations.
#[derive(Default)]
pub struct ServerState {
    pub name: String,
    /// Each light's data, and the number of the order that set it last.
    lamps: std::collections::HashMap<i32, (f32, u64)>,
    orders: u64,
    player_scores: std::collections::HashMap<i32, i32>,
    digits: std::collections::HashMap<i32, i32>,
    /// The script's orders for the animations, waiting for the backglass's next frame.
    animation_orders: Vec<AnimationOrder>,
    /// The animations playing, by name in lower case, as the backglass last played them.
    playing: std::collections::HashSet<String>,
}

/// An order from the script to the backglass's animations.
enum AnimationOrder {
    Start { name: String, reverse: bool },
    Stop(String),
    StopAll,
}

impl ServerState {
    /// `B2SSetData`: a light's state, which scripts give as 0 and 1 or as
    /// any number.
    pub fn set_data(&mut self, id: i32, value: i32) {
        self.orders += 1;
        self.lamps.insert(id, (value as f32, self.orders));
    }

    /// A number after every order so far, for a ROM change that takes a light's channel.
    fn next_order(&mut self) -> u64 {
        self.orders += 1;
        self.orders
    }

    /// `B2SStartAnimation`, `B2SStartAnimationReverse`: it counts as playing
    /// at once.
    pub fn start_animation(&mut self, name: &str, reverse: bool) {
        self.playing.insert(name.to_lowercase());
        self.animation_orders.push(AnimationOrder::Start { name: name.to_string(), reverse });
    }

    /// `B2SStopAnimation`: it stops as its file says (`AnimationStopBehaviour`).
    pub fn stop_animation(&mut self, name: &str) {
        self.animation_orders.push(AnimationOrder::Stop(name.to_string()));
    }

    /// `B2SStopAllAnimations`.
    pub fn stop_all_animations(&mut self) {
        self.animation_orders.push(AnimationOrder::StopAll);
    }

    /// `B2SIsAnimationRunning`.
    pub fn animation_running(&self, name: &str) -> bool {
        self.playing.contains(&name.to_lowercase())
    }

    /// `B2SSetScore`, `B2SSetReel`, `B2SSetScoreDigit`: a digit, or a
    /// score's value under its first digit's number.
    pub fn set_score(&mut self, digit: i32, value: i32) {
        self.digits.insert(digit, value);
    }

    /// `B2SSetScorePlayer`: a player's score.
    pub fn set_player_score(&mut self, player: i32, score: i32) {
        self.player_scores.insert(player, score);
    }

    pub fn lamp(&self, id: i32) -> f32 {
        self.lamps.get(&id).map_or(0.0, |&(value, _)| value)
    }

    pub fn digit(&self, number: i32) -> i32 {
        self.digits.get(&number).copied().unwrap_or(0)
    }

    pub fn player_score(&self, player: i32) -> i32 {
        self.player_scores.get(&player).copied().unwrap_or(0)
    }

    /// Every player's score the script has shown, by player.
    pub fn player_scores(&self) -> Vec<(i32, i32)> {
        let mut out: Vec<(i32, i32)> = self.player_scores.iter().map(|(&p, &s)| (p, s)).collect();
        out.sort_unstable();
        out
    }
}

/// A table's backglass file: being read, read, or missing.
pub enum Loading {
    Reading(std::thread::JoinHandle<Result<B2sFile, String>>),
    Ready(std::sync::Arc<B2sFile>),
    Failed,
}

impl Loading {
    /// Starts reading the table's backglass file, when it has one.
    pub fn start(table: &Path) -> Option<Loading> {
        let path = find(table)?;
        let reading = std::thread::Builder::new().name("backglass".into()).spawn(move || read(&path)).ok()?;
        Some(Loading::Reading(reading))
    }

    /// The file, once read.
    pub fn ready(&mut self) -> Option<std::sync::Arc<B2sFile>> {
        if let Loading::Reading(r) = self
            && r.is_finished()
        {
            let Loading::Reading(r) = std::mem::replace(self, Loading::Failed) else { unreachable!() };
            match r.join() {
                Ok(Ok(file)) => *self = Loading::Ready(std::sync::Arc::new(file)),
                Ok(Err(e)) => crate::script::log::once(&format!("backglass: {e}")),
                Err(_) => crate::script::log::once("backglass: reading the file failed"),
            }
        }
        match self {
            Loading::Ready(f) => Some(f.clone()),
            _ => None,
        }
    }
}

/// What the backglass shows now: the file, the brightness of its lit
/// picture, and each page's lights, turns, reels and segment digits.
#[derive(Clone, Debug)]
pub struct BackglassNow {
    pub file: std::sync::Arc<B2sFile>,
    /// The backglass picture's height shown: the grill is cut off unless
    /// asked to show (VPX's `ShowGrill`, off by default).
    pub shown_height: f32,
    pub on: f32,
    /// The backglass, then the score panel.
    pub pages: [PageNow; 2],
}

/// One page of the backglass now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageNow {
    /// Each light's brightness.
    pub bulbs: Vec<f32>,
    /// Degrees, for lights turned by a mechanism.
    pub turns: Vec<f32>,
    /// Each score's digits, in order, for the scores shown.
    pub reels: Vec<(usize, Vec<i32>)>,
    /// Each segment score's digits (`Dream7`), for the scores shown: the
    /// digit's kind, its style and shape family, and its segments' levels.
    pub dream7: Vec<(usize, Vec<SegmentDigit>)>,
}

/// A digit of a segment score: its kind, its style and shape family, and its
/// segments' levels.
pub type SegmentDigit = (crate::segments::SegKind, crate::segments::SegStyle, crate::segments::SegHint, [f32; 16]);

impl BackglassNow {
    /// The backglass picture: the lit one's base, else the unlit one.
    pub fn base(&self) -> Option<&Picture> {
        self.file.backglass.as_ref().or(self.file.backglass_off.as_ref())
    }

    /// Whether `page` shows the same in both.
    pub fn same_page(&self, o: &BackglassNow, page: Page) -> bool {
        std::sync::Arc::ptr_eq(&self.file, &o.file) && self.pages[page.index()] == o.pages[page.index()] && (page == Page::Panel || self.on == o.on)
    }
}

impl PartialEq for BackglassNow {
    fn eq(&self, o: &BackglassNow) -> bool {
        std::sync::Arc::ptr_eq(&self.file, &o.file) && self.on == o.on && self.pages == o.pages
    }
}

/// An output of the machine as a brightness from 0 to 1, as VPX's backglass
/// reads it: lamps and coils on or off, or from 0 to 255 when modulated;
/// WPC's lighting strings in their 8 steps (or 0 to 255 when the script
/// asks PinMAME for physical outputs).
pub fn level(kind: RomKind, value: i32, physical: bool) -> f32 {
    match kind {
        RomKind::GiString if physical || value > 8 => value as f32 / 255.0,
        RomKind::GiString => value as f32 / 8.0,
        RomKind::Mech => value as f32,
        _ if value <= 1 => value.max(0) as f32,
        _ => (value as f32 / 255.0).min(1.0),
    }
}

/// The machine's outputs, read for the backglass.
pub trait Outputs {
    /// A lamp's, coil's, lighting string's or mechanism's value now, as a
    /// brightness (a position for a mechanism); None when there's no machine.
    fn get(&self, kind: RomKind, number: i32) -> Option<f32>;
}

/// One page's lights as the backglass plays.
struct Lights {
    /// Each light's brightness.
    level: Vec<f32>,
    /// Each light's ROM state as last read, through its value filter or inversion; -1 before the
    /// first reading.
    probe: Vec<f32>,
    /// The number of the order a ROM change last took each light's channel with: script writes
    /// and ROM changes share the channel, the latest wins.
    rom_order: Vec<u64>,
    /// The number of the script's order each light last followed.
    order: Vec<u64>,
    /// How many playing animations lock each light.
    locks: Vec<u32>,
    /// Degrees, for lights turned by a mechanism.
    turns: Vec<f32>,
}

impl Lights {
    fn new(bulbs: &[Bulb]) -> Lights {
        Lights {
            // rotating images always show at the start, their lamp only turning them (VPX's plugin
            // since 1ea259b74, as B2S Server)
            level: bulbs.iter().map(|b| if b.snippit != Snippit::Standard || b.initial_state { 1.0 } else { 0.0 }).collect(),
            probe: vec![-1.0; bulbs.len()],
            rom_order: vec![0; bulbs.len()],
            order: vec![0; bulbs.len()],
            locks: vec![0; bulbs.len()],
            turns: vec![0.0; bulbs.len()],
        }
    }
}

/// An animation's lights, found by name: each step's to switch on and to
/// switch off, and every light it switches.
struct Cast {
    page: Page,
    steps: Vec<(Vec<usize>, Vec<usize>)>,
    involved: Vec<usize>,
}

/// An animation as it plays.
#[derive(Default)]
struct Run {
    playing: bool,
    reverse: bool,
    step: usize,
    /// The step's lights to switch off come next.
    off_next: bool,
    /// When the next lights switch, in milliseconds of game time.
    due: f64,
    /// When the pass through the steps began.
    pass_began: f64,
    loops_done: i32,
    /// Asked to stop: it ends with its pass.
    stopping: bool,
    /// Its page's lights as they were when it started, for `EndLights::Reset`.
    saved: Vec<f32>,
}

/// The backglass as it plays, as B2S Server plays it (B2S Backglass
/// Designer's description): every light switched by orders — the machine's
/// outputs as they change, the script's data, the animations' steps — and
/// left as last switched. (VPX 5436's B2S plugin instead lights each light
/// from its output every frame, and leaves the animations unplayed.)
pub struct Show {
    file: std::sync::Arc<B2sFile>,
    /// The backglass's lights, then the score panel's.
    pages: [Lights; 2],
    on: f32,
    casts: Vec<Cast>,
    runs: Vec<Run>,
    /// Whether what each animation is joined to was on at the last frame.
    joined: Vec<bool>,
    started: bool,
    /// An animation started or ended since the script was last told.
    dirty: bool,
}

impl Show {
    pub fn new(file: std::sync::Arc<B2sFile>) -> Show {
        let casts = file
            .animations
            .iter()
            .map(|a| {
                let page = if a.on_display { Page::Panel } else { Page::Backglass };
                let bulbs = file.bulbs_of(page);
                // lights of the same name switch together
                let named = |names: &[String]| -> Vec<usize> {
                    let mut out: Vec<usize> = names.iter().flat_map(|n| bulbs.iter().enumerate().filter(move |(_, b)| b.name == *n).map(|(i, _)| i)).collect();
                    out.sort_unstable();
                    out.dedup();
                    out
                };
                let steps: Vec<(Vec<usize>, Vec<usize>)> = a.steps.iter().map(|s| (named(&s.on), named(&s.off))).collect();
                let mut involved: Vec<usize> = steps.iter().flat_map(|(on, off)| on.iter().chain(off)).copied().collect();
                involved.sort_unstable();
                involved.dedup();
                Cast { page, steps, involved }
            })
            .collect();
        let n = file.animations.len();
        Show {
            pages: [Lights::new(&file.bulbs), Lights::new(&file.dmd_bulbs)],
            on: 0.0,
            casts,
            runs: (0..n).map(|_| Run::default()).collect(),
            joined: vec![false; n],
            started: false,
            dirty: false,
            file,
        }
    }

    /// Plays the backglass up to `now`, in milliseconds of game time: the
    /// animations that start with it, the script's orders, the animations'
    /// joins, the machine's outputs and the script's data as they change,
    /// then the animations' steps due.
    pub fn play(&mut self, now: f64, mut server: Option<&mut ServerState>, outputs: &dyn Outputs) {
        if !self.started {
            self.started = true;
            for a in 0..self.runs.len() {
                if self.file.animations[a].at_startup {
                    self.start(a, false, now);
                }
            }
        }
        let orders = server.as_deref_mut().map(|s| std::mem::take(&mut s.animation_orders)).unwrap_or_default();
        if !orders.is_empty() {
            // a name the file doesn't have stops counting as playing
            self.dirty = true;
        }
        for order in orders {
            match order {
                AnimationOrder::Start { name, reverse } => {
                    if let Some(a) = self.find(&name) {
                        self.start(a, reverse, now);
                    }
                }
                AnimationOrder::Stop(name) => {
                    if let Some(a) = self.find(&name) {
                        self.stop(a);
                    }
                }
                AnimationOrder::StopAll => {
                    for a in 0..self.runs.len() {
                        self.stop(a);
                    }
                }
            }
        }
        // an animation starts as what it's joined to switches on, and is stopped as it switches off
        for a in 0..self.runs.len() {
            let Some(join) = self.file.animations[a].join else { continue };
            let active = match join {
                Join::Machine { kind, number, inverted } => outputs.get(kind, number).is_some_and(|v| (v > 0.0) != inverted),
                Join::Script { id } => server.as_deref().is_some_and(|s| s.lamp(id) != 0.0),
            };
            if active != self.joined[a] {
                self.joined[a] = active;
                if active {
                    self.start(a, false, now);
                } else {
                    self.stop(a);
                }
            }
        }
        // the lights, as their outputs change and the script orders: lights locked by an animation
        // let the orders pass
        let (kind, id) = self.file.backglass_on_rom;
        self.on = outputs.get(kind, id).unwrap_or(self.on);
        let Show { file, pages, .. } = self;
        for page in Page::BOTH {
            let lights = &mut pages[page.index()];
            for (i, b) in file.bulbs_of(page).iter().enumerate() {
                if b.snippit != Snippit::Standard {
                    if b.snippit == Snippit::MechRotating
                        && let Some(v) = outputs.get(b.rom_kind, b.rom_id)
                    {
                        lights.turns[i] = if b.rotating_steps != 0 { 360.0 * v / b.rotating_steps as f32 } else { 0.0 };
                    }
                    continue;
                }
                // a light's channel, as VPX's plugin has it since 1ea259b74 (B2S Server's): the
                // script's writes to its lamp's number (its B2SID, or a lamp RomID) and the ROM's
                // changes, the latest winning; the ROM's first reading isn't a change, so the light
                // keeps its first state until something happens (with no server, it counts). A light
                // locked by an animation takes neither; its ROM state is still followed, so once
                // free it keeps the animation's last state until the lamp next changes, as B2S
                // Server, which acts on changes (VPX stops reading while locked)
                let locked = lights.locks[i] > 0;
                let value = if b.b2s_id >= 0 { b.b2s_value } else { 0 };
                let mut rom_took = false;
                if let Some(raw) = outputs.get(b.rom_kind, b.rom_id) {
                    let probe = if value > 0 {
                        if raw as i32 == value { 1.0 } else { 0.0 }
                    } else if b.rom_inverted {
                        1.0 - raw
                    } else {
                        raw
                    };
                    let before = std::mem::replace(&mut lights.probe[i], probe);
                    if !locked && before >= 0.0 && probe != before {
                        lights.rom_order[i] = server.as_deref_mut().map_or(lights.rom_order[i] + 1, ServerState::next_order);
                        rom_took = true;
                    } else if !locked && before < 0.0 && server.is_none() {
                        rom_took = true;
                    }
                }
                if locked {
                    continue;
                }
                let lamp = if b.rom_kind == RomKind::Lamp && b.rom_id > 0 { b.rom_id } else { -1 };
                let written = server.as_deref().and_then(|s| s.lamps.get(&lamp).copied()).filter(|&(_, order)| order != lights.order[i]);
                if let Some((state, order)) = written {
                    lights.order[i] = order;
                    if order > lights.rom_order[i] {
                        // a write lights it when it matches B2SValue, else when it isn't 0 (RomInverted
                        // turning that round)
                        lights.level[i] = if value > 0 {
                            if state as i32 == value { 1.0 } else { 0.0 }
                        } else if (state != 0.0) != b.rom_inverted {
                            1.0
                        } else {
                            0.0
                        };
                        continue;
                    }
                }
                if rom_took {
                    lights.level[i] = lights.probe[i];
                }
            }
        }
        for a in 0..self.runs.len() {
            if self.runs[a].playing {
                self.advance(a, now);
            }
        }
        if self.dirty
            && let Some(s) = server
        {
            self.dirty = false;
            s.playing = self.runs.iter().zip(&self.file.animations).filter(|(r, _)| r.playing).map(|(_, a)| a.name.to_lowercase()).collect();
        }
    }

    /// The animation of a name: as written, else whatever the case of its letters.
    fn find(&self, name: &str) -> Option<usize> {
        let names = || self.file.animations.iter().map(|a| &a.name);
        names().position(|n| n == name).or_else(|| {
            let wanted = name.to_lowercase();
            names().position(|n| n.to_lowercase() == wanted)
        })
    }

    /// Starts animation `a` at `now`, from its first step (its last, played
    /// in reverse); one playing carries on, no longer stopping.
    fn start(&mut self, a: usize, reverse: bool, now: f64) {
        let Show { file, pages, casts, runs, dirty, .. } = self;
        let (anim, cast, run) = (&file.animations[a], &casts[a], &mut runs[a]);
        if run.playing {
            run.stopping = false;
            return;
        }
        let lights = &mut pages[cast.page.index()];
        *run = Run {
            playing: true,
            reverse,
            step: if reverse { cast.steps.len().saturating_sub(1) } else { 0 },
            due: now,
            pass_began: now,
            saved: if anim.end_lights == EndLights::Reset { lights.level.clone() } else { Vec::new() },
            ..Run::default()
        };
        *dirty = true;
        match anim.start_lights {
            StartLights::InvolvedOff => cast.involved.iter().for_each(|&i| lights.level[i] = 0.0),
            StartLights::InvolvedOn => cast.involved.iter().for_each(|&i| lights.level[i] = 1.0),
            StartLights::AllOff => {
                // every light but those always on: lit from the start, and switched by nothing
                for (i, b) in file.bulbs_of(cast.page).iter().enumerate() {
                    if !(b.initial_state && b.b2s_id < 0 && b.rom_kind == RomKind::None) {
                        lights.level[i] = 0.0;
                    }
                }
            }
            StartLights::NoChange => {}
        }
        if anim.lock {
            cast.involved.iter().for_each(|&i| lights.locks[i] += 1);
        }
    }

    /// Stops animation `a` as its file says: at once, at the end of its
    /// loops (of its pass, for one that loops endlessly), or back at its
    /// first step.
    fn stop(&mut self, a: usize) {
        let anim = &self.file.animations[a];
        if !self.runs[a].playing {
            return;
        }
        match anim.stop {
            StopBehaviour::Immediately => self.end(a),
            StopBehaviour::TillEnd if anim.loops > 0 => {}
            StopBehaviour::TillEnd | StopBehaviour::ToFirstStep => self.runs[a].stopping = true,
        }
    }

    /// Ends animation `a`, its lights left as its file says.
    fn end(&mut self, a: usize) {
        let Show { file, pages, casts, runs, dirty, .. } = self;
        let (anim, cast, run) = (&file.animations[a], &casts[a], &mut runs[a]);
        if !run.playing {
            return;
        }
        run.playing = false;
        *dirty = true;
        let lights = &mut pages[cast.page.index()];
        if anim.lock {
            cast.involved.iter().for_each(|&i| lights.locks[i] = lights.locks[i].saturating_sub(1));
        }
        match anim.end_lights {
            EndLights::InvolvedOff => cast.involved.iter().for_each(|&i| lights.level[i] = 0.0),
            EndLights::InvolvedOn => cast.involved.iter().for_each(|&i| lights.level[i] = 1.0),
            EndLights::Reset if run.saved.len() == lights.level.len() => lights.level.copy_from_slice(&run.saved),
            EndLights::Reset | EndLights::NoChange => {}
        }
        run.saved = Vec::new();
    }

    /// Plays animation `a`'s steps due by `now`: each step switches its
    /// lights on, waits, switches its lights off and waits, the waits counted
    /// in the animation's interval.
    fn advance(&mut self, a: usize, now: f64) {
        let Show { file, pages, casts, runs, .. } = self;
        let (anim, cast, run) = (&file.animations[a], &casts[a], &mut runs[a]);
        let lights = &mut pages[cast.page.index()];
        let interval = f64::from(anim.interval_ms.max(0));
        let n = cast.steps.len();
        let mut ended = n == 0;
        // far behind (the clock jumped): carry on from now
        let mut budget = 100_000;
        while !ended && run.due <= now {
            budget -= 1;
            if budget == 0 {
                run.due = now;
                break;
            }
            let (on, off) = &cast.steps[run.step];
            let step = &anim.steps[run.step];
            if !run.off_next {
                on.iter().for_each(|&i| lights.level[i] = 1.0);
                run.due += f64::from(step.wait_on.max(0)) * interval;
                run.off_next = true;
                continue;
            }
            off.iter().for_each(|&i| lights.level[i] = 0.0);
            run.due += f64::from(step.wait_off.max(0)) * interval;
            run.off_next = false;
            let last = if run.reverse { run.step == 0 } else { run.step + 1 == n };
            if !last {
                if run.reverse {
                    run.step -= 1;
                } else {
                    run.step += 1;
                }
                continue;
            }
            // a pass through the steps, which takes an interval at least
            run.loops_done += 1;
            if run.due <= run.pass_began {
                run.due = run.pass_began + interval.max(1.0);
            }
            run.pass_began = run.due;
            ended = run.stopping || (anim.loops > 0 && run.loops_done >= anim.loops);
            run.step = if run.reverse { n - 1 } else { 0 };
        }
        if ended {
            self.end(a);
        }
    }

    /// What the backglass shows now (VPX's `RenderBackglass` and
    /// `RenderScoreView`): each page's lights as played, and its scores —
    /// reels from the script's digits and players' scores, segment scores
    /// lit by the machine's segment displays. `show_grill`: the backglass
    /// picture's grill shows (VPX's `ShowGrill`), else it's cut off.
    pub fn now(&self, server: Option<&ServerState>, segments: Option<&crate::segments::SegmentsNow>, show_grill: bool) -> BackglassNow {
        let file = &self.file;
        let base = file.backglass.as_ref().or(file.backglass_off.as_ref());
        let grill_cut = if show_grill { 0.0 } else { file.grill_height as f32 };
        let shown_height = base.map_or(768.0, |p| p.height as f32) - grill_cut;
        let page = |page: Page, height: f32| {
            let lights = &self.pages[page.index()];
            let mut now = PageNow { bulbs: lights.level.clone(), turns: lights.turns.clone(), ..PageNow::default() };
            scores(&mut now, file.scores_of(page), height, server, segments);
            now
        };
        let pages = [page(Page::Backglass, shown_height), page(Page::Panel, file.panel_size().1)];
        BackglassNow { file: file.clone(), shown_height, on: self.on, pages }
    }
}

/// A page's scores (VPX's `RenderScores`): each one's digits, from a
/// player's score or digit by digit, counted in order from 1; and the
/// segment scores' digits, lit as the machine's. A score below `height` (on
/// the grill, while it's cut off) isn't shown.
fn scores(page: &mut PageNow, list: &[Score], height: f32, server: Option<&ServerState>, segments: Option<&crate::segments::SegmentsNow>) {
    let mut index = 1;
    for (n, s) in list.iter().enumerate() {
        if s.y as f32 > height {
            continue;
        }
        let mut digits = Vec::with_capacity(s.digits.max(0) as usize);
        let mut lit = Vec::new();
        for i in 0..s.digits {
            // the digit's number, for scores set digit by digit (VPX leaves it at -1 for a player's)
            let number = if s.player != 0 { -1 } else if s.start_digit > 0 { s.start_digit + i } else { index };
            let digit = if s.player != 0 {
                let mut score = server.map_or(0, |v| v.player_score(s.player)).abs();
                for _ in 0..(s.digits - 1 - i) {
                    score /= 10;
                }
                score % 10
            } else {
                server.map_or(0, |v| v.digit(number))
            };
            digits.push(digit);
            if s.kind == ScoreKind::Dream7 {
                lit.push(segment_digit(segments, number));
            }
            index += 1;
        }
        page.reels.push((n, digits));
        if s.kind == ScoreKind::Dream7 {
            page.dream7.push((n, lit));
        }
    }
}

/// A segment score's digit `number` (from 1): the machine's digit of that
/// number, its segments as lit now; else, as VPX draws it, an unlit
/// fourteen-segment digit.
fn segment_digit(segments: Option<&crate::segments::SegmentsNow>, number: i32) -> SegmentDigit {
    use crate::segments::{SegHint, SegKind, SegStyle, hint, style};
    let found = usize::try_from(number - 1).ok().and_then(|i| {
        let s = segments?;
        let (display, digit) = s.layout.digits().nth(i)?;
        Some((digit.kind, style(display.hardware), hint(display.hardware), *s.levels.get(i)?))
    });
    found.unwrap_or((SegKind::Fourteen, SegStyle::Plasma, SegHint::Generic, [0.0; 16]))
}

/// The running machine's outputs.
struct MachineOutputs<'a> {
    machine: Option<&'a crate::pinmame::Machine>,
    /// The script asked PinMAME for physical outputs (`SolMask(2)`).
    physical: bool,
}

impl Outputs for MachineOutputs<'_> {
    fn get(&self, kind: RomKind, number: i32) -> Option<f32> {
        use pinmame_link::OutputKind;
        let m = self.machine?;
        let value = match kind {
            RomKind::Lamp => m.output(OutputKind::Lamp, number),
            RomKind::Solenoid => m.output(OutputKind::Solenoid, number),
            RomKind::GiString => m.output(OutputKind::Gi, number),
            RomKind::Mech => m.mech_position(number),
            RomKind::None => return None,
        };
        Some(level(kind, value, self.physical))
    }
}

/// The backglass this frame, `now` milliseconds into the game, once its file
/// is read: the same as the last frame's when nothing changed.
pub fn shown(w: &mut crate::world::World, now: f64) -> Option<std::sync::Arc<BackglassNow>> {
    let file = w.backglass.as_mut()?.ready()?;
    let segments = w.machine_segments.as_ref().map(|(_, s)| s.clone());
    // the machine's digits, read as figures, set the server's digits one by one, as VPX's
    // backglass sets them each time it draws (for scores drawn from pictures)
    if let (Some(s), Some(server)) = (&segments, w.controller.as_mut().and_then(|c| c.b2s.as_mut())) {
        for (i, levels) in s.levels.iter().enumerate() {
            server.set_score(i as i32 + 1, crate::segments::figure(levels));
        }
    }
    if w.backglass_show.as_ref().is_none_or(|s| !std::sync::Arc::ptr_eq(&s.file, &file)) {
        w.backglass_show = Some(Show::new(file.clone()));
    }
    let show = w.backglass_show.as_mut()?;
    let (state, mut server) = match w.controller.as_deref_mut() {
        Some(c) => (Some(&c.state), c.b2s.as_mut()),
        None => (None, None),
    };
    let outputs = MachineOutputs { machine: state.and_then(|s| s.machine.as_ref()), physical: state.is_some_and(|s| s.device_mode == crate::pinmame::DEVICE_PHYSICAL) };
    show.play(now, server.as_deref_mut(), &outputs);
    let now = show.now(server.as_deref(), segments.as_deref(), w.show_grill);
    if w.backglass_now.as_deref() != Some(&now) {
        w.backglass_now = Some(std::sync::Arc::new(now));
    }
    w.backglass_now.clone()
}

/// The backglass file for a table (VPX's `B2SServer`): `<table>.directb2s`
/// beside it, else one named after the table's folder, whatever the case of
/// the letters.
pub fn find(table: &Path) -> Option<PathBuf> {
    let folder = table.parent()?;
    let wanted = |name: &std::ffi::OsStr| {
        let mut n = PathBuf::from(name);
        n.set_extension("directb2s");
        n.into_os_string().to_string_lossy().to_lowercase()
    };
    let names = [table.file_name().map(wanted), folder.file_name().map(wanted)];
    let entries: Vec<PathBuf> = std::fs::read_dir(folder).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    names.into_iter().flatten().find_map(|name| entries.iter().find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().to_lowercase() == name)).cloned())
}

/// Reads a backglass file.
pub fn read(path: &Path) -> Result<B2sFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    parse(&text)
}

/// The pictures still to decode, as base64 text inside the file.
struct Pictures<'a>(Vec<&'a str>);

impl<'a> Pictures<'a> {
    fn add(&mut self, text: Option<&'a str>) -> Option<usize> {
        let text = text.filter(|t| !t.is_empty())?;
        self.0.push(text);
        Some(self.0.len() - 1)
    }
}

/// The file's model before its pictures are decoded: pictures by number.
struct Raw<T>(T);

/// A light before its pictures (lit, unlit) are decoded.
type RawBulb = Raw<(Bulb, Option<usize>, Option<usize>)>;

/// Reads a backglass file's text.
pub fn parse(text: &str) -> Result<B2sFile, String> {
    let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(text, options).map_err(|e| format!("not a backglass file: {e}"))?;
    let root = doc.root().children().find(|n| n.has_tag_name("DirectB2SData")).ok_or("not a backglass file: no DirectB2SData")?;
    let mut pictures = Pictures(Vec::new());

    let name = node(root, "Name").and_then(|n| n.attribute("Value")).unwrap_or_default().to_string();
    let grill_height = node(root, "GrillHeight").map_or(0, |n| int_attribute(n, "Value", 0));
    let backglass = pictures.add(node(root, "Images/BackglassImage").and_then(|n| n.attribute("Value")));
    let backglass_on = pictures.add(node(root, "Images/BackglassOnImage").and_then(|n| n.attribute("Value")));
    let backglass_on_rom = node(root, "Images/BackglassOnImage").map_or((RomKind::None, 0), |n| (RomKind::from(int_attribute(n, "RomIDType", 0)), int_attribute(n, "RomID", 0)));
    let backglass_off = pictures.add(node(root, "Images/BackglassOffImage").and_then(|n| n.attribute("Value")));
    let dmd_image = pictures.add(node(root, "Images/DMDImage").and_then(|n| n.attribute("Value")));

    // lights and scores belong to the backglass or to the third screen's display
    let on_backglass = |n: &roxmltree::Node<'_, '_>| n.attribute("Parent") == Some("Backglass");
    let mut bulbs = Vec::new();
    let mut dmd_bulbs = Vec::new();
    if let Some(list) = node(root, "Illumination") {
        for n in list.children().filter(|n| n.has_tag_name("Bulb")) {
            let bulb = bulb(n, &mut pictures);
            if on_backglass(&n) { bulbs.push(bulb) } else { dmd_bulbs.push(bulb) }
        }
    }
    let mut scores = Vec::new();
    let mut dmd_scores = Vec::new();
    if let Some(list) = node(root, "Scores") {
        for n in list.children().filter(|n| n.has_tag_name("Score")) {
            if on_backglass(&n) { scores.push(score(n)) } else { dmd_scores.push(score(n)) }
        }
    }
    let mut reels = Vec::new();
    if let Some(list) = node(root, "Reels/Images") {
        for n in list.children().filter(|n| n.has_tag_name("Image")) {
            reels.push(Raw((n.attribute("Name").unwrap_or_default().to_string(), pictures.add(n.attribute("Image")))));
        }
    }
    let mut animations: Vec<Animation> = Vec::new();
    if let Some(list) = node(root, "Animations") {
        for n in list.children().filter(|n| n.has_tag_name("Animation")) {
            let a = animation(n);
            if !animations.iter().any(|b| b.name == a.name) {
                animations.push(a);
            }
        }
    }

    // every picture decoded side by side, then put in its place
    let mut decoded = decode_all(&pictures.0);
    let mut take = |i: Option<usize>| i.and_then(|i| decoded[i].take());
    let mut finish = |raw: Vec<RawBulb>| -> Vec<Bulb> {
        raw.into_iter()
            .map(|Raw((mut b, image, off))| {
                b.image = take(image);
                b.off_image = take(off);
                b
            })
            .collect()
    };
    let bulbs = finish(bulbs);
    let dmd_bulbs = finish(dmd_bulbs);
    let reels = reels.into_iter().map(|Raw((name, image))| ReelImage { name, image: take(image) }).collect();
    Ok(B2sFile {
        name,
        grill_height,
        backglass: take(backglass),
        backglass_on: take(backglass_on),
        backglass_on_rom,
        backglass_off: take(backglass_off),
        dmd_image: take(dmd_image),
        bulbs,
        dmd_bulbs,
        scores,
        dmd_scores,
        reels,
        animations,
    })
}

/// The element at a path of child names below `root`, the first of each
/// name (VPX's `GetNode`).
fn node<'a, 'i>(root: roxmltree::Node<'a, 'i>, path: &str) -> Option<roxmltree::Node<'a, 'i>> {
    let mut n = root;
    for name in path.split('/').filter(|s| !s.is_empty()) {
        n = n.children().find(|c| c.has_tag_name(name))?;
    }
    Some(n)
}

/// A whole-number attribute as tinyxml2 reads one: its leading digits, else
/// the default.
fn int_attribute(n: roxmltree::Node<'_, '_>, attribute: &str, default: i32) -> i32 {
    let Some(text) = n.attribute(attribute) else { return default };
    let text = text.trim_start();
    let end = text.char_indices().find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+')))).map_or(text.len(), |(i, _)| i);
    text[..end].parse().unwrap_or(default)
}

/// A colour attribute, "red.green.blue" from 0 to 255 (VPX's `GetColorAttribute`).
fn color_attribute(n: roxmltree::Node<'_, '_>, attribute: &str, default: [f32; 3]) -> [f32; 3] {
    let Some(text) = n.attribute(attribute) else { return default };
    let parts: Vec<i32> = text.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect();
    match parts[..] {
        [r, g, b] => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0],
        _ => default,
    }
}

fn bulb<'a>(n: roxmltree::Node<'a, '_>, pictures: &mut Pictures<'a>) -> RawBulb {
    let int = |a: &str, d: i32| int_attribute(n, a, d);
    let snippit = match int("SnippitType", 0) {
        1 => Snippit::SelfRotating,
        2 => Snippit::MechRotating,
        _ => Snippit::Standard,
    };
    let angle = int("SnippitRotatingAngle", 0);
    let bulb = Bulb {
        name: n.attribute("Name").unwrap_or_default().to_string(),
        b2s_id: int("B2SID", -1),
        b2s_value: int("B2SValue", 0),
        // a B2SID replaces the ROM binding: it's a lamp's number, not inverted (B2S Server's, as
        // VPX's plugin reads it since ecbbe7275)
        rom_id: if int("B2SID", -1) >= 0 { int("B2SID", -1) } else { int("RomID", -1) },
        rom_kind: if int("B2SID", -1) >= 0 { RomKind::Lamp } else { RomKind::from(int("RomIDType", 0)) },
        rom_inverted: int("B2SID", -1) < 0 && int("RomInverted", 0) != 0,
        initial_state: int("InitialState", 0) != 0,
        z_order: int("ZOrder", 0),
        light_color: color_attribute(n, "LightColor", [1.0; 3]),
        visible: int("Visible", 1) != 0,
        x: int("LocX", 0),
        y: int("LocY", 0),
        width: int("Width", 0),
        height: int("Height", 0),
        snippit,
        rotating_steps: if angle != 0 { 360 / angle } else { int("SnippitRotatingSteps", 0) },
        image: None,
        off_image: None,
    };
    let image = pictures.add(n.attribute("Image"));
    let off = pictures.add(n.attribute("OffImage"));
    Raw((bulb, image, off))
}

/// An animation (B2S's `Animation`): the settings of B2S Designer's own
/// time, and older files' flags where they're unset.
fn animation(n: roxmltree::Node<'_, '_>) -> Animation {
    let int = |a: &str, d: i32| int_attribute(n, a, d);
    let flag = |a: &str| int(a, 0) != 0;
    // lights by name, separated by commas (VPX's `GetStringList`)
    let names = |s: Option<&str>| -> Vec<String> { s.unwrap_or_default().split(',').filter(|t| !t.is_empty()).map(str::to_string).collect() };
    let steps = n
        .children()
        .filter(|c| c.has_tag_name("AnimationStep"))
        .map(|s| AnimationStep {
            on: names(s.attribute("On")),
            wait_on: int_attribute(s, "WaitLoopsAfterOn", 0),
            off: names(s.attribute("Off")),
            wait_off: int_attribute(s, "WaitLoopsAfterOff", 0),
        })
        .collect();
    Animation {
        name: n.attribute("Name").unwrap_or_default().to_string(),
        on_display: n.attribute("Parent") != Some("Backglass"),
        interval_ms: int("Interval", 0),
        loops: int("Loops", 0),
        join: join(n.attribute("IDJoin").unwrap_or_default()),
        at_startup: flag("StartAnimationAtBackglassStartup"),
        start_lights: match int("LightsStateAtAnimationStart", 0) {
            1 => StartLights::InvolvedOff,
            2 => StartLights::InvolvedOn,
            3 => StartLights::AllOff,
            4 => StartLights::NoChange,
            _ if flag("AllLightsOffAtAnimationStart") => StartLights::AllOff,
            _ => StartLights::NoChange,
        },
        end_lights: match int("LightsStateAtAnimationEnd", 0) {
            1 => EndLights::InvolvedOff,
            2 => EndLights::InvolvedOn,
            3 => EndLights::Reset,
            4 => EndLights::NoChange,
            _ if flag("ResetLightsAtAnimationEnd") => EndLights::Reset,
            _ => EndLights::InvolvedOff,
        },
        stop: match int("AnimationStopBehaviour", 0) {
            1 => StopBehaviour::Immediately,
            2 => StopBehaviour::TillEnd,
            3 => StopBehaviour::ToFirstStep,
            _ if flag("RunAnimationTilEnd") => StopBehaviour::TillEnd,
            _ => StopBehaviour::Immediately,
        },
        lock: flag("LockInvolvedLamps"),
        steps,
    }
}

/// What an animation is joined to (B2S's `IDJoin`): a number is a light the
/// script sets by B2S id; `L`, `S` or `GI` and a number, one of the
/// machine's lamps, coils or lighting strings, inverted with an `I` before.
fn join(text: &str) -> Option<Join> {
    let text = text.trim().to_ascii_uppercase();
    if text.is_empty() {
        return None;
    }
    if let Ok(id) = text.parse() {
        return Some(Join::Script { id });
    }
    let (inverted, rest) = match text.strip_prefix('I') {
        Some(rest) if rest.starts_with(['L', 'S', 'G']) => (true, rest),
        _ => (false, text.as_str()),
    };
    let (kind, number) = if let Some(n) = rest.strip_prefix("GI") {
        (RomKind::GiString, n)
    } else if let Some(n) = rest.strip_prefix('L') {
        (RomKind::Lamp, n)
    } else {
        (RomKind::Solenoid, rest.strip_prefix('S')?)
    };
    Some(Join::Machine { kind, number: number.trim().parse().ok()?, inverted })
}

fn score(n: roxmltree::Node<'_, '_>) -> Score {
    let int = |a: &str, d: i32| int_attribute(n, a, d);
    let reel_type = n.attribute("ReelType").unwrap_or_default().to_string();
    let starts = |prefix: &str| reel_type.len() >= prefix.len() && reel_type[..prefix.len()].eq_ignore_ascii_case(prefix);
    let kind = if starts("dream7") {
        ScoreKind::Dream7
    } else if starts("rendered") {
        ScoreKind::RenderedLed
    } else if starts("LED") {
        ScoreKind::Led
    } else if starts("ImportedLED") {
        ScoreKind::ImportedLed
    } else {
        ScoreKind::Reel
    };
    Score {
        start_digit: int("B2SStartDigit", 0),
        player: int("B2SPlayerNo", 0),
        digits: int("Digits", 0),
        spacing: int("Spacing", 0),
        x: int("LocX", 0),
        y: int("LocY", 0),
        width: int("Width", 0),
        height: int("Height", 0),
        kind,
        reel_type,
    }
}

/// Decodes the pictures, several at a time.
fn decode_all(texts: &[&str]) -> Vec<Option<Picture>> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(texts.len().max(1));
    let mut out: Vec<Option<Picture>> = Vec::new();
    out.resize_with(texts.len(), || None);
    let results = std::sync::Mutex::new(out);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(text) = texts.get(i) else { break };
                    let picture = decode(text);
                    results.lock().expect("pictures")[i] = picture;
                }
            });
        }
    });
    results.into_inner().expect("pictures")
}

/// One picture: base64 text (padding or none, line breaks or none) holding
/// a PNG, JPEG or other image file.
fn decode(text: &str) -> Option<Picture> {
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new().with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
    );
    let bytes = engine.decode(text).or_else(|_| engine.decode(text.split_ascii_whitespace().collect::<String>())).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    Some(Picture { width: image.width(), height: image.height(), rgba: image.into_raw() })
}

#[cfg(test)]
mod tests {
    use super::*;

    // a 2x1 PNG, a red dot and a half-clear green one
    const PNG: &str =
        "iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8YQUAAAAJcEhZcwAADsMAAA7DAcdvqGQAAAARSURBVBhXY/jPwPCf4T9DAwAQeQN+en2aMAAAAABJRU5ErkJggg==";

    #[test]
    fn a_backglass_file_reads_as_vpx_reads_it() {
        let xml = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<DirectB2SData Version="1.2">
  <Name Value="Test" />
  <GrillHeight Value="12" Small="0" />
  <Illumination>
    <Bulb ID="1" Parent="Backglass" Name="Lamp 5" RomID="5" RomIDType="1" LightColor="255.128.0" LocX="10" LocY="20" Width="30" Height="40" Image="{PNG}" />
    <Bulb ID="2" Parent="Backglass" Name="Credits" B2SID="31" B2SValue="2" Visible="0" InitialState="1" />
    <Bulb ID="3" Parent="DMD" Name="Speaker" SnippitType="2" SnippitRotatingAngle="10" RomID="1" RomIDType="4" />
  </Illumination>
  <Scores ReelCountOfIntermediates="0">
    <Score ID="1" Parent="Backglass" ReelType="EMR_T1_0" B2SPlayerNo="1" Digits="6" Spacing="4" LocX="1" LocY="2" Width="60" Height="20" />
    <Score ID="2" Parent="Backglass" ReelType="Dream7LED8" B2SStartDigit="7" Digits="7" />
  </Scores>
  <Reels>
    <Images>
      <Image Name="EMR_T1_3" CountOfIntermediates="0" Image="{PNG}" />
      <Image Name="EMR_T1_Empty" CountOfIntermediates="0" Image="{PNG}" />
    </Images>
  </Reels>
  <Images>
    <BackglassImage Value="{PNG}" FileName="bg.png" />
    <BackglassOnImage Value="{PNG}" RomID="12" RomIDType="2" />
  </Images>
</DirectB2SData>"#
        );
        let b = parse(&xml).expect("parses");
        assert_eq!(b.name, "Test");
        assert_eq!(b.grill_height, 12);
        let bg = b.backglass.as_ref().expect("backglass");
        assert_eq!((bg.width, bg.height), (2, 1));
        assert_eq!(bg.rgba, vec![255, 0, 0, 255, 0, 255, 0, 128]);
        assert_eq!(b.backglass_on_rom, (RomKind::Solenoid, 12));
        assert_eq!(b.bulbs.len(), 2);
        assert_eq!(b.dmd_bulbs.len(), 1);
        let lamp = &b.bulbs[0];
        assert_eq!((lamp.rom_kind, lamp.rom_id, lamp.b2s_id), (RomKind::Lamp, 5, -1));
        assert_eq!(lamp.light_color, [1.0, 128.0 / 255.0, 0.0]);
        assert_eq!((lamp.x, lamp.y, lamp.width, lamp.height), (10, 20, 30, 40));
        assert!(lamp.image.is_some() && lamp.off_image.is_none());
        let credits = &b.bulbs[1];
        assert_eq!((credits.b2s_id, credits.b2s_value, credits.visible, credits.initial_state), (31, 2, false, true));
        // a turning picture's steps from its angle
        assert_eq!((b.dmd_bulbs[0].snippit, b.dmd_bulbs[0].rotating_steps), (Snippit::MechRotating, 36));
        assert_eq!(b.scores.len(), 2);
        assert_eq!((b.scores[0].kind, b.scores[0].player, b.scores[0].digits), (ScoreKind::Reel, 1, 6));
        assert_eq!((b.scores[1].kind, b.scores[1].start_digit), (ScoreKind::Dream7, 7));
        // reel pictures by digit
        assert!(b.reel_image("EMR_T1_0", 3).is_some());
        assert!(b.reel_image("EMR_T1_0", -1).is_some());
        assert!(b.reel_image("EMR_T1_0", 4).is_none());
        assert!(b.reel_image("EMR_T1", 3).is_none());
    }

    #[test]
    fn segment_scores_light_as_the_machines_digits() {
        let xml = format!(
            r#"<DirectB2SData Version="1.2">
  <Scores>
    <Score ID="1" Parent="Backglass" ReelType="Dream7LED8" B2SStartDigit="7" Digits="2" LocX="0" LocY="0" Width="2" Height="1" />
    <Score ID="2" Parent="Backglass" ReelType="Dream7LED8" B2SPlayerNo="1" Digits="1" />
  </Scores>
  <Images>
    <BackglassImage Value="{PNG}" />
  </Images>
</DirectB2SData>"#
        );
        let file = std::sync::Arc::new(parse(&xml).expect("parses"));
        // two seven-digit scores of System 7, the first's last digit showing a 5
        let score = |left| pinmame_link::DisplayLayout { kind: 7, left, length: 7, ..pinmame_link::DisplayLayout::default() };
        let layout = std::sync::Arc::new(crate::segments::layout(&[(0, score(0)), (1, score(16))], 0x1_0000).expect("segment displays"));
        let mut levels = vec![[0.0; 16]; 14];
        levels[6] = crate::segments::levels(0x6D, false);
        let segments = crate::segments::SegmentsNow { layout, levels, id: 1 };
        let mut show = Show::new(file);
        show.play(0.0, None, &Machine::default());
        let now = show.now(None, Some(&segments), false);
        use crate::segments::{SegHint, SegKind, SegStyle};
        // digits 7 and 8: the first score's last, then the second's first, before its millions comma
        assert_eq!(
            now.pages[0].dream7[0].1,
            [(SegKind::Seven, SegStyle::Plasma, SegHint::Generic, crate::segments::levels(0x6D, false)), (SegKind::SevenComma, SegStyle::Plasma, SegHint::Generic, [0.0; 16])]
        );
        // a player's segment score isn't numbered: an unlit fourteen-segment digit, as VPX draws it
        assert_eq!(now.pages[0].dream7[1].1, [(SegKind::Fourteen, SegStyle::Plasma, SegHint::Generic, [0.0; 16])]);
    }

    /// A machine for tests: outputs by kind and number; none at all until
    /// it runs.
    #[derive(Default)]
    struct Machine {
        running: bool,
        outputs: std::collections::HashMap<(i32, i32), f32>,
    }

    impl Machine {
        fn set(&mut self, kind: RomKind, number: i32, value: f32) {
            self.running = true;
            self.outputs.insert((kind as i32, number), value);
        }
    }

    impl Outputs for Machine {
        fn get(&self, kind: RomKind, number: i32) -> Option<f32> {
            if !self.running || kind == RomKind::None {
                return None;
            }
            Some(self.outputs.get(&(kind as i32, number)).copied().unwrap_or(0.0))
        }
    }

    /// A backglass with lights A (two of them), B (lit from the start), a
    /// lamp's light C, a light A on the score panel, and the animations given.
    fn show_with(animations: &str) -> Show {
        let xml = format!(
            r#"<DirectB2SData Version="1.2">
  <Illumination>
    <Bulb ID="1" Parent="Backglass" Name="A" />
    <Bulb ID="2" Parent="Backglass" Name="A" />
    <Bulb ID="3" Parent="Backglass" Name="B" InitialState="1" />
    <Bulb ID="4" Parent="Backglass" Name="C" RomID="9" RomIDType="1" />
    <Bulb ID="5" Parent="DMD" Name="A" />
  </Illumination>
  <Animations>{animations}</Animations>
</DirectB2SData>"#
        );
        Show::new(std::sync::Arc::new(parse(&xml).expect("parses")))
    }

    fn lit(show: &Show) -> Vec<f32> {
        show.pages[0].level.clone()
    }

    #[test]
    fn a_b2sid_light_shares_its_lamp_with_the_rom() {
        // VPX's plugin since ecbbe7275 and 1ea259b74, as B2S Server: a B2SID is a lamp's number,
        // the script's writes and the ROM's changes sharing it, the latest winning
        let file = std::sync::Arc::new(
            parse(
                r#"<DirectB2SData Version="1.2">
  <Illumination>
    <Bulb ID="1" Parent="Backglass" Name="layer" B2SID="6" InitialState="1" />
    <Bulb ID="2" Parent="Backglass" Name="hoss" B2SID="190" B2SValue="2" />
    <Bulb ID="3" Parent="Backglass" Name="rom" RomID="10" RomIDType="1" RomInverted="1" />
    <Bulb ID="4" Parent="Backglass" Name="both" B2SID="8" RomID="10" RomIDType="1" RomInverted="1" />
  </Illumination>
</DirectB2SData>"#,
            )
            .expect("parses"),
        );
        let bulbs = file.bulbs_of(Page::Backglass);
        let binding = |i: usize| (bulbs[i].rom_kind, bulbs[i].rom_id, bulbs[i].rom_inverted);
        // the B2SID replaces the ROM binding, RomInverted ignored
        assert_eq!((binding(0), binding(3)), ((RomKind::Lamp, 6, false), (RomKind::Lamp, 8, false)));
        assert_eq!(binding(2), (RomKind::Lamp, 10, true));
        let mut show = Show::new(file);
        let (mut machine, mut server) = (Machine::default(), ServerState::default());
        machine.set(RomKind::Lamp, 6, 0.0);
        // the ROM's first reading isn't a change: each light keeps its first state
        show.play(0.0, Some(&mut server), &machine);
        assert_eq!(lit(&show), [1.0, 0.0, 0.0, 0.0]);
        // a lamp change takes the light, then a script write, then the ROM again: the latest wins
        machine.set(RomKind::Lamp, 6, 1.0);
        show.play(1.0, Some(&mut server), &machine);
        machine.set(RomKind::Lamp, 6, 0.0);
        show.play(2.0, Some(&mut server), &machine);
        assert_eq!(lit(&show)[0], 0.0);
        server.set_data(6, 1);
        show.play(3.0, Some(&mut server), &machine);
        assert_eq!(lit(&show)[0], 1.0);
        machine.set(RomKind::Lamp, 6, 1.0);
        show.play(4.0, Some(&mut server), &machine);
        machine.set(RomKind::Lamp, 6, 0.0);
        show.play(5.0, Some(&mut server), &machine);
        assert_eq!(lit(&show)[0], 0.0);
        // B2SValue: lit only by its value, from the script or the ROM
        server.set_data(190, 2);
        show.play(6.0, Some(&mut server), &machine);
        assert_eq!(lit(&show)[1], 1.0);
        server.set_data(190, 1);
        show.play(7.0, Some(&mut server), &machine);
        assert_eq!(lit(&show)[1], 0.0);
        // a RomID light, inverted: on as its lamp goes off
        machine.set(RomKind::Lamp, 10, 1.0);
        show.play(8.0, Some(&mut server), &machine);
        machine.set(RomKind::Lamp, 10, 0.0);
        show.play(9.0, Some(&mut server), &machine);
        assert_eq!(lit(&show)[2], 1.0);
    }

    #[test]
    fn animations_read_as_b2s_designer_writes_them() {
        assert_eq!(join("S23"), Some(Join::Machine { kind: RomKind::Solenoid, number: 23, inverted: false }));
        assert_eq!(join("IL1"), Some(Join::Machine { kind: RomKind::Lamp, number: 1, inverted: true }));
        assert_eq!(join("GI3"), Some(Join::Machine { kind: RomKind::GiString, number: 3, inverted: false }));
        assert_eq!(join("igi2"), Some(Join::Machine { kind: RomKind::GiString, number: 2, inverted: true }));
        assert_eq!(join("12"), Some(Join::Script { id: 12 }));
        assert_eq!((join(""), join("X5"), join("I5")), (None, None, None));
        let show = show_with(
            r#"<Animation Name="Twice" Parent="Backglass" Interval="10" Loops="2" LightsStateAtAnimationStart="0" AllLightsOffAtAnimationStart="1" LightsStateAtAnimationEnd="0" ResetLightsAtAnimationEnd="1">
                 <AnimationStep Step="1" On="A,B" WaitLoopsAfterOn="2" Off="A,,Nobody" WaitLoopsAfterOff="1" />
               </Animation>
               <Animation Name="Twice" Parent="DMD" />
               <Animation Name="Panel" Parent="DMD" AnimationStopBehaviour="3" />"#,
        );
        let f = &show.file;
        assert_eq!(f.animations.len(), 2, "the first of each name");
        let a = &f.animations[0];
        assert_eq!((a.interval_ms, a.loops, a.on_display, a.start_lights, a.end_lights, a.stop), (10, 2, false, StartLights::AllOff, EndLights::Reset, StopBehaviour::Immediately));
        assert_eq!((a.steps[0].on.clone(), a.steps[0].off.clone()), (vec!["A".to_string(), "B".to_string()], vec!["A".to_string(), "Nobody".to_string()]));
        // the lights by name: both A's, and B; a name with no light is only a pause
        assert_eq!(show.casts[0].steps[0], (vec![0, 1, 2], vec![0, 1]));
        assert_eq!((f.animations[1].on_display, f.animations[1].stop, show.casts[1].page), (true, StopBehaviour::ToFirstStep, Page::Panel));
    }

    #[test]
    fn an_animation_steps_through_its_lights_and_loops() {
        let mut show = show_with(
            r#"<Animation Name="Run" Parent="Backglass" Interval="10" Loops="2" StartAnimationAtBackglassStartup="1" LightsStateAtAnimationStart="1" LightsStateAtAnimationEnd="2">
                 <AnimationStep On="A" WaitLoopsAfterOn="2" Off="A" WaitLoopsAfterOff="1" />
                 <AnimationStep On="B" WaitLoopsAfterOn="1" Off="B" WaitLoopsAfterOff="0" />
               </Animation>"#,
        );
        let machine = Machine::default();
        let mut server = ServerState::default();
        // B starts lit, and goes off as the animation starts (its lights off): A on at once
        show.play(0.0, Some(&mut server), &machine);
        assert_eq!(lit(&show), [1.0, 1.0, 0.0, 0.0]);
        assert!(server.animation_running("run"));
        show.play(19.0, Some(&mut server), &machine);
        assert_eq!(lit(&show), [1.0, 1.0, 0.0, 0.0]);
        show.play(20.0, Some(&mut server), &machine);
        assert_eq!(lit(&show), [0.0, 0.0, 0.0, 0.0]);
        show.play(30.0, Some(&mut server), &machine);
        assert_eq!(lit(&show), [0.0, 0.0, 1.0, 0.0]);
        // B off and, with no wait, the second pass's A on
        show.play(40.0, Some(&mut server), &machine);
        assert_eq!(lit(&show), [1.0, 1.0, 0.0, 0.0]);
        // after two passes it ends, its lights on
        show.play(79.0, Some(&mut server), &machine);
        assert!(server.animation_running("Run"));
        show.play(80.0, Some(&mut server), &machine);
        assert_eq!(lit(&show), [1.0, 1.0, 1.0, 0.0]);
        assert!(!server.animation_running("Run"));
    }

    #[test]
    fn a_joined_animation_plays_while_its_coil_is_on() {
        let mut show = show_with(
            r#"<Animation Name="Kick" Parent="Backglass" Interval="10" Loops="0" IDJoin="S5" AnimationStopBehaviour="1" LightsStateAtAnimationEnd="1" LockInvolvedLamps="1">
                 <AnimationStep On="C" WaitLoopsAfterOn="1" Off="C" WaitLoopsAfterOff="1" />
               </Animation>
               <Animation Name="Attract" Parent="Backglass" Interval="10" Loops="0" IDJoin="IL1" AnimationStopBehaviour="3" LightsStateAtAnimationEnd="4">
                 <AnimationStep On="A" WaitLoopsAfterOn="1" Off="A" WaitLoopsAfterOff="1" />
               </Animation>"#,
        );
        let mut machine = Machine::default();
        // no machine yet: nothing plays
        show.play(0.0, None, &machine);
        assert!(!show.runs[0].playing && !show.runs[1].playing);
        // the machine runs, lamp 1 off: the inverted join starts its animation
        machine.set(RomKind::Lamp, 9, 0.0);
        show.play(10.0, None, &machine);
        assert!(show.runs[1].playing);
        // coil 5 fires: Kick plays, its light locked against lamp 9
        machine.set(RomKind::Solenoid, 5, 1.0);
        show.play(20.0, None, &machine);
        assert_eq!(lit(&show)[3], 1.0);
        machine.set(RomKind::Lamp, 9, 0.5);
        show.play(25.0, None, &machine);
        assert_eq!(lit(&show)[3], 1.0, "locked");
        // the coil releases: Kick stops at once, its light off
        machine.set(RomKind::Solenoid, 5, 0.0);
        show.play(26.0, None, &machine);
        assert!(!show.runs[0].playing);
        assert_eq!(lit(&show)[3], 0.0);
        // unlocked, the light follows its lamp again as it changes
        machine.set(RomKind::Lamp, 9, 1.0);
        show.play(27.0, None, &machine);
        assert_eq!(lit(&show)[3], 1.0);
        // lamp 1 on: Attract runs back to its first step, then stops
        machine.set(RomKind::Lamp, 1, 1.0);
        show.play(30.0, None, &machine);
        assert!(show.runs[1].playing && show.runs[1].stopping);
        show.play(60.0, None, &machine);
        assert!(!show.runs[1].playing);
    }

    #[test]
    fn the_script_orders_lights_and_animations() {
        let mut show = show_with(
            r#"<Animation Name="Blink" Parent="Backglass" Interval="10" Loops="0" AnimationStopBehaviour="2" LightsStateAtAnimationEnd="3">
                 <AnimationStep On="A" WaitLoopsAfterOn="1" Off="A" WaitLoopsAfterOff="1" />
               </Animation>"#,
        );
        let machine = Machine::default();
        let mut server = ServerState::default();
        show.play(0.0, Some(&mut server), &machine);
        assert!(!server.animation_running("Blink"));
        server.start_animation("blink", false);
        assert!(server.animation_running("Blink"));
        show.play(1.0, Some(&mut server), &machine);
        assert_eq!(lit(&show)[0], 1.0);
        // endless and running till its end: stopped, it finishes its pass, and its lights reset
        server.stop_animation("Blink");
        show.play(2.0, Some(&mut server), &machine);
        assert!(server.animation_running("Blink"));
        show.play(21.0, Some(&mut server), &machine);
        assert!(!server.animation_running("Blink"));
        assert_eq!(lit(&show), [0.0, 0.0, 1.0, 0.0]);
        // a name the file doesn't have never plays
        server.start_animation("Nothing", false);
        show.play(30.0, Some(&mut server), &machine);
        assert!(!server.animation_running("Nothing"));
    }

    /// The test set's backglasses, when they're on this machine:
    /// `cargo test --release -p player every_test_backglass -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn every_test_backglass_reads() {
        let folder = Path::new(r"C:\vPinball\VisualPinball\Tables");
        let Ok(entries) = std::fs::read_dir(folder) else { return };
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("directb2s"))) {
            let t0 = std::time::Instant::now();
            let b = read(&path).expect("reads");
            let size = |p: &Option<Picture>| p.as_ref().map_or("-".to_string(), |p| format!("{}x{}", p.width, p.height));
            let pictures = b.bulbs.iter().chain(&b.dmd_bulbs).filter(|l| l.image.is_some()).count();
            println!(
                "{}: {:.0} ms, backglass {}, on {}, off {}, grill {}, dmd {}, {} bulbs ({} with pictures) + {} on the display, {} scores, {} reel pictures",
                path.file_stem().unwrap_or_default().to_string_lossy(),
                t0.elapsed().as_secs_f64() * 1e3,
                size(&b.backglass),
                size(&b.backglass_on),
                size(&b.backglass_off),
                b.grill_height,
                size(&b.dmd_image),
                b.bulbs.len(),
                pictures,
                b.dmd_bulbs.len(),
                b.scores.len(),
                b.reels.len()
            );
        }
    }
}
