// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/Actors/Actor.cs, Group.cs, Frame.cs,
// Label.cs, Image.cs and FadeOverlay.cs. Copyright 2019 Vincent Bousquet, licensed under the Apache License,
// Version 2.0 (LICENSE-FlexDMD.txt beside this file). Changed in the porting, from C# to Rust, for Visual Pinball
// Infinite: one actor type whose kind says what it is; the port is GPL-3.0-or-later.

//! FlexDMD's actors: what a scene is made of. Every actor has a name, a
//! box, visibility and actions; a group holds others, drawn moved by its
//! position and optionally clipped to its box; a frame is a filled or
//! bordered box; a label is text in a font; an image is a picture, scaled
//! and aligned in its box; animated actors (GIFs, picture sequences,
//! videos, in `animated`) and UltraDMD's scenes (`scenes`) are kinds too.
//!
//! An actor is "on stage" while it's in the tree under a running FlexDMD's
//! stage: only then does it hold its pictures, and only then is it updated
//! and drawn. FlexDMD's quirks are kept: a group's cleared background is
//! drawn moved twice by its position; a nested clip replaces its parent's;
//! a left-aligned label's second line starts at its group's left edge.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::action::{self, ActionRef};
use super::animated::{self, Anim};
use super::assets::{AssetManager, AssetSrc};
use super::font::Font;
use super::layout::{self, Alignment, Scaling};
use super::scenes::{self, Special};
use super::surface::{Bitmap, Surface};

pub type ActorRef = Rc<RefCell<Actor>>;

/// A property tweens move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    X,
    Y,
    /// A fade's opacity.
    Alpha,
    /// A scrolling line's offset (UltraDMD's scrolling scenes).
    ScrollX,
}

/// An actor (FlexDMD's `Actor` and what derives from it).
pub struct Actor {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub parent: Weak<RefCell<Actor>>,
    pub fill_parent: bool,
    pub clear_background: bool,
    pub visible: bool,
    pub on_stage: bool,
    pub actions: Vec<ActionRef>,
    pub kind: Kind,
}

/// What an actor is.
pub enum Kind {
    /// A bare actor (what UltraDMD's stand-in shows for a missing picture).
    Plain,
    Group(Group),
    Frame(Frame),
    Label(Label),
    Image(Image),
    Anim(Anim),
    Fade(Fade),
}

/// A group of actors.
pub struct Group {
    pub children: Vec<ActorRef>,
    pub clip: bool,
    /// FlexDMD's `RuntimeVersion`: how names are found below it.
    pub runtime: Rc<Cell<i32>>,
    pub special: Special,
}

/// A box, filled and bordered.
pub struct Frame {
    pub thickness: i32,
    /// GDI+ colours: alpha, red, green, blue.
    pub border_color: [u8; 4],
    pub fill: bool,
    pub fill_color: [u8; 4],
}

/// Text in a font.
pub struct Label {
    pub font: Option<Rc<Font>>,
    /// The text, its line breaks made `\r\n` as FlexDMD makes them.
    text: Option<Vec<u16>>,
    lines: Vec<Vec<u16>>,
    text_size: (f32, f32),
    pub auto_pack: bool,
    pub alignment: Alignment,
    runtime: Rc<Cell<i32>>,
}

/// A still picture.
pub struct Image {
    pub src: AssetSrc,
    pub assets: Rc<RefCell<AssetManager>>,
    pref: (f32, f32),
    /// Held while on stage; the script may set one of its own.
    pub bitmap: Option<Rc<Bitmap>>,
    pub scaling: Scaling,
    pub alignment: Alignment,
}

/// A fade over a scene (UltraDMD's scene animations).
pub struct Fade {
    pub alpha: f32,
    /// A GDI+ colour: alpha, red, green, blue.
    pub color: [u8; 4],
}

pub const BLACK: [u8; 4] = [255, 0, 0, 0];
pub const WHITE: [u8; 4] = [255, 255, 255, 255];

