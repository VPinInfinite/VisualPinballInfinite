// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/Actors/AnimatedActor.cs, GIFImage.cs,
// ImageSequence.cs and Video.cs. Copyright 2019 Vincent Bousquet, licensed under the Apache License, Version 2.0
// (LICENSE-FlexDMD.txt beside this file). Changed in the porting, from C# to Rust, for Visual Pinball Infinite:
// GIFs decoded by the `gif` crate and composited as GDI+ shows them, videos through our bridge to FFmpeg
// instead of Windows Media Foundation; the port is GPL-3.0-or-later.

//! Actors that move through frames: an animated GIF, a sequence of
//! pictures, a video. Each keeps the time of its frame and how long it
//! lasts; time passes while it's visible and not paused, at its play
//! speed; at the end it stops on its last frame, or starts over when it
//! loops — from 0, as FlexDMD rewinds (the time past the end is lost).
//!
//! FlexDMD's GIFs come out opaque: it swaps each frame's red and blue
//! through a 24-bit copy, which loses alpha, so what the GIF marks
//! transparent shows its colour. Kept.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use super::actor::{self, ActorRef, Kind};
use super::assets::{AssetManager, AssetSrc};
use super::layout::{Alignment, Scaling};
use super::surface::{Bitmap, Surface};

/// What an animated actor shares (FlexDMD's `AnimatedActor`).
pub struct Anim {
    /// When the frame shown began, and how long it lasts, in seconds.
    pub frame_time: f32,
    pub frame_duration: f32,
    pub time: f32,
    pub end: bool,
    pub scaling: Scaling,
    pub alignment: Alignment,
    pub paused: bool,
    pub looping: bool,
    pub play_speed: f32,
    pub kind: AnimKind,
}

pub enum AnimKind {
    Gif(Gif),
    Sequence(Sequence),
    Video(Video),
}

/// An animated GIF (FlexDMD's `GIFImage`).
pub struct Gif {
    src: AssetSrc,
    assets: Rc<RefCell<AssetManager>>,
    size: (f32, f32),
    delays: Vec<f32>,
    length: f32,
    /// Its frames, held while on stage.
    frames: Option<Rc<GifFrames>>,
    pos: usize,
}

/// A GIF's frames, composited and made opaque as FlexDMD draws them, and
/// their delays in seconds.
pub struct GifFrames {
    pub frames: Vec<Rc<Bitmap>>,
    pub delays: Vec<f32>,
}

/// Pictures shown one after another (FlexDMD's `ImageSequence`).
pub struct Sequence {
    pub fps: i32,
    frame: usize,
    frames: Vec<ActorRef>,
}

/// A video (FlexDMD's `Video`).
pub struct Video {
    path: PathBuf,
    size: (u32, u32),
    length: f32,
    fps: f64,
    media: Option<crate::pup::av::Media>,
    frame: Option<Rc<Bitmap>>,
    seek: f32,
    /// Its sound, playing once from where the video was opened or last
    /// sought, whatever the video does meanwhile (as FlexDMD plays it).
    sound: Option<crate::pup::playback::Playback>,
}

fn anim(kind: AnimKind, looping: bool) -> Anim {
    Anim {
        frame_time: 0.0,
        frame_duration: 0.0,
        time: 0.0,
        end: false,
        scaling: Scaling::Stretch,
        alignment: Alignment::Center,
        paused: false,
        looping,
        play_speed: 1.0,
        kind,
    }
}

/// FlexDMD's `new GIFImage(manager, path, name)`.
pub fn new_gif(name: &str, assets: &Rc<RefCell<AssetManager>>, path: &str) -> Result<ActorRef, String> {
    let (src, frames) = {
        let mut m = assets.borrow_mut();
        let src = m.resolve(path, None)?;
        let frames = m.gif(&src)?;
        (src, frames)
    };
    let size = frames.frames.first().map_or((0.0, 0.0), |f| (f.width as f32, f.height as f32));
    let delays = frames.delays.clone();
    let length = delays.iter().sum();
    let gif = Gif { src, assets: assets.clone(), size, delays, length, frames: None, pos: 0 };
    let a = actor::Actor::new(name, Kind::Anim(anim(AnimKind::Gif(gif), true)));
    rewind(&a);
    actor::pack(&a);
    Ok(a)
}

