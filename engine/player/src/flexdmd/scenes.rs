// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/Scenes/Scene.cs, BackgroundScene.cs,
// SingleLineScene.cs, TwoLineScene.cs, ScrollingCreditsScene.cs, AnimationType.cs, FlexDMD/Scenes/Sequence.cs and
// FlexDMD/Actors/ScoreBoard.cs. Copyright 2019 Vincent Bousquet, licensed under the Apache License, Version 2.0
// (LICENSE-FlexDMD.txt beside this file). Changed in the porting, from C# to Rust, for Visual Pinball Infinite; the
// port is GPL-3.0-or-later.

//! The groups UltraDMD's stand-in is made of: scenes (a picture with one
//! or two lines of text, or scrolling credits) that come in, stay a while
//! and go out with an animation; the queue that shows them one after
//! another; and the scoreboard of up to four players' scores.

use std::rc::Rc;

use super::actor::{self, Actor, ActorRef, Prop};
use super::font::Font;
use super::layout::Alignment;
use super::surface::Surface;
use super::tween::Tweener;

/// What makes a group special.
pub enum Special {
    None,
    ScoreBoard(ScoreBoard),
    Queue(Queue),
    Scene(Scene),
}

/// FlexDMD's `ScoreBoard`.
pub struct ScoreBoard {
    pub scores: [ActorRef; 4],
    pub background: Option<ActorRef>,
    pub highlighted: i32,
    pub lower_left: ActorRef,
    pub lower_right: ActorRef,
    pub score_font: Rc<Font>,
    pub highlight_font: Rc<Font>,
    pub text_font: Rc<Font>,
}

/// FlexDMD's `Sequence`: UltraDMD's scenes, one after another.
#[derive(Default)]
pub struct Queue {
    pending: Vec<ActorRef>,
    pub active: Option<ActorRef>,
    finished: bool,
}

/// FlexDMD's `Scene` (each is a `BackgroundScene`).
pub struct Scene {
    tweener: Tweener,
    in_len: f32,
    out_len: f32,
    animate_in: i32,
    animate_out: i32,
    active: bool,
    pub time: f32,
    /// How long it stays between its animations; below 0, forever.
    pub pause: f32,
    pub background: Option<ActorRef>,
    pub kind: SceneKind,
}

pub enum SceneKind {
    Background,
    SingleLine { text: ActorRef, scroll: bool, scroll_x: f32 },
    TwoLine { top: ActorRef, bottom: ActorRef },
    Credits { container: ActorRef, length: f32 },
}

/// UltraDMD's animations, by its numbers (`AnimationType`).
pub mod anim {
    pub const FADE_IN: i32 = 0;
    pub const FADE_OUT: i32 = 1;
    pub const SCROLL_OFF_LEFT: i32 = 4;
    pub const SCROLL_OFF_RIGHT: i32 = 5;
    pub const SCROLL_ON_LEFT: i32 = 6;
    pub const SCROLL_ON_RIGHT: i32 = 7;
    pub const SCROLL_OFF_UP: i32 = 8;
    pub const SCROLL_OFF_DOWN: i32 = 9;
    pub const SCROLL_ON_UP: i32 = 10;
    pub const SCROLL_ON_DOWN: i32 = 11;
    pub const FILL_FADE_IN: i32 = 12;
    pub const FILL_FADE_OUT: i32 = 13;
    pub const NONE: i32 = 14;
}

fn group_of(rt: &Rc<std::cell::Cell<i32>>, name: &str, special: Special) -> ActorRef {
    let g = Actor::new_group(name, rt);
    if let Some(gr) = g.borrow_mut().group_mut() {
        gr.special = special;
    }
    g
}

fn scene(background: Option<ActorRef>, animate_in: i32, pause: f32, animate_out: i32, kind: SceneKind) -> Scene {
    Scene { tweener: Tweener::default(), in_len: 0.0, out_len: -1.0, animate_in, animate_out, active: false, time: 0.0, pause, background, kind }
}

