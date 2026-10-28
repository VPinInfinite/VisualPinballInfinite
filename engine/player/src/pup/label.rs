// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its PinUp Player
// plugin's labels (plugins/pup/PUPLabel.cpp, PUPLabel.h: SetCaption, SetVisible, SetSpecial, Render,
// UpdateImageTexture, UpdateLabelTexture, RotateSurface, ApplyFilter, Animation), "derived from notes at
// nailbuster.com" (PinUP Player's own documentation of its labels), with the newer fixes matching PinUP Player
// (to commit 401567c15: animations that put the label back as they found it, with PinUP's timing,
// tweens, zooms, pulses and wiggles, filter flashes and delayed shows, d858d33ef, 653dd4ff6 and 6a6bff191; sizes
// in points, drop shadows and outlines, underlines, 608f3da84 and ff10c248e; picture labels at their own size,
// captions from the pack's folder and in a colour of their own, 99e37745d; GIF frame times, b4295d34c;
// disposing, maximum lengths and blur, 303cf471d; FN 46's padding, 401567c15).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A label: text a table's script lays over a screen's media — a score, a
//! player's number, a mode's name — or a picture or animated GIF from one of
//! the pack's playlists, on a page of labels the script switches between, as
//! VPX's plugin keeps and draws it: its placement by alignment and percent,
//! its `Special` settings (size, colour, shadows and borders, gradients,
//! filters, rotation, zoom, opacity) and animations (flashing, sliding,
//! zooming, pulsing, fading, wiggling, counting, filter flashes, delayed
//! shows), each putting the label back as it found it when it ends. Its
//! picture is made on a thread of its own (`LabelWorker`), as VPX makes it,
//! and shows once made. Times follow the game's clock, where VPX's follow the
//! wall clock.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

use super::fonts::{FontLibrary, PupFont, TextStyle};
use super::playback::Picture;
use super::playlist::Playlist;

/// How a label is placed across its screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XAlign {
    Left,
    Center,
    Right,
}

/// How it's placed up and down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YAlign {
    Top,
    Center,
    Bottom,
}

impl XAlign {
    pub fn from_number(n: i32) -> XAlign {
        match n {
            1 => XAlign::Center,
            2 => XAlign::Right,
            _ => XAlign::Left,
        }
    }
}

impl YAlign {
    pub fn from_number(n: i32) -> YAlign {
        match n {
            1 => YAlign::Center,
            2 => YAlign::Bottom,
            _ => YAlign::Top,
        }
    }
}

/// What a label shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Image,
    Gif,
}

/// A colour as Windows keeps it: red in the low byte.
fn rgb(c: i32) -> [u8; 3] {
    [c as u8, (c >> 8) as u8, (c >> 16) as u8]
}

/// What a label's picture was made from, and how it's sized (VPX's `RenderState`).
#[derive(Clone, Default)]
pub struct RenderState {
    pub picture: Option<Arc<Picture>>,
    /// The window height and colour a text was made for.
    pub prerendered_height: i32,
    pub prerendered_color: i32,
    /// A picture's or an animation's own size, in pixels.
    pub source_size: [f32; 2],
    /// A text's picture's size, in pixels.
    pub width: f32,
    pub height: f32,
    /// A text box's height as Windows would make it, for alignment.
    pub logical_height: f32,
    /// What moves a text from SDL_ttf's baseline to Windows' (PinUP Player's) baseline.
    pub baseline_offset: f32,
    /// A turned picture grows to its bounding box: its size is scaled back.
    pub image_scale: [f32; 2],
    /// An animated GIF's frames, and when each ends (ms, from the first's start).
    pub frames: Vec<Arc<Picture>>,
    pub accumulated_delays: Vec<i32>,
}

/// A label's picture being made, filled by the worker.
type Pending = Arc<Mutex<Option<RenderState>>>;

/// What the worker needs to make a label's picture.
enum Job {
    Text { font: Arc<PupFont>, out_width: i32, out_height: i32, text: String, size: f32, color: i32, style: TextOptions },
    Image { path: PathBuf, filter: i32, angle: f32 },
    Gif { path: PathBuf },
}

/// A text's settings, as the label had them when its picture was asked for.
#[derive(Clone, Copy)]
struct TextOptions {
    italic: bool,
    underline: bool,
    x_align: XAlign,
    shadow_type: i32,
    shadow_color: i32,
    /// The drop shadow's or border's offset, in pixels.
    offset: [f32; 2],
    grad_state: i32,
    grad_color: i32,
    filter: i32,
    angle: f32,
    auto_fit: [f32; 2],
}

/// Makes labels' pictures on a thread of its own, one after another.
pub struct LabelWorker {
    jobs: Sender<(Job, Pending)>,
}

impl LabelWorker {
    pub fn start() -> Option<LabelWorker> {
        let (tx, rx) = channel::<(Job, Pending)>();
        std::thread::Builder::new()
            .name("pup labels".into())
            .spawn(move || {
                while let Ok((job, slot)) = rx.recv() {
                    let made = match job {
                        Job::Text { font, out_width, out_height, text, size, color, style } => text_picture(&font, out_width, out_height, &text, size, color, &style),
                        Job::Image { path, filter, angle } => image_picture(&path, filter, angle),
                        Job::Gif { path } => gif_frames(&path),
                    };
                    *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(made);
                }
            })
            .ok()?;
        Some(LabelWorker { jobs: tx })
    }

    fn submit(&self, job: Job) -> Pending {
        let slot: Pending = Arc::new(Mutex::new(None));
        let _ = self.jobs.send((job, slot.clone()));
        slot
    }
}

/// What a label draws this frame (VPX's `ClipDrawImage`): its picture over `dest`, clipped
/// to its screen's place, at `alpha`.
pub struct LabelDraw {
    pub picture: Arc<Picture>,
    /// Left, top, width, height, in the window's pixels.
    pub dest: [f32; 4],
    pub clip: [i32; 4],
    pub alpha: f32,
}

/// Robert Penner's easing of a move's progress (VPX's `ApplyTween`, after easings.net).
fn tween(pos: f32, kind: i32) -> f32 {
    match kind {
        1 => pos * pos,
        2 => pos * (2.0 - pos),
        3 => {
            if pos < 0.5 {
                2.0 * pos * pos
            } else {
                -1.0 + (4.0 - 2.0 * pos) * pos
            }
        }
        4 => pos * pos * pos,
        5 => {
            let t = pos - 1.0;
            t * t * t + 1.0
        }
        6 => {
            if pos < 0.5 {
                4.0 * pos * pos * pos
            } else {
                (pos - 1.0) * (2.0 * pos - 2.0) * (2.0 * pos - 2.0) + 1.0
            }
        }
        7 => pos * pos * pos * pos,
        8 => {
            let t = pos - 1.0;
            -(t * t * t * t) + 1.0
        }
        9 => {
            if pos < 0.5 {
                8.0 * pos * pos * pos * pos
            } else {
                -8.0 * (pos - 1.0) * (pos - 1.0) * (pos - 1.0) * (pos - 1.0) + 1.0
            }
        }
        10 => pos * pos * pos * pos * pos,
        _ => pos,
    }
}

/// A counted number as PinUP writes it (VPX's `FormatPupNumber`): format 1 with thousands
/// separators, padded to two characters and set between spaces, as PinUP's `%2.0n`.
fn format_pup_number(n: i32, format: i32) -> String {
    let mut s = n.to_string();
    if format == 1 {
        let first_digit = i32::from(n < 0);
        let mut at = s.len() as i32 - 3;
        while at > first_digit {
            s.insert(at as usize, ',');
            at -= 3;
        }
        if s.len() < 2 {
            s.insert(0, ' ');
        }
        s = format!(" {s} ");
    }
    s
}

/// A triangle wave's place in a cycle of `period` ms, 0 to 1 and back.
fn bounce(elapsed: f32, period: f32) -> f32 {
    let cycle = elapsed % period / period;
    if cycle < 0.5 { cycle * 2.0 } else { 2.0 - cycle * 2.0 }
}