/// FlexDMD's `new ImageSequence(manager, paths, name, fps, loop)`: the
/// pictures `|`-separated.
pub fn new_sequence(name: &str, assets: &Rc<RefCell<AssetManager>>, paths: &str, fps: i32, looping: bool) -> Result<ActorRef, String> {
    let frames = paths.split('|').map(|p| actor::Actor::new_image("", assets, p)).collect::<Result<Vec<_>, _>>()?;
    let mut an = anim(AnimKind::Sequence(Sequence { fps, frame: 0, frames }), looping);
    an.frame_duration = 1.0 / fps as f32;
    let a = actor::Actor::new(name, Kind::Anim(an));
    actor::pack(&a);
    Ok(a)
}

/// FlexDMD's `new Video(path, name, loop)`: its size and length read now;
/// an error when it can't be read (FlexDMD's `NewVideo` then answers Nothing).
pub fn new_video(name: &str, path: PathBuf, looping: bool) -> Result<ActorRef, String> {
    if !crate::script::files::readable(&path) {
        return Err(format!("{} is out of the table's reach", path.display()));
    }
    let media = crate::pup::av::Media::open(&path, true, false, 44100).ok_or_else(|| format!("can't read the video {}", path.display()))?;
    let size = media.info.video.ok_or_else(|| format!("{} has no pictures", path.display()))?;
    let length = media.info.duration_ms.map_or(0.0, |ms| ms as f32 / 1000.0);
    let fps = media.info.fps;
    drop(media);
    let video = Video { path, size, length, fps, media: None, frame: None, seek: -1.0, sound: None };
    let a = actor::Actor::new(name, Kind::Anim(anim(AnimKind::Video(video), looping)));
    actor::pack(&a);
    Ok(a)
}

pub fn pref_size(an: &Anim) -> (f32, f32) {
    match &an.kind {
        AnimKind::Gif(g) => g.size,
        AnimKind::Sequence(s) => s.frames.first().map_or((0.0, 0.0), |f| (f.borrow().width, f.borrow().height)),
        AnimKind::Video(v) => (v.size.0 as f32, v.size.1 as f32),
    }
}

/// FlexDMD's `Length`, in seconds.
pub fn length(an: &Anim) -> f32 {
    match &an.kind {
        AnimKind::Gif(g) => g.length,
        AnimKind::Sequence(s) => s.frames.len() as f32 * an.frame_duration,
        AnimKind::Video(v) => v.length,
    }
}

/// FlexDMD's `ImageSequence.FPS` setter.
pub fn set_fps(a: &ActorRef, fps: i32) {
    if let Kind::Anim(an) = &mut a.borrow_mut().kind
        && let AnimKind::Sequence(s) = &mut an.kind
        && s.fps != fps
    {
        s.fps = fps;
        an.frame_duration = 1.0 / fps as f32;
    }
}

fn with_anim<R>(a: &ActorRef, f: impl FnOnce(&mut Anim) -> R) -> Option<R> {
    match &mut a.borrow_mut().kind {
        Kind::Anim(an) => Some(f(an)),
        _ => None,
    }
}

/// FlexDMD's `OnStageStateChanged` for animated actors.
pub fn on_stage_changed(a: &ActorRef) {
    let (on, visible) = (a.borrow().on_stage, a.borrow().visible);
    let todo = {
        let b = a.borrow();
        match &b.kind {
            Kind::Anim(an) => match &an.kind {
                AnimKind::Gif(g) => Some(Todo::Gif(g.assets.clone(), g.src.clone())),
                AnimKind::Sequence(s) => Some(Todo::Frames(s.frames.clone())),
                AnimKind::Video(_) => Some(Todo::Video),
            },
            _ => None,
        }
    };
    match todo {
        Some(Todo::Gif(assets, src)) => {
            let frames = if on { assets.borrow_mut().gif(&src).ok() } else { None };
            with_anim(a, |an| {
                if let AnimKind::Gif(g) = &mut an.kind {
                    g.frames = frames;
                }
            });
        }
        Some(Todo::Frames(frames)) => {
            for f in &frames {
                actor::set_on_stage(f, on);
            }
        }
        Some(Todo::Video) => video_open_or_close(a, on && visible),
        None => {}
    }
}

enum Todo {
    Gif(Rc<RefCell<AssetManager>>, AssetSrc),
    Frames(Vec<ActorRef>),
    Video,
}