/// A scene's group with its background first, then `then` added.
fn new_scene(rt: &Rc<std::cell::Cell<i32>>, id: &str, sc: Scene, then: &[ActorRef]) -> ActorRef {
    let background = sc.background.clone();
    let g = group_of(rt, id, Special::Scene(sc));
    if let Some(b) = background {
        actor::add_actor(&g, &b, None);
    }
    for a in then {
        actor::add_actor(&g, a, None);
    }
    g
}

/// FlexDMD's `BackgroundScene`.
pub fn background_scene(rt: &Rc<std::cell::Cell<i32>>, background: Option<ActorRef>, animate_in: i32, pause: f32, animate_out: i32, id: &str) -> ActorRef {
    new_scene(rt, id, scene(background, animate_in, pause, animate_out, SceneKind::Background), &[])
}

/// FlexDMD's `SingleLineScene`.
#[allow(clippy::too_many_arguments)]
pub fn single_line_scene(rt: &Rc<std::cell::Cell<i32>>, background: Option<ActorRef>, text: &[u16], font: Rc<Font>, animate_in: i32, pause: f32, animate_out: i32, scroll: bool, id: &str) -> ActorRef {
    let label = Actor::new_label("", rt, Some(font), text);
    let kind = SceneKind::SingleLine { text: label.clone(), scroll, scroll_x: 0.0 };
    new_scene(rt, id, scene(background, animate_in, pause, animate_out, kind), &[label])
}

/// FlexDMD's `TwoLineScene`.
#[allow(clippy::too_many_arguments)]
pub fn two_line_scene(rt: &Rc<std::cell::Cell<i32>>, background: Option<ActorRef>, top: &[u16], top_font: Rc<Font>, bottom: &[u16], bottom_font: Rc<Font>, animate_in: i32, pause: f32, animate_out: i32, id: &str) -> ActorRef {
    let t = Actor::new_label("", rt, Some(top_font), top);
    let b = Actor::new_label("", rt, Some(bottom_font), bottom);
    let kind = SceneKind::TwoLine { top: t.clone(), bottom: b.clone() };
    new_scene(rt, id, scene(background, animate_in, pause, animate_out, kind), &[t, b])
}

/// FlexDMD's `ScrollingCreditsScene`: each line centred, the whole rising
/// over 3 s and 0.4 s a line (FlexDMD's own choice of timing).
#[allow(clippy::too_many_arguments)]
pub fn credits_scene(rt: &Rc<std::cell::Cell<i32>>, background: Option<ActorRef>, lines: &[Vec<u16>], font: Rc<Font>, animate_in: i32, pause: f32, animate_out: i32, id: &str) -> ActorRef {
    let container = Actor::new_group("", rt);
    let length = 3.0 + lines.len() as f32 * 0.4;
    let mut y = 0.0;
    for line in lines {
        let trimmed = trim(line);
        let txt = if trimmed.is_empty() { vec![u16::from(b' ')] } else { trimmed.to_vec() };
        let label = Actor::new_label("", rt, Some(font.clone()), &txt);
        label.borrow_mut().y = y;
        y += label.borrow().height;
        actor::add_actor(&container, &label, None);
    }
    container.borrow_mut().height = y;
    let kind = SceneKind::Credits { container: container.clone(), length };
    new_scene(rt, id, scene(background, animate_in, pause, animate_out, kind), &[container])
}

/// C#'s `Trim()`: white space off both ends.
fn trim(s: &[u16]) -> &[u16] {
    let white = |c: &u16| char::from_u32(u32::from(*c)).is_some_and(char::is_whitespace);
    let start = s.iter().position(|c| !white(c)).unwrap_or(s.len());
    let end = s.iter().rposition(|c| !white(c)).map_or(start, |e| e + 1);
    &s[start..end]
}