/// A label's animation (VPX's `PUPLabel::Animation`), timed by the game's clock. It keeps the
/// label's state from when it began, and puts it back when it ends or is stopped.
#[derive(Clone, Debug)]
pub struct Animation {
    pub visible: bool,
    pub x_offset: f32,
    pub y_offset: f32,
    pub alpha: f32,
    pub zoom: f32,
    pub wiggle: f32,
    pub color: i32,
    /// Its kind (`at`).
    kind: i32,
    /// `xp` / `yp`: where the animated copy is, in pixels from the screen's corner.
    pixel: [Option<f32>; 2],
    /// `at` 4 bounces between its sizes, `at` 3 grows from one to the other, in percent of the
    /// display — or of the label, with `ns`.
    zoom_is_pulse: bool,
    zoom_use_draw_height: bool,
    /// `at` 9: the label's filter flashes instead of the label.
    invert_filter: bool,
    filter_mode: i32,
    saved_visible: bool,
    saved_alpha: f32,
    saved_zoom: f32,
    saved_color: i32,
    saved_filter_mode: i32,
    length_ms: u32,
    foreground_color: i32,
    start: u64,
    flashing_period: i32,
    xps: f32,
    xpe: f32,
    yps: f32,
    ype: f32,
    motion_len: i32,
    motion_tween: i32,
    motion_color: i32,
    alpha_fade: i32,
    alpha_start: i32,
    alpha_end: i32,
    pulse_speed: i32,
    screen_fade: bool,
    zoom_start: f32,
    zoom_end: f32,
    zoom_pulse_speed: i32,
    wiggle_start: f32,
    wiggle_end: f32,
    wiggle_speed: i32,
    delay_show: i32,
    num_start: i32,
    num_end: i32,
    num_format: i32,
    num_text: String,
}

impl Animation {
    /// An animation of `label` for `length_ms` in `foreground_color`, its state kept to put back
    /// (VPX's constructors and `CaptureState`).
    fn base(label: &Label, length_ms: i32, foreground_color: i32, now: u64) -> Animation {
        Animation {
            visible: true,
            x_offset: 0.0,
            y_offset: 0.0,
            alpha: 1.0,
            zoom: 100.0,
            wiggle: 0.0,
            color: foreground_color,
            kind: 0,
            pixel: [None; 2],
            zoom_is_pulse: false,
            zoom_use_draw_height: false,
            invert_filter: false,
            filter_mode: 0,
            saved_visible: label.visible,
            saved_alpha: label.alpha,
            saved_zoom: label.zoom,
            saved_color: label.color,
            saved_filter_mode: label.filter_mode,
            length_ms: length_ms.max(0) as u32,
            foreground_color,
            start: now,
            flashing_period: 0,
            xps: 0.0,
            xpe: 0.0,
            yps: 0.0,
            ype: 0.0,
            motion_len: 0,
            motion_tween: 0,
            motion_color: 0,
            alpha_fade: 0,
            alpha_start: 255,
            alpha_end: 255,
            pulse_speed: 0,
            screen_fade: false,
            zoom_start: 100.0,
            zoom_end: 100.0,
            zoom_pulse_speed: 0,
            wiggle_start: 0.0,
            wiggle_end: 0.0,
            wiggle_speed: 1,
            delay_show: 0,
            num_start: i32::MIN,
            num_end: i32::MIN,
            num_format: 0,
            num_text: String::new(),
        }
    }

    fn is_wiggle(&self) -> bool {
        self.wiggle_start != self.wiggle_end
    }

    /// The label as it was when the animation began (VPX's `Restore`).
    fn restore(&self, label: &mut Label) {
        label.visible = self.saved_visible;
        label.alpha = self.saved_alpha;
        label.zoom = self.saved_zoom;
        if label.color != self.saved_color {
            label.color = self.saved_color;
            label.dirty = true;
        }
        if self.invert_filter && label.filter_mode != self.saved_filter_mode {
            label.filter_mode = self.saved_filter_mode;
            label.dirty = true;
        }
    }

    /// Steps the animation on (VPX's `Animation::Update`) for a label drawn at `rect` on a
    /// screen at `screen`: true when it's over, the label put back. A counting animation writes
    /// the label's caption; a screen fade sets the overlay's opacity in `overlay` at its end.
    fn update(&mut self, label: &mut Label, now: u64, screen: [i32; 4], rect: [f32; 4], overlay: &mut Option<f32>) -> bool {
        let elapsed = now.saturating_sub(self.start);
        if elapsed >= u64::from(self.length_ms) {
            if self.num_start != i32::MIN {
                let text = format_pup_number(self.num_end, self.num_format);
                if text != self.num_text {
                    self.num_text = text.clone();
                    label.caption = text;
                    label.dirty = true;
                }
            }
            if self.screen_fade {
                *overlay = Some(self.alpha_end as f32 / 255.0);
            }
            self.restore(label);
            return true;
        }
        let elapsed_f = elapsed as f32;
        if self.flashing_period != 0 {
            let odd = (elapsed / self.flashing_period.max(1) as u64) & 1 != 0;
            if self.invert_filter {
                // pDMDLabelFlashFilter: the filter on every other period
                let mode = if odd { self.filter_mode } else { 0 };
                if label.filter_mode != mode {
                    label.filter_mode = mode;
                    label.dirty = true;
                }
            } else {
                // pDMDLabelFlash: hidden first, then shown every other period
                self.visible = odd;
            }
            self.color = self.foreground_color;
        }
        if self.delay_show > 0 {
            self.visible = elapsed as i32 >= self.delay_show;
        }
        if self.motion_len != 0 {
            let pos = tween((elapsed_f / self.motion_len as f32).clamp(0.0, 1.0), self.motion_tween);
            let [sx, sy, sw, sh] = screen.map(|v| v as f32);
            // -1 off the screen's left (top), 0 where the label is, 1 off its right (bottom), 2 at
            // the end the `xp`/`yp` place (already the label's), else a percent of the display
            let place = |value: f32, start: bool, base: f32, low: f32, high: f32, origin: f32, extent: f32| -> f32 {
                if value == 0.0 || (value == 2.0 && !start) {
                    base
                } else if value == 1.0 {
                    high
                } else if value == -1.0 {
                    low
                } else {
                    origin + extent * value / 100.0
                }
            };
            let (x_base, y_base) = (rect[0], rect[1]);
            let xs = place(self.xps, true, x_base, sx - rect[2], sx + sw, sx, sw);
            let xe = place(self.xpe, false, x_base, sx - rect[2], sx + sw, sx, sw);
            let ys = place(self.yps, true, y_base, sy - rect[3], sy + sh, sy, sh);
            let ye = place(self.ype, false, y_base, sy - rect[3], sy + sh, sy, sh);
            self.color = if pos < 1.0 { self.motion_color } else { self.foreground_color };
            self.x_offset = xs + (xe - xs) * pos - x_base;
            self.y_offset = ys + (ye - ys) * pos - y_base;
        }
        if self.alpha_start != self.alpha_end {
            let (a_start, a_end) = (self.alpha_start as f32 / 255.0, self.alpha_end as f32 / 255.0);
            if self.pulse_speed > 0 {
                // pDMDLabelFadePulse: back and forth, pspeed alpha steps every 30 ms
                let range = ((self.alpha_end - self.alpha_start) as f32).abs();
                let period = ((range / self.pulse_speed as f32) * (2.0 * 30.0)).max(1.0);
                self.alpha = a_start + bounce(elapsed_f, period) * (a_end - a_start);
            } else {
                let fade = if self.motion_len > 0 { self.motion_len as f32 } else { self.length_ms as f32 };
                let pos = (elapsed_f / fade.max(1.0)).clamp(0.0, 1.0);
                self.alpha = a_start + pos * (a_end - a_start);
            }
            self.color = self.foreground_color;
        }
        if self.zoom_start != self.zoom_end {
            if self.zoom_is_pulse {
                // pDMDLabelPulseText: back and forth, pspeed pixels every 30 ms (3% of the
                // label's height when it gives none)
                let range = (self.zoom_end - self.zoom_start).abs() * rect[3] / 100.0;
                let speed = if self.zoom_pulse_speed > 0 { self.zoom_pulse_speed as f32 } else { (rect[3] * 0.03).max(1.0) };
                let period = ((range / speed) * (2.0 * 30.0)).max(1.0);
                self.zoom = self.zoom_start + bounce(elapsed_f, period) * (self.zoom_end - self.zoom_start);
            } else {
                // pDMDZoomBig: heights in percent of the display (of the label with `ns`)
                let (mut s, mut e) = (self.zoom_start, self.zoom_end);
                if self.zoom_use_draw_height {
                    s /= 100.0;
                    e /= 100.0;
                } else if label.kind == Kind::Text {
                    let size = label.effective_size().max(1.0);
                    s /= size;
                    e /= size;
                } else {
                    let height = rect[3].max(1.0);
                    s = screen[3] as f32 * self.zoom_start / 100.0 / height;
                    e = screen[3] as f32 * self.zoom_end / 100.0 / height;
                }
                let pos = tween((elapsed_f / (self.motion_len as f32).max(1.0)).clamp(0.0, 1.0), self.motion_tween);
                self.zoom = 100.0 * (s + pos * (e - s));
            }
            self.color = self.foreground_color;
        }
        if self.is_wiggle() {
            // pDMDLabelWiggleText: back and forth, rspeed degrees every 30 ms
            let range = (self.wiggle_end - self.wiggle_start).abs();
            let period = ((range / self.wiggle_speed.max(1) as f32) * (2.0 * 30.0)).max(1.0);
            self.wiggle = self.wiggle_start + bounce(elapsed_f, period) * (self.wiggle_end - self.wiggle_start);
            self.color = self.foreground_color;
        }
        if self.num_start != i32::MIN {
            // pDMDLabelPulseNumber: counting from numstart to numend
            let pos = tween((elapsed_f / (self.motion_len as f32).max(1.0)).clamp(0.0, 1.0), self.motion_tween);
            let n = if pos >= 1.0 { self.num_end } else { self.num_start + (pos * (i64::from(self.num_end) - i64::from(self.num_start)) as f32).round() as i32 };
            let text = format_pup_number(n, self.num_format);
            if text != self.num_text {
                self.num_text = text.clone();
                label.caption = text;
                label.dirty = true;
            }
        }
        // pDMDLabelMoveHorzFade: the last `af` ms fade out, on the kinds PinUP animates a copy of
        if self.alpha_fade > 50 && (self.kind <= 4 || self.kind == 10) {
            let fade_start = self.length_ms as i32 - self.alpha_fade;
            if elapsed as i32 > fade_start {
                self.alpha = ((self.length_ms as i32 - elapsed as i32) as f32 / self.alpha_fade as f32).clamp(0.0, 1.0);
            }
        }
        false
    }
}

