//! Decodes PinUP media files through the player's bridge to FFmpeg and reports what came out:
//! `cargo run --release -p player --example pup_media -- [--motion] <file>...`. For checking the
//! bridge without a table: each file's size, pictures and sound, and how long decoding took; with
//! `--motion`, how much each second of pictures changes (a preview video that should move: the
//! share of texels moving more than 24 levels from one picture to the next, at its most that second).

use std::time::Instant;

use player::pup::av::{Item, Media};

fn main() {
    let mut files: Vec<String> = std::env::args().skip(1).collect();
    let motion = files.iter().any(|a| a == "--motion");
    files.retain(|a| a != "--motion");
    if files.is_empty() {
        eprintln!("usage: pup_media [--motion] <file>...");
        std::process::exit(2);
    }
    if !player::pup::av::available() {
        eprintln!("FFmpeg's libraries weren't found");
        std::process::exit(1);
    }
    for f in files {
        let t0 = Instant::now();
        let Some(mut m) = Media::open(std::path::Path::new(&f), true, true, 48_000) else {
            println!("{f}: can't be opened");
            continue;
        };
        let info = m.info;
        let (mut pictures, mut sound_frames, mut last_pts, mut first) = (0u32, 0usize, 0i64, None);
        let mut alpha = false;
        // `--motion`: the picture before, and each second's most-changed share of texels
        let (mut before, mut seconds): (Vec<u8>, Vec<f64>) = (Vec::new(), Vec::new());
        while let Some(item) = m.next_item() {
            match item {
                Item::Picture { pts_ms, rgba, .. } => {
                    pictures += 1;
                    last_pts = last_pts.max(pts_ms);
                    first.get_or_insert(pts_ms);
                    alpha |= rgba.iter().skip(3).step_by(4).any(|&a| a < 255);
                    if motion {
                        if before.len() == rgba.len() {
                            let moved = rgba.chunks_exact(4).zip(before.chunks_exact(4)).filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 24)).count();
                            let share = moved as f64 / (rgba.len() / 4) as f64;
                            let second = (pts_ms.max(0) / 1000) as usize;
                            if seconds.len() <= second {
                                seconds.resize(second + 1, 0.0);
                            }
                            seconds[second] = seconds[second].max(share);
                        }
                        before.clear();
                        before.extend_from_slice(rgba);
                    }
                }
                Item::Sound { pts_ms, pcm } => {
                    sound_frames += pcm.len() / 2;
                    last_pts = last_pts.max(pts_ms);
                }
            }
        }
        println!(
            "{f}: {:?} {} fps, sound {}, {} ms long: {pictures} pictures (first at {} ms, last at {last_pts} ms{}), {:.2} s of sound, decoded in {:.0} ms",
            info.video,
            info.fps,
            info.audio,
            info.duration_ms.map_or_else(|| "?".to_string(), |d| d.to_string()),
            first.unwrap_or(0),
            if alpha { ", with see-through texels" } else { "" },
            sound_frames as f64 / 48_000.0,
            t0.elapsed().as_secs_f64() * 1000.0
        );
        if motion {
            let each: Vec<String> = seconds.iter().enumerate().map(|(s, share)| format!("{s} s {:.2}%", share * 100.0)).collect();
            println!("  moving, at its most each second: {}", each.join(", "));
        }
    }
}
