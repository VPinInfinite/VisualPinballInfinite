// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/UltraDMD.cs (its stand-in for UltraDMD)
// and FlexUDMD/UltraDMD.cs (`UltraDMD.DMDObject`, which reads UltraDMD's settings when made). Copyright 2019 Vincent
// Bousquet, licensed under the Apache License, Version 2.0 (LICENSE-FlexDMD.txt beside this file). Changed in the
// porting, from C# to Rust, for Visual Pinball Infinite: UltraDMD's settings read from our app's settings
// instead of the registry; the port is GPL-3.0-or-later.

//! FlexDMD's stand-in for UltraDMD: UltraDMD's calls made into FlexDMD
//! scenes — one or two lines of text over a picture, a scrolling line,
//! scrolling credits, the four players' scoreboard — queued one after
//! another. Commands are posted and run at FlexDMD's next step, so a scene
//! just asked for isn't "rendering" yet; what's rendering is read at once.

use std::cell::RefCell;
use std::rc::Rc;

use super::actor::{self, Actor, ActorRef};
use super::font::Font;
use super::layout::{Alignment, Scaling};
use super::surface::Surface;
use super::{FlexRef, animated, mode, scenes};

/// A font UltraDMD's scenes use (FlexDMD's `FontDef`): its path, tint and
/// border tint as GDI+ colours (alpha, red, green, blue), border size.
#[derive(Clone)]
struct FontDef {
    path: &'static str,
    tint: [u8; 4],
    border: [u8; 4],
}

const fn def(path: &'static str, grey: u8) -> FontDef {
    FontDef { path, tint: [255, grey, grey, grey], border: [255; 4] }
}

const SCORE_TEXT: FontDef = def("FlexDMD.Resources.udmd-f4by5.fnt", 168);
const SCORE_NORMAL: FontDef = def("FlexDMD.Resources.udmd-f5by7.fnt", 168);
const SCORE_HIGHLIGHT: FontDef = def("FlexDMD.Resources.udmd-f6by12.fnt", 255);
const SINGLE_LINE: [FontDef; 3] = [def("FlexDMD.Resources.udmd-f14by26.fnt", 255), def("FlexDMD.Resources.udmd-f12by24.fnt", 255), def("FlexDMD.Resources.udmd-f7by13.fnt", 255)];
const TWO_LINES_TOP: FontDef = def("FlexDMD.Resources.udmd-f5by7.fnt", 255);
const TWO_LINES_BOTTOM: FontDef = def("FlexDMD.Resources.udmd-f6by12.fnt", 255);

/// A picture or video registered ahead, by its number.
#[derive(Clone, PartialEq)]
enum Preload {
    Video { file: String, scaling: Scaling, alignment: Alignment, looping: bool },
    Images { images: String, fps: i32, looping: bool },
}

/// FlexDMD's `UltraDMD`.
pub struct Ultra {
    pub flex: FlexRef,
    pub queue: ActorRef,
    pub board: ActorRef,
    preloads: Vec<(i32, Preload)>,
    visible: bool,
    stretch_mode: i32,
    next_id: i32,
}

pub type UltraRef = Rc<RefCell<Ultra>>;

/// FlexDMD's `NewFont`: the font with its tint and border as options.
pub fn new_font(flex: &FlexRef, path: &str, tint: [u8; 4], border: [u8; 4], border_size: i32) -> Result<Rc<Font>, String> {
    let argb = |c: [u8; 4]| u32::from_be_bytes(c);
    let src = format!("{path}&tint={:08X}&border_size={border_size}&border_tint={:08X}", argb(tint), argb(border));
    let assets = flex.borrow().assets.clone();
    let mut m = assets.borrow_mut();
    let s = m.resolve(&src, None)?;
    m.font(&s)
}

impl Ultra {
    /// FlexDMD's `new UltraDMD(flex)`: the scoreboard (hidden) and the queue
    /// on its stage.
    pub fn new(flex: &FlexRef) -> Result<UltraRef, String> {
        let rt = flex.borrow().runtime.clone();
        let font = |d: &FontDef| new_font(flex, d.path, d.tint, d.border, 0);
        let queue = scenes::queue(&rt);
        queue.borrow_mut().fill_parent = true;
        let board = scenes::score_board(&rt, font(&SCORE_NORMAL)?, font(&SCORE_HIGHLIGHT)?, font(&SCORE_TEXT)?);
        board.borrow_mut().visible = false;
        let stage = flex.borrow().stage.clone();
        actor::add_actor(&stage, &board, None);
        actor::add_actor(&stage, &queue, None);
        Ok(Rc::new(RefCell::new(Ultra { flex: flex.clone(), queue, board, preloads: Vec::new(), visible: true, stretch_mode: 0, next_id: 1 })))
    }
}