/// FlexDMD's `Video.Visible` setter: a video opens only while shown.
pub fn on_visible(a: &ActorRef) {
    let is_video = matches!(&a.borrow().kind, Kind::Anim(an) if matches!(an.kind, AnimKind::Video(_)));
    if is_video {
        let (on, visible) = (a.borrow().on_stage, a.borrow().visible);
        video_open_or_close(a, on && visible);
    }
}

fn video_open_or_close(a: &ActorRef, open: bool) {
    let path = with_anim(a, |an| match &mut an.kind {
        AnimKind::Video(v) if open && v.media.is_none() => Some(v.path.clone()),
        AnimKind::Video(v) if !open => {
            v.media = None;
            v.sound = None;
            None
        }
        _ => None,
    })
    .flatten();
    if let Some(path) = path {
        let media = crate::pup::av::Media::open(&path, true, false, 44100);
        let sound = crate::pup::playback::Playback::start(&path, 100.0, false, false);
        let opened = media.is_some();
        with_anim(a, |an| {
            if let AnimKind::Video(v) = &mut an.kind {
                v.media = media;
                v.sound = sound;
            }
        });
        if opened {
            read_next_frame(a);
        }
    }
}

/// FlexDMD's `AnimatedActor.Update`, past the actor's own.
pub fn update(a: &ActorRef, dt: f32) {
    if !a.borrow().visible {
        return;
    }
    let step = with_anim(a, |an| (!an.paused).then_some(dt * an.play_speed)).flatten();
    if let Some(step) = step {
        advance(a, step);
    }
}

/// FlexDMD's `Advance`.
fn advance(a: &ActorRef, dt: f32) {
    with_anim(a, |an| an.time += dt);
    loop {
        let due = with_anim(a, |an| !an.end && an.time >= an.frame_time + an.frame_duration).unwrap_or(false);
        if !due {
            break;
        }
        read_next_frame(a);
    }
    let again = with_anim(a, |an| {
        if an.end && an.looping {
            let length = an.frame_time + an.frame_duration;
            an.time %= length;
            true
        } else {
            false
        }
    })
    .unwrap_or(false);
    if again {
        rewind(a);
    }
}

/// FlexDMD's `Seek`: back to the start, then on by `position` seconds; a
/// video goes straight there.
pub fn seek(a: &ActorRef, position: f32) {
    let is_video = matches!(&a.borrow().kind, Kind::Anim(an) if matches!(an.kind, AnimKind::Video(_)));
    if is_video {
        video_seek(a, position);
        return;
    }
    rewind(a);
    advance(a, position);
}

fn video_seek(a: &ActorRef, position: f32) {
    let has_media = with_anim(a, |an| {
        an.time = position;
        if let AnimKind::Video(v) = &mut an.kind {
            v.seek = position;
            // its sound starts again (from the start: our sound player doesn't seek)
            if v.sound.is_some() {
                v.sound = crate::pup::playback::Playback::start(&v.path, 100.0, false, false);
            }
            v.media.is_some()
        } else {
            false
        }
    })
    .unwrap_or(false);
    if has_media {
        read_next_frame(a);
        with_anim(a, |an| an.time = an.frame_time);
    }
}

/// FlexDMD's `Rewind`.
pub fn rewind(a: &ActorRef) {
    let is_video = with_anim(a, |an| {
        an.time = 0.0;
        an.frame_time = 0.0;
        an.end = false;
        match &mut an.kind {
            AnimKind::Gif(g) => {
                g.pos = 0;
                an.frame_duration = g.delays.first().copied().unwrap_or(0.0);
                false
            }
            AnimKind::Sequence(s) => {
                s.frame = 0;
                false
            }
            AnimKind::Video(_) => true,
        }
    })
    .unwrap_or(false);
    if is_video {
        video_seek(a, 0.0);
    }
}