/// FlexDMD's `new ScoreBoard(flex, scoreFont, highlightFont, textFont)`.
pub fn score_board(rt: &Rc<std::cell::Cell<i32>>, score_font: Rc<Font>, highlight_font: Rc<Font>, text_font: Rc<Font>) -> ActorRef {
    let zero: Vec<u16> = "0".encode_utf16().collect();
    let lower_left = Actor::new_label("", rt, Some(text_font.clone()), &[]);
    let lower_right = Actor::new_label("", rt, Some(text_font.clone()), &[]);
    let scores = [(); 4].map(|_| Actor::new_label("", rt, Some(score_font.clone()), &zero));
    let sb = ScoreBoard { scores: scores.clone(), background: None, highlighted: 0, lower_left: lower_left.clone(), lower_right: lower_right.clone(), score_font, highlight_font, text_font };
    let g = group_of(rt, "", Special::ScoreBoard(sb));
    actor::add_actor(&g, &lower_left, None);
    actor::add_actor(&g, &lower_right, None);
    for s in &scores {
        actor::add_actor(&g, s, None);
    }
    g
}

/// UltraDMD's queue (FlexDMD's `Sequence`).
pub fn queue(rt: &Rc<std::cell::Cell<i32>>) -> ActorRef {
    group_of(rt, "", Special::Queue(Queue { finished: true, ..Default::default() }))
}

fn with_queue<R>(q: &ActorRef, f: impl FnOnce(&mut Queue) -> R) -> Option<R> {
    match q.borrow_mut().group_mut().map(|g| &mut g.special) {
        Some(Special::Queue(qq)) => Some(f(qq)),
        _ => None,
    }
}

fn with_scene<R>(s: &ActorRef, f: impl FnOnce(&mut Scene) -> R) -> Option<R> {
    match s.borrow_mut().group_mut().map(|g| &mut g.special) {
        Some(Special::Scene(sc)) => Some(f(sc)),
        _ => None,
    }
}

pub fn with_board<R>(b: &ActorRef, f: impl FnOnce(&mut ScoreBoard) -> R) -> Option<R> {
    match b.borrow_mut().group_mut().map(|g| &mut g.special) {
        Some(Special::ScoreBoard(sb)) => Some(f(sb)),
        _ => None,
    }
}

/// `Sequence.Enqueue`.
pub fn enqueue(q: &ActorRef, scene: ActorRef) {
    with_queue(q, |qq| {
        qq.pending.push(scene);
        qq.finished = false;
    });
}

/// `Sequence.RemoveAllScenes`.
pub fn remove_all_scenes(q: &ActorRef) {
    let active = with_queue(q, |qq| {
        qq.pending.clear();
        qq.finished = true;
        qq.active.take()
    })
    .flatten();
    if let Some(a) = active {
        actor::remove(&a);
    }
}

/// `Sequence.RemoveScene`, as FlexDMD has it: the active scene goes when its
/// name is the queue's own (empty, so scenes without an id go); the waiting
/// ones with that name go.
pub fn remove_scene(q: &ActorRef, name: &str) {
    let own = q.borrow().name.clone();
    let gone = with_queue(q, |qq| {
        let gone = if qq.active.as_ref().is_some_and(|a| a.borrow().name == own) { qq.active.take() } else { None };
        qq.pending.retain(|s| s.borrow().name != name);
        qq.finished = qq.active.is_none() && qq.pending.is_empty();
        gone
    })
    .flatten();
    if let Some(a) = gone {
        actor::remove(&a);
    }
}

pub fn is_finished(q: &ActorRef) -> bool {
    with_queue(q, |qq| qq.finished).unwrap_or(true)
}

pub fn active_scene(q: &ActorRef) -> Option<ActorRef> {
    with_queue(q, |qq| qq.active.clone()).flatten()
}