impl Actor {
    pub fn new(name: &str, kind: Kind) -> ActorRef {
        Rc::new(RefCell::new(Actor {
            name: name.to_string(),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            parent: Weak::new(),
            fill_parent: false,
            clear_background: false,
            visible: true,
            on_stage: false,
            actions: Vec::new(),
            kind,
        }))
    }

    pub fn new_plain() -> ActorRef {
        Actor::new("", Kind::Plain)
    }

    pub fn new_group(name: &str, runtime: &Rc<Cell<i32>>) -> ActorRef {
        Actor::new(name, Kind::Group(Group { children: Vec::new(), clip: false, runtime: runtime.clone(), special: Special::None }))
    }

    pub fn new_frame(name: &str) -> ActorRef {
        Actor::new(name, Kind::Frame(Frame { thickness: 2, border_color: WHITE, fill: false, fill_color: BLACK }))
    }

    /// FlexDMD's `new Label(flex, font, text)`: packed to its text.
    pub fn new_label(name: &str, runtime: &Rc<Cell<i32>>, font: Option<Rc<Font>>, text: &[u16]) -> ActorRef {
        let label = Label {
            font,
            text: None,
            lines: Vec::new(),
            text_size: (0.0, 0.0),
            auto_pack: runtime.get() <= 1008,
            alignment: Alignment::Center,
            runtime: runtime.clone(),
        };
        let a = Actor::new(name, Kind::Label(label));
        set_text(&a, text);
        pack(&a);
        a
    }

    /// FlexDMD's `new Image(manager, path, name)`: its picture read once for
    /// its size (an error where FlexDMD throws), then let go until on stage.
    pub fn new_image(name: &str, assets: &Rc<RefCell<AssetManager>>, path: &str) -> Result<ActorRef, String> {
        let (src, size) = {
            let mut m = assets.borrow_mut();
            let src = m.resolve(path, None)?;
            let bmp = m.bitmap(&src)?;
            (src, (bmp.width as f32, bmp.height as f32))
        };
        let a = Actor::new(name, Kind::Image(Image { src, assets: assets.clone(), pref: size, bitmap: None, scaling: Scaling::Stretch, alignment: Alignment::Center }));
        pack(&a);
        Ok(a)
    }

    pub fn new_fade(alpha: f32, color: [u8; 4]) -> ActorRef {
        Actor::new("", Kind::Fade(Fade { alpha, color }))
    }

    pub fn group(&self) -> Option<&Group> {
        match &self.kind {
            Kind::Group(g) => Some(g),
            _ => None,
        }
    }

    pub fn group_mut(&mut self) -> Option<&mut Group> {
        match &mut self.kind {
            Kind::Group(g) => Some(g),
            _ => None,
        }
    }

    pub fn label(&self) -> Option<&Label> {
        match &self.kind {
            Kind::Label(l) => Some(l),
            _ => None,
        }
    }

    pub fn label_mut(&mut self) -> Option<&mut Label> {
        match &mut self.kind {
            Kind::Label(l) => Some(l),
            _ => None,
        }
    }

    pub fn set_bounds(&mut self, x: f32, y: f32, w: f32, h: f32) {
        (self.x, self.y, self.width, self.height) = (x, y, w, h);
    }

    /// FlexDMD's `SetAlignedPosition`: the actor placed so its aligned
    /// corner or edge is at (x, y); an unknown alignment moves nothing.
    pub fn set_aligned_position(&mut self, x: f32, y: f32, alignment: Alignment) {
        use Alignment::*;
        match alignment {
            BottomLeft | Left | TopLeft => self.x = x,
            Bottom | Center | Top => self.x = x - self.width * 0.5,
            BottomRight | Right | TopRight => self.x = x - self.width,
            Other(_) => {}
        }
        match alignment {
            BottomLeft | Bottom | BottomRight => self.y = y - self.height,
            Left | Center | Right => self.y = y - self.height * 0.5,
            TopLeft | Top | TopRight => self.y = y,
            Other(_) => {}
        }
    }
}