/// What a label's `Special` asks of its screen: to draw it behind the others, in front, or just
/// in front of the background label.
pub enum Reorder {
    Back,
    Front,
    Background,
}

pub struct Label {
    pub name: String,
    /// The font as the script names it, and the pack's font of that name, if found.
    pub font_name: String,
    font: Option<Arc<PupFont>>,
    /// Its height, in percent of the screen's: a Windows cell height, or a point size (`size`).
    pub size: f32,
    size_is_points: bool,
    /// Windows' colour number (red in the low byte).
    pub color: i32,
    /// Turned, in degrees.
    pub angle: f32,
    pub x_align: XAlign,
    pub y_align: YAlign,
    /// Its place, in percent of the screen (0: placed by its alignment, unless `v2`).
    pub x_pos: f32,
    pub y_pos: f32,
    /// Shown when its page comes up.
    pub on_show_visible: bool,
    pub visible: bool,
    pub caption: String,
    /// The page it belongs to; one below 0 shows on every page.
    pub page: i32,
    shadow_color: i32,
    shadow_type: i32,
    /// The shadow's offset: pixels (PinUP's 3 by 3 at first), or percent of the font's height.
    x_offset: f32,
    y_offset: f32,
    x_offset_is_percent: bool,
    y_offset_is_percent: bool,
    outline: i32,
    bold: bool,
    italic: bool,
    underline: bool,
    alpha: f32,
    use_absolute_pos: bool,
    /// Shown as made when its page comes up (`dodefault`).
    pub do_default: bool,
    /// Removed once its `LabelSet` is done (`dispose`).
    pub do_dispose: bool,
    /// Its picture dropped when its animated picture ends (`anidispose`), and then dropped.
    dispose_on_anim_end: bool,
    dispose_pending: bool,
    zoom: f32,
    grad_color: i32,
    grad_state: i32,
    filter_mode: i32,
    auto_fit_width: f32,
    auto_fit_height: f32,
    /// An animated picture's frame time, in ms (0: its own frames' delays).
    anigif: i32,
    gif_loop: bool,
    gif_start: i32,
    gif_end: i32,
    hide_on_anim_end: bool,
    /// An animated picture playing (VPX's `m_animating`).
    gif_playing: bool,
    pub kind: Kind,
    /// The picture it shows, for a picture caption.
    pub path: Option<PathBuf>,
    /// A picture's size, in percent of the screen, once set; its own size before.
    image_width: f32,
    image_height: f32,
    image_width_set: bool,
    image_height_set: bool,
    /// The angle its picture was made at (its own, or a wiggle's).
    render_angle: f32,
    dirty: bool,
    animation: Option<Animation>,
    render: RenderState,
    pending: Option<Pending>,
    animation_frame: i32,
    animation_start: u64,
    /// The animated picture's frame shown, filtered where asked: (frame, filter, picture).
    frame_cache: Option<(i32, i32, Arc<Picture>)>,
}