/// A scene's `SetText`, for the two kinds that have text.
pub fn set_scene_text(s: &ActorRef, top: &[u16], bottom: &[u16]) {
    let labels = with_scene(s, |sc| match &sc.kind {
        SceneKind::TwoLine { top: t, bottom: b } => vec![(t.clone(), top.to_vec()), (b.clone(), bottom.to_vec())],
        SceneKind::SingleLine { text, .. } => vec![(text.clone(), top.to_vec())],
        _ => Vec::new(),
    })
    .unwrap_or_default();
    for (l, t) in labels {
        actor::set_text(&l, &t);
    }
}

/// `scene.Pause = scene.Time + extra` (`ModifyScene00Ex`).
pub fn extend_scene(s: &ActorRef, seconds: f32) {
    with_scene(s, |sc| sc.pause = sc.time + seconds);
}

pub fn scroll_x(a: &Actor) -> f32 {
    match a.group().map(|g| &g.special) {
        Some(Special::Scene(Scene { kind: SceneKind::SingleLine { scroll_x, .. }, .. })) => *scroll_x,
        _ => 0.0,
    }
}

pub fn set_scroll_x(a: &mut Actor, v: f32) {
    if let Some(Special::Scene(Scene { kind: SceneKind::SingleLine { scroll_x, .. }, .. })) = a.group_mut().map(|g| &mut g.special) {
        *scroll_x = v;
    }
}

/// A group's `OnStageStateChanged` past its children's: a scene starts or stops.
pub fn on_stage_changed(a: &ActorRef) {
    let is_scene = matches!(a.borrow().group().map(|g| &g.special), Some(Special::Scene(_)));
    if is_scene {
        update_state(a);
    }
}

/// `Scene.UpdateState`: a scene shown on stage starts — sized to its
/// parent, its animation in begun, its own start (`Begin`); hidden or off
/// stage it stops its tweens.
pub fn update_state(a: &ActorRef) {
    let (should, parent) = {
        let b = a.borrow();
        (b.visible && b.on_stage, b.parent.upgrade())
    };
    let active = with_scene(a, |sc| sc.active).unwrap_or(false);
    if should && !active {
        with_scene(a, |sc| sc.active = true);
        if let Some(p) = parent {
            let (w, h) = (p.borrow().width, p.borrow().height);
            let mut b = a.borrow_mut();
            (b.width, b.height) = (w, h);
        }
        let animate_in = with_scene(a, |sc| sc.animate_in).unwrap_or(anim::NONE);
        let len = start_animation(a, animate_in);
        with_scene(a, |sc| {
            sc.in_len = len;
            sc.out_len = -1.0;
        });
        begin(a);
    } else if !should && active {
        with_scene(a, |sc| {
            sc.active = false;
            sc.tweener.cancel();
        });
    }
}

/// A tween on the scene's tweener, the tweener taken out meanwhile (its
/// target may be the scene itself).
fn scene_tween(a: &ActorRef, target: &ActorRef, prop: Prop, to: f32, len: f32) {
    let Some(mut tw) = with_scene(a, |sc| std::mem::take(&mut sc.tweener)) else { return };
    tw.tween(target, &[(prop, to)], len, 0.0);
    with_scene(a, |sc| sc.tweener = tw);
}

