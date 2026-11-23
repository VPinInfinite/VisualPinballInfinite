//! What a picture, video or sound that came with a table is, where nothing in its name says a
//! screen as the drop's analysis reads one: guessed from its name — a screen's word a letter or
//! two off (`playfiels`), a wheel's `logo`, a screenshot's word in several languages — and from its
//! shape: a square cut out round is a wheel, a tall picture a playfield, a wide one lying on its
//! side a cabinet's playfield, 4:1 a DMD. A guess both agree on, or the name's alone, is the app's
//! guess; one from the shape alone is to check; neither, the table's own media.

use std::io::Cursor;

/// A media file's type, by its file's ending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaType {
    Picture,
    Video,
    Sound,
}

impl MediaType {
    pub fn key(self) -> &'static str {
        match self {
            Self::Picture => "picture",
            Self::Video => "video",
            Self::Sound => "sound",
        }
    }
}

/// A file's media type by its ending, if it's a picture, video or sound.
pub fn media_type(path: &str) -> Option<MediaType> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "apng" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" => Some(MediaType::Picture),
        "mp4" | "f4v" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" | "m4v" => Some(MediaType::Video),
        "mp3" | "wav" | "ogg" => Some(MediaType::Sound),
        _ => None,
    }
}

/// The kinds a guess names, as the drop's places do for the front ends' screens — `wheel`,
/// `playfield`, `backglass`, `topper`, `dmd`, `fulldmd`, `loading`, `flyer`, `card`, `audio`,
/// `launch` — and the table's `screenshot`s.
const WORDS: [(&str, &[&str]); 12] = [
    ("wheel", &["wheel", "logo", "wheels"]),
    ("playfield", &["playfield", "playfields", "pf", "table"]),
    ("backglass", &["backglass", "backglas", "bg", "b2s", "translite", "overlay"]),
    ("topper", &["topper"]),
    ("fulldmd", &["fulldmd", "menu"]),
    ("dmd", &["dmd", "dmdvideo"]),
    ("loading", &["loading", "loader", "intro"]),
    ("flyer", &["flyer", "flyers", "gameinfo", "credits"]),
    ("card", &["card", "cards", "instruction", "instructions", "rules", "gamehelp"]),
    ("launch", &["launch"]),
    ("audio", &["audio", "theme", "music"]),
    ("screenshot", &["screenshot", "screenshots", "capture", "screen", "bildschirmfoto", "schermata", "captura", "snap"]),
];

/// The edits between two short words (Levenshtein's distance), for a screen's word a letter or two
/// off.
fn edits(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(row[j + 1] + 1);
            prev = here;
        }
    }
    row[b.len()]
}

/// What a file's name says it is: a kind's word among the words of its name and the folder it's
/// in, a long one (`playfield`) a letter or two off too.
pub fn by_name(path: &str) -> Option<&'static str> {
    let low = path.replace('\\', "/").to_lowercase();
    let mut parts: Vec<&str> = low.rsplit('/').take(2).collect();
    // the file's own name first, then its folder's
    if let Some(name) = parts.first_mut() {
        *name = name.rsplit_once('.').map_or(*name, |(s, _)| s);
    }
    for part in parts {
        let words: Vec<&str> = part.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
        for (kind, said) in WORDS {
            if words.iter().any(|w| said.contains(w) || (w.len() >= 7 && said.iter().any(|s| s.len() >= 7 && edits(w, s) <= 2))) {
                return Some(kind);
            }
        }
    }
    None
}

/// A picture's size and whether it's cut out (its corners see-through).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub width: u32,
    pub height: u32,
    pub cut_out: bool,
}

/// A picture's shape from its file's bytes: its size from its header, its corners read where it
/// has an alpha channel.
pub fn shape(bytes: &[u8]) -> Option<Shape> {
    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    let mut cut_out = false;
    // corners only for one small enough to decode quickly
    if u64::from(width) * u64::from(height) <= 16_000_000
        && let Ok(img) = image::load_from_memory(bytes)
        && img.color().has_alpha()
    {
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        let corners = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)];
        cut_out = corners.iter().filter(|&&(x, y)| rgba.get_pixel(x, y)[3] < 16).count() >= 3;
    }
    Some(Shape { width, height, cut_out })
}

/// What a picture's shape says it is: a square cut out round a wheel; tall a playfield; wide and
/// big lying on its side a cabinet's playfield, else a backglass; about 4:1 a DMD.
pub fn by_shape(s: Shape) -> Option<&'static str> {
    if s.width == 0 || s.height == 0 {
        return None;
    }
    let ratio = f64::from(s.width) / f64::from(s.height);
    if (0.85..=1.18).contains(&ratio) {
        return s.cut_out.then_some("wheel");
    }
    if ratio < 0.75 {
        return Some("playfield");
    }
    if (3.4..=4.6).contains(&ratio) {
        return Some("dmd");
    }
    if (1.18..=1.45).contains(&ratio) {
        return Some("backglass");
    }
    None
}

