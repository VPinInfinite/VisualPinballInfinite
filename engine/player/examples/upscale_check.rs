//! The DMD upscaler's check against VPX 5436's own scalers
//! (`tools/upscale-oracle`, which its `run.ps1` runs):
//!
//! `cargo run --release -p player --example upscale_check -- --write <folder> [pictures...]`
//! writes the test frames for the oracle: some drawn here, and the DMD
//! pictures given (the headless runner's `--dmd` pictures, four pixels a dot).
//!
//! `cargo run --release -p player --example upscale_check -- --compare <report.md> <title>=<folder>...`
//! compares what the port's scalers make of each frame with what the oracle
//! made in each folder, word for word, and writes the report; the first
//! folder is the check (the exit code says whether every frame matched), the
//! others are reported only.

use std::path::{Path, PathBuf};

use player::dmd::DmdFrame;
use player::upscale::{self, Mode};

const MODES: [Mode; 9] = [Mode::ScaleFxAa, Mode::ScaleFx3x, Mode::Mmpx2x, Mode::Xbrz2x, Mode::Xbrz3x, Mode::Xbrz4x, Mode::Xbrz5x, Mode::Xbrz6x, Mode::SuperXbr2x];

/// A test frame's dots: brightness (linear, a float a dot) or colour (sRGB bytes).
enum Dots {
    Brightness(Vec<f32>),
    Colour(Vec<[u8; 3]>),
}

struct Frame {
    name: String,
    width: u32,
    height: u32,
    dots: Dots,
}

impl Frame {
    fn write(&self, folder: &Path) -> std::io::Result<()> {
        let mut out = b"VPIU".to_vec();
        out.extend(self.width.to_le_bytes());
        out.extend(self.height.to_le_bytes());
        match &self.dots {
            Dots::Brightness(d) => {
                out.extend(0u32.to_le_bytes());
                d.iter().for_each(|f| out.extend(f.to_le_bytes()));
            }
            Dots::Colour(d) => {
                out.extend(1u32.to_le_bytes());
                d.iter().for_each(|c| out.extend(c));
            }
        }
        std::fs::write(folder.join(format!("{}.vpiu", self.name)), out)
    }

    fn read(path: &Path) -> Option<Frame> {
        let bytes = std::fs::read(path).ok()?;
        let word = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if bytes.get(..4)? != b"VPIU" {
            return None;
        }
        let (width, height, kind) = (word(4)?, word(8)?, word(12)?);
        let n = (width * height) as usize;
        let dots = if kind == 0 {
            Dots::Brightness((0..n).map(|i| word(16 + 4 * i).map(f32::from_bits)).collect::<Option<_>>()?)
        } else {
            Dots::Colour((0..n).map(|i| bytes.get(16 + 3 * i..19 + 3 * i).map(|c| [c[0], c[1], c[2]])).collect::<Option<_>>()?)
        };
        let name = path.file_stem()?.to_string_lossy().into_owned();
        Some(Frame { name, width, height, dots })
    }

    /// As the player's display holds it.
    fn dmd(&self) -> DmdFrame {
        match &self.dots {
            Dots::Brightness(d) => DmdFrame::with_dots(self.width, self.height, d.iter().map(|&f| [f, f, f, 1.0]), true, 1),
            Dots::Colour(d) => {
                let values: Vec<u32> = d.iter().map(|c| u32::from(c[0]) | (u32::from(c[1]) << 8) | (u32::from(c[2]) << 16)).collect();
                DmdFrame::from_srgb(self.width, self.height, &values, 1)
            }
        }
    }
}