/// Posts a command for FlexDMD's next step.
fn post(u: &UltraRef, f: impl FnOnce(&UltraRef, &mut Surface) + 'static) {
    let flex = u.borrow().flex.clone();
    let u = u.clone();
    flex.borrow_mut().posted.push(Box::new(move |_flex, s| f(&u, s)));
}

/// `LoadSetup`: UltraDMD's colour and full-colour settings, from our app's
/// settings, where UltraDMD keeps them in the registry.
pub fn load_setup(u: &UltraRef) {
    let flex = u.borrow().flex.clone();
    if let Some(c) = crate::addon_settings::get("UltraDMD", "color")
        && let Some(rgb) = super::colors::from_name(&c)
        && rgb != [0, 0, 0]
    {
        flex.borrow_mut().color = [255, rgb[0], rgb[1], rgb[2]];
    }
    if let Some(fc) = crate::addon_settings::get("UltraDMD", "fullcolor") {
        flex.borrow_mut().render_mode = if fc.eq_ignore_ascii_case("True") { mode::RGB } else { mode::GRAY_4 };
    }
}

/// UltraDMD's `IsRendering`: scenes still queued or showing. A script asking in a loop, while
/// scenes show or commands wait for FlexDMD's next step, has FlexDMD step meanwhile, as its own
/// thread would (`super::wait_step`): a scene it queues reaches the queue at that step.
pub fn is_rendering(u: &UltraRef) -> bool {
    let flex = u.borrow().flex.clone();
    let pending = !flex.borrow().posted.is_empty();
    if scenes::is_finished(&u.borrow().queue) && !pending {
        return false;
    }
    super::wait_step(&flex);
    !scenes::is_finished(&u.borrow().queue)
}

pub fn set_visible_virtual_dmd(u: &UltraRef, visible: bool) -> bool {
    let was = std::mem::replace(&mut u.borrow_mut().visible, visible);
    u.borrow().flex.borrow_mut().show = visible;
    was
}

pub fn set_video_stretch_mode(u: &UltraRef, mode: i32) {
    u.borrow_mut().stretch_mode = mode;
}

/// The scaling and alignment UltraDMD's stretch modes give a video:
/// stretched, or as wide as the display and at its top, middle or bottom.
fn stretch(mode: i32) -> Option<(Scaling, Alignment)> {
    match mode {
        0 => Some((Scaling::Stretch, Alignment::Center)),
        1 => Some((Scaling::FillX, Alignment::Top)),
        2 => Some((Scaling::FillX, Alignment::Center)),
        3 => Some((Scaling::FillX, Alignment::Bottom)),
        _ => None,
    }
}

pub fn create_animation_from_images(u: &UltraRef, fps: i32, looping: bool, images: &str) -> i32 {
    let mut b = u.borrow_mut();
    let id = b.next_id;
    b.preloads.push((id, Preload::Images { images: images.replace(',', "|"), fps, looping }));
    b.next_id += 1;
    id
}

/// `RegisterVideo`: the same video registered again gets the same number.
pub fn register_video(u: &UltraRef, stretch_mode: i32, looping: bool, file: &str) -> i32 {
    let (scaling, alignment) = stretch(stretch_mode).unwrap_or((Scaling::Stretch, Alignment::Center));
    let v = Preload::Video { file: file.to_string(), scaling, alignment, looping };
    let mut b = u.borrow_mut();
    if let Some((id, _)) = b.preloads.iter().find(|(_, p)| *p == v) {
        return *id;
    }
    let id = b.next_id;
    b.preloads.push((id, v));
    b.next_id += 1;
    id
}

/// FlexDMD's `NewVideo`: a picture sequence, video or GIF; None where
/// FlexDMD answers null.
pub fn new_video(flex: &FlexRef, name: &str, path: &str) -> Option<ActorRef> {
    let assets = flex.borrow().assets.clone();
    if path.contains('|') {
        return animated::new_sequence(name, &assets, path, 30, true).ok();
    }
    let src = assets.borrow_mut().resolve(path, None).ok()?;
    match src.kind {
        super::assets::AssetType::Video => {
            let file = assets.borrow().file_path(&src.path);
            animated::new_video(name, file, false).ok()
        }
        super::assets::AssetType::Gif => animated::new_gif(name, &assets, path).ok(),
        super::assets::AssetType::Image => animated::new_sequence(name, &assets, path, 30, true).ok(),
        _ => None,
    }
}

