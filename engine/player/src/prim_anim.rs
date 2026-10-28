// Ported from Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/primitive.cpp (the animation frames read from M3AY/M3AX; PlayAnim, PlayAnimEndless,
// StopAnim, ContinueAnim, ShowFrame and UpdateAnimation) and src/math/Mesh.cpp (UploadToVB).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A primitive's animation: frames of its mesh kept in the table file,
//! which scripts show one at a time (`ShowFrame`) or play at a speed
//! (`PlayAnim`, `PlayAnimEndless`). A fractional frame blends the two
//! frames either side of it, point by point, as VPX does.

use std::io::Read;

use vpin::vpx::gameitem::primitive::Primitive;

/// A frame's vertex as VPX keeps it (`Mesh::VertData`): its place, then its
/// normal, in the primitive's own space (its picture places stay the mesh's).
pub type FrameVertex = [f32; 6];

/// The primitive's animation frames (`M3AX`, each zlib-compressed), for its
/// mesh of `vertex_count` points: none when it has none or one can't be
/// read; a short frame is filled out with zeros, as VPX's buffer is.
pub fn frames(p: &Primitive, vertex_count: usize) -> Option<Vec<Vec<FrameVertex>>> {
    let data = p.compressed_animation_vertices_data.as_ref()?;
    let mut out = Vec::with_capacity(data.len());
    for compressed in data {
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice()).read_to_end(&mut raw).ok()?;
        raw.resize(vertex_count * size_of::<FrameVertex>(), 0);
        out.push(raw.as_chunks::<24>().0.iter().map(|c| std::array::from_fn(|k| f32::from_le_bytes([c[4 * k], c[4 * k + 1], c[4 * k + 2], c[4 * k + 3]]))).collect());
    }
    (!out.is_empty()).then_some(out)
}

/// `Mesh::UploadToVB`: the mesh at `frame`, blended between the frames
/// either side of it (the last one as it is).
pub fn blend(frames: &[Vec<FrameVertex>], frame: f32) -> Vec<FrameVertex> {
    let i = (frame.trunc() as usize).min(frames.len() - 1);
    let t = frame.fract();
    match frames.get(i + 1) {
        Some(next) => frames[i].iter().zip(next).map(|(a, b)| std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)).collect(),
        None => frames[i].clone(),
    }
}

/// A primitive's animation while playing (`Primitive`'s `m_currentFrame`,
/// `m_speed`, `m_doAnimation`, `m_endless`).
#[derive(Clone, Debug)]
pub struct PrimAnim {
    frame_count: usize,
    /// The frame shown; -1 until a script first shows or plays one (the
    /// mesh as loaded).
    current: f32,
    /// Frames per sixtieth of a second.
    speed: f32,
    playing: bool,
    endless: bool,
}

impl PrimAnim {
    /// A primitive's animation, when it has frames and isn't drawn ahead
    /// (VPX animates none of those it renders once, `m_staticRendering`).
    pub fn new(p: &Primitive) -> Option<PrimAnim> {
        let frame_count = p.compressed_animation_vertices_data.as_ref().map_or(0, Vec::len);
        (frame_count > 0 && !p.static_rendering).then_some(PrimAnim { frame_count, current: -1.0, speed: 0.0, playing: false, endless: false })
    }

    /// The frame to draw: none while the mesh is as loaded.
    pub fn frame(&self) -> Option<f32> {
        (self.current >= 0.0).then_some(self.current)
    }

    /// `PlayAnim`: from `start` (the first frame when past the last, or
    /// below the first, which VPX's unsigned test reads as far past) at
    /// `speed`, once.
    pub fn play(&mut self, start: f32, speed: f32) {
        self.current = if start < 0.0 || start as usize >= self.frame_count { 0.0 } else { start };
        self.speed = speed.abs();
        self.playing = true;
        self.endless = false;
    }

    /// `PlayAnimEndless`: from the first frame, round and round.
    pub fn play_endless(&mut self, speed: f32) {
        self.current = 0.0;
        self.speed = speed.abs();
        self.playing = true;
        self.endless = true;
    }

    /// `StopAnim`: held where it is.
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// `ContinueAnim`: on again at `speed`, once it has moved past the first
    /// frame.
    pub fn resume(&mut self, speed: f32) {
        if self.current > 0.0 {
            self.speed = speed.abs();
            self.playing = true;
        }
    }

    /// `ShowFrame`: held at `frame` (the last when past it); a negative one
    /// changes nothing.
    pub fn show(&mut self, frame: f32) {
        if frame >= 0.0 {
            self.current = if frame as usize >= self.frame_count { (self.frame_count - 1) as f32 } else { frame };
            self.playing = false;
        }
    }

    /// `UpdateAnimation`, once a frame: on by its speed in frames per
    /// sixtieth of a second, stopping at the last frame or going round.
    pub fn update(&mut self, diff_ms: f32) {
        if self.current == -1.0 || !self.playing {
            return;
        }
        self.current += self.speed * (diff_ms * (60.0 / 1000.0) as f32);
        let max = (self.frame_count - 1) as f32;
        if self.current > max {
            if self.endless {
                self.current = (self.current - max).min(max);
            } else {
                self.current = max;
                self.playing = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(frame_count: usize) -> PrimAnim {
        PrimAnim { frame_count, current: -1.0, speed: 0.0, playing: false, endless: false }
    }

    #[test]
    fn a_shown_frame_holds_and_stops_at_the_last() {
        let mut a = anim(7);
        assert_eq!(a.frame(), None);
        a.show(2.5);
        assert_eq!(a.frame(), Some(2.5));
        a.show(9.0);
        assert_eq!(a.frame(), Some(6.0));
        a.show(-1.0);
        assert_eq!(a.frame(), Some(6.0));
        a.update(1000.0);
        assert_eq!(a.frame(), Some(6.0));
    }

    #[test]
    fn playing_runs_at_frames_per_sixtieth_and_stops_or_goes_round() {
        let mut a = anim(5);
        a.play(0.0, 1.0);
        a.update(50.0); // three sixtieths
        assert!((a.frame().unwrap() - 3.0).abs() < 1e-5);
        a.update(50.0);
        assert_eq!(a.frame(), Some(4.0));
        assert!(!a.playing);
        let mut e = anim(5);
        e.play_endless(-2.0);
        e.update(50.0); // six frames on: past the last (4) by two
        assert!((e.frame().unwrap() - 2.0).abs() < 1e-5);
        // StopAnim holds it; ContinueAnim goes on
        e.stop();
        e.update(50.0);
        assert!((e.frame().unwrap() - 2.0).abs() < 1e-5);
        e.resume(1.0);
        e.update(1000.0 / 60.0);
        assert!((e.frame().unwrap() - 3.0).abs() < 1e-4);
    }

    #[test]
    fn a_fractional_frame_blends_the_two_either_side() {
        let frames = vec![vec![[0.0; 6]], vec![[2.0, 4.0, 6.0, 0.0, 0.0, 1.0]]];
        assert_eq!(blend(&frames, 0.5)[0], [1.0, 2.0, 3.0, 0.0, 0.0, 0.5]);
        assert_eq!(blend(&frames, 1.0)[0], frames[1][0]);
        assert_eq!(blend(&frames, 7.0)[0], frames[1][0]);
    }
}