/// Frames drawn here: the cases a scaler's rules turn on.
fn drawn() -> Vec<Frame> {
    let mut seed = 0x2545_F491u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed >> 8
    };
    let (width, height) = (128u32, 32u32);
    let n = (width * height) as usize;
    let at = |i: usize| ((i % width as usize) as i32, (i / width as usize) as i32);
    let frame = |name: &str, dots: Dots| Frame { name: name.into(), width, height, dots };
    // a 2-bit DMD's four shades, at random
    let shades = frame("drawn-shades", Dots::Brightness((0..n).map(|_| (next() % 4) as f32 / 3.0).collect()));
    // lines of several slopes, and blocks
    let lines = frame(
        "drawn-lines",
        Dots::Brightness(
            (0..n)
                .map(|i| {
                    let (x, y) = at(i);
                    let lit = x == y || x == 40 + 2 * y || 2 * (x - 80) == y || x - 100 == 3 * (y / 3) || ((112..120).contains(&x) && (4..12).contains(&y));
                    if lit { 1.0 } else { 0.0 }
                })
                .collect(),
        ),
    );
    // any brightness at all, through VPX's sRGB rounding
    let any = frame("drawn-any-brightness", Dots::Brightness((0..n).map(|_| (next() % 100_001) as f32 / 100_000.0).collect()));
    // a few colours at random, and colour blocks with steps
    const PALETTE: [[u8; 3]; 6] = [[0, 0, 0], [255, 0, 0], [255, 170, 0], [0, 90, 255], [255, 255, 255], [40, 40, 40]];
    let palette = frame("drawn-palette", Dots::Colour((0..n).map(|_| PALETTE[(next() % 6) as usize]).collect()));
    let steps = frame(
        "drawn-steps",
        Dots::Colour(
            (0..n)
                .map(|i| {
                    let (x, y) = at(i);
                    PALETTE[((x / 9 + y / 5 + (x + y) / 13) % 6) as usize]
                })
                .collect(),
        ),
    );
    vec![shades, lines, any, palette, steps]
}

/// A DMD picture the headless runner saved (four pixels a dot): grey ones
/// as brightness (a byte's share of 255, as PinMAME's), coloured ones as colour.
fn from_picture(path: &Path) -> Option<Frame> {
    const SCALE: u32 = 4;
    let picture = image::open(path).ok()?;
    let (width, height) = (picture.width() / SCALE, picture.height() / SCALE);
    let name: String = path.file_stem()?.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let dots = match picture {
        image::DynamicImage::ImageLuma8(g) => Dots::Brightness((0..height).flat_map(|y| (0..width).map(move |x| (x, y))).map(|(x, y)| f32::from(g.get_pixel(x * SCALE, y * SCALE)[0]) / 255.0).collect()),
        other => {
            let rgb = other.to_rgb8();
            Dots::Colour((0..height).flat_map(|y| (0..width).map(move |x| (x, y))).map(|(x, y)| rgb.get_pixel(x * SCALE, y * SCALE).0).collect())
        }
    };
    Some(Frame { name, width, height, dots })
}

fn write(folder: &Path, pictures: &[String]) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut frames = drawn();
    for p in pictures {
        match from_picture(Path::new(p)) {
            Some(f) => frames.push(f),
            None => eprintln!("{p}: not a picture"),
        }
    }
    for f in &frames {
        f.write(folder).map_err(|e| format!("{}: {e}", f.name))?;
    }
    println!("{} frames in {}", frames.len(), folder.display());
    Ok(())
}

/// The oracle's words for `frame` in `mode`, as it wrote them beside the frame.
fn oracle(frame: &Path, mode: Mode) -> Option<(u32, u32, Vec<u32>)> {
    let bytes = std::fs::read(format!("{}.mode{}.out", frame.display(), mode as usize)).ok()?;
    let word = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if bytes.get(..4)? != b"VPIO" {
        return None;
    }
    let (w, h) = (word(4)?, word(8)?);
    let words = (0..(w * h) as usize).map(|i| word(12 + 4 * i)).collect::<Option<_>>()?;
    Some((w, h, words))
}