/// Why a guess was made: `name` (its name says it, its shape agreeing or saying nothing), `shape`
/// (its shape alone: to check).
pub type Why = &'static str;

/// A file's guessed kind and why, from its name and, for a picture, its shape.
pub fn guess(path: &str, shape: Option<Shape>) -> Option<(&'static str, Why)> {
    let named = by_name(path);
    let shaped = shape.and_then(by_shape);
    match (named, shaped) {
        (Some(n), _) => Some((n, "name")),
        (None, Some(s)) => Some((s, "shape")),
        (None, None) => None,
    }
}

/// The place a guessed kind goes for the front end a drop's media go in, among the drop's places
/// (`drop::places`, by key): a video's own place where the front end keeps videos apart.
pub fn place_for(kind: &str, media: MediaType, places: &[&'static str]) -> Option<&'static str> {
    let folder = match kind {
        "wheel" => "Wheel",
        "playfield" => "PlayField",
        "backglass" => "BackGlass",
        "topper" => "Topper",
        "dmd" => "DMD",
        "fulldmd" => "Menu",
        "loading" => "Loading",
        "flyer" => "GameInfo",
        "card" => "GameHelp",
        "audio" => "Audio",
        "launch" => "AudioLaunch",
        "screenshot" => "",
        _ => return None,
    };
    let want = |name: &str| places.iter().copied().find(|k| *k == name);
    if folder.is_empty() {
        return want("screenshots");
    }
    let plain = format!("media:{folder}");
    if media == MediaType::Video
        && let Some(k) = want(&format!("{plain}Video"))
    {
        return Some(k);
    }
    want(&plain)
}

/// A picture made small to compare with another: 64 by 64 whatever its shape, in colour.
pub struct Small(image::Rgb32FImage);

impl Small {
    pub fn of(picture: &image::DynamicImage) -> Small {
        Small(picture.resize_exact(64, 64, image::imageops::FilterType::Triangle).to_rgb32f())
    }

    /// A picture's file made small, turned clockwise by `turn` degrees (0, 90 or 270) first; none
    /// when it won't read.
    pub fn of_bytes(bytes: &[u8], turn: u16) -> Option<Small> {
        let picture = image::load_from_memory(bytes).ok()?;
        let picture = match turn {
            90 => picture.rotate90(),
            270 => picture.rotate270(),
            _ => picture,
        };
        Some(Small::of(&picture))
    }

    /// Its colours laid out on a grid `w` by `h`, each colour less its average: where its light,
    /// dark and colours lie.
    fn layout(&self, w: u32, h: u32) -> Vec<f32> {
        let grid = image::imageops::resize(&self.0, w, h, image::imageops::FilterType::Triangle);
        let n = (w * h) as f32;
        (0..3)
            .flat_map(|c| {
                let mean = grid.pixels().map(|p| p[c]).sum::<f32>() / n;
                grid.pixels().map(move |p| p[c] - mean).collect::<Vec<f32>>()
            })
            .collect()
    }
}

/// A playfield's grids, tall as it stands; a backglass's, a little wider than tall.
pub const PLAYFIELD_GRIDS: [(u32, u32); 3] = [(3, 6), (6, 12), (12, 24)];
pub const BACKGLASS_GRIDS: [(u32, u32); 3] = [(6, 5), (12, 10), (24, 20)];

/// How alike two pictures are in where their colours lie, -1 to 1: their correlation on each grid,
/// averaged — a front end's picture and a table's own differ in their detail, less in where their
/// colours lie.
pub fn alike(a: &Small, b: &Small, grids: &[(u32, u32)]) -> f32 {
    let one = |w: u32, h: u32| {
        let (x, y) = (a.layout(w, h), b.layout(w, h));
        let dot: f32 = x.iter().zip(&y).map(|(p, q)| p * q).sum();
        let norm = (x.iter().map(|p| p * p).sum::<f32>() * y.iter().map(|q| q * q).sum::<f32>()).sqrt();
        if norm > 0.0 { dot / norm } else { 0.0 }
    };
    grids.iter().map(|&(w, h)| one(w, h)).sum::<f32>() / grids.len().max(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How alike real pictures are to tables' own playfields and backglasses, printed:
    /// `VPI_LIKEN_TABLES` names table files (`;` between them; each one's backglass file beside
    /// it), `VPI_LIKEN_PICTURES` a folder of pictures. `cargo test --release -p library
    /// likeness_on_real_pictures -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn likeness_on_real_pictures() {
        let (Ok(tables), Ok(folder)) = (std::env::var("VPI_LIKEN_TABLES"), std::env::var("VPI_LIKEN_PICTURES")) else { return };
        let refs: Vec<(String, Option<Small>, Option<Small>)> = tables
            .split(';')
            .map(|t| {
                let path = std::path::Path::new(t.trim());
                let pf = crate::table::playfield_picture(path).ok().flatten().and_then(|b| Small::of_bytes(&b, 0));
                let bg = std::fs::read(path.with_extension("directb2s")).ok().and_then(|text| crate::backglass_dmd::picture_for(&text, crate::backglass_dmd::Target::Backglass)).and_then(|b| Small::of_bytes(&b, 0));
                (path.file_stem().unwrap().to_string_lossy().into_owned(), pf, bg)
            })
            .collect();
        for (name, pf, bg) in &refs {
            println!("{name}: its playfield {}, its backglass {}", pf.is_some(), bg.is_some());
        }
        let mut files: Vec<_> = std::fs::read_dir(&folder).unwrap().flatten().map(|e| e.path()).collect();
        files.sort();
        for f in files {
            let Ok(bytes) = std::fs::read(&f) else { continue };
            let Ok(picture) = image::load_from_memory(&bytes) else { continue };
            let wide = picture.width() > picture.height();
            let turns: &[u16] = if wide { &[90, 270] } else { &[0] };
            let turned: Vec<Small> = turns.iter().filter_map(|&t| Small::of_bytes(&bytes, t)).collect();
            let flat = Small::of(&picture);
            let mut line = format!("{:42}", f.file_name().unwrap().to_string_lossy().chars().take(40).collect::<String>());
            for (name, pf, bg) in &refs {
                let p = pf.as_ref().map(|r| turned.iter().map(|s| alike(s, r, &PLAYFIELD_GRIDS)).fold(f32::MIN, f32::max));
                let b = bg.as_ref().map(|r| alike(&flat, r, &BACKGLASS_GRIDS));
                line += &format!("  {}: p{} b{}", &name.chars().take(10).collect::<String>(), p.map_or("  -  ".into(), |v| format!("{v:+.2}")), b.map_or("  -  ".into(), |v| format!("{v:+.2}")));
            }
            println!("{line}");
        }
    }

    #[test]
    fn what_a_name_says() {
        assert_eq!(by_name("Medias/playfiels_Jurassic park pro.png"), Some("playfield"));
        assert_eq!(by_name("media/overlayJP.png"), Some("backglass"));
        assert_eq!(by_name("media/Capture d’écran (179).png"), Some("screenshot"));
        assert_eq!(by_name("Jaws/wheel.png"), Some("wheel"));
        assert_eq!(by_name("stuff/Logo.png"), Some("wheel"));
        // a name naming no screen, in a folder naming none
        assert_eq!(by_name("media/jurassic park limited edition.png"), None);
        assert_eq!(by_name("Medias/jurassic.png"), None);
        // the folder's word when the file's says nothing
        assert_eq!(by_name("Wheel/jurassic.png"), Some("wheel"));
    }

    #[test]
    fn what_a_shape_says() {
        assert_eq!(by_shape(Shape { width: 512, height: 512, cut_out: true }), Some("wheel"));
        assert_eq!(by_shape(Shape { width: 512, height: 512, cut_out: false }), None);
        assert_eq!(by_shape(Shape { width: 1080, height: 1920, cut_out: false }), Some("playfield"));
        assert_eq!(by_shape(Shape { width: 1280, height: 320, cut_out: false }), Some("dmd"));
        assert_eq!(by_shape(Shape { width: 1280, height: 1024, cut_out: false }), Some("backglass"));
        // 16:9 lying wide: a screenshot or a playfield on its side, not told by its shape
        assert_eq!(by_shape(Shape { width: 1920, height: 1080, cut_out: false }), None);
        assert_eq!(guess("media/jurassic.png", Some(Shape { width: 600, height: 600, cut_out: true })), Some(("wheel", "shape")));
        assert_eq!(guess("media/wheel.png", Some(Shape { width: 600, height: 600, cut_out: true })), Some(("wheel", "name")));
    }

    #[test]
    fn a_guess_placed_for_the_front_end() {
        let popper = ["media:Wheel", "media:PlayField", "media:BackGlass", "media:Audio", "screenshots", "tableMedia"];
        assert_eq!(place_for("playfield", MediaType::Video, &popper), Some("media:PlayField"));
        assert_eq!(place_for("screenshot", MediaType::Picture, &popper), Some("screenshots"));
        let pinballx = ["media:PlayField", "media:PlayFieldVideo", "media:Wheel"];
        assert_eq!(place_for("playfield", MediaType::Video, &pinballx), Some("media:PlayFieldVideo"));
        assert_eq!(place_for("playfield", MediaType::Picture, &pinballx), Some("media:PlayField"));
        assert_eq!(place_for("topper", MediaType::Picture, &pinballx), None);
        assert_eq!(media_type("a/b.MP4"), Some(MediaType::Video));
        assert_eq!(media_type("a/b.txt"), None);
    }
}