impl Label {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        font_name: &str,
        fonts: &FontLibrary,
        size: f32,
        color: i32,
        angle: f32,
        x_align: XAlign,
        y_align: YAlign,
        x_pos: f32,
        y_pos: f32,
        page: i32,
        visible: bool,
    ) -> Label {
        let font = if font_name.is_empty() { None } else { fonts.get(font_name) };
        if !font_name.is_empty() && font.is_none() {
            fonts.missing(name, font_name);
        }
        Label {
            name: name.to_string(),
            font_name: font_name.to_string(),
            font,
            size,
            size_is_points: false,
            color,
            angle,
            x_align,
            y_align,
            x_pos,
            y_pos,
            on_show_visible: visible,
            visible,
            caption: String::new(),
            page,
            shadow_color: 0x80_80_80,
            shadow_type: 0,
            x_offset: 3.0,
            y_offset: 3.0,
            x_offset_is_percent: false,
            y_offset_is_percent: false,
            outline: 0,
            bold: false,
            italic: false,
            underline: false,
            alpha: 1.0,
            use_absolute_pos: false,
            do_default: true,
            do_dispose: false,
            dispose_on_anim_end: false,
            dispose_pending: false,
            zoom: 100.0,
            grad_color: 0,
            grad_state: 0,
            filter_mode: 0,
            auto_fit_width: 0.0,
            auto_fit_height: 0.0,
            anigif: 0,
            gif_loop: true,
            gif_start: -1,
            gif_end: -1,
            hide_on_anim_end: false,
            gif_playing: false,
            kind: Kind::Text,
            path: None,
            image_width: 0.0,
            image_height: 0.0,
            image_width_set: false,
            image_height_set: false,
            render_angle: 0.0,
            dirty: true,
            animation: None,
            render: RenderState::default(),
            pending: None,
            animation_frame: 0,
            animation_start: 0,
            frame_cache: None,
        }
    }

    /// An animation is running (VPX's `IsAnimating`): the script's visibility waits.
    pub fn is_animating(&self) -> bool {
        self.animation.is_some()
    }

    /// Ends its animation, the label put back as the animation found it (VPX's `ClearAnimation`).
    pub fn clear_animation(&mut self) {
        if let Some(a) = self.animation.take() {
            a.restore(self);
        }
    }

    /// Its height for drawing (VPX's `EffectiveSize`): a `size` is a point size at 96 dots an
    /// inch, made a cell height through the font's own ratio; `LabelNew`'s and `fonth`'s are cell
    /// heights already.
    fn effective_size(&self) -> f32 {
        match &self.font {
            Some(font) if self.size_is_points => self.size * (96.0 / 72.0) / font.win_scale(),
            _ => self.size,
        }
    }

    /// The shadow's offset in pixels, for a font `font_height` pixels tall (VPX's `GetShadowOffsetPixels`).
    fn shadow_offset_pixels(&self, font_height: f32) -> [f32; 2] {
        [
            if self.x_offset_is_percent { font_height * self.x_offset / 100.0 } else { self.x_offset },
            if self.y_offset_is_percent { font_height * self.y_offset / 100.0 } else { self.y_offset },
        ]
    }

    /// Sets what it shows (VPX's `SetCaption`): `~` and `\r` break lines (with FN 46's padding,
    /// `~` is two spaces and the caption is set between spaces); `` `u` `` leaves it as it is; a
    /// colour after `|` (`TEXT|16744448`) colours it; a picture's name (`playlist\file.png`) shows
    /// that picture from the screen's playlist, or from the pack's folder — one that isn't there
    /// clears the label, and hides it if it showed a picture.
    pub fn set_caption(&mut self, text: &str, playlists: &mut [Playlist], pack: Option<&Path>, pad_text: bool, now: u64) {
        if text.is_empty() && self.kind != Kind::Text {
            return;
        }
        if text == "`u`" {
            return;
        }
        let mut text = if pad_text { text.replace('~', "  ") } else { text.replace('~', "\n") }.replace("\\r", "\n");
        if let Some(at) = text.find('|') {
            let color = super::string_to_int(&text[at + 1..], 16_744_448);
            text.truncate(at);
            if color >= 0 && self.color != color {
                self.color = color;
                self.dirty = true;
            }
        }
        if self.caption == text {
            return;
        }
        let extension = text.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        let animated = matches!(extension.as_str(), "gif" | "apng" | "agif");
        let picture = animated || matches!(extension.as_str(), "png" | "bmp" | "jpg" | "jpeg");
        if !picture {
            let text = if pad_text && !text.is_empty() { format!(" {text} ") } else { text };
            // a plain caption (pDMDLabelShow's " ") keeps a picture the label shows
            if self.kind != Kind::Text {
                self.caption = text;
                return;
            }
            self.path = None;
            self.caption = text;
            self.dirty = true;
            return;
        }
        let was_picture = self.kind != Kind::Text;
        // separators made one kind, runs of them one (VPX's normalize_path_separators); the
        // first part names the playlist, the rest the file in it
        let mut normal = String::with_capacity(text.len());
        for c in text.chars().map(|c| if c == '/' { '\\' } else { c }) {
            if !(c == '\\' && normal.ends_with('\\')) {
                normal.push(c);
            }
        }
        let (folder, rest) = normal.split_once('\\').unwrap_or((normal.as_str(), ""));
        let from_playlist = playlists.iter_mut().find(|p| !folder.is_empty() && p.folder.eq_ignore_ascii_case(folder)).and_then(|p| if rest.is_empty() { None } else { p.play_file_path(rest) });
        // also a picture named from the pack's own folder, without a playlist of its own
        let path = from_playlist.or_else(|| pack.and_then(|p| super::find_case_insensitive(&p.join(&normal), false)).filter(|p| p.is_file()));
        match path {
            Some(path) => {
                let changed = self.path.as_ref() != Some(&path);
                self.path = Some(path);
                self.kind = if animated { Kind::Gif } else { Kind::Image };
                self.caption = text;
                self.dirty = true;
                if changed {
                    self.anigif = 0;
                    self.gif_playing = self.kind == Kind::Gif;
                    self.animation_frame = -1;
                    self.animation_start = now;
                }
            }
            None => {
                // `clear.png` and the like, never shipped: nothing shows
                self.kind = Kind::Text;
                self.path = None;
                if was_picture {
                    self.visible = false;
                }
                self.caption.clear();
                self.dirty = true;
            }
        }
    }

    /// Its caption emptied, as `set_caption("")` does: a picture is kept.
    pub fn clear_caption(&mut self) {
        if self.kind == Kind::Text && !self.caption.is_empty() {
            self.path = None;
            self.caption.clear();
            self.dirty = true;
        }
    }

    /// The script's `Special` settings (VPX's `SetSpecial`): `mt` 1 an animation, `mt` 2 the
    /// label's settings. Returns a move asked of its screen.
    pub fn set_special(&mut self, special: &str, fonts: &FontLibrary, now: u64) -> Option<Reorder> {
        let text = special.trim();
        if text.is_empty() {
            return None;
        }
        // scripts write the JSON with double quotes, or single ones as PinUP's own examples do
        let json = serde_json::from_str::<serde_json::Value>(text).or_else(|_| serde_json::from_str::<serde_json::Value>(&text.replace('\'', "\""))).ok()?;
        let int = |key: &str, default: i32| -> i32 { number(&json, key).map_or(default, |v| v as i32) };
        let float = |key: &str, default: f64| -> f64 { number(&json, key).unwrap_or(default) };
        match int("mt", 0) {
            1 => {
                // one animation a label: a new one ends the last, unless `ad` asks for both
                if int("ad", 0) == 0 {
                    self.clear_animation();
                } else if self.animation.is_some() {
                    crate::script::log::once(&format!("PinUP: label {} asks for animations side by side (ad=1); the new one replaces the last, as VPX does", self.name));
                }
                let fc = int("fc", self.color);
                let at = int("at", 0);
                let len = int("len", 0);
                let mlen = if int("mlen", 0) == 0 { len } else { int("mlen", 0) };
                let tt = int("tt", 0);
                let has_len = json.get("len").is_some();
                let fq = int("fq", 0);
                let or = |len: i32, fallback: i32| if len > 0 { len } else { fallback };
                let animation = match at {
                    // pDMDLabelFlash
                    1 if has_len && fq > 0 => {
                        let mut a = Animation::base(self, len, fc, now);
                        a.flashing_period = fq;
                        Some(a)
                    }
                    // pDMDLabelMoveHorz, …Vert, …TO, and their fades
                    2 if has_len => {
                        let mut a = Animation::base(self, len, fc, now);
                        a.xps = float("xps", 0.0) as f32;
                        a.xpe = float("xpe", 0.0) as f32;
                        a.yps = float("yps", 0.0) as f32;
                        a.ype = float("ype", 0.0) as f32;
                        a.motion_len = mlen;
                        a.motion_tween = tt;
                        a.motion_color = int("mColor", fc);
                        Some(a)
                    }
                    // pDMDZoomBig
                    3 => {
                        let mut a = Animation::base(self, or(len, 1000), fc, now);
                        a.zoom_start = float("hstart", 100.0) as f32;
                        a.zoom_end = float("hend", 200.0) as f32;
                        a.zoom = a.zoom_start;
                        a.zoom_use_draw_height = json.get("ns").is_some();
                        (a.motion_len, a.motion_tween) = (mlen, tt);
                        Some(a)
                    }
                    // pDMDLabelPulseText, …Image, …Number
                    4 => {
                        let mut a = Animation::base(self, or(len, 3000), fc, now);
                        a.zoom_start = float("hstart", 80.0) as f32;
                        a.zoom_end = float("hend", 120.0) as f32;
                        a.zoom_pulse_speed = int("pspeed", 0);
                        a.zoom = a.zoom_start;
                        a.zoom_is_pulse = true;
                        (a.motion_len, a.motion_tween) = (mlen, tt);
                        if json.get("numend").is_some() {
                            a.num_start = int("numstart", 0);
                            a.num_end = int("numend", 0);
                            a.num_format = int("numformat", 0);
                        }
                        Some(a)
                    }
                    // pDMDLabelFadeOut, …In; pDMDScreenFadeOut, …In (the overlay)
                    5 | 7 => {
                        let mut a = Animation::base(self, or(len, 1000), fc, now);
                        a.alpha_start = int("astart", 255);
                        a.alpha_end = int("aend", 0);
                        a.screen_fade = at == 7;
                        a.alpha = a.alpha_start as f32 / 255.0;
                        (a.motion_len, a.motion_tween) = (mlen, tt);
                        Some(a)
                    }
                    // pDMDLabelFadePulse
                    6 => {
                        let mut a = Animation::base(self, or(len, 3000), fc, now);
                        a.alpha_start = int("astart", 70);
                        a.alpha_end = int("aend", 255);
                        a.pulse_speed = int("pspeed", 1).max(1);
                        a.alpha = a.alpha_start as f32 / 255.0;
                        Some(a)
                    }
                    // pDMDLabelWiggleText, …Image: degrees, where LabelNew's are tenths
                    8 => {
                        let mut a = Animation::base(self, or(len, 3000), fc, now);
                        a.wiggle_start = float("rstart", -45.0) as f32;
                        a.wiggle_end = float("rend", 45.0) as f32;
                        a.wiggle_speed = int("rspeed", 1).max(1);
                        a.wiggle = a.wiggle_start;
                        (a.motion_len, a.motion_tween) = (mlen, tt);
                        Some(a)
                    }
                    // pDMDLabelFlashFilter
                    9 if has_len && fq > 0 => {
                        let mut a = Animation::base(self, len, fc, now);
                        a.flashing_period = fq;
                        a.invert_filter = true;
                        a.filter_mode = int("fm", 0);
                        Some(a)
                    }
                    // pDMDLabelClone, …Delay: shown after the delay (PinUP draws a copy there,
                    // which looks the same)
                    10 => {
                        let mut a = Animation::base(self, len, fc, now);
                        a.delay_show = int("dl", 0);
                        a.visible = a.delay_show <= 0;
                        Some(a)
                    }
                    1 | 2 | 9 => {
                        crate::script::log::once(&format!("PinUP: label {} has an animation PinUP can't play: {text}", self.name));
                        None
                    }
                    _ => {
                        crate::script::log::once(&format!("PinUP: label animation {at} isn't one PinUP plays ({})", self.name));
                        None
                    }
                };
                if let Some(mut a) = animation {
                    a.kind = at;
                    a.alpha_fade = int("af", 0);
                    a.pixel = [number(&json, "xp").map(|v| v as i32 as f32), number(&json, "yp").map(|v| v as i32 as f32)];
                    self.animation = Some(a);
                }
                None
            }
            2 => {
                let mut reorder = None;
                let map = json.as_object()?;
                for (key, value) in map {
                    let v_int = || value.as_i64().or_else(|| value.as_f64().map(|f| f as i64)).or_else(|| value.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0) as i32;
                    let v_float = || value.as_f64().or_else(|| value.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0) as f32;
                    let mut set_dirty = |changed: bool| {
                        if changed {
                            self.dirty = true;
                        }
                    };
                    match key.as_str() {
                        // -1 behind everything, another value just in front of the background label
                        "zback" => reorder = Some(if v_int() == -1 { Reorder::Back } else { Reorder::Background }),
                        "ztop" => reorder = Some(Reorder::Front),
                        // a point size, as PinUP applies it (TFont.Size)
                        "size" => {
                            let size = v_float().max(1.0);
                            if self.size != size || !self.size_is_points {
                                self.size = size;
                                self.size_is_points = true;
                                self.dirty = true;
                            }
                        }
                        "fonth" => {
                            let size = v_float();
                            if self.size != size || self.size_is_points {
                                self.size = size;
                                self.size_is_points = false;
                                self.dirty = true;
                            }
                        }
                        "xpos" => self.x_pos = v_float(),
                        "ypos" => self.y_pos = v_float(),
                        "fname" => {
                            let name = value.as_str().unwrap_or_default().to_string();
                            let font = fonts.get(&name);
                            if font.is_none() {
                                fonts.missing(&self.name, &name);
                            }
                            let changed = !same_font(&font, &self.font);
                            self.font = font;
                            self.font_name = name;
                            set_dirty(changed);
                        }
                        "color" => {
                            let c = v_int();
                            let changed = self.color != c;
                            self.color = c;
                            set_dirty(changed);
                        }
                        "xalign" => {
                            let a = XAlign::from_number(v_int());
                            let changed = self.x_align != a;
                            self.x_align = a;
                            set_dirty(changed);
                        }
                        "yalign" => self.y_align = YAlign::from_number(v_int()),
                        "pagenum" => self.page = v_int(),
                        // pDMDLabelStopAnis: the label as its animation found it
                        "stopani" => self.clear_animation(),
                        "rotate" => {
                            let angle = v_float() / 10.0;
                            let changed = self.angle != angle;
                            self.angle = angle;
                            set_dirty(changed);
                        }
                        "zoom" => self.zoom = v_float(),
                        "alpha" => self.alpha = (v_int() as f32 / 255.0).clamp(0.0, 1.0),
                        "gradstate" => {
                            let g = v_int();
                            let changed = self.grad_state != g;
                            self.grad_state = g;
                            set_dirty(changed);
                        }
                        "gradcolor" => {
                            let g = v_int();
                            let changed = self.grad_color != g;
                            self.grad_color = g;
                            set_dirty(changed);
                        }
                        "grayscale" => {
                            let f = if v_int() == 1 { 3 } else { 0 };
                            let changed = self.filter_mode != f;
                            self.filter_mode = f;
                            set_dirty(changed);
                        }
                        "filter" => {
                            let f = v_int();
                            let changed = self.filter_mode != f;
                            self.filter_mode = f;
                            set_dirty(changed);
                        }
                        "shadowstate" => {
                            let t = i32::from(v_int() != 0);
                            let changed = self.shadow_type != t;
                            self.shadow_type = t;
                            set_dirty(changed);
                        }
                        "shadowcolor" => {
                            let c = v_int();
                            let changed = self.shadow_color != c;
                            self.shadow_color = c;
                            set_dirty(changed);
                        }
                        "shadowtype" => {
                            let t = v_int();
                            let changed = self.shadow_type != t;
                            self.shadow_type = t;
                            set_dirty(changed);
                        }
                        // the shadow's offset in percent of the font's height
                        "xoffset" => {
                            let o = v_float();
                            self.x_offset_is_percent = true;
                            let changed = self.x_offset != o;
                            self.x_offset = o;
                            set_dirty(changed);
                        }
                        "yoffset" => {
                            let o = v_float();
                            self.y_offset_is_percent = true;
                            let changed = self.y_offset != o;
                            self.y_offset = o;
                            set_dirty(changed);
                        }
                        // …or in pixels
                        "xoffpix" => {
                            self.x_offset = v_int() as f32;
                            self.x_offset_is_percent = false;
                            self.dirty = true;
                        }
                        "yoffpix" => {
                            self.y_offset = v_int() as f32;
                            self.y_offset_is_percent = false;
                            self.dirty = true;
                        }
                        // pDMDPNGAnimate: 100 a frame every 40 ms, slower below (0: 330 ms)
                        "anigif" => {
                            let speed = v_int();
                            self.anigif = if speed >= 100 { 40 } else { 30 + ((100 - speed) as f32 / 10.0 * 30.0).round() as i32 };
                            self.gif_playing = true;
                            self.animation_frame = -1;
                            self.animation_start = now;
                            self.dirty = true;
                        }
                        // a frame every so many ms, 0 stops
                        "animate" => {
                            let interval = v_int();
                            if interval < 1 {
                                self.gif_playing = false;
                            } else {
                                self.visible = true;
                                self.anigif = interval;
                                self.gif_playing = true;
                                self.animation_frame = -1;
                                self.animation_start = now;
                                self.dirty = true;
                            }
                        }
                        "anistart" => {
                            let interval = v_int();
                            self.anigif = interval;
                            self.gif_playing = interval > 0;
                            self.animation_frame = -1;
                            self.animation_start = now;
                            self.dirty = true;
                        }
                        "gifloop" => self.gif_loop = v_int() == 1,
                        "gifstart" => {
                            self.gif_start = v_int();
                            if self.gif_start < 0 {
                                self.gif_start = 0;
                                self.gif_end = -1;
                            }
                            self.animation_frame = -1;
                            self.animation_start = now;
                        }
                        "gifend" => {
                            self.gif_end = v_int();
                            if self.gif_start == self.gif_end && self.gif_end >= 0 {
                                self.dirty = true;
                            }
                        }
                        "aniendhide" => self.hide_on_anim_end = v_int() == 1,
                        // pDMDPNGAnimateOnceAndDispose: the picture dropped when it ends
                        "anidispose" => self.dispose_on_anim_end = v_int() == 1,
                        // a picture's size in percent of the screen; a text's isn't set this way
                        "width" if self.kind != Kind::Text => {
                            let w = v_float();
                            self.image_width_set = true;
                            let changed = self.image_width != w;
                            self.image_width = w;
                            set_dirty(changed);
                        }
                        "height" if self.kind != Kind::Text => {
                            let h = v_float();
                            self.image_height_set = true;
                            let changed = self.image_height != h;
                            self.image_height = h;
                            set_dirty(changed);
                        }
                        "autow" => {
                            let w = v_float();
                            let changed = self.auto_fit_width != w;
                            self.auto_fit_width = w;
                            set_dirty(changed);
                        }
                        "autoh" => {
                            let h = v_float();
                            let changed = self.auto_fit_height != h;
                            self.auto_fit_height = h;
                            set_dirty(changed);
                        }
                        // pDMDLabelSetBorder: a drop shadow stays one at 0, becomes an outline at 1;
                        // otherwise the value is the shadow's kind itself (its older meaning)
                        "outline" => {
                            let o = v_int();
                            let t = if self.shadow_type == 1 {
                                if o == 1 { 2 } else { self.shadow_type }
                            } else if o == 1 {
                                2
                            } else {
                                o
                            };
                            let changed = self.outline != o || self.shadow_type != t;
                            self.outline = o;
                            self.shadow_type = t;
                            set_dirty(changed);
                        }
                        "bold" => {
                            let b = v_int() == 1;
                            let changed = self.bold != b;
                            self.bold = b;
                            set_dirty(changed);
                        }
                        "italic" => {
                            let i = v_int() == 1;
                            let changed = self.italic != i;
                            self.italic = i;
                            set_dirty(changed);
                        }
                        "underline" => {
                            let u = v_int() == 1;
                            let changed = self.underline != u;
                            self.underline = u;
                            set_dirty(changed);
                        }
                        "normal" => {
                            if self.bold || self.italic || self.underline {
                                (self.bold, self.italic, self.underline) = (false, false, false);
                                self.dirty = true;
                            }
                        }
                        "dodefault" => self.do_default = v_int() == 1,
                        // pDMDLabelSetMaxLen: a text cut to its length
                        "maxlen" => {
                            let max = v_int();
                            if self.kind == Kind::Text && max > 0 && self.caption.chars().count() > max as usize {
                                self.caption = self.caption.chars().take(max as usize).collect();
                                self.dirty = true;
                            }
                        }
                        // pDMDLabelDispose: hidden, then removed once this update is done
                        "dispose" => {
                            self.clear_animation();
                            self.visible = false;
                            self.do_dispose = true;
                        }
                        // pDMDLabelHideAni
                        "anihide" => {
                            self.clear_animation();
                            self.visible = false;
                        }
                        "v2" if !self.use_absolute_pos => {
                            self.use_absolute_pos = true;
                            self.dirty = true;
                        }
                        // mt, and what VPX leaves undone
                        _ => {}
                    }
                }
                reorder
            }
            _ => None,
        }
    }

    /// The label this frame on a screen at `rect` (VPX's `Render`): its picture asked for when
    /// what it shows changed, its animation stepped on, and where it goes. Returns what to draw,
    /// and the opacity a screen fade gives its screen's overlay. In `exact` mode (test mode) its
    /// picture is waited for.
    pub fn render(&mut self, rect: [i32; 4], page: i32, screen_alpha: f32, now: u64, worker: &LabelWorker, exact: bool) -> (Option<LabelDraw>, Option<f32>) {
        if self.dispose_pending {
            // a picture being made is waited for before what it was made from is dropped
            if let Some(slot) = &self.pending
                && slot.lock().unwrap_or_else(|e| e.into_inner()).is_none()
            {
                return (None, None);
            }
            self.pending = None;
            self.path = None;
            self.kind = Kind::Text;
            self.caption.clear();
            self.render = RenderState::default();
            self.frame_cache = None;
            self.dispose_pending = false;
            self.dirty = false;
            return (None, None);
        }
        let mut overlay = None;
        // a label of page -1 (pBackground's) shows on every page
        if (!self.visible && self.animation.is_none()) || (self.page >= 0 && page != self.page) || self.caption.is_empty() {
            return (None, None);
        }
        let font_color = self.animation.as_ref().map_or(self.color, |a| a.color);
        // a wiggle turns the label to its own angle, the other way from 'rotate' (as PinUP)
        let want_angle = match &self.animation {
            Some(a) if a.is_wiggle() => -a.wiggle,
            _ => self.angle,
        };
        if want_angle != self.render_angle {
            self.render_angle = want_angle;
            self.dirty = true;
        }
        if let Some(slot) = self.pending.clone() {
            if exact {
                for _ in 0..5000 {
                    if slot.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            if let Some(made) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
                self.render = made;
                self.pending = None;
                self.frame_cache = None;
            }
        } else if self.dirty
            || (self.path.is_none() && rect[3] != self.render.prerendered_height)
            || (self.path.is_none() && font_color != self.render.prerendered_color)
        {
            self.dirty = false;
            let job = match (&self.path, self.kind) {
                (Some(path), Kind::Gif) => Some(Job::Gif { path: path.clone() }),
                (Some(path), _) => Some(Job::Image { path: path.clone(), filter: self.filter_mode, angle: self.render_angle }),
                (None, _) => self.font.clone().map(|font| {
                    let size = self.effective_size();
                    let font_height = size / 100.0 * rect[3] as f32;
                    Job::Text {
                        font,
                        out_width: rect[2],
                        out_height: rect[3],
                        text: self.caption.clone(),
                        size,
                        color: font_color,
                        style: TextOptions {
                            italic: self.italic,
                            underline: self.underline,
                            x_align: self.x_align,
                            shadow_type: self.shadow_type,
                            shadow_color: self.shadow_color,
                            offset: self.shadow_offset_pixels(font_height),
                            grad_state: self.grad_state,
                            grad_color: self.grad_color,
                            filter: self.filter_mode,
                            angle: self.render_angle,
                            auto_fit: [self.auto_fit_width, self.auto_fit_height],
                        },
                    }
                }),
            };
            if let Some(job) = job {
                self.pending = Some(worker.submit(job));
                if exact {
                    // made at once in test mode, so this frame shows it
                    return self.render(rect, page, screen_alpha, now, worker, exact);
                }
            }
        }
        let Some(mut picture) = self.render.picture.clone() else { return (None, None) };
        // an animated picture's frame for now
        if !self.render.frames.is_empty() {
            let count = self.render.frames.len() as i32;
            let start = if self.gif_start >= 0 && self.gif_start < count { self.gif_start } else { 0 };
            let end = if self.gif_end >= 0 && self.gif_end < count { self.gif_end } else { count - 1 };
            let mut expected = start;
            if start != end && self.gif_playing {
                let elapsed = now.saturating_sub(self.animation_start) as i32;
                let mut finished = false;
                if self.anigif > 0 {
                    // anigif, anistart and animate give each frame's time, not a speed
                    let frames = end - start + 1;
                    if !self.gif_loop && elapsed >= self.anigif * frames {
                        expected = end;
                        finished = true;
                    } else {
                        expected = start + (elapsed / self.anigif) % frames;
                    }
                } else {
                    let delays = &self.render.accumulated_delays;
                    let start_offset = if start > 0 { delays[start as usize - 1] } else { 0 };
                    let range = delays[end as usize] - start_offset;
                    let mut frame_time = elapsed;
                    if range > 0 {
                        if self.gif_loop {
                            frame_time = elapsed % range;
                        } else if elapsed >= range {
                            frame_time = range - 1;
                            finished = true;
                        }
                    }
                    let target = start_offset + frame_time;
                    expected = (start as usize..=end as usize).find(|&i| delays[i] > target).map_or(end, |i| i as i32).clamp(start, end);
                }
                if finished {
                    self.gif_playing = false;
                    if self.hide_on_anim_end {
                        self.visible = false;
                    }
                    if self.dispose_on_anim_end {
                        self.dispose_pending = true;
                        return (None, None);
                    }
                }
            } else if !self.gif_playing && self.animation_frame >= start && self.animation_frame <= end {
                // a stopped animation stays on its frame
                expected = self.animation_frame;
            }
            self.animation_frame = expected;
            picture = match &self.frame_cache {
                Some((frame, filter, p)) if *frame == expected && *filter == self.filter_mode => p.clone(),
                _ => {
                    let frame = self.render.frames[expected as usize].clone();
                    let shown = if self.filter_mode > 0 {
                        let mut rgba = frame.rgba.clone();
                        apply_filter(&mut rgba, frame.width, frame.height, self.filter_mode);
                        picture_of(frame.width, frame.height, rgba)
                    } else {
                        frame
                    };
                    self.frame_cache = Some((expected, self.filter_mode, shown.clone()));
                    shown
                }
            };
        }
        let (width, height) = if self.kind == Kind::Text {
            (self.render.width, self.render.height)
        } else {
            // a picture's own size until its size is set; pBackground stretched over the screen
            let background = self.name.eq_ignore_ascii_case("pBackground");
            let w = if background {
                rect[2] as f32
            } else if self.image_width_set {
                self.image_width / 100.0 * rect[2] as f32
            } else {
                self.render.source_size[0]
            };
            let h = if background {
                rect[3] as f32
            } else if self.image_height_set {
                self.image_height / 100.0 * rect[3] as f32
            } else {
                self.render.source_size[1]
            };
            (w * self.render.image_scale[0], h * self.render.image_scale[1])
        };
        let mut dest = [rect[0] as f32, rect[1] as f32, width, height];
        let mut x_percent = self.x_pos / 100.0;
        if self.x_pos == 0.0 && !self.use_absolute_pos {
            x_percent = match self.x_align {
                XAlign::Center => 0.5,
                XAlign::Right => 1.0,
                XAlign::Left => x_percent,
            };
        }
        dest[0] += rect[2] as f32 * x_percent;
        match self.x_align {
            XAlign::Center => dest[0] -= width / 2.0,
            XAlign::Right => dest[0] -= width,
            XAlign::Left => {}
        }
        let mut y_percent = self.y_pos / 100.0;
        if self.y_pos == 0.0 && !self.use_absolute_pos {
            y_percent = match self.y_align {
                YAlign::Center => 0.5,
                YAlign::Bottom => 1.0,
                YAlign::Top => y_percent,
            };
        }
        dest[1] += rect[3] as f32 * y_percent;
        let align_height = if self.kind == Kind::Text && self.render.logical_height > 0.0 { self.render.logical_height } else { height };
        match self.y_align {
            YAlign::Center => dest[1] -= align_height / 2.0,
            YAlign::Bottom => dest[1] -= align_height,
            YAlign::Top => {}
        }
        if self.kind == Kind::Text {
            dest[1] += self.render.baseline_offset;
        }
        if self.shadow_type == 2 {
            let font_height = self.effective_size() / 100.0 * rect[3] as f32;
            dest[0] += (self.shadow_offset_pixels(font_height)[0] as i32) as f32 * -2.0;
        }
        let mut visible = true;
        if let Some(mut a) = self.animation.take() {
            // an animated copy at `xp`/`yp`'s pixels, for flashes, moves and delayed shows
            if matches!(a.kind, 1 | 2 | 10) {
                if let Some(x) = a.pixel[0] {
                    dest[0] = rect[0] as f32 + x;
                }
                if let Some(y) = a.pixel[1] {
                    dest[1] = rect[1] as f32 + y;
                }
            }
            if a.update(self, now, rect, dest, &mut overlay) {
                // over, the label put back as it was
                return (None, overlay);
            }
            if a.screen_fade {
                overlay = Some(a.alpha);
            }
            dest[0] += a.x_offset;
            dest[1] += a.y_offset;
            visible = a.visible;
            self.animation = Some(a);
        }
        let alpha = self.animation.as_ref().filter(|a| !a.screen_fade).map_or(self.alpha, |a| a.alpha) * screen_alpha;
        let zoom = self.animation.as_ref().map_or(self.zoom, |a| a.zoom);
        if zoom != 100.0 {
            let scale = zoom / 100.0;
            let (cx, cy) = (dest[0] + dest[2] * 0.5, dest[1] + dest[3] * 0.5);
            dest[2] *= scale;
            dest[3] *= scale;
            dest[0] = cx - dest[2] * 0.5;
            dest[1] = cy - dest[3] * 0.5;
        }
        if !visible || alpha <= 0.0 {
            return (None, overlay);
        }
        (Some(LabelDraw { picture, dest, clip: rect, alpha }), overlay)
    }
}

