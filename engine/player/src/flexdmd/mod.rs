// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/FlexDMD.cs (its settings, Run and Show,
// the render loop's step and its conversion to grey or colour, the pictures scripts read back). Copyright 2019
// Vincent Bousquet, licensed under the Apache License, Version 2.0 (LICENSE-FlexDMD.txt beside this file). Changed
// in the porting, from C# to Rust, for Visual Pinball Infinite: run on the game thread, stepped by the game's clock
// at FlexDMD's 60 steps a second, its frames handed to our displays as VPX 5436's FlexDMD plugin offers them
// (plugins/flexdmd/FlexDMDPlugin.cpp, license:GPLv3+); the port is GPL-3.0-or-later.

//! Our FlexDMD: the score display an original table's
//! own script draws — and, through its stand-in, UltraDMD's (`ultradmd`).
//!
//! FlexDMD draws 60 times a second while it runs (in a grey or colour
//! mode): it runs what UltraDMD posted, updates the stage by the time
//! passed, draws it, and makes the picture its display shows and the one
//! scripts read back (`DmdPixels`, `DmdColoredPixels`). FlexDMD does that
//! on a thread of its own; ours runs on the game thread, stepped by the
//! game's clock, so test mode's pictures repeat exactly.

pub mod action;
pub mod actor;
pub mod animated;
pub mod assets;
pub mod bmfont;
pub mod colors;
pub mod font;
#[cfg(windows)]
pub mod gdiplus;
pub mod layout;
pub mod resources;
pub mod scenes;
pub mod surface;
pub mod tween;
pub mod ultradmd;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use actor::{Actor, ActorRef};
use assets::AssetManager;
use surface::Surface;

pub type FlexRef = Rc<RefCell<FlexDmd>>;

/// A command UltraDMD posts, run at the start of FlexDMD's next step.
pub type Posted = Box<dyn FnOnce(&FlexRef, &mut Surface)>;

/// FlexDMD's render modes, by its numbers.
pub mod mode {
    pub const GRAY_2: i32 = 0;
    pub const GRAY_4: i32 = 1;
    pub const RGB: i32 = 2;
    pub const SEG_2X16_ALPHA: i32 = 3;
    /// The last segment mode (`SEG_1x16Alpha_1x16Num_1x7Num`).
    pub const SEG_LAST: i32 = 16;

    pub fn is_dmd(m: i32) -> bool {
        matches!(m, GRAY_2 | GRAY_4 | RGB)
    }
}

/// The step: 60 a second, as FlexDMD's thread aims.
const STEP_MS: f64 = 1000.0 / 60.0;

/// A FlexDMD (FlexDMD's `FlexDMD` object).
pub struct FlexDmd {
    /// Its number among the table's FlexDMDs, by order of making.
    pub id: u32,
    pub runtime: Rc<Cell<i32>>,
    pub assets: Rc<RefCell<AssetManager>>,
    pub stage: ActorRef,
    run: bool,
    pub show: bool,
    pub width: u16,
    pub height: u16,
    pub game_name: String,
    /// The display's colour, as GDI+ keeps it: alpha, red, green, blue.
    pub color: [u8; 4],
    pub render_mode: i32,
    pub clear: bool,
    /// Whether its frames are being drawn: it runs, and ran in a grey or
    /// colour mode (FlexDMD starts its thread only then).
    drawing: bool,
    surface: Option<Surface>,
    pub posted: Vec<Posted>,
    /// When the next step is due, on the game's clock (ms).
    next_step_ms: Option<f64>,
    /// The call into the script that last asked whether UltraDMD is still drawing, and how many
    /// times it asked (`wait_step`).
    wait_call: u64,
    wait_asks: u32,
    /// Whether the script asked for the pictures it reads back: FlexDMD fills
    /// them only once asked.
    pixels_asked: bool,
    colored_asked: bool,
    /// The last frame's brightness a dot (0–255), as `DmdPixels` reads it.
    pub pixels: Vec<u8>,
    /// The last frame's colours a dot, red in the lowest byte, as `DmdColoredPixels` reads it.
    pub colored: Vec<u32>,
    /// The last frame as the display gets it, and its number.
    pub frame: Option<Arc<crate::dmd::DmdFrame>>,
    pub frames: u64,
    /// The segments the script last sent (`Segments`), for a segment mode.
    pub segments: Vec<u16>,
    pub lock_count: i32,
}

