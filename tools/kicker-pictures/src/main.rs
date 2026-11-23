//! Our own kicker pictures, painted from VPX's kicker shapes.
//!
//! VPX draws each kind of kicker with a built-in picture (`Kicker::RenderSetup`:
//! `KickerCup.webp`, `KickerWilliams.webp`, `KickerGottlieb.webp`, `KickerT1.webp`,
//! `KickerHoleWood.webp`). They were drawn in 2014–2017, before VPX's GPL rule,
//! so the player can't take them. These are ours, made from the kicker
//! meshes alone (VPX's, GPL, ported in `player::shapes::kicker_mesh`): each
//! texel's point on the kicker is found through the mesh's picture coordinates,
//! then painted — turned metal with lathe rings for the cups and the
//! Williams and Gottlieb kickers, darker the deeper it lies; plywood plies
//! round the hole. Nothing of VPX's pictures is
//! used but their overall colour: each of ours is tinted so the texels its
//! kicker shows average the colour VPX's do (measured once, below), so
//! tables keep the colour of kicker their authors saw.
//!
//! `cargo run --release -p kicker-pictures` writes them into
//! `engine/player/assets/vpi/kickers/`, which the player builds into itself.

use std::path::PathBuf;

use image::{Rgb, RgbImage};
use player::shapes::kicker_mesh::{
    CUP_INDICES, CUP_VERTICES, CUP2_INDICES, CUP2_VERTICES, GOTTLIEB_INDICES, GOTTLIEB_VERTICES, HOLE_INDICES, HOLE_VERTICES,
    WILLIAMS_INDICES, WILLIAMS_VERTICES,
};

/// How a kicker's surface is painted.
#[derive(Clone, Copy)]
enum Paint {
    /// Turned metal of this colour (sRGB).
    Metal([f32; 3]),
    /// Plywood: the playfield's edge round a hole.
    Plywood,
}

/// A kind of kicker: its mesh, its picture's size and how it's painted.
struct Kind {
    file: &'static str,
    vertices: &'static [[f32; 8]],
    indices: &'static [u16],
    size: (u32, u32),
    paint: Paint,
    seed: u32,
    /// The average colour (sRGB, 0 to 255) of the texels VPX's picture for
    /// this kind shows on the kicker, measured on build 5436's pictures.
    average: [f32; 3],
}

/// A texel's point on the kicker: where it is and which way it faces (the
/// mesh's unit space: its rim near height 0, its axis upright).
#[derive(Clone, Copy)]
struct Point {
    at: [f32; 3],
    normal: [f32; 3],
}

fn main() -> Result<(), String> {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../engine/player/assets/vpi/kickers");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let steel = [0.72, 0.73, 0.75];
    let kinds = [
        Kind { file: "cup.png", vertices: &CUP_VERTICES, indices: &CUP_INDICES, size: (1024, 512), paint: Paint::Metal(steel), seed: 11, average: [99.0, 88.0, 78.0] },
        Kind {
            file: "williams.png",
            vertices: &WILLIAMS_VERTICES,
            indices: &WILLIAMS_INDICES,
            size: (512, 256),
            paint: Paint::Metal(steel),
            seed: 23,
            average: [103.0, 80.0, 79.0],
        },
        Kind {
            file: "gottlieb.png",
            vertices: &GOTTLIEB_VERTICES,
            indices: &GOTTLIEB_INDICES,
            size: (512, 512),
            paint: Paint::Metal(steel),
            seed: 37,
            average: [120.0, 49.0, 49.0],
        },
        Kind { file: "cup2.png", vertices: &CUP2_VERTICES, indices: &CUP2_INDICES, size: (1024, 512), paint: Paint::Metal(steel), seed: 41, average: [175.0, 156.0, 131.0] },
        Kind { file: "hole.png", vertices: &HOLE_VERTICES, indices: &HOLE_INDICES, size: (1024, 128), paint: Paint::Plywood, seed: 53, average: [83.0, 57.0, 22.0] },
    ];
    for kind in &kinds {
        let (w, h) = kind.size;
        let points = bake(kind.vertices, kind.indices, w, h);
        let top = kind.vertices.iter().map(|v| v[2]).fold(f32::MIN, f32::max);
        let bottom = kind.vertices.iter().map(|v| v[2]).fold(f32::MAX, f32::min);
        let mut colours: Vec<Option<[f32; 3]>> = points.iter().map(|p| p.map(|p| paint(kind, top, bottom, p))).collect();
        // tinted to VPX's overall colour for the kind
        let shown: Vec<[f32; 3]> = colours.iter().flatten().copied().collect();
        let mean = shown.iter().fold([0.0f32; 3], |s, c| [s[0] + c[0], s[1] + c[1], s[2] + c[2]]).map(|s| s / shown.len().max(1) as f32);
        let tint = [0, 1, 2].map(|k| kind.average[k] / 255.0 / mean[k].max(1e-3));
        for c in colours.iter_mut().flatten() {
            *c = [c[0] * tint[0], c[1] * tint[1], c[2] * tint[2]];
        }
        fill(&mut colours, w, h);
        let mut picture = RgbImage::new(w, h);
        for (i, c) in colours.iter().enumerate() {
            let c = c.unwrap_or([0.0; 3]);
            picture.put_pixel(i as u32 % w, i as u32 / w, Rgb(c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)));
        }
        let path = out.join(kind.file);
        picture.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{} ({w}x{h})", path.display());
    }
    Ok(())
}