fn set_anim(a: &ActorRef, scaling: Scaling, alignment: Alignment, looping: Option<bool>, fps: Option<i32>) {
    if let Some(fps) = fps {
        animated::set_fps(a, fps);
    }
    if let actor::Kind::Anim(an) = &mut a.borrow_mut().kind {
        an.scaling = scaling;
        an.alignment = alignment;
        if let Some(l) = looping {
            an.looping = l;
        }
    }
}

/// FlexDMD's `ResolveImage`: a registered number, a picture, a list of
/// pictures (`,`-separated), a video or GIF; when nothing comes of it, an
/// empty frame (with its white border, as in FlexDMD) or a bare actor.
fn resolve_image(u: &UltraRef, filename: &str, use_frame: bool) -> ActorRef {
    let (flex, stretch_mode, preload) = {
        let b = u.borrow();
        let n: Option<i32> = filename.trim().parse().ok();
        (b.flex.clone(), b.stretch_mode, n.and_then(|n| b.preloads.iter().find(|(id, _)| *id == n).map(|(_, p)| p.clone())))
    };
    let assets = flex.borrow().assets.clone();
    let found = match preload {
        Some(Preload::Video { file, scaling, alignment, looping }) => new_video(&flex, "", &file).inspect(|v| {
            if matches!(&v.borrow().kind, actor::Kind::Anim(_)) {
                set_anim(v, scaling, alignment, Some(looping), None);
            }
        }),
        Some(Preload::Images { images, fps, looping }) => animated::new_sequence("", &assets, &images, 30, true).ok().inspect(|v| set_anim(v, Scaling::Stretch, Alignment::Center, Some(looping), Some(fps))),
        None => {
            let path = filename.replace(',', "|");
            if path.contains('|') {
                animated::new_sequence("", &assets, &path, 30, true).ok()
            } else {
                let src = assets.borrow_mut().resolve(&path, None).ok();
                match src.map(|s| s.kind) {
                    Some(super::assets::AssetType::Image) => Actor::new_image("", &assets, &path).ok(),
                    Some(super::assets::AssetType::Video | super::assets::AssetType::Gif) => new_video(&flex, "", &path).inspect(|v| {
                        if let Some((s, a)) = stretch(stretch_mode) {
                            set_anim(v, s, a, None, None);
                        }
                    }),
                    _ => None,
                }
            }
        }
    };
    found.unwrap_or_else(|| if use_frame { Actor::new_frame("") } else { Actor::new_plain() })
}

/// FlexDMD's `GetFont`: in colour, the display's colour scaled by the
/// brightness (0 to 1; below 0, clear); else white; an outline brightness
/// of 0 or more draws a border in it.
fn get_font(u: &UltraRef, path: &str, brightness: f32, outline: f32) -> Option<Rc<Font>> {
    let flex = u.borrow().flex.clone();
    let (b, o) = (brightness.min(1.0), outline.min(1.0));
    let base = {
        let f = flex.borrow();
        if f.render_mode == mode::RGB { f.color } else { [255; 4] }
    };
    let scaled = |k: f32| [255, (f32::from(base[1]) * k) as u8, (f32::from(base[2]) * k) as u8, (f32::from(base[3]) * k) as u8];
    let tint = if b >= 0.0 { scaled(b) } else { [0; 4] };
    if o >= 0.0 { new_font(&flex, path, tint, scaled(o), 1).ok() } else { new_font(&flex, path, tint, [255; 4], 0).ok() }
}

/// FlexDMD's `GetFittedLabel`: the biggest single-line font in which the
/// text fits the display, else the smallest.
fn fitted_label(u: &UltraRef, text: &[u16], fill: f32, outline: f32) -> Option<ActorRef> {
    let flex = u.borrow().flex.clone();
    let (w, h, rt) = {
        let f = flex.borrow();
        (f32::from(f.width), f32::from(f.height), f.runtime.clone())
    };
    for (i, d) in SINGLE_LINE.iter().enumerate() {
        let font = get_font(u, d.path, fill, outline);
        let label = Actor::new_label("", &rt, font, text);
        {
            let mut l = label.borrow_mut();
            let (lw, lh) = (l.width, l.height);
            (l.x, l.y) = ((w - lw) / 2.0, (h - lh) / 2.0);
            if (l.x >= 0.0 && l.y >= 0.0) || i == SINGLE_LINE.len() - 1 {
                drop(l);
                return Some(label);
            }
        }
    }
    None
}