impl Label {
    pub fn text(&self) -> Vec<u16> {
        self.text.clone().unwrap_or_default()
    }
}

/// A property's value, for tweens.
pub fn get_prop(a: &Actor, p: Prop) -> f32 {
    match p {
        Prop::X => a.x,
        Prop::Y => a.y,
        Prop::Alpha => match &a.kind {
            Kind::Fade(f) => f.alpha,
            _ => 0.0,
        },
        Prop::ScrollX => scenes::scroll_x(a),
    }
}

pub fn set_prop(a: &mut Actor, p: Prop, v: f32) {
    match p {
        Prop::X => a.x = v,
        Prop::Y => a.y = v,
        Prop::Alpha => {
            if let Kind::Fade(f) = &mut a.kind {
                f.alpha = v;
            }
        }
        Prop::ScrollX => scenes::set_scroll_x(a, v),
    }
}

/// The size an actor would like (FlexDMD's `PrefWidth`, `PrefHeight`).
pub fn pref_size(a: &Actor) -> (f32, f32) {
    match &a.kind {
        Kind::Label(l) => l.text_size,
        Kind::Image(i) => i.pref,
        Kind::Anim(an) => animated::pref_size(an),
        _ => (0.0, 0.0),
    }
}

/// FlexDMD's `Pack`: the actor's size made the one it would like.
pub fn pack(a: &ActorRef) {
    let size = pref_size(&a.borrow());
    let mut b = a.borrow_mut();
    (b.width, b.height) = size;
}

/// FlexDMD's `Label.Text`: line breaks made `\r\n`, then measured, packed
/// if it packs itself.
pub fn set_text(a: &ActorRef, text: &[u16]) {
    let (cr, lf) = (u16::from(b'\r'), u16::from(b'\n'));
    let mut norm = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        if c == cr || c == lf {
            // \r\n, \n\r, \n or \r: one break
            if i + 1 < text.len() && (text[i + 1] == cr || text[i + 1] == lf) && text[i + 1] != c {
                i += 1;
            }
            norm.extend([cr, lf]);
        } else {
            norm.push(c);
        }
        i += 1;
    }
    {
        let mut b = a.borrow_mut();
        let Some(l) = b.label_mut() else { return };
        if l.text.as_deref() == Some(&norm[..]) {
            return;
        }
        l.lines = norm.split(|&c| c == lf).map(<[u16]>::to_vec).collect();
        l.text = Some(norm);
    }
    update_bounds(a);
}

/// FlexDMD's `Label.Font`.
pub fn set_font(a: &ActorRef, font: Option<Rc<Font>>) {
    {
        let mut b = a.borrow_mut();
        let Some(l) = b.label_mut() else { return };
        if match (&l.font, &font) {
            (Some(x), Some(y)) => Rc::ptr_eq(x, y),
            (None, None) => true,
            _ => false,
        } {
            return;
        }
        l.font = font;
    }
    update_bounds(a);
}

fn update_bounds(a: &ActorRef) {
    let pack_it = {
        let mut b = a.borrow_mut();
        let Some(l) = b.label_mut() else { return };
        let (Some(text), Some(font)) = (&l.text, &l.font) else { return };
        let (w, h) = font.measure(text);
        l.text_size = (w as f32, h as f32);
        l.auto_pack
    };
    if pack_it {
        pack(a);
    }
}

/// FlexDMD's `OnStage` setter.
pub fn set_on_stage(a: &ActorRef, on: bool) {
    {
        let mut b = a.borrow_mut();
        if b.on_stage == on {
            return;
        }
        b.on_stage = on;
    }
    on_stage_changed(a);
}