fn compare(report: &Path, folders: &[String]) -> Result<bool, String> {
    let mut md = vec![
        "# The DMD upscaler against VPX 5436's own scalers".to_string(),
        String::new(),
        "Generated by `tools/upscale-oracle/run.ps1`: each test frame through the player's port of VPX 5436's DMD upscaler (`engine/player/src/upscale/`) and through VPX's own scalers (`plugins/upscaledmd`, built by the oracle), the words each scaler made compared one for one — \"same\" when every one matches.".to_string(),
        String::new(),
        "The first section is the check: VPX's code compiled with strict floating point, each sum as C++ defines it, which the port must match word for word. The second compiles it as VPX builds its plugin (`/fp:fast`, which lets the compiler reorder sums, so a rounding can differ and now and then flip a scaler's choice of edge): there a few words may differ, which no port can reproduce.".to_string(),
    ];
    let mut all_same = true;
    // the port's time per frame, by mode and the frame's size: (total, count)
    let mut times: std::collections::BTreeMap<(usize, (u32, u32)), (std::time::Duration, u32)> = Default::default();
    for (n, spec) in folders.iter().enumerate() {
        let (title, folder) = spec.split_once('=').ok_or_else(|| format!("{spec}: expected <title>=<folder>"))?;
        let mut paths: Vec<PathBuf> = std::fs::read_dir(folder).map_err(|e| format!("{folder}: {e}"))?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "vpiu")).collect();
        paths.sort();
        md.push(String::new());
        md.push(format!("## {title}"));
        md.push(String::new());
        md.push(format!("| frame | size | {} |", MODES.map(Mode::name).join(" | ")));
        md.push(format!("|---|---|{}", "---|".repeat(MODES.len())));
        let (mut frames, mut same) = (0, 0);
        for path in &paths {
            let Some(frame) = Frame::read(path) else { continue };
            let dmd = frame.dmd();
            let mut cells = Vec::new();
            for mode in MODES {
                let begun = std::time::Instant::now();
                let ours = upscale::scale_words(&upscale::source_words(&dmd, mode), frame.width as usize, frame.height as usize, dmd.luminance, mode).unwrap_or_default();
                if n == 0 {
                    let t = times.entry((mode as usize, (frame.width, frame.height))).or_default();
                    t.0 += begun.elapsed();
                    t.1 += 1;
                }
                let cell = match oracle(path, mode) {
                    None => "no output".to_string(),
                    Some((w, h, theirs)) if (w, h) != (frame.width * mode.factor(), frame.height * mode.factor()) || theirs.len() != ours.len() => format!("{w} × {h}, not ours"),
                    Some((w, _, theirs)) => {
                        let differ: Vec<usize> = (0..ours.len()).filter(|&i| ours[i] != theirs[i]).collect();
                        if let Some(&i) = differ.first() {
                            let (x, y) = (i as u32 % w, i as u32 / w);
                            eprintln!("{title}: {} {}: {} of {} words differ, the first at {x},{y}: ours {:08x}, VPX's {:08x}", frame.name, mode.name(), differ.len(), ours.len(), ours[i], theirs[i]);
                        }
                        if differ.is_empty() { "same".to_string() } else { format!("{} of {} differ", differ.len(), ours.len()) }
                    }
                };
                cells.push(cell);
            }
            frames += 1;
            if cells.iter().all(|c| c == "same") {
                same += 1;
            } else if n == 0 {
                all_same = false;
            }
            md.push(format!("| {} | {} × {} | {} |", frame.name, frame.width, frame.height, cells.join(" | ")));
        }
        md.push(String::new());
        md.push(format!("**{same} of {frames} frames the same in every mode.**"));
        println!("{title}: {same} of {frames} frames the same in every mode");
    }
    md.push(String::new());
    md.push("## The port's time per frame".to_string());
    md.push(String::new());
    md.push("On this machine, one thread, release build: the mean over the frames of each size above.".to_string());
    md.push(String::new());
    md.push("| mode | frame | ms |".to_string());
    md.push("|---|---|---|".to_string());
    for ((mode, (w, h)), (total, count)) in &times {
        md.push(format!("| {} | {w} × {h} | {:.2} |", Mode::ALL[*mode].name(), total.as_secs_f64() * 1000.0 / f64::from(*count)));
    }
    md.push(String::new());
    std::fs::write(report, md.join("\n")).map_err(|e| format!("{}: {e}", report.display()))?;
    Ok(all_same)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--write") if args.len() >= 2 => write(Path::new(&args[1]), &args[2..]).map(|()| true),
        Some("--compare") if args.len() >= 3 => compare(Path::new(&args[1]), &args[2..]),
        _ => Err("usage: upscale_check --write <folder> [pictures...] | --compare <report.md> <title>=<folder>...".into()),
    };
    match result {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}