fn label_font(l: &ActorRef) -> Option<Rc<Font>> {
    l.borrow().label().and_then(|l| l.font.clone())
}

pub fn cancel_rendering(u: &UltraRef) {
    post(u, |u, _| scenes::remove_all_scenes(&u.borrow().queue));
}

pub fn cancel_rendering_with_id(u: &UltraRef, id: String) {
    post(u, move |u, _| scenes::remove_scene(&u.borrow().queue, &id));
}

pub fn clear(u: &UltraRef) {
    post(u, |u, s| {
        s.clear([0, 0, 0]);
        let (board, queue) = (u.borrow().board.clone(), u.borrow().queue.clone());
        actor::set_visible(&board, false);
        if scenes::is_finished(&queue) {
            actor::set_visible(&queue, false);
        }
    });
}

pub fn display_version_info(u: &UltraRef) {
    post(u, |u, _| {
        let (board, queue) = (u.borrow().board.clone(), u.borrow().queue.clone());
        actor::set_visible(&board, false);
        actor::set_visible(&queue, false);
    });
}

/// `DisplayScene00ExWithId` (and `DisplayScene00`, `DisplayScene00Ex`, whose
/// outlines are -15: none). Brightnesses are 0 to 15.
#[allow(clippy::too_many_arguments)]
pub fn display_scene00(u: &UltraRef, id: String, cancel_previous: bool, background: String, top: Vec<u16>, top_b: i32, top_ob: i32, bottom: Vec<u16>, bottom_b: i32, bottom_ob: i32, animate_in: i32, pause_ms: i32, animate_out: i32) {
    post(u, move |u, _| {
        let (queue, board, flex) = (u.borrow().queue.clone(), u.borrow().board.clone(), u.borrow().flex.clone());
        if cancel_previous && !id.is_empty() && scenes::active_scene(&queue).is_some_and(|s| s.borrow().name == id) {
            scenes::remove_scene(&queue, &id);
        }
        actor::set_visible(&board, false);
        actor::set_visible(&queue, true);
        let rt = flex.borrow().runtime.clone();
        let pause = pause_ms as f32 / 1000.0;
        let scene = if !top.is_empty() && !bottom.is_empty() {
            let (Some(ft), Some(fb)) = (get_font(u, TWO_LINES_TOP.path, top_b as f32 / 15.0, top_ob as f32 / 15.0), get_font(u, TWO_LINES_BOTTOM.path, bottom_b as f32 / 15.0, bottom_ob as f32 / 15.0)) else { return };
            let bg = resolve_image(u, &background, true);
            scenes::two_line_scene(&rt, Some(bg), &top, ft, &bottom, fb, animate_in, pause, animate_out, &id)
        } else if !top.is_empty() || !bottom.is_empty() {
            let (text, b, ob) = if !top.is_empty() { (&top, top_b, top_ob) } else { (&bottom, bottom_b, bottom_ob) };
            let Some(font) = fitted_label(u, text, b as f32 / 15.0, ob as f32 / 15.0).and_then(|l| label_font(&l)) else { return };
            let bg = resolve_image(u, &background, true);
            scenes::single_line_scene(&rt, Some(bg), text, font, animate_in, pause, animate_out, false, &id)
        } else {
            let bg = resolve_image(u, &background, true);
            scenes::background_scene(&rt, Some(bg), animate_in, pause, animate_out, &id)
        };
        scenes::enqueue(&queue, scene);
    });
}

/// `ModifyScene00` and `ModifyScene00Ex` (which also gives the scene
/// `pause_ms` more from now): only the active scene, by its id.
pub fn modify_scene00(u: &UltraRef, id: String, top: Vec<u16>, bottom: Vec<u16>, pause_ms: Option<i32>) {
    post(u, move |u, _| {
        let queue = u.borrow().queue.clone();
        if let Some(scene) = scenes::active_scene(&queue)
            && !id.is_empty()
            && scene.borrow().name == id
        {
            scenes::set_scene_text(&scene, &top, &bottom);
            if let Some(ms) = pause_ms {
                scenes::extend_scene(&scene, ms as f32 / 1000.0);
            }
        }
    });
}