/// FlexDMD's `OnStageStateChanged`: a group's children follow it; a
/// picture is held only while on stage.
pub fn on_stage_changed(a: &ActorRef) {
    let (on, children) = {
        let b = a.borrow();
        (b.on_stage, b.group().map(|g| g.children.clone()))
    };
    if let Some(children) = children {
        for c in &children {
            set_on_stage(c, on);
        }
        scenes::on_stage_changed(a);
        return;
    }
    let image = {
        let b = a.borrow();
        match &b.kind {
            Kind::Image(i) => Some((i.assets.clone(), i.src.clone())),
            _ => None,
        }
    };
    if let Some((assets, src)) = image {
        let bitmap = if on { assets.borrow_mut().bitmap(&src).ok() } else { None };
        if let Kind::Image(i) = &mut a.borrow_mut().kind {
            i.bitmap = bitmap;
        }
        return;
    }
    animated::on_stage_changed(a);
}

/// FlexDMD's `Visible` setter: videos open and close with it, scenes start.
pub fn set_visible(a: &ActorRef, v: bool) {
    a.borrow_mut().visible = v;
    let kind = {
        let b = a.borrow();
        match &b.kind {
            Kind::Anim(_) => 1,
            Kind::Group(g) if matches!(g.special, Special::Scene(_)) => 2,
            _ => 0,
        }
    };
    match kind {
        1 => animated::on_stage_changed(a),
        2 => scenes::update_state(a),
        _ => {}
    }
}

/// FlexDMD's `Remove`: off its parent.
pub fn remove(a: &ActorRef) {
    let parent = a.borrow().parent.upgrade();
    if let Some(p) = parent {
        remove_actor(&p, a);
    }
}

/// Whether `a` is `b` or holds it, at any depth.
fn holds(a: &ActorRef, b: &ActorRef) -> bool {
    if Rc::ptr_eq(a, b) {
        return true;
    }
    let children = a.borrow().group().map(|g| g.children.clone()).unwrap_or_default();
    children.iter().any(|c| holds(c, b))
}

/// FlexDMD's `AddActor` (and `AddActorAt` with an index): off its old
/// parent, onto this one, on stage as this one is. An actor can't go inside
/// itself (FlexDMD would never finish drawing it).
pub fn add_actor(group: &ActorRef, child: &ActorRef, at: Option<usize>) {
    if group.borrow().group().is_none() || holds(child, group) {
        return;
    }
    remove(child);
    child.borrow_mut().parent = Rc::downgrade(group);
    let on = {
        let mut g = group.borrow_mut();
        let on = g.on_stage;
        if let Some(gr) = g.group_mut() {
            match at {
                Some(i) => gr.children.insert(i.min(gr.children.len()), child.clone()),
                None => gr.children.push(child.clone()),
            }
        }
        on
    };
    set_on_stage(child, on);
}

/// FlexDMD's `RemoveActor`: the child loses its parent and the stage even
/// when it wasn't this group's.
pub fn remove_actor(group: &ActorRef, child: &ActorRef) {
    child.borrow_mut().parent = Weak::new();
    if let Some(g) = group.borrow_mut().group_mut()
        && let Some(i) = g.children.iter().position(|c| Rc::ptr_eq(c, child))
    {
        g.children.remove(i);
    }
    set_on_stage(child, false);
}

/// FlexDMD's `RemoveAll`.
pub fn remove_all(group: &ActorRef) {
    let children = group.borrow_mut().group_mut().map(|g| std::mem::take(&mut g.children)).unwrap_or_default();
    for c in &children {
        c.borrow_mut().parent = Weak::new();
        set_on_stage(c, false);
    }
}

/// FlexDMD's `Group.Get`: the group itself or an actor below it by name. Up
/// to runtime version 1008, any depth, the first found; from 1009, a path:
/// `name` a child, `a/b` a child of child `a`, `/a/b` from the root.
pub fn get(group: &ActorRef, name: &str) -> Option<ActorRef> {
    let (own, runtime, children) = {
        let g = group.borrow();
        let gr = g.group()?;
        (g.name == name, gr.runtime.get(), gr.children.clone())
    };
    if own {
        return Some(group.clone());
    }
    if runtime <= 1008 {
        for c in &children {
            if c.borrow().name == name {
                return Some(c.clone());
            }
            if c.borrow().group().is_some()
                && let Some(found) = get(c, name)
            {
                return Some(found);
            }
        }
        return None;
    }
    match name.find('/') {
        None => children.iter().find(|c| c.borrow().name == name).cloned(),
        Some(0) => {
            let mut root = group.clone();
            loop {
                let up = root.borrow().parent.upgrade();
                match up {
                    Some(p) => root = p,
                    None => break,
                }
            }
            get(&root, &name[1..])
        }
        Some(pos) => {
            let head = &name[..pos];
            let child = children.iter().find(|c| c.borrow().group().is_some() && c.borrow().name == head)?.clone();
            get(&child, &name[pos + 1..])
        }
    }
}