/// FlexDMD's `ReadNextFrame`.
fn read_next_frame(a: &ActorRef) {
    let mut b = a.borrow_mut();
    let Kind::Anim(an) = &mut b.kind else { return };
    match &mut an.kind {
        AnimKind::Gif(g) => {
            if g.pos + 1 >= g.delays.len() {
                an.end = true;
            } else {
                g.pos += 1;
                an.frame_time = g.delays[..g.pos].iter().sum();
                an.frame_duration = g.delays[g.pos];
            }
        }
        AnimKind::Sequence(s) => {
            if s.frame + 1 >= s.frames.len() {
                an.end = true;
            } else {
                s.frame += 1;
                an.frame_time = s.frame as f32 * an.frame_duration;
            }
        }
        AnimKind::Video(v) => {
            if an.end {
                return;
            }
            let Some(media) = v.media.as_mut() else {
                an.end = true;
                return;
            };
            let duration = if v.fps > 0.0 { (1.0 / v.fps) as f32 } else { 1.0 / 30.0 };
            if v.seek >= 0.0 {
                // FFmpeg's bridge goes back to the start; frames before the position are skipped
                let target_ms = (f64::from(v.seek) * 1000.0) as i64;
                v.seek = -1.0;
                if !media.rewind() {
                    an.end = true;
                    return;
                }
                loop {
                    match media.next_item() {
                        Some(crate::pup::av::Item::Picture { pts_ms, width, height, rgba }) => {
                            if pts_ms + (f64::from(duration) * 1000.0) as i64 > target_ms {
                                v.frame = Some(Rc::new(video_bitmap(width, height, rgba)));
                                an.frame_time = pts_ms as f32 / 1000.0;
                                an.frame_duration = duration;
                                return;
                            }
                        }
                        Some(_) => {}
                        None => {
                            an.end = true;
                            return;
                        }
                    }
                }
            }
            loop {
                match media.next_item() {
                    Some(crate::pup::av::Item::Picture { pts_ms, width, height, rgba }) => {
                        v.frame = Some(Rc::new(video_bitmap(width, height, rgba)));
                        an.frame_time = pts_ms as f32 / 1000.0;
                        an.frame_duration = duration;
                        return;
                    }
                    Some(_) => {}
                    None => {
                        an.end = true;
                        return;
                    }
                }
            }
        }
    }
}

/// A video's picture as FlexDMD holds it: red first (swapped from Media
/// Foundation's order), opaque.
fn video_bitmap(width: u32, height: u32, rgba: &[u8]) -> Bitmap {
    let px = rgba.as_chunks::<4>().0.iter().map(|p| [p[0], p[1], p[2], 255]).collect();
    Bitmap { width, height, px, alpha: false }
}

/// Draws an animated actor (FlexDMD's `Draw` overrides).
pub fn draw(a: &ActorRef, s: &mut Surface) {
    let b = a.borrow();
    let Kind::Anim(an) = &b.kind else { return };
    match &an.kind {
        AnimKind::Gif(g) => {
            if b.visible
                && let Some(frame) = g.frames.as_ref().and_then(|f| f.frames.get(g.pos))
            {
                actor::draw_picture(s, frame, (b.x, b.y, b.width, b.height), g.size, an.scaling, an.alignment);
            }
        }
        AnimKind::Sequence(seq) => {
            actor::draw_background(&b, s);
            if b.visible
                && let Some(frame) = seq.frames.get(seq.frame).cloned()
            {
                let (scaling, alignment, bounds) = (an.scaling, an.alignment, (b.x, b.y, b.width, b.height));
                drop(b);
                {
                    let mut f = frame.borrow_mut();
                    if let Kind::Image(i) = &mut f.kind {
                        i.scaling = scaling;
                        i.alignment = alignment;
                    }
                    f.set_bounds(bounds.0, bounds.1, bounds.2, bounds.3);
                }
                actor::draw(&frame, s);
            }
        }
        AnimKind::Video(v) => {
            if b.visible
                && let Some(frame) = &v.frame
            {
                actor::draw_picture(s, frame, (b.x, b.y, b.width, b.height), (v.size.0 as f32, v.size.1 as f32), an.scaling, an.alignment);
            }
        }
    }
}