impl FlexDmd {
    /// FlexDMD's `new FlexDMD()`: 128×32, grey in 16 shades, UltraDMD's
    /// orange, runtime version 1008, its stage named `Stage`.
    pub fn new(id: u32, assets: AssetManager) -> FlexRef {
        let runtime = Rc::new(Cell::new(1008));
        let stage = Actor::new_group("Stage", &runtime);
        Rc::new(RefCell::new(FlexDmd {
            id,
            runtime,
            assets: Rc::new(RefCell::new(assets)),
            stage,
            run: false,
            show: true,
            width: 128,
            height: 32,
            game_name: String::new(),
            color: [0xFF, 0xFF, 0x58, 0x20],
            render_mode: mode::GRAY_4,
            clear: false,
            drawing: false,
            surface: None,
            posted: Vec::new(),
            next_step_ms: None,
            wait_call: u64::MAX,
            wait_asks: 0,
            pixels_asked: false,
            colored_asked: false,
            pixels: Vec::new(),
            colored: Vec::new(),
            frame: None,
            frames: 0,
            segments: Vec::new(),
            lock_count: 0,
        }))
    }

    pub fn run(&self) -> bool {
        self.run
    }

    /// FlexDMD's `Version`: its file version, major × 1000 + minor (1.9).
    pub const VERSION: i32 = 1009;

    /// FlexDMD's `DmdPixels`: asked once, then kept up each frame; zeros
    /// (empty, in FlexDMD) until the first frame after.
    pub fn ask_pixels(&mut self) -> Option<&[u8]> {
        let n = usize::from(self.width) * usize::from(self.height);
        let fresh = !self.pixels_asked || self.pixels.len() != n;
        if fresh {
            self.pixels_asked = true;
            self.pixels = vec![0; n];
            return None;
        }
        Some(&self.pixels)
    }

    /// FlexDMD's `DmdColoredPixels`, likewise.
    pub fn ask_colored(&mut self) -> Option<&[u32]> {
        let n = usize::from(self.width) * usize::from(self.height);
        let fresh = !self.colored_asked || self.colored.len() != n;
        if fresh {
            self.colored_asked = true;
            self.colored = vec![0; n];
            return None;
        }
        Some(&self.colored)
    }

    /// The frame its display shows now: while it shows and draws a grey or
    /// colour mode (VPX 5436's plugin offers its display only then).
    pub fn display_frame(&self) -> Option<Arc<crate::dmd::DmdFrame>> {
        (self.run && self.show && self.drawing && mode::is_dmd(self.render_mode)).then(|| self.frame.clone()).flatten()
    }

    /// The segments it shows: while it runs and shows in a segment mode.
    pub fn display_segments(&self) -> Option<(i32, &[u16])> {
        (self.run && self.show && (mode::SEG_2X16_ALPHA..=mode::SEG_LAST).contains(&self.render_mode)).then_some((self.render_mode, &self.segments[..]))
    }
}

/// FlexDMD's `Run` setter: running starts its drawing (in a grey or colour
/// mode), stopping takes its stage off and forgets its pictures.
pub fn set_run(flex: &FlexRef, run: bool) {
    let stage = {
        let mut f = flex.borrow_mut();
        if f.run == run {
            return;
        }
        f.run = run;
        if run {
            f.drawing = mode::is_dmd(f.render_mode);
            if !f.drawing {
                return;
            }
            f.surface = Some(Surface::new(u32::from(f.width), u32::from(f.height)));
            f.next_step_ms = None;
            let (w, h) = (f32::from(f.width), f32::from(f.height));
            {
                let mut st = f.stage.borrow_mut();
                (st.width, st.height) = (w, h);
            }
            Some((f.stage.clone(), true))
        } else {
            let was = f.drawing;
            f.drawing = false;
            f.surface = None;
            f.frame = None;
            was.then(|| (f.stage.clone(), false))
        }
    };
    if let Some((stage, on)) = stage {
        actor::set_on_stage(&stage, on);
    }
    if !run {
        flex.borrow().assets.borrow_mut().clear_all();
    }
}