/// `Scene.StartAnimation`: its length in seconds.
fn start_animation(a: &ActorRef, animation: i32) -> f32 {
    let (w, h) = (a.borrow().width, a.borrow().height);
    let (alpha_len, scroll_w) = (0.5f32, 0.5f32);
    let scroll_h = scroll_w * h / w;
    let fade = |alpha: f32, color: [u8; 4]| {
        let f = Actor::new_fade(alpha, color);
        actor::add_actor(a, &f, None);
        f
    };
    let tween = |target: &ActorRef, prop: Prop, to: f32, len: f32| scene_tween(a, target, prop, to, len);
    let set = |prop: Prop, v: f32| actor::set_prop(&mut a.borrow_mut(), prop, v);
    match animation {
        anim::FADE_IN => {
            let f = fade(1.0, actor::BLACK);
            tween(&f, Prop::Alpha, 0.0, alpha_len);
            alpha_len
        }
        anim::FADE_OUT => {
            let f = fade(0.0, actor::BLACK);
            tween(&f, Prop::Alpha, 1.0, alpha_len);
            alpha_len
        }
        anim::SCROLL_OFF_LEFT | anim::SCROLL_OFF_RIGHT | anim::SCROLL_ON_LEFT | anim::SCROLL_ON_RIGHT => {
            let (from, to) = match animation {
                anim::SCROLL_OFF_LEFT => (0.0, -w),
                anim::SCROLL_OFF_RIGHT => (0.0, w),
                anim::SCROLL_ON_LEFT => (w, 0.0),
                _ => (-w, 0.0),
            };
            set(Prop::X, from);
            tween(a, Prop::X, to, scroll_w);
            scroll_w
        }
        anim::SCROLL_OFF_UP | anim::SCROLL_OFF_DOWN | anim::SCROLL_ON_UP | anim::SCROLL_ON_DOWN => {
            let (from, to) = match animation {
                anim::SCROLL_OFF_UP => (0.0, -h),
                anim::SCROLL_OFF_DOWN => (0.0, h),
                anim::SCROLL_ON_UP => (h, 0.0),
                _ => (-h, 0.0),
            };
            set(Prop::Y, from);
            tween(a, Prop::Y, to, scroll_h);
            scroll_h
        }
        anim::FILL_FADE_IN | anim::FILL_FADE_OUT => {
            let (under, over) = if animation == anim::FILL_FADE_IN { (actor::BLACK, actor::WHITE) } else { (actor::WHITE, actor::BLACK) };
            fade(1.0, under);
            let f = fade(0.0, over);
            tween(&f, Prop::Alpha, 1.0, alpha_len);
            alpha_len
        }
        // none, and the zooms FlexDMD leaves out
        _ => 0.0,
    }
}

/// A scene's `Begin`: a scrolling line starts off the right, a credits'
/// lines below the bottom.
fn begin(a: &ActorRef) {
    let (w, h) = (a.borrow().width, a.borrow().height);
    let what = with_scene(a, |sc| match &sc.kind {
        SceneKind::SingleLine { text, scroll: true, .. } => Some((text.clone(), true)),
        SceneKind::Credits { container, .. } => Some((container.clone(), false)),
        _ => None,
    })
    .flatten();
    match what {
        Some((text, true)) => {
            let tw = text.borrow().width;
            let offset = -(w - tw) / 2.0;
            actor::set_prop(&mut a.borrow_mut(), Prop::ScrollX, offset + w);
            // UltraDMD moves text 1.2 pixels a frame at 60 frames a second
            let len = (w + tw) / (60.0 * 1.2);
            scene_tween(a, a, Prop::ScrollX, offset - tw, len);
        }
        Some((container, false)) => {
            container.borrow_mut().y = h;
            let ch = container.borrow().height;
            let len = with_scene(a, |sc| if let SceneKind::Credits { length, .. } = &sc.kind { *length } else { 0.0 }).unwrap_or(0.0);
            scene_tween(a, &container, Prop::Y, -ch, len);
        }
        None => {}
    }
}

/// `Scene.IsFinished`.
pub fn scene_finished(s: &ActorRef) -> bool {
    with_scene(s, |sc| sc.pause >= 0.0 && sc.out_len >= 0.0 && sc.time >= sc.in_len + sc.pause + sc.out_len).unwrap_or(false)
}

/// A group's `Update` past the actor's own: its children, then what its kind does.
pub fn update_group(a: &ActorRef, dt: f32) {
    actor::update_children(a, dt);
    if !a.borrow().on_stage {
        return;
    }
    let special = match a.borrow().group().map(|g| &g.special) {
        Some(Special::ScoreBoard(_)) => 1,
        Some(Special::Queue(_)) => 2,
        Some(Special::Scene(_)) => 3,
        _ => 0,
    };
    match special {
        1 => update_board(a),
        2 => update_queue(a),
        3 => update_scene(a, dt),
        _ => {}
    }
}