/// Whether two fonts are the same one.
fn same_font(a: &Option<Arc<PupFont>>, b: &Option<Arc<PupFont>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// A number in a JSON value, however the script wrote it.
fn number(json: &serde_json::Value, key: &str) -> Option<f64> {
    let v = json.get(key)?;
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

static NEXT_LABEL_PICTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1 << 62);

/// RGBA pixels as a picture of their own.
fn picture_of(width: u32, height: u32, rgba: Vec<u8>) -> Arc<Picture> {
    Arc::new(Picture { id: NEXT_LABEL_PICTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed), at_ms: 0, width, height, rgba })
}

/// VPX's `ApplyFilter`: 1 invert the colours, 2 invert them and the opacity, 3 grey, 4 invert
/// the opacity, 5 clear, 6 blur (a box blur of radius 5, across then down, edges repeated).
fn apply_filter(rgba: &mut [u8], width: u32, height: u32, mode: i32) {
    if !(1..=6).contains(&mode) {
        return;
    }
    if mode == 6 {
        let (w, h) = (width as usize, height as usize);
        if w == 0 || h == 0 || rgba.len() < w * h * 4 {
            return;
        }
        const RADIUS: i32 = 5;
        let pass = |src: &[u8], dst: &mut [u8], across: bool| {
            for y in 0..h {
                for x in 0..w {
                    let mut sum = [0u32; 4];
                    let mut count = 0;
                    for d in -RADIUS..=RADIUS {
                        let (sx, sy) = if across { ((x as i32 + d).clamp(0, w as i32 - 1) as usize, y) } else { (x, (y as i32 + d).clamp(0, h as i32 - 1) as usize) };
                        let px = &src[(sy * w + sx) * 4..][..4];
                        for k in 0..4 {
                            sum[k] += u32::from(px[k]);
                        }
                        count += 1;
                    }
                    for k in 0..4 {
                        dst[(y * w + x) * 4 + k] = (sum[k] / count) as u8;
                    }
                }
            }
        };
        let mut across = vec![0u8; w * h * 4];
        pass(rgba, &mut across, true);
        pass(&across, rgba, false);
        return;
    }
    for px in rgba.as_chunks_mut::<4>().0 {
        let [r, g, b, a] = *px;
        let out = match mode {
            1 => [255 - r, 255 - g, 255 - b, a],
            2 => [255 - r, 255 - g, 255 - b, 255 - a],
            3 => {
                let gray = (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)) as u8;
                [gray, gray, gray, a]
            }
            4 => [r, g, b, 255 - a],
            _ => [0; 4],
        };
        px.copy_from_slice(&out);
    }
}