/// FlexDMD's `Update`: actions, then what the kind does.
pub fn update(a: &ActorRef, dt: f32) {
    if !a.borrow().on_stage {
        return;
    }
    // FlexDMD's loop: a finished action is removed and the next one waits a frame
    let mut i = 0;
    loop {
        let act = {
            let b = a.borrow();
            match b.actions.get(i) {
                Some(x) => x.clone(),
                None => break,
            }
        };
        if action::update(&act, dt) {
            let mut b = a.borrow_mut();
            if i < b.actions.len() {
                b.actions.remove(i);
            }
        }
        i += 1;
    }
    let fill = {
        let b = a.borrow();
        if b.fill_parent { b.parent.upgrade().map(|p| (p.borrow().width, p.borrow().height)) } else { None }
    };
    if let Some((w, h)) = fill {
        a.borrow_mut().set_bounds(0.0, 0.0, w, h);
    }
    let is_group = a.borrow().group().is_some();
    if is_group {
        scenes::update_group(a, dt);
        return;
    }
    animated::update(a, dt);
}

/// FlexDMD's `Group.Update`, past the actor's own: each child, the list read
/// afresh as children may leave it.
pub fn update_children(a: &ActorRef, dt: f32) {
    if !a.borrow().on_stage {
        return;
    }
    let mut i = 0;
    loop {
        let child = {
            let b = a.borrow();
            match b.group().and_then(|g| g.children.get(i)) {
                Some(c) => c.clone(),
                None => break,
            }
        };
        update(&child, dt);
        let same = a.borrow().group().and_then(|g| g.children.get(i)).is_some_and(|c| Rc::ptr_eq(c, &child));
        if same {
            i += 1;
        }
    }
}

/// FlexDMD's `Actor.Draw`: its cleared background.
pub fn draw_background(a: &Actor, s: &mut Surface) {
    if a.visible && a.clear_background {
        s.fill(a.x, a.y, a.width, a.height, BLACK);
    }
}

/// FlexDMD's `Group.Draw`: moved by its position, clipped to its box if it
/// clips (the clip replacing any other), its own background then each child.
pub fn draw_group(a: &ActorRef, s: &mut Surface) {
    let (visible, x, y, w, h, clip, children) = {
        let b = a.borrow();
        let Some(g) = b.group() else { return };
        (b.visible, b.x, b.y, b.width, b.height, g.clip, g.children.clone())
    };
    if !visible {
        return;
    }
    s.translate(x, y);
    if clip {
        s.set_clip(0.0, 0.0, w, h);
    }
    draw_background(&a.borrow(), s);
    for c in &children {
        draw(c, s);
    }
    if clip {
        s.reset_clip();
    }
    s.translate(-x, -y);
}