fn update_queue(q: &ActorRef) {
    let active = active_scene(q);
    if let Some(s) = &active
        && scene_finished(s)
    {
        actor::remove(s);
        with_queue(q, |qq| qq.active = None);
    }
    let next = with_queue(q, |qq| (qq.active.is_none() && !qq.pending.is_empty()).then(|| qq.pending.remove(0))).flatten();
    if let Some(s) = next {
        with_queue(q, |qq| qq.active = Some(s.clone()));
        actor::add_actor(q, &s, None);
        actor::update(&s, 0.0);
    }
    with_queue(q, |qq| qq.finished = qq.active.is_none() && qq.pending.is_empty());
}

fn update_scene(a: &ActorRef, dt: f32) {
    let start_out = with_scene(a, |sc| {
        sc.time += dt;
        (sc.pause >= 0.0 && sc.out_len < 0.0 && sc.time >= sc.in_len + sc.pause).then_some(sc.animate_out)
    })
    .flatten();
    if let Some(out) = start_out {
        let len = start_animation(a, out);
        with_scene(a, |sc| sc.out_len = len);
    }
    // the tweener taken out while it runs: its tweens borrow the scene
    let mut tw = with_scene(a, |sc| std::mem::take(&mut sc.tweener)).unwrap_or_default();
    tw.update(dt);
    with_scene(a, |sc| sc.tweener = tw);
    // a background scene: its picture fills it
    let (w, h) = (a.borrow().width, a.borrow().height);
    let (background, kind) = with_scene(a, |sc| {
        let kind = match &sc.kind {
            SceneKind::SingleLine { text, scroll_x, .. } => Some((1, text.clone(), None, *scroll_x)),
            SceneKind::TwoLine { top, bottom } => Some((2, top.clone(), Some(bottom.clone()), 0.0)),
            SceneKind::Credits { container, .. } => Some((3, container.clone(), None, 0.0)),
            SceneKind::Background => None,
        };
        (sc.background.clone(), kind)
    })
    .unwrap_or((None, None));
    if let Some(b) = background {
        let mut bb = b.borrow_mut();
        (bb.width, bb.height) = (w, h);
    }
    match kind {
        Some((1, text, _, scroll_x)) => {
            let mut t = text.borrow_mut();
            let (tw, th) = (t.width, t.height);
            (t.x, t.y) = ((w - tw) / 2.0 + scroll_x, (h - th) / 2.0);
        }
        Some((2, top, Some(bottom), _)) => {
            let mut t = top.borrow_mut();
            t.x = (w - t.width) / 2.0;
            t.y = 4.0;
            drop(t);
            let mut b = bottom.borrow_mut();
            b.x = (w - b.width) / 2.0;
            b.y = 15.0;
        }
        Some((3, container, _, _)) => {
            let lines = {
                let mut c = container.borrow_mut();
                if c.width != w {
                    c.width = w;
                    c.group().map(|g| g.children.clone())
                } else {
                    None
                }
            };
            for line in lines.unwrap_or_default() {
                let mut l = line.borrow_mut();
                l.x = (w - l.width) / 2.0;
            }
        }
        _ => {}
    }
}