/// VPX's `RotateSurface`: turned by `angle` degrees about its centre, into its bounding box,
/// nearest texel, transparent outside.
fn rotate(width: u32, height: u32, rgba: &[u8], angle: f32) -> Option<(u32, u32, Vec<u8>)> {
    if angle == 0.0 {
        return None;
    }
    let rad = -angle * (std::f64::consts::PI / 180.0) as f32;
    let (c, s) = (rad.cos(), rad.sin());
    let dst_w = (width as f32 * c.abs() + height as f32 * s.abs()).ceil() as i32;
    let dst_h = (width as f32 * s.abs() + height as f32 * c.abs()).ceil() as i32;
    let mut out = vec![0u8; dst_w.max(0) as usize * dst_h.max(0) as usize * 4];
    let (src_cx, src_cy, dst_cx, dst_cy) = (width as f32 * 0.5, height as f32 * 0.5, dst_w as f32 * 0.5, dst_h as f32 * 0.5);
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let rx = (dx as f32 - dst_cx) * c + (dy as f32 - dst_cy) * s + src_cx;
            let ry = -(dx as f32 - dst_cx) * s + (dy as f32 - dst_cy) * c + src_cy;
            let (sx, sy) = (rx as i32, ry as i32);
            if (sx as u32) < width && (sy as u32) < height {
                let from = (sy as usize * width as usize + sx as usize) * 4;
                let to = (dy as usize * dst_w as usize + dx as usize) * 4;
                out[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
            }
        }
    }
    Some((dst_w as u32, dst_h as u32, out))
}