/// Steps a FlexDMD to the game's time `now_ms`: one step as soon as it
/// starts (with no time passed, as FlexDMD's first), then one every 60th of
/// a second; behind by more than four, it skips ahead, as FlexDMD limits a
/// step to four frames' time.
pub fn advance(flex: &FlexRef, now_ms: f64) {
    if !flex.borrow().drawing {
        return;
    }
    let first = flex.borrow().next_step_ms.is_none();
    if first {
        flex.borrow_mut().next_step_ms = Some(now_ms + STEP_MS);
        step(flex, 0.0);
        return;
    }
    let mut steps = 0;
    while flex.borrow().next_step_ms.is_some_and(|t| t <= now_ms) && steps < 4 {
        step(flex, (STEP_MS / 1000.0) as f32);
        if let Some(t) = flex.borrow_mut().next_step_ms.as_mut() {
            *t += STEP_MS;
        }
        steps += 1;
        if !flex.borrow().drawing {
            return;
        }
    }
    let mut f = flex.borrow_mut();
    if f.next_step_ms.is_some_and(|t| t <= now_ms) {
        f.next_step_ms = Some(now_ms + STEP_MS);
    }
}

/// A script waiting for FlexDMD in a loop, within one call into the script (UltraDMD's
/// `IsRendering` asked again and again: `While UltraDMD.IsRendering : Wend`). FlexDMD draws on a
/// thread of its own, so in VPX its scenes go on while the script — and the table with it —
/// waits; ours steps with the game, between the script's calls, so such a loop would never end.
/// From the eighth ask in one call, each ask steps FlexDMD a 60th of a second, after that long a
/// wait (none in test mode, whose pictures repeat), as its thread would have; the game's clock
/// stays where it is, as VPX's does while the script holds it. True when it stepped.
pub fn wait_step(flex: &FlexRef) -> bool {
    let call = crate::script::host::calls();
    let asks = {
        let mut f = flex.borrow_mut();
        if f.wait_call != call {
            f.wait_call = call;
            f.wait_asks = 0;
        }
        f.wait_asks += 1;
        f.wait_asks
    };
    if asks < 8 || !flex.borrow().drawing {
        return false;
    }
    if !crate::pup::playback::exact() {
        std::thread::sleep(std::time::Duration::from_secs_f64(STEP_MS / 1000.0));
    }
    step(flex, (STEP_MS / 1000.0) as f32);
    true
}

/// One step of FlexDMD's render loop.
pub fn step(flex: &FlexRef, dt: f32) {
    let (mut surface, posted, stage, clear) = {
        let mut f = flex.borrow_mut();
        let Some(s) = f.surface.take() else { return };
        (s, std::mem::take(&mut f.posted), f.stage.clone(), f.clear)
    };
    if clear {
        surface.clear([0, 0, 0]);
    }
    for p in posted {
        p(flex, &mut surface);
    }
    actor::update(&stage, dt);
    actor::draw(&stage, &mut surface);
    let mut f = flex.borrow_mut();
    convert(&mut f, &surface);
    if f.drawing {
        f.surface = Some(surface);
    }
}