/// `ScoreBoard.Update`: the scores and the two lower lines laid out in the
/// parent's box, the highlighted score in its bigger font.
fn update_board(a: &ActorRef) {
    let parent = a.borrow().parent.upgrade();
    if let Some(p) = parent {
        let (w, h) = (p.borrow().width, p.borrow().height);
        a.borrow_mut().set_bounds(0.0, 0.0, w, h);
    }
    let (w, h) = (a.borrow().width, a.borrow().height);
    let Some((scores, lower_left, lower_right, hp, text_base, score_base, high_base)) = with_board(a, |sb| {
        (sb.scores.clone(), sb.lower_left.clone(), sb.lower_right.clone(), sb.highlighted, sb.text_font.base_height(), sb.score_font.base_height(), sb.highlight_font.base_height())
    }) else {
        return;
    };
    let y_text = h - text_base as f32 - 1.0;
    let y_line2 = (h - text_base as f32) / 2.0;
    let dec = (high_base - score_base) as f32 / 2.0;
    for s in scores.iter().chain([&lower_left, &lower_right]) {
        actor::pack(s);
    }
    let at = |n: i32| if hp == n { 0.0 } else { dec };
    scores[0].borrow_mut().set_aligned_position(1.0, 1.0 + at(1), Alignment::TopLeft);
    scores[1].borrow_mut().set_aligned_position(w - 1.0, 1.0 + at(2), Alignment::TopRight);
    scores[2].borrow_mut().set_aligned_position(1.0, y_line2 + at(3), Alignment::TopLeft);
    scores[3].borrow_mut().set_aligned_position(w - 1.0, y_line2 + at(4), Alignment::TopRight);
    lower_left.borrow_mut().set_aligned_position(1.0, y_text, Alignment::TopLeft);
    lower_right.borrow_mut().set_aligned_position(w - 1.0, y_text, Alignment::TopRight);
}

/// A group's `Draw`, by kind: the scoreboard and an active queue clear the
/// frame black first.
pub fn draw_group(a: &ActorRef, s: &mut Surface) {
    let (visible, special) = {
        let b = a.borrow();
        let special = match b.group().map(|g| &g.special) {
            Some(Special::ScoreBoard(_)) => 1,
            Some(Special::Queue(q)) => {
                if q.active.is_some() {
                    2
                } else {
                    3
                }
            }
            _ => 0,
        };
        (b.visible, special)
    };
    match special {
        1 => {
            if visible {
                s.clear([0, 0, 0]);
                let (w, h) = (a.borrow().width, a.borrow().height);
                if let Some(bg) = with_board(a, |sb| sb.background.clone()).flatten() {
                    let mut b = bg.borrow_mut();
                    (b.width, b.height) = (w, h);
                }
                actor::draw_group(a, s);
            }
        }
        2 => {
            if visible {
                s.clear([0, 0, 0]);
                actor::draw_group(a, s);
            }
        }
        3 => {}
        _ => actor::draw_group(a, s),
    }
}

/// `ScoreBoard.SetBackground`: the old one off, the new one first.
pub fn set_board_background(b: &ActorRef, background: Option<ActorRef>) {
    let old = with_board(b, |sb| std::mem::replace(&mut sb.background, background.clone())).flatten();
    if let Some(o) = old {
        actor::remove_actor(b, &o);
    }
    if let Some(n) = background {
        actor::add_actor(b, &n, Some(0));
    }
}

/// `ScoreBoard.SetNPlayers`.
pub fn set_players(b: &ActorRef, n: i32) {
    if let Some(scores) = with_board(b, |sb| sb.scores.clone()) {
        for (i, s) in scores.iter().enumerate() {
            actor::set_visible(s, (i as i32) < n);
        }
    }
}

/// `ScoreBoard.SetFonts`.
pub fn set_board_fonts(b: &ActorRef, score: Rc<Font>, highlight: Rc<Font>, text: Rc<Font>) {
    let Some((hp, ll, lr)) = with_board(b, |sb| {
        sb.score_font = score;
        sb.highlight_font = highlight;
        sb.text_font = text.clone();
        (sb.highlighted, sb.lower_left.clone(), sb.lower_right.clone())
    }) else {
        return;
    };
    set_highlighted(b, hp);
    actor::set_font(&ll, Some(text.clone()));
    actor::set_font(&lr, Some(text));
}