/// A picture label's picture (VPX's `UpdateImageTexture` for an image): filtered, turned.
fn image_picture(path: &Path, filter: i32, angle: f32) -> RenderState {
    let mut rs = RenderState { image_scale: [1.0; 2], ..RenderState::default() };
    let Ok(img) = image::open(path) else {
        crate::script::log::once(&format!("PinUP: label picture {} can't be read", path.display()));
        return rs;
    };
    let img = img.to_rgba8();
    let (w, h) = img.dimensions();
    rs.source_size = [w as f32, h as f32];
    let mut rgba = img.into_raw();
    apply_filter(&mut rgba, w, h, filter);
    match rotate(w, h, &rgba, angle) {
        Some((rw, rh, turned)) => {
            rs.image_scale = [rw as f32 / w as f32, rh as f32 / h as f32];
            rs.picture = Some(picture_of(rw, rh, turned));
        }
        None => rs.picture = Some(picture_of(w, h, rgba)),
    }
    rs
}

/// An animated GIF label's frames, each with when it ends (VPX's `IMG_LoadAnimation`).
fn gif_frames(path: &Path) -> RenderState {
    use image::AnimationDecoder;
    let mut rs = RenderState { image_scale: [1.0; 2], ..RenderState::default() };
    let frames = std::fs::File::open(path)
        .ok()
        .and_then(|f| image::codecs::gif::GifDecoder::new(std::io::BufReader::new(f)).ok())
        .and_then(|d| d.into_frames().collect_frames().ok());
    let Some(frames) = frames else {
        crate::script::log::once(&format!("PinUP: label animation {} can't be read", path.display()));
        return rs;
    };
    let mut total = 0;
    for frame in frames {
        let (numer, denom) = frame.delay().numer_denom_ms();
        total += numer.checked_div(denom).unwrap_or(0) as i32;
        rs.accumulated_delays.push(total);
        let buffer = frame.into_buffer();
        let (w, h) = buffer.dimensions();
        rs.frames.push(picture_of(w, h, buffer.into_raw()));
    }
    if let Some(first) = rs.frames.first() {
        rs.source_size = [first.width as f32, first.height as f32];
    }
    rs.picture = rs.frames.first().cloned();
    rs
}