/// Each texel's point on the mesh (None where the mesh puts nothing): its
/// triangles laid out at their picture coordinates (u across, v down).
fn bake(vertices: &[[f32; 8]], indices: &[u16], w: u32, h: u32) -> Vec<Option<Point>> {
    let mut out = vec![None; (w * h) as usize];
    for t in indices.chunks_exact(3) {
        let v = [t[0], t[1], t[2]].map(|i| vertices[i as usize]);
        let uv = v.map(|v| [v[6] * w as f32 - 0.5, v[7] * h as f32 - 0.5]);
        let d = (uv[1][1] - uv[2][1]) * (uv[0][0] - uv[2][0]) + (uv[2][0] - uv[1][0]) * (uv[0][1] - uv[2][1]);
        if d.abs() < 1e-9 {
            continue;
        }
        let low = |k: usize, size: u32| (uv.iter().map(|p| p[k]).fold(f32::MAX, f32::min).floor().max(0.0) as u32).min(size - 1);
        let high = |k: usize, size: u32| (uv.iter().map(|p| p[k]).fold(f32::MIN, f32::max).ceil().max(0.0) as u32).min(size - 1);
        for y in low(1, h)..=high(1, h) {
            for x in low(0, w)..=high(0, w) {
                let (px, py) = (x as f32, y as f32);
                let a = ((uv[1][1] - uv[2][1]) * (px - uv[2][0]) + (uv[2][0] - uv[1][0]) * (py - uv[2][1])) / d;
                let b = ((uv[2][1] - uv[0][1]) * (px - uv[2][0]) + (uv[0][0] - uv[2][0]) * (py - uv[2][1])) / d;
                let c = 1.0 - a - b;
                if a < -1e-4 || b < -1e-4 || c < -1e-4 {
                    continue;
                }
                let mix = |k: usize| a * v[0][k] + b * v[1][k] + c * v[2][k];
                let n = [mix(3), mix(4), mix(5)];
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
                out[(y * w + x) as usize] = Some(Point { at: [mix(0), mix(1), mix(2)], normal: n.map(|c| c / len) });
            }
        }
    }
    out
}