/// `ScoreBoard.SetHighlightedPlayer`.
pub fn set_highlighted(b: &ActorRef, player: i32) {
    let Some((scores, high, normal)) = with_board(b, |sb| {
        sb.highlighted = player;
        (sb.scores.clone(), sb.highlight_font.clone(), sb.score_font.clone())
    }) else {
        return;
    };
    for (i, s) in scores.iter().enumerate() {
        actor::set_font(s, Some(if i as i32 == player - 1 { high.clone() } else { normal.clone() }));
    }
}

/// `ScoreBoard.SetScore`: each with thousands separated, as .NET's
/// `ToString("#,##0")` writes them in the system's own way.
pub fn set_scores(b: &ActorRef, scores: [i64; 4]) {
    if let Some(labels) = with_board(b, |sb| sb.scores.clone()) {
        for (l, s) in labels.iter().zip(scores) {
            actor::set_text(l, &thousands(s).encode_utf16().collect::<Vec<_>>());
        }
    }
}

/// A whole number with the system's thousands separator every three digits.
pub fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let sep = thousands_separator();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(&sep);
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn thousands_separator() -> String {
    static SEP: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SEP.get_or_init(|| {
        #[cfg(windows)]
        {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn GetLocaleInfoEx(name: *const u16, lctype: u32, data: *mut u16, len: i32) -> i32;
            }
            const LOCALE_STHOUSAND: u32 = 0x0F;
            let mut buf = [0u16; 8];
            // SAFETY: the user's own locale (null name), into a buffer of its length
            let n = unsafe { GetLocaleInfoEx(std::ptr::null(), LOCALE_STHOUSAND, buf.as_mut_ptr(), buf.len() as i32) };
            if n > 1 {
                return String::from_utf16_lossy(&buf[..n as usize - 1]);
            }
        }
        ",".into()
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt() -> Rc<std::cell::Cell<i32>> {
        Rc::new(std::cell::Cell::new(1008))
    }

    fn stage(rt: &Rc<std::cell::Cell<i32>>) -> ActorRef {
        let s = Actor::new_group("Stage", rt);
        {
            let mut b = s.borrow_mut();
            b.on_stage = true;
            b.set_bounds(0.0, 0.0, 128.0, 32.0);
        }
        s
    }

    #[test]
    fn a_queue_shows_scenes_in_turn_until_they_end() {
        let rt = rt();
        let st = stage(&rt);
        let q = queue(&rt);
        q.borrow_mut().fill_parent = true;
        actor::add_actor(&st, &q, None);
        assert!(is_finished(&q));
        enqueue(&q, background_scene(&rt, None, anim::NONE, 1.0, anim::NONE, "a"));
        enqueue(&q, background_scene(&rt, None, anim::SCROLL_ON_LEFT, 1.0, anim::FADE_OUT, "b"));
        assert!(!is_finished(&q));
        actor::update(&st, 0.0);
        let a = active_scene(&q).unwrap();
        assert_eq!(a.borrow().name, "a");
        assert_eq!(a.borrow().width, 128.0);
        let mut frames = 0;
        while active_scene(&q).is_some_and(|s| s.borrow().name == "a") {
            actor::update(&st, 1.0 / 60.0);
            frames += 1;
        }
        // a second shown, then the next
        assert!((60..=61).contains(&frames), "{frames}");
        let b = active_scene(&q).unwrap();
        assert_eq!(b.borrow().name, "b");
        // scrolling on from the right: it starts a scene's width over
        assert_eq!(b.borrow().x, 128.0);
        // half a second in, a second shown, half a second fading: done
        for _ in 0..125 {
            actor::update(&st, 1.0 / 60.0);
        }
        assert_eq!(b.borrow().x, 0.0);
        assert!(is_finished(&q));
        assert!(active_scene(&q).is_none());
    }

    #[test]
    fn thousands_are_separated() {
        let s = thousands_separator();
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(1234567), format!("1{s}234{s}567"));
        assert_eq!(thousands(-1000), format!("-1{s}000"));
    }
}