/// FlexDMD's `Draw`, by kind.
pub fn draw(a: &ActorRef, s: &mut Surface) {
    if !a.borrow().on_stage {
        return;
    }
    if a.borrow().group().is_some() {
        scenes::draw_group(a, s);
        return;
    }
    let b = a.borrow();
    match &b.kind {
        Kind::Plain => draw_background(&b, s),
        Kind::Frame(f) => {
            draw_background(&b, s);
            if b.visible {
                let t = f.thickness as f32;
                if f.fill {
                    s.fill(b.x + t, b.y + t, b.width - 2.0 * t, b.height - 2.0 * t, f.fill_color);
                }
                if f.thickness > 0 {
                    s.fill(b.x, b.y, b.width, t, f.border_color);
                    s.fill(b.x, b.y + b.height - t, b.width, t, f.border_color);
                    s.fill(b.x, b.y + t, t, b.height - 2.0 * t, f.border_color);
                    s.fill(b.x + b.width - t, b.y + t, t, b.height - 2.0 * t, f.border_color);
                }
            }
        }
        Kind::Label(l) => {
            draw_background(&b, s);
            draw_label(&b, l, s);
        }
        Kind::Image(i) => {
            if b.visible
                && let Some(bmp) = &i.bitmap
            {
                draw_picture(s, bmp, (b.x, b.y, b.width, b.height), (i.pref.0, i.pref.1), i.scaling, i.alignment);
            }
        }
        Kind::Fade(f) => {
            draw_background(&b, s);
            let parent = b.parent.upgrade();
            let (alpha, color) = (f.alpha, f.color);
            drop(b);
            if let Some(p) = parent {
                let (pw, ph) = (p.borrow().width, p.borrow().height);
                a.borrow_mut().set_bounds(0.0, 0.0, pw, ph);
                let a8 = (255.0 * alpha).clamp(0.0, 255.0) as u8;
                s.fill(0.0, 0.0, pw, ph, [a8, color[1], color[2], color[3]]);
            }
        }
        Kind::Anim(_) => {
            drop(b);
            animated::draw(a, s);
        }
        Kind::Group(_) => {}
    }
}

/// A picture scaled and aligned in its actor's box, at whole pixels, as
/// FlexDMD's images, GIFs and videos draw.
pub fn draw_picture(s: &mut Surface, bmp: &Bitmap, (x, y, w, h): (f32, f32, f32, f32), pref: (f32, f32), scaling: Scaling, alignment: Alignment) {
    let (sw, sh) = layout::scale(scaling, pref, (w, h));
    let (ax, ay) = layout::align(alignment, (sw, sh), (w, h));
    let dest = [(x + ax) as i32 as f32, (y + ay) as i32 as f32, sw as i32 as f32, sh as i32 as f32];
    s.draw(bmp, dest, [0.0, 0.0, bmp.width as f32, bmp.height as f32]);
}