/// A point's colour (sRGB), by its kind's paint.
fn paint(kind: &Kind, top: f32, bottom: f32, p: Point) -> [f32; 3] {
    let [x, y, z] = p.at;
    let n = p.normal;
    // how deep inside: 0 at the rim, 1 at the bottom
    let depth = ((top - z) / (top - bottom).max(1e-6)).clamp(0.0, 1.0);
    match kind.paint {
        Paint::Metal(base) => {
            // turned on a lathe: rings round the axis, across a flat face by the radius, down a wall by the height
            let r = (x * x + y * y).sqrt();
            let ring = if n[2].abs() > 0.5 { r } else { z };
            let rings = 1.0 + 0.10 * (noise([ring * 160.0, 0.5, 0.5], kind.seed) - 0.5) + 0.05 * (noise([ring * 900.0, 3.5, 0.5], kind.seed + 1) - 0.5);
            // and fine grain over it
            let grain = 1.0 + 0.06 * (fbm([x * 70.0, y * 70.0, z * 70.0], kind.seed + 2, 3) - 0.5);
            // darker the deeper inside, a little lighter facing up
            let shade = (1.0 - 0.62 * depth.powf(0.9)) * (0.88 + 0.12 * n[2].max(0.0));
            base.map(|c| c * shade * rings * grain)
        }
        Paint::Plywood => {
            // the playfield's plies, top to bottom: long grain and cut grain in turn, glued with thin dark lines
            const PLIES: f32 = 7.0;
            let at = depth * PLIES;
            let ply = at.floor().min(PLIES - 1.0);
            let within = at - ply;
            let k = ply as u32;
            let long = k % 2 == 0;
            let base = if long { [0.83, 0.67, 0.46] } else { [0.76, 0.58, 0.38] };
            let texture = if long {
                // fibres running round the hole: slow along them, fast across
                1.0 + 0.16 * (fbm([x * 3.0, y * 3.0, depth * 260.0], kind.seed + k, 3) - 0.5)
            } else {
                // cut fibres: speckle
                1.0 + 0.22 * (fbm([x * 80.0, y * 80.0, depth * 300.0], kind.seed + k, 2) - 0.5)
            };
            let glue = if k > 0 && within < 0.07 { 0.45 } else { 1.0 };
            let shade = 1.0 - 0.55 * depth.powf(1.3);
            base.map(|c| c * texture * glue * shade)
        }
    }
}

/// Fills the texels the mesh leaves empty from their neighbours, so a
/// picture's smaller copies (its mipmaps) don't bleed dark into the edges
/// of what it covers; what's still empty takes the picture's average.
fn fill(c: &mut [Option<[f32; 3]>], w: u32, h: u32) {
    for _ in 0..16 {
        let before = c.to_vec();
        let mut changed = false;
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let i = (y as u32 * w + x as u32) as usize;
                if before[i].is_some() {
                    continue;
                }
                let (mut sum, mut count) = ([0.0f32; 3], 0);
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    if let Some(v) = before[(ny as u32 * w + nx as u32) as usize] {
                        for k in 0..3 {
                            sum[k] += v[k];
                        }
                        count += 1;
                    }
                }
                if count > 0 {
                    c[i] = Some(sum.map(|s| s / count as f32));
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let covered: Vec<[f32; 3]> = c.iter().flatten().copied().collect();
    let mean = if covered.is_empty() {
        [0.5; 3]
    } else {
        let sum = covered.iter().fold([0.0f32; 3], |s, v| [s[0] + v[0], s[1] + v[1], s[2] + v[2]]);
        sum.map(|s| s / covered.len() as f32)
    };
    for v in c.iter_mut().filter(|v| v.is_none()) {
        *v = Some(mean);
    }
}

/// A repeatable pseudo-random number in 0..1 for a lattice point.
fn hash(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ (z as u32).wrapping_mul(0xCB1A_B31F) ^ seed.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Smooth value noise in 0..1.
fn noise(p: [f32; 3], seed: u32) -> f32 {
    let i = p.map(f32::floor);
    let f = [p[0] - i[0], p[1] - i[1], p[2] - i[2]].map(|t| t * t * (3.0 - 2.0 * t));
    let [ix, iy, iz] = i.map(|v| v as i32);
    let mut sum = 0.0;
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let weight = (if dx == 1 { f[0] } else { 1.0 - f[0] }) * (if dy == 1 { f[1] } else { 1.0 - f[1] }) * (if dz == 1 { f[2] } else { 1.0 - f[2] });
                sum += weight * hash(ix + dx, iy + dy, iz + dz, seed);
            }
        }
    }
    sum
}

/// Noise over several scales, each half the last's strength, in 0..1.
fn fbm(p: [f32; 3], seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut strength, mut total, mut q) = (0.0, 1.0, 0.0, p);
    for o in 0..octaves {
        sum += strength * noise(q, seed.wrapping_add(o * 7919));
        total += strength;
        strength *= 0.5;
        q = q.map(|v| v * 2.0);
    }
    sum / total
}