/// `DisplayScene01`: one line scrolling across, in the biggest font.
#[allow(clippy::too_many_arguments)]
pub fn display_scene01(u: &UltraRef, id: String, background: String, text: Vec<u16>, b: i32, ob: i32, animate_in: i32, pause_ms: i32, animate_out: i32) {
    post(u, move |u, _| {
        let (queue, board, flex) = (u.borrow().queue.clone(), u.borrow().board.clone(), u.borrow().flex.clone());
        let Some(font) = get_font(u, SINGLE_LINE[0].path, b as f32 / 15.0, ob as f32 / 15.0) else { return };
        let rt = flex.borrow().runtime.clone();
        let bg = resolve_image(u, &background, false);
        let scene = scenes::single_line_scene(&rt, Some(bg), &text, font, animate_in, pause_ms as f32 / 1000.0, animate_out, true, &id);
        actor::set_visible(&board, false);
        actor::set_visible(&queue, true);
        scenes::enqueue(&queue, scene);
    });
}

/// `SetScoreboardBackgroundImage`: its picture, and its fonts at the given
/// brightnesses (0 to 15), without borders.
pub fn set_scoreboard_background(u: &UltraRef, file: String, selected: i32, unselected: i32) {
    post(u, move |u, _| {
        let board = u.borrow().board.clone();
        let bg = resolve_image(u, &file, false);
        scenes::set_board_background(&board, Some(bg));
        let fonts = (get_font(u, SCORE_NORMAL.path, unselected as f32 / 15.0, -1.0), get_font(u, SCORE_HIGHLIGHT.path, selected as f32 / 15.0, -1.0), get_font(u, SCORE_TEXT.path, unselected as f32 / 15.0, -1.0));
        if let (Some(n), Some(h), Some(t)) = fonts {
            scenes::set_board_fonts(&board, n, h, t);
        }
    });
}

/// `DisplayScoreboard` (and `DisplayScoreboard00`): shown at once when no
/// scene is queued, else once the scenes are done.
#[allow(clippy::too_many_arguments)]
pub fn display_scoreboard(u: &UltraRef, players: i32, highlighted: i32, scores: [i64; 4], lower_left: Vec<u16>, lower_right: Vec<u16>) {
    post(u, move |u, _| {
        let (queue, board) = (u.borrow().queue.clone(), u.borrow().board.clone());
        scenes::set_players(&board, players);
        scenes::set_highlighted(&board, highlighted);
        scenes::set_scores(&board, scores);
        if let Some((ll, lr)) = scenes::with_board(&board, |sb| (sb.lower_left.clone(), sb.lower_right.clone())) {
            actor::set_text(&ll, &lower_left);
            actor::set_text(&lr, &lower_right);
        }
        if scenes::is_finished(&queue) {
            actor::set_visible(&queue, false);
            actor::set_visible(&board, true);
        }
    });
}

/// `DisplayText`: FlexDMD draws a fitted label straight onto the frame when
/// no scene is queued (in FlexDMD it fails, the label being off stage; here
/// it draws nothing, likewise).
pub fn display_text(u: &UltraRef) {
    post(u, |u, _| {
        let (queue, board) = (u.borrow().queue.clone(), u.borrow().board.clone());
        actor::set_visible(&board, false);
        if scenes::is_finished(&queue) {
            actor::set_visible(&queue, false);
        }
    });
}

/// `ScrollingCredits`: lines split at line feeds and `|`, in the scoreboard's
/// small font.
pub fn scrolling_credits(u: &UltraRef, background: String, text: Vec<u16>, b: i32, animate_in: i32, pause_ms: i32, animate_out: i32) {
    post(u, move |u, _| {
        let (queue, board, flex) = (u.borrow().queue.clone(), u.borrow().board.clone(), u.borrow().flex.clone());
        actor::set_visible(&board, false);
        let lines: Vec<Vec<u16>> = text.split(|&c| c == u16::from(b'\n') || c == u16::from(b'|')).map(<[u16]>::to_vec).collect();
        let Some(font) = get_font(u, SCORE_TEXT.path, b as f32 / 15.0, -1.0) else { return };
        let rt = flex.borrow().runtime.clone();
        let bg = resolve_image(u, &background, false);
        let scene = scenes::credits_scene(&rt, Some(bg), &lines, font, animate_in, pause_ms as f32 / 1000.0, animate_out, "");
        actor::set_visible(&queue, true);
        scenes::enqueue(&queue, scene);
    });
}

/// UltraDMD's version as FlexDMD 1.9.1 answers it: 1.9, build 10000.
pub const MAJOR: i32 = 1;
pub const MINOR: i32 = 9;
pub const BUILD: i32 = 10000;