/// FlexDMD's conversion of its frame: each dot's brightness
/// (0.2126 red + 0.7152 green + 0.0722 blue, in single precision, truncated)
/// for `DmdPixels` and the grey modes — whose display gets 4 or 16 shades of
/// it — and its colours for `DmdColoredPixels` and the colour mode.
fn convert(f: &mut FlexDmd, s: &Surface) {
    let n = (s.width * s.height) as usize;
    let lum: Vec<u8> = s.px.iter().map(|p| ((0.2126f32 * f32::from(p[0]) + 0.7152f32 * f32::from(p[1]) + 0.0722f32 * f32::from(p[2])) as i32).min(255) as u8).collect();
    if f.pixels_asked && f.pixels.len() == n {
        f.pixels.copy_from_slice(&lum);
    }
    if f.colored_asked && f.colored.len() == n {
        for (c, p) in f.colored.iter_mut().zip(&s.px) {
            *c = (u32::from(p[2]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[0]);
        }
    }
    f.frames += 1;
    let id = f.frames;
    let frame = match f.render_mode {
        mode::GRAY_2 | mode::GRAY_4 => {
            let (shift, levels) = if f.render_mode == mode::GRAY_2 { (6, 3.0) } else { (4, 15.0) };
            let dots = lum.iter().map(|&v| {
                let l = f32::from(v >> shift) / levels;
                [l, l, l, 1.0]
            });
            Some(crate::dmd::DmdFrame::with_dots(s.width, s.height, dots, true, id))
        }
        mode::RGB => {
            let values: Vec<u32> = s.px.iter().map(|p| (u32::from(p[2]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[0])).collect();
            Some(crate::dmd::DmdFrame::from_srgb(s.width, s.height, &values, id))
        }
        _ => None,
    };
    f.frame = frame.map(|mut fr| {
        fr.tint = Some(f.color);
        Arc::new(fr)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flex() -> FlexRef {
        FlexDmd::new(0, AssetManager::new(None, None, None))
    }

    #[test]
    fn it_steps_sixty_times_a_second_of_game_time() {
        let f = flex();
        f.borrow_mut().render_mode = mode::RGB;
        set_run(&f, true);
        let frame = Actor::new_frame("F");
        frame.borrow_mut().set_bounds(0.0, 0.0, 8.0, 8.0);
        let stage = f.borrow().stage.clone();
        actor::add_actor(&stage, &frame, None);
        advance(&f, 1000.0);
        assert_eq!(f.borrow().frames, 1);
        advance(&f, 1016.0);
        assert_eq!(f.borrow().frames, 1);
        advance(&f, 1017.0);
        assert_eq!(f.borrow().frames, 2);
        advance(&f, 1100.0);
        assert_eq!(f.borrow().frames, 6);
        // far behind: four, then on from now
        advance(&f, 5000.0);
        assert_eq!(f.borrow().frames, 10);
        let fr = f.borrow().display_frame().unwrap();
        assert_eq!((fr.width, fr.height), (128, 32));
        // a white border, 2 wide
        assert_eq!(fr.dots[0], [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(fr.dots[2 * 128 + 2], [0.0, 0.0, 0.0, 1.0]);
        set_run(&f, false);
        assert!(f.borrow().display_frame().is_none());
        assert!(!frame.borrow().on_stage);
    }

    #[test]
    fn scripts_read_pictures_back_once_asked() {
        let f = flex();
        set_run(&f, true);
        let frame = Actor::new_frame("F");
        frame.borrow_mut().set_bounds(0.0, 0.0, 128.0, 32.0);
        let stage = f.borrow().stage.clone();
        actor::add_actor(&stage, &frame, None);
        assert!(f.borrow_mut().ask_pixels().is_none());
        advance(&f, 0.0);
        let px = f.borrow_mut().ask_pixels().map(<[u8]>::to_vec).unwrap();
        // white: 0.2126 + 0.7152 + 0.0722 of 255, truncated
        assert_eq!(px[0], 255);
        assert_eq!(px[3 * 128 + 3], 0);
        // grey in 16 shades for the display
        let fr = f.borrow().display_frame().unwrap();
        assert!(fr.luminance);
        assert_eq!(fr.dots[0][0], 1.0);
    }
}