/// A text label's picture (VPX's `UpdateLabelTexture`): the caption in the label's font,
/// lines broken and aligned, shrunk to fit where asked, coloured or graded, with its drop
/// shadow or border, filtered and turned; with what places it as PinUP Player places text.
fn text_picture(font: &PupFont, out_width: i32, out_height: i32, caption: &str, size: f32, color: i32, o: &TextOptions) -> RenderState {
    let mut rs = RenderState { prerendered_height: out_height, prerendered_color: color, image_scale: [1.0; 2], ..RenderState::default() };
    let font_height = size / 100.0 * out_height as f32;
    let metrics = font.metrics(font_height);
    let shadow_top = if o.shadow_type == 2 { (((o.offset[0].abs().max(o.offset[1].abs())) as i32 - 2).max(1)) as f32 } else { 0.0 };
    rs.baseline_offset = metrics.win_ascent - metrics.sdl_ascent - shadow_top;
    // characters the font lacks become spaces
    let text: String = caption.chars().map(|c| if c != '\n' && !font.has_glyph(c) { ' ' } else { c }).collect();
    let lines = 1 + text.matches('\n').count();
    let max_w = if o.auto_fit[0] > 0.0 { o.auto_fit[0] / 100.0 * out_width as f32 } else { 0.0 };
    let max_h = if o.auto_fit[1] > 0.0 { o.auto_fit[1] / 100.0 * out_height as f32 } else { 0.0 };
    let style = TextStyle { italic: o.italic, underline: o.underline, align: o.x_align, outline: 0 };
    let mut height = font_height;
    let Some(mut surface) = font.render(&text, metrics.ptsize, rgb(color), style) else { return rs };
    // shrunk until it fits where asked (pDMDLabelSetAutoSize)
    if max_w > 0.0 || max_h > 0.0 {
        loop {
            let fits = !(max_w > 0.0 && surface.0 as f32 > max_w) && !(max_h > 0.0 && surface.1 as f32 > max_h);
            if fits || height < 4.0 {
                break;
            }
            height -= 2.0;
            match font.render(&text, font.metrics(height).ptsize, rgb(color), style) {
                Some(s) => surface = s,
                None => return rs,
            }
        }
    }
    rs.logical_height = height * lines as f32;
    let (mut tw, mut th, mut pixels) = surface;
    if o.grad_state > 0 {
        // gradstate: the share of the text kept in its colour before the gradient starts
        let (c0, c1) = (rgb(color), rgb(o.grad_color));
        let hm1 = (th.max(2) - 1) as i32;
        let start_line = (hm1 as f32 * (o.grad_state.clamp(0, 100) as f32 / 100.0)).floor() as i32;
        let grad_height = (hm1 - start_line).max(1);
        for y in 0..th as i32 {
            let gy = (y - start_line).clamp(0, grad_height);
            let g = |k: usize| (i32::from(c0[k]) + gy * (i32::from(c1[k]) - i32::from(c0[k])) / grad_height) as u8;
            let row = [g(0), g(1), g(2)];
            for px in pixels[(y as usize * tw as usize * 4)..((y as usize + 1) * tw as usize * 4)].as_chunks_mut::<4>().0 {
                if px[3] != 0 {
                    px[..3].copy_from_slice(&row);
                }
            }
        }
    }
    let (xoffset, yoffset) = (o.offset[0] as i32, o.offset[1] as i32);
    let (ax, ay) = (xoffset.abs(), yoffset.abs());
    let (mut mw, mut mh, mut merged);
    if o.shadow_type == 2 && (ax != 0 || ay != 0) {
        // a border: the text inside its outline in the shadow's colour (pDMDLabelSetBorder)
        let outline = (ax.max(ay) - 2).max(1);
        let Some((ow, oh, outer)) = font.render(&text, metrics.ptsize, rgb(o.shadow_color), TextStyle { outline, ..style }) else { return rs };
        (mw, mh, merged) = (ow, oh, outer);
        blit(&mut merged, mw, mh, &pixels, tw, th, outline, outline);
    } else {
        (mw, mh) = (tw + ax as u32, th + ay as u32);
        merged = vec![0u8; mw as usize * mh as usize * 4];
        let text_at = (if xoffset < 0 { -xoffset } else { 0 }, if yoffset < 0 { -yoffset } else { 0 });
        if o.shadow_type != 0 && (xoffset != 0 || yoffset != 0) {
            // a drop shadow: the text in the shadow's colour, moved, under it (pDMDLabelSetShadow)
            if let Some((sw, sh, shadow)) = font.render(&text, metrics.ptsize, rgb(o.shadow_color), style) {
                blit(&mut merged, mw, mh, &shadow, sw, sh, xoffset.max(0), yoffset.max(0));
            }
        }
        blit(&mut merged, mw, mh, &pixels, tw, th, text_at.0, text_at.1);
    }
    apply_filter(&mut merged, mw, mh, o.filter);
    if let Some((rw, rh, turned)) = rotate(mw, mh, &merged, o.angle) {
        (mw, mh, merged) = (rw, rh, turned);
    }
    (tw, th, pixels) = (mw, mh, merged);
    rs.width = tw as f32;
    rs.height = th as f32;
    rs.picture = Some(picture_of(tw, th, pixels));
    rs
}

/// SDL's `SDL_BlitSurface` of one RGBA picture over another at `x`, `y`, blended by the source's
/// opacity (SDL's default blend for a surface with alpha).
#[allow(clippy::too_many_arguments)]
fn blit(dst: &mut [u8], dw: u32, dh: u32, src: &[u8], sw: u32, sh: u32, x: i32, y: i32) {
    for sy in 0..sh as i32 {
        let ty = y + sy;
        if ty < 0 || ty >= dh as i32 {
            continue;
        }
        for sx in 0..sw as i32 {
            let tx = x + sx;
            if tx < 0 || tx >= dw as i32 {
                continue;
            }
            let s = &src[(sy as usize * sw as usize + sx as usize) * 4..][..4];
            let d = &mut dst[(ty as usize * dw as usize + tx as usize) * 4..][..4];
            let a = u32::from(s[3]);
            for k in 0..3 {
                d[k] = ((u32::from(s[k]) * a + u32::from(d[k]) * (255 - a)) / 255) as u8;
            }
            d[3] = (a + u32::from(d[3]) * (255 - a) / 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_and_numbers_as_vpx_makes_them() {
        let mut px = vec![10, 20, 30, 40];
        apply_filter(&mut px, 1, 1, 1);
        assert_eq!(px, [245, 235, 225, 40]);
        let mut px = vec![10, 20, 30, 40];
        apply_filter(&mut px, 1, 1, 4);
        assert_eq!(px, [10, 20, 30, 215]);
        // a blur of one bright texel spreads it over its neighbours, 11 by 11
        let mut px = vec![0u8; 21 * 21 * 4];
        px[(10 * 21 + 10) * 4] = 121;
        apply_filter(&mut px, 21, 21, 6);
        assert_eq!(px[(10 * 21 + 10) * 4], 1);
        assert_eq!(px[(5 * 21 + 5) * 4], 1);
        assert_eq!(px[(4 * 21 + 10) * 4], 0);
        assert_eq!(format_pup_number(1_234_567, 1), " 1,234,567 ");
        assert_eq!(format_pup_number(5, 1), "  5 ");
        assert_eq!(format_pup_number(-1234, 1), " -1,234 ");
        assert_eq!(format_pup_number(1234, 0), "1234");
    }

    #[test]
    fn tweens_start_at_0_and_end_at_1() {
        for kind in 0..=10 {
            assert!(tween(0.0, kind).abs() < 1e-6, "tween {kind}");
            assert!((tween(1.0, kind) - 1.0).abs() < 1e-6, "tween {kind}");
        }
        assert!((tween(0.5, 1) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn a_quarter_turn_swaps_the_size() {
        let rgba = vec![255u8; 4 * 3 * 4];
        // a pixel wider, as in VPX: cos 90° isn't quite 0 in floats, and the width rounds up
        let (w, h, _) = rotate(4, 3, &rgba, 90.0).expect("turned");
        assert_eq!((w, h), (4, 4));
    }
}