/// FlexDMD's `Label.Draw`: lines aligned one by one unless left-aligned;
/// whole pixels by truncation up to runtime version 1008, by floor after.
fn draw_label(a: &Actor, l: &Label, s: &mut Surface) {
    let (Some(font), Some(text)) = (&l.font, &l.text) else { return };
    if !a.visible {
        return;
    }
    let to_px = |v: f32| if l.runtime.get() <= 1008 { v as i32 } else { v.floor() as i32 };
    let (pw, ph) = l.text_size;
    if l.lines.len() > 1 && !l.alignment.is_left() {
        let (_, mut y) = layout::align(l.alignment, (pw, ph), (a.width, a.height));
        for line in &l.lines {
            let (lw, _) = font.measure(line);
            let (lx, _) = layout::align(l.alignment, (lw as f32, ph), (a.width, a.height));
            font.draw_text(Some(s), to_px(a.x + lx) as f32, to_px(a.y + y) as f32, line);
            y += font.line_height() as f32;
        }
    } else {
        let (x, y) = layout::align(l.alignment, (pw, ph), (a.width, a.height));
        font.draw_text(Some(s), to_px(a.x + x) as f32, to_px(a.y + y) as f32, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> Rc<Cell<i32>> {
        Rc::new(Cell::new(1008))
    }

    #[test]
    fn groups_find_by_name_as_their_runtime_says() {
        let rt = runtime();
        let stage = Actor::new_group("Stage", &rt);
        let scene = Actor::new_group("Scene", &rt);
        let inner = Actor::new_group("Inner", &rt);
        let lbl = Actor::new_frame("Score");
        add_actor(&stage, &scene, None);
        add_actor(&scene, &inner, None);
        add_actor(&inner, &lbl, None);
        // 1008: any depth
        assert!(get(&stage, "Score").is_some_and(|f| Rc::ptr_eq(&f, &lbl)));
        // 1009: paths
        rt.set(1009);
        assert!(get(&stage, "Score").is_none());
        assert!(get(&stage, "Scene/Inner/Score").is_some_and(|f| Rc::ptr_eq(&f, &lbl)));
        assert!(get(&inner, "/Scene/Inner").is_some_and(|f| Rc::ptr_eq(&f, &inner)));
        assert!(get(&stage, "Stage").is_some());
    }

    #[test]
    fn actors_move_between_groups_and_the_stage() {
        let rt = runtime();
        let stage = Actor::new_group("Stage", &rt);
        stage.borrow_mut().on_stage = true;
        let a = Actor::new_group("A", &rt);
        let b = Actor::new_group("B", &rt);
        let f = Actor::new_frame("F");
        add_actor(&a, &f, None);
        assert!(!f.borrow().on_stage);
        add_actor(&stage, &a, None);
        assert!(f.borrow().on_stage);
        // moving it to a group off stage takes it off
        add_actor(&b, &f, None);
        assert!(!f.borrow().on_stage);
        assert!(a.borrow().group().unwrap().children.is_empty());
        // a group can't go inside itself, at any depth; a frame holds nothing
        add_actor(&f, &a, None);
        add_actor(&a, &a, None);
        assert!(a.borrow().parent.upgrade().is_some_and(|p| Rc::ptr_eq(&p, &stage)));
        add_actor(&b, &stage, None);
        add_actor(&a, &b, None);
        assert!(a.borrow().group().unwrap().children.is_empty());
        assert!(b.borrow().parent.upgrade().is_none());
        remove(&stage);
        remove_all(&stage);
        assert!(!a.borrow().on_stage);
    }

    #[test]
    fn aligned_positions_place_the_box() {
        let f = Actor::new_frame("F");
        f.borrow_mut().set_bounds(0.0, 0.0, 10.0, 6.0);
        f.borrow_mut().set_aligned_position(64.0, 32.0, Alignment::Bottom);
        assert_eq!((f.borrow().x, f.borrow().y), (59.0, 26.0));
        f.borrow_mut().set_aligned_position(1.0, 1.0, Alignment::Other(42));
        assert_eq!((f.borrow().x, f.borrow().y), (59.0, 26.0));
    }

    #[test]
    fn frames_draw_their_fill_and_border_through_gdiplus() {
        let rt = runtime();
        let stage = Actor::new_group("Stage", &rt);
        stage.borrow_mut().on_stage = true;
        let f = Actor::new_frame("F");
        {
            let mut b = f.borrow_mut();
            b.set_bounds(1.0, 1.0, 4.0, 4.0);
            if let Kind::Frame(fr) = &mut b.kind {
                fr.thickness = 1;
                fr.fill = true;
                fr.fill_color = [255, 255, 0, 0];
            }
        }
        add_actor(&stage, &f, None);
        let mut s = Surface::new(6, 6);
        draw(&stage, &mut s);
        // white border, red fill read back blue, as FlexDMD reports it
        assert_eq!(s.rgb(1, 1), [255, 255, 255]);
        assert_eq!(s.rgb(2, 2), [0, 0, 255]);
        assert_eq!(s.rgb(0, 0), [0, 0, 0]);
    }

    #[test]
    fn labels_measure_pack_and_break_lines() {
        let rt = runtime();
        let font = Rc::new(super::super::font::tests::tiny());
        let text: Vec<u16> = "A\nAA".encode_utf16().collect();
        let l = Actor::new_label("L", &rt, Some(font), &text);
        let b = l.borrow();
        let lab = b.label().unwrap();
        assert_eq!(lab.text(), "A\r\nAA".encode_utf16().collect::<Vec<_>>());
        assert_eq!(lab.lines.len(), 2);
        // AA is 6 wide; two lines of 4
        assert_eq!((b.width, b.height), (6.0, 8.0));
    }
}