/// A GIF's frames, composited as GDI+ shows them: each frame over what the
/// last one left, as its disposal says; what's transparent where nothing is
/// yet keeps the GIF's transparent colour. Then made as FlexDMD holds them:
/// red first, opaque.
pub fn decode_gif(bytes: &[u8]) -> Option<GifFrames> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(std::io::Cursor::new(bytes)).ok()?;
    let (w, h) = (u32::from(decoder.width()), u32::from(decoder.height()));
    let mut canvas: Vec<Option<[u8; 4]>> = vec![None; (w * h) as usize];
    let (mut frames, mut delays) = (Vec::new(), Vec::new());
    while let Ok(Some(frame)) = decoder.read_next_frame() {
        let before = (frame.dispose == gif::DisposalMethod::Previous).then(|| canvas.clone());
        let (fl, ft, fw, fh) = (u32::from(frame.left), u32::from(frame.top), u32::from(frame.width), u32::from(frame.height));
        for y in 0..fh {
            for x in 0..fw {
                let (cx, cy) = (fl + x, ft + y);
                if cx >= w || cy >= h {
                    continue;
                }
                let i = ((y * fw + x) * 4) as usize;
                let Some(p) = frame.buffer.get(i..i + 4) else { continue };
                let p = [p[0], p[1], p[2], p[3]];
                let c = &mut canvas[(cy * w + cx) as usize];
                if p[3] != 0 || c.is_none() {
                    *c = Some(p);
                }
            }
        }
        frames.push(Rc::new(Bitmap { width: w, height: h, px: canvas.iter().map(|c| c.map_or([0, 0, 0, 255], |p| [p[0], p[1], p[2], 255])).collect(), alpha: false }));
        delays.push(f32::from(frame.delay) / 100.0);
        match frame.dispose {
            gif::DisposalMethod::Background => {
                for y in ft..(ft + fh).min(h) {
                    for x in fl..(fl + fw).min(w) {
                        canvas[(y * w + x) as usize] = None;
                    }
                }
            }
            gif::DisposalMethod::Previous => {
                if let Some(b) = before {
                    canvas = b;
                }
            }
            _ => {}
        }
    }
    if frames.is_empty() {
        return None;
    }
    Some(GifFrames { frames, delays })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequence_of(n: usize, fps: i32) -> ActorRef {
        let frames = (0..n).map(|_| actor::Actor::new_plain()).collect();
        let mut an = anim(AnimKind::Sequence(Sequence { fps, frame: 0, frames }), true);
        an.frame_duration = 1.0 / fps as f32;
        actor::Actor::new("S", Kind::Anim(an))
    }

    fn frame_of(a: &ActorRef) -> usize {
        match &a.borrow().kind {
            Kind::Anim(an) => match &an.kind {
                AnimKind::Sequence(s) => s.frame,
                _ => 99,
            },
            _ => 99,
        }
    }

    #[test]
    fn a_sequence_steps_and_loops_from_zero() {
        let s = sequence_of(3, 10);
        s.borrow_mut().on_stage = true;
        update(&s, 0.05);
        assert_eq!(frame_of(&s), 0);
        update(&s, 0.06);
        assert_eq!(frame_of(&s), 1);
        update(&s, 0.1);
        assert_eq!(frame_of(&s), 2);
        // past the end: back to the start, the time past it lost
        update(&s, 0.15);
        assert_eq!(frame_of(&s), 0);
        assert_eq!(with_anim(&s, |an| an.time), Some(0.0));
        // not looping: stays on its last frame
        let s = sequence_of(2, 10);
        with_anim(&s, |an| an.looping = false);
        update(&s, 1.0);
        assert_eq!(frame_of(&s), 1);
    }

    #[test]
    fn paused_and_hidden_actors_keep_their_frame() {
        let s = sequence_of(3, 10);
        with_anim(&s, |an| an.paused = true);
        update(&s, 0.5);
        assert_eq!(frame_of(&s), 0);
        with_anim(&s, |an| an.paused = false);
        s.borrow_mut().visible = false;
        update(&s, 0.5);
        assert_eq!(frame_of(&s), 0);
        // and a seek steps on from the start
        s.borrow_mut().visible = true;
        seek(&s, 0.15);
        assert_eq!(frame_of(&s), 1);
    }

    #[test]
    fn gif_frames_keep_their_transparent_colour_opaque() {
        // a 2×1 GIF: one opaque red pixel and one transparent (index 1, palette green)
        let mut bytes = Vec::new();
        {
            let palette = [255, 0, 0, 0, 255, 0];
            let mut enc = gif::Encoder::new(&mut bytes, 2, 1, &palette).unwrap();
            let mut frame = gif::Frame { width: 2, height: 1, buffer: std::borrow::Cow::Owned(vec![0, 1]), transparent: Some(1), delay: 30, ..Default::default() };
            enc.write_frame(&frame).unwrap();
            frame.buffer = std::borrow::Cow::Owned(vec![1, 1]);
            enc.write_frame(&frame).unwrap();
        }
        let g = decode_gif(&bytes).unwrap();
        assert_eq!(g.delays, vec![0.3, 0.3]);
        assert_eq!(g.frames[0].px, vec![[255, 0, 0, 255], [0, 255, 0, 255]]);
        // the second frame is all transparent: the first shows through
        assert_eq!(g.frames[1].px, g.frames[0].px);
    }
}
