//! The front ends a library's tables are shown in, beside PinUP Popper (`popper.rs`): PinballX, and
//! PinballY, made to read PinballX's media and game lists. Each is found in its usual folders and
//! counts only where one of its systems plays the library's tables folder (PinballX's
//! `Config\PinballX.ini`, PinballY's `Settings.txt`); that system's media are in folders named for
//! their screens (`Media\Visual Pinball\Wheel Images` and the rest; instruction cards, flyers and
//! loading videos in folders every system shares), each named for its table as the front end names
//! it: its description in the system's game list (`Databases\<system>\*.xml`), made safe for a
//! file's name as PinballY makes it, else the table file's name. The one named so is the one it
//! shows; others of a kind are kept numbered beside it as PinballY numbers its instruction cards
//! (`<name> 1.png`). The folders and names follow PinballY's own source (GPL-3.0) and VPin Studio's
//! (MIT); nothing of theirs is copied. Ours.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::plan::Layout;

/// A front end whose media folders the app reads and puts media in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Front {
    Popper,
    PinballX,
    PinballY,
}

impl Front {
    /// Its name in words.
    pub fn name(self) -> &'static str {
        match self {
            Front::Popper => "PinUP Popper",
            Front::PinballX => "PinballX",
            Front::PinballY => "PinballY",
        }
    }

    /// Its name as the screens and the user's records keep it (`Popper`, `PinballX`, `PinballY`).
    pub fn key(self) -> &'static str {
        match self {
            Front::Popper => "Popper",
            Front::PinballX => "PinballX",
            Front::PinballY => "PinballY",
        }
    }

    pub fn from_key(key: &str) -> Option<Front> {
        [Front::Popper, Front::PinballX, Front::PinballY].into_iter().find(|f| f.key().eq_ignore_ascii_case(key))
    }
}

impl Default for Front {
    fn default() -> Front {
        Front::Popper
    }
}

/// PinballX or PinballY, as found for the library.
#[derive(Clone, Debug)]
pub struct FrontEnd {
    pub front: Front,
    /// Its own folder.
    pub folder: PathBuf,
    /// The system playing the library's tables, as its settings name it (`Visual Pinball`).
    pub system: String,
    /// The media folder every system shares (`Media`): instruction cards, flyers, loading videos.
    pub media_root: PathBuf,
    /// The system's own media folder (`Media\Visual Pinball`).
    pub media: PathBuf,
    /// The system's game lists' folder (`Databases\Visual Pinball`), each `.xml` in it; None where
    /// its settings name none.
    pub databases: Option<PathBuf>,
}

/// Each kind's folders, as `installed::Media` names the kinds (PinUP Popper's own names), the
/// videos' first; `../` the media folder every system shares. PinballY shows neither a menu's
/// (full DMD's) videos nor loading videos; PinballX does.
const KINDS: [(&str, &[&str]); 9] = [
    ("Wheel", &["Wheel Images"]),
    ("PlayField", &["Table Videos", "Table Images"]),
    ("BackGlass", &["Backglass Videos", "Backglass Images"]),
    ("DMD", &["DMD Videos", "DMD Images"]),
    ("Topper", &["Topper Videos", "Topper Images"]),
    ("Menu", &["FullDMD Videos"]),
    ("Loading", &["../Loading Videos"]),
    ("GameHelp", &["../Instruction Cards"]),
    ("GameInfo", &["../Flyer Images/Front", "../Flyer Images/Inside1", "../Flyer Images/Inside2", "../Flyer Images/Inside3", "../Flyer Images/Inside4", "../Flyer Images/Inside5", "../Flyer Images/Inside6", "../Flyer Images/Back"]),
];

/// Pictures and videos as PinballY reads them, and the videos the app plays of them.
pub const PICTURES: [&str; 5] = ["png", "jpg", "jpeg", "gif", "apng"];
pub const VIDEOS: [&str; 8] = ["f4v", "mp4", "mpg", "mkv", "wmv", "m4v", "avi", "mov"];
const PLAYED: [&str; 4] = ["mp4", "m4v", "mov", "f4v"];

fn ext_of(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

/// The folder a front end's file of `kind` (Popper's name for it) goes in: a video's or a
/// picture's, by `video`; None for a kind it has no folder for.
pub fn folder_for(front: Front, kind: &str, video: bool) -> Option<&'static str> {
    let (_, folders) = KINDS.iter().find(|(k, _)| k.eq_ignore_ascii_case(kind))?;
    if front == Front::PinballY && matches!(*folders, [f] if f.contains("FullDMD") || f.contains("Loading")) {
        return None;
    }
    match folders {
        [videos, pictures] => Some(if video { videos } else { pictures }),
        // one folder for a kind of one family: a wheel's and the cards' are pictures, the menu's
        // and the loading's videos
        [one, ..] => {
            let videos_only = one.contains("Videos");
            (videos_only == video).then_some(*one)
        }
        [] => None,
    }
}

/// `chosen` made the one PinballX or PinballY shows for a kind of a game's media: put in front in
/// its `folder` for the kind, named for its name for the game (`name`), the one it showed there
/// kept beside it, numbered as they number a kind's others (`Name 1.png`), never deleted — but a
/// copy of VPS's or ours put in before, which `recycle` sends away (`popper::put_in_front_as`); a
/// picture it may not show (VPS's WebP) as a PNG.
pub fn put_in_front(folder: &Path, name: &str, chosen: &Path, ours: &dyn Fn(&Path) -> bool, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<Option<PathBuf>, String> {
    crate::popper::put_in_front_as(folder, name, chosen, &|name, n| format!("{name} {n}"), ours, recycle)
}

/// The audio folders, by Popper's names for them.
pub fn audio_folder(kind: &str) -> Option<&'static str> {
    match kind {
        "Audio" => Some("Table Audio"),
        "AudioLaunch" => Some("Launch Audio"),
        _ => None,
    }
}

/// The folders the drop window offers for a file nothing placed: its key, the folder in words,
/// Popper's name for its kind, and whether it holds videos.
pub const PLACES: [(&str, &str, &str, bool); 13] = [
    ("media:Wheel", "wheel", "Wheel", false),
    ("media:PlayFieldVideo", "playfield videos", "PlayField", true),
    ("media:PlayField", "playfield pictures", "PlayField", false),
    ("media:BackGlassVideo", "backglass videos", "BackGlass", true),
    ("media:BackGlass", "backglass pictures", "BackGlass", false),
    ("media:DMDVideo", "DMD videos", "DMD", true),
    ("media:DMD", "DMD pictures", "DMD", false),
    ("media:TopperVideo", "topper videos", "Topper", true),
    ("media:Topper", "topper pictures", "Topper", false),
    ("media:Menu", "full DMD videos", "Menu", true),
    ("media:Loading", "loading videos", "Loading", true),
    ("media:GameHelp", "instruction cards", "GameHelp", false),
    ("media:GameInfo", "flyers", "GameInfo", false),
];

/// One of PinballX's or PinballY's media folders, by its name: a file put in it is named for its
/// table as they name media.
pub fn is_media_folder(folder: &Path) -> bool {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let own = name(folder);
    let flyer = folder.parent().is_some_and(|p| name(p) == "flyer images");
    KINDS.iter().flat_map(|(_, f)| f.iter()).chain(["Table Audio", "Launch Audio"].iter()).any(|f| {
        let last = f.rsplit('/').next().unwrap_or(f).to_lowercase();
        last == own && (!f.contains("Flyer Images") || flyer)
    })
}

/// The name a table's media go under where a drop's media go in PinballX's or PinballY's folders:
/// its description in their game list, else its file's name (`stem`); None where they go in PinUP
/// Popper's, or nowhere.
pub fn media_name_for(layout: &Layout, stem: &str) -> Option<String> {
    match media_target(layout)? {
        Target::Other(fe) => Some(fe.media_names().get(&stem.to_lowercase()).cloned().unwrap_or_else(|| stem.to_string())),
        Target::Popper(_) => None,
    }
}

/// Text decoded as each writes it: UTF-16 with its mark (PinballX's settings), UTF-8 with its mark
/// or without, else Windows' own Western encoding (older game lists).
fn decode(bytes: &[u8]) -> String {
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
        },
    }
}

fn read_text(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| decode(&b))
}

/// Two folders the same one, case and a trailing separator aside, or as Windows resolves them.
fn same_folder(a: &Path, b: &Path) -> bool {
    let plain = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    plain(a) == plain(b) || matches!((dunce(a), dunce(b)), (Some(x), Some(y)) if x == y)
}

fn dunce(p: &Path) -> Option<String> {
    let c = std::fs::canonicalize(p).ok()?;
    Some(c.to_string_lossy().trim_start_matches(r"\\?\").trim_end_matches('\\').to_lowercase())
}

/// The folders to look in for each front end: the ones the settings name, then beside the
/// library's VPX folder and the folder above it (`C:\vPinball\PinballX`, as VPX's installers lay
/// them out), the drive's top and Program Files. A folder is the front end its files say it is.
pub fn usual_places(tables: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let names = ["PinballX", "PinballY"];
    for up in tables.ancestors().skip(1).take(2) {
        out.extend(names.iter().map(|n| up.join(n)));
    }
    out.extend(names.iter().map(|n| PathBuf::from(format!(r"C:\{n}"))));
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(pf) = std::env::var_os(var) {
            out.extend(names.iter().map(|n| PathBuf::from(&pf).join(n)));
        }
    }
    out
}

/// The front ends found among `layout.front_ends`, each once, that play the library's tables.
pub fn find(layout: &Layout) -> Vec<FrontEnd> {
    let mut found: Vec<FrontEnd> = Vec::new();
    let pinballx_folder = layout.front_ends.iter().find(|f| f.join("Config").join("PinballX.ini").is_file()).cloned();
    for folder in &layout.front_ends {
        if found.iter().any(|f| same_folder(&f.folder, folder)) {
            continue;
        }
        let fe = if folder.join("Config").join("PinballX.ini").is_file() {
            pinballx(folder, &layout.tables)
        } else if folder.join("Settings.txt").is_file() {
            pinbally(folder, &layout.tables, pinballx_folder.as_deref())
        } else {
            None
        };
        if let Some(fe) = fe.filter(|fe| !found.iter().any(|f| f.front == fe.front)) {
            found.push(fe);
        }
    }
    found
}

/// One system of a front end's settings.
struct System {
    name: String,
    enabled: bool,
    tables: Option<PathBuf>,
    media_dir: String,
    database_dir: String,
}

/// The system playing `tables`, an enabled one before one switched off.
fn playing(systems: Vec<System>, tables: &Path) -> Option<System> {
    let mut playing: Vec<System> = systems.into_iter().filter(|s| s.tables.as_deref().is_some_and(|t| same_folder(t, tables))).collect();
    playing.sort_by_key(|s| !s.enabled);
    playing.into_iter().next()
}

/// PinballX at `folder`: its settings' Visual Pinball systems (`[VisualPinball]`, and
/// `[System_<n>]` of type 1), the one playing `tables`.
fn pinballx(folder: &Path, tables: &Path) -> Option<FrontEnd> {
    let ini = vpi_settings::parse_ini(&read_text(&folder.join("Config").join("PinballX.ini"))?);
    let get = |section: &str, name: &str| vpi_settings::get(&ini, section, name).map(str::to_string);
    let mut sections: Vec<(String, String)> = vec![("VisualPinball".into(), "Visual Pinball".into())];
    for n in 1..20 {
        let section = format!("System_{n}");
        if get(&section, "SystemType").is_some_and(|t| t.trim() == "1") {
            sections.push((section.clone(), get(&section, "Name").unwrap_or(section)));
        }
    }
    let systems: Vec<System> = sections
        .into_iter()
        .filter(|(s, _)| ini.iter().any(|(sec, _, _)| sec.eq_ignore_ascii_case(s)))
        .map(|(section, name)| System {
            enabled: get(&section, "Enabled").is_none_or(|e| !e.eq_ignore_ascii_case("false")),
            tables: get(&section, "TablePath").map(PathBuf::from),
            media_dir: name.clone(),
            database_dir: name.clone(),
            name,
        })
        .collect();
    let system = playing(systems, tables)?;
    let media_root = folder.join("Media");
    Some(FrontEnd {
        front: Front::PinballX,
        folder: folder.to_path_buf(),
        media: media_root.join(&system.media_dir),
        media_root,
        databases: Some(folder.join("Databases").join(&system.database_dir)),
        system: system.name,
    })
}

/// PinballY's settings, `name = value` lines, `#` starting a comment.
fn pinbally_settings(text: &str) -> HashMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .collect()
}

/// A path in PinballY's settings: one of its own folder's, or of PinballX's where it starts
/// `[PinballX]`, or whole; None where it's left empty (PinballY asks for it when it starts).
fn pinbally_path(value: Option<&String>, folder: &Path, pinballx: Option<&Path>) -> Option<PathBuf> {
    let value = value.map(|v| v.trim()).filter(|v| !v.is_empty())?;
    let path = match value.get(..10) {
        Some(head) if head.eq_ignore_ascii_case("[PinballX]") => pinballx?.join(value[10..].trim_start_matches(['\\', '/'])),
        _ => PathBuf::from(value),
    };
    Some(if path.is_absolute() { path } else { folder.join(path) })
}

/// PinballY at `folder`: its settings' Visual Pinball X systems (`System<n>` of class VPX, or
/// playing `.vpx` files), the one playing `tables` — a relative table folder being one of the
/// system's program's, VPX's own beside the tables where it names no program by its path.
fn pinbally(folder: &Path, tables: &Path, pinballx: Option<&Path>) -> Option<FrontEnd> {
    let s = pinbally_settings(&read_text(&folder.join("Settings.txt"))?);
    let media_root = pinbally_path(s.get("mediapath"), folder, pinballx)?;
    let databases_root = pinbally_path(s.get("tabledatabasepath"), folder, pinballx);
    let mut numbers: Vec<u32> = s.keys().filter_map(|k| k.strip_prefix("system")?.parse::<u32>().ok()).collect();
    numbers.sort_unstable();
    let systems: Vec<System> = numbers
        .into_iter()
        .filter_map(|n| {
            let get = |what: &str| s.get(&format!("system{n}.{what}")).map(|v| v.trim()).filter(|v| !v.is_empty());
            let name = s.get(&format!("system{n}"))?.trim().to_string();
            let vpx = get("class").is_some_and(|c| c.eq_ignore_ascii_case("VPX")) || get("defext").is_some_and(|e| e.eq_ignore_ascii_case(".vpx"));
            if name.is_empty() || !vpx {
                return None;
            }
            let table_path = get("tablepath").map(PathBuf::from).map(|p| {
                if p.is_absolute() {
                    p
                } else {
                    let program = get("exe").map(PathBuf::from).filter(|e| e.is_absolute()).and_then(|e| e.parent().map(Path::to_path_buf));
                    program.or_else(|| tables.parent().map(Path::to_path_buf)).unwrap_or_default().join(p)
                }
            });
            Some(System {
                enabled: get("enabled").is_none_or(|e| !(e.eq_ignore_ascii_case("false") || e == "0")),
                tables: table_path,
                media_dir: get("mediadir").unwrap_or(&name).to_string(),
                database_dir: get("databasedir").unwrap_or(&name).to_string(),
                name,
            })
        })
        .collect();
    let system = playing(systems, tables)?;
    Some(FrontEnd {
        front: Front::PinballY,
        folder: folder.to_path_buf(),
        media: media_root.join(&system.media_dir),
        media_root,
        databases: databases_root.map(|d| d.join(&system.database_dir)),
        system: system.name,
    })
}

/// A description made a file's name, as PinballY makes its media's names (`CleanMediaName`).
pub fn clean_media_name(name: &str) -> String {
    name.chars()
        .filter_map(|c| match c {
            '<' => Some('('),
            '>' => Some(')'),
            ':' | '?' => None,
            '/' | '|' | '\\' => Some(';'),
            '*' => Some('+'),
            '"' => Some('\''),
            c => Some(c),
        })
        .collect()
}

/// One game of a front end's game lists.
#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    /// Its table file's name, without its extension as the lists keep it.
    pub name: String,
    /// The name its media go under, from its description.
    pub media_name: Option<String>,
    /// The list it's in.
    pub list: PathBuf,
    /// Its rating as the list keeps it (`<rating>`): whole stars, 0 none.
    pub rating: f64,
    /// Its description, title, maker and year as the list gives them: PinballY names a game by
    /// them in its stats (`id_in_pinbally`).
    pub description: String,
    pub title: Option<String>,
    pub manufacturer: Option<String>,
    pub year: Option<i32>,
}

/// The games of one game list's text (`<menu><game name="…"><description>…`).
pub fn games_in(text: &str, list: &Path) -> Vec<Game> {
    let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let Ok(doc) = roxmltree::Document::parse_with_options(text, options) else { return Vec::new() };
    doc.descendants()
        .filter(|n| n.has_tag_name("game"))
        .filter_map(|g| {
            let name = g.attribute("name")?.trim().to_string();
            let child = |tag: &str| g.children().find(|c| c.has_tag_name(tag)).and_then(|d| d.text()).map(str::trim).filter(|d| !d.is_empty());
            let description = child("description");
            Some(Game {
                name,
                media_name: description.map(clean_media_name),
                list: list.to_path_buf(),
                rating: child("rating").and_then(|r| r.parse().ok()).unwrap_or(0.0),
                description: description.unwrap_or_default().to_string(),
                title: child("title").map(str::to_string),
                manufacturer: child("manufacturer").map(str::to_string),
                year: child("year").and_then(|y| y.parse().ok()).filter(|y| *y != 0),
            })
        })
        .collect()
}

/// A game's names in PinballY's stats (`GameStats.csv`, its `Game` column), as PinballY makes them
/// (`GameListItem::GetGameId`): its title, maker and year — the list's own, else read off its
/// description's `(Maker Year)` — and the system's name: `Title (Maker Year).System`; then its name
/// before PinballY's 1.0 beta 3, `Title.System`, which PinballY still finds.
pub fn id_in_pinbally(g: &Game, system: &str) -> [String; 2] {
    let desc = g.description.as_str();
    let mut title = String::new();
    let mut maker = g.manufacturer.clone().unwrap_or_default();
    let mut year = g.year.unwrap_or(0);
    // a parenthesised suffix at its end, nested parentheses respected
    let trimmed = desc.trim_end();
    if let Some(inner_end) = trimmed.strip_suffix(')').map(str::len) {
        let bytes = trimmed.as_bytes();
        let (mut level, mut at) = (1, inner_end);
        while level > 0 && at > 0 {
            at -= 1;
            match bytes[at] {
                b'(' => level -= 1,
                b')' => level += 1,
                _ => {}
            }
        }
        if level == 0 {
            let before = trimmed[..at].trim();
            let suffix = &trimmed[at + 1..inner_end];
            if !before.is_empty() {
                let words: Vec<&str> = suffix.split_whitespace().collect();
                let four_digits = |w: &str| w.len() == 4 && w.bytes().all(|b| b.is_ascii_digit());
                title = before.to_string();
                match words.as_slice() {
                    // (Maker Year)
                    [.., last] if words.len() > 1 && four_digits(last) => {
                        if maker.is_empty() {
                            maker = words[..words.len() - 1].join(" ");
                        }
                        if year == 0 {
                            year = last.parse().unwrap_or(0);
                        }
                    }
                    // (Year): unless it's the maker's own name
                    [only] if four_digits(only) => {
                        if year == 0 && (maker.is_empty() || *only != maker) {
                            year = only.parse().unwrap_or(0);
                        }
                    }
                    // (Maker)
                    _ => {
                        if maker.is_empty() {
                            maker = suffix.trim().to_string();
                        }
                    }
                }
            }
        }
    }
    if title.is_empty() {
        title = desc.to_string();
    }
    if let Some(t) = &g.title {
        title = t.clone();
    }
    // a maker of nothing but spaces is none, as PinballY takes it
    let id = match (maker.trim().is_empty(), year) {
        (false, y) if y != 0 => format!("{title} ({maker} {y}).{system}"),
        (false, _) => format!("{title} ({maker}).{system}"),
        (true, y) if y != 0 => format!("{title} ({y}).{system}"),
        _ => format!("{title}.{system}"),
    };
    [id, format!("{title}.{system}")]
}

/// One CSV line's fields, as PinballY reads its stats: a quoted field's doubled quotes made one.
fn csv_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        let mut field = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                    } else {
                        break;
                    }
                } else {
                    field.push(c);
                }
            }
            // anything after the closing quote, up to the separator, is dropped
            while chars.peek().is_some_and(|c| *c != ',') {
                chars.next();
            }
        } else {
            while let Some(c) = chars.peek().copied() {
                if c == ',' {
                    break;
                }
                field.push(c);
                chars.next();
            }
        }
        out.push(field);
        if chars.next().is_none() {
            break;
        }
    }
    out
}

/// A field written as PinballY writes one: quoted, its quotes doubled, only where it holds a comma,
/// a quote or a line's end.
fn csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// PinballY's stats file, beside its program unless its command line names another place.
fn pinbally_stats_file(folder: &Path) -> PathBuf {
    folder.join("GameStats.csv")
}

/// The ratings PinballY's stats keep, by the game's name there: those set (a rating left empty
/// leaves the game list's in place).
pub fn pinbally_stats_ratings(folder: &Path) -> HashMap<String, f64> {
    let Some(text) = read_text(&pinbally_stats_file(folder)) else { return HashMap::new() };
    let mut lines = text.lines();
    let Some(head) = lines.next().map(csv_fields) else { return HashMap::new() };
    let (Some(game), Some(rating)) = (head.iter().position(|c| c == "Game"), head.iter().position(|c| c == "Rating")) else { return HashMap::new() };
    lines
        .map(csv_fields)
        .filter_map(|row| {
            let value = row.get(rating)?.trim();
            (!value.is_empty()).then(|| Some((row.get(game)?.clone(), value.parse().ok()?))).flatten()
        })
        .collect()
}

/// The game named `ids` (its name now, then its older one) in PinballY's stats given `stars` —
/// none left empty, so PinballY takes the game list's — only where the stats hold a row for it with
/// a rating column: else PinballY reads the game list's, which `set_rating` changes. The file
/// written back as it was written, every other row as it was; whether it changed.
pub fn set_pinbally_stats_rating(folder: &Path, ids: &[String; 2], stars: Option<u8>) -> Result<bool, String> {
    let path = pinbally_stats_file(folder);
    let Ok(bytes) = std::fs::read(&path) else { return Ok(false) };
    let text = decode(&bytes);
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    let Some(head) = lines.first().map(|l| csv_fields(l.trim_end_matches(['\r', '\n']))) else { return Ok(false) };
    let (Some(game), Some(rating)) = (head.iter().position(|c| c == "Game"), head.iter().position(|c| c == "Rating")) else { return Ok(false) };
    let row_of = |id: &str| lines.iter().skip(1).position(|l| csv_fields(l.trim_end_matches(['\r', '\n'])).get(game).is_some_and(|g| g == id)).map(|i| i + 1);
    let Some(at) = row_of(&ids[0]).or_else(|| row_of(&ids[1])) else { return Ok(false) };
    let line = lines[at];
    let body = line.trim_end_matches(['\r', '\n']);
    let mut fields = csv_fields(body);
    if fields.len() <= rating {
        fields.resize(rating + 1, String::new());
    }
    fields[rating] = stars.map(|s| s.to_string()).unwrap_or_default();
    let changed = format!("{}{}", fields.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(","), &line[body.len()..]);
    lines[at] = &changed;
    let out = lines.concat();
    write_list(&path, &bytes, &out)?;
    Ok(true)
}

impl FrontEnd {
    /// The games of its game lists for the system, the list named for the system read first.
    pub fn games(&self) -> Vec<Game> {
        let Some(dir) = &self.databases else { return Vec::new() };
        let mut lists: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("xml"))).collect();
        lists.sort_by_key(|p| (!p.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(&self.system)), p.clone()));
        lists.iter().flat_map(|l| read_text(l).map(|t| games_in(&t, l)).unwrap_or_default()).collect()
    }

    /// Its media names by table file, from its game lists: each game's name in lower case, without
    /// a `.vpx` it may carry, to the name its media go under.
    pub fn media_names(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for g in self.games() {
            // a table kept in a folder of its own may be listed by `<folder>\<name>`
            let low = g.name.to_lowercase();
            let last = low.rsplit(['\\', '/']).next().unwrap_or(&low);
            let key = last.strip_suffix(".vpx").unwrap_or(last).to_string();
            if let Some(m) = g.media_name {
                out.entry(key).or_insert(m);
            }
        }
        out
    }

    /// A kind's folder, by its name in `KINDS`.
    fn folder(&self, name: &str) -> PathBuf {
        match name.strip_prefix("../") {
            Some(shared) => shared.split('/').fold(self.media_root.clone(), |p, part| p.join(part)),
            None => self.media.join(name),
        }
    }

    /// Where a file of `kind` (Popper's name for it) goes: its folder, by whether it's a video.
    pub fn place(&self, kind: &str, video: bool) -> Option<PathBuf> {
        folder_for(self.front, kind, video).map(|f| self.folder(f)).or_else(|| audio_folder(kind).map(|f| self.media.join(f)))
    }

    /// Each kind's folders that are there, with the names of the files in them: read once for the
    /// whole library.
    pub fn read(&self) -> Found {
        let mut folders = Vec::new();
        for (kind, names) in KINDS {
            for name in names {
                let dir = self.folder(name);
                let Ok(entries) = std::fs::read_dir(&dir) else { continue };
                let files: Vec<String> = entries.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
                folders.push((kind.to_string(), dir, files));
            }
        }
        Found { front: self.clone(), names: self.media_names(), folders }
    }
}

/// A front end's media folders as read for the library.
#[derive(Clone, Debug)]
pub struct Found {
    pub front: FrontEnd,
    /// Its media names by table file (`FrontEnd::media_names`).
    pub names: HashMap<String, String>,
    /// Each kind's folders, with the names of the files in them.
    pub folders: Vec<(String, PathBuf, Vec<String>)>,
}

/// A file's name the front end's for `name`'s media: `<name>.<ext>`, the one it shows, or
/// `<name> <n>.<ext>` kept numbered beside it; its number (0 for the one it shows).
pub fn number_of(file: &str, name: &str) -> Option<u32> {
    let base = file.rsplit_once('.').map_or(file, |(b, _)| b);
    let low = base.to_lowercase();
    let rest = low.strip_prefix(&name.to_lowercase())?;
    if rest.is_empty() {
        return Some(0);
    }
    let n = rest.strip_prefix(' ')?;
    (!n.is_empty() && n.len() <= 2 && n.bytes().all(|b| b.is_ascii_digit())).then(|| n.parse().ok()).flatten().filter(|n| *n > 0)
}

impl Found {
    /// The name the table file `file` (its name alone)'s media go under.
    pub fn media_name(&self, file: &str) -> String {
        let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
        self.names.get(&stem.to_lowercase()).cloned().unwrap_or_else(|| stem.to_string())
    }

    /// The pictures and videos it holds for the table file `file`, kind by kind, the one it shows
    /// first in each folder: those the app shows (a picture, or a video in a kind it plays).
    pub fn media_for(&self, file: &str) -> Vec<crate::installed::Media> {
        let name = self.media_name(file);
        let mut out = Vec::new();
        for (kind, dir, files) in &self.folders {
            let mut mine: Vec<(u32, &String)> = files
                .iter()
                .filter(|f| {
                    let ext = ext_of(f);
                    PICTURES.contains(&ext.as_str()) || PLAYED.contains(&ext.as_str())
                })
                .filter_map(|f| number_of(f, &name).map(|n| (n, f)))
                .collect();
            mine.sort_by_key(|(n, f)| (*n, f.to_lowercase()));
            for (_, f) in mine {
                let video = VIDEOS.contains(&ext_of(f).as_str());
                out.push(crate::installed::Media { kind: kind.clone(), path: dir.join(f), video, from: self.front.front });
            }
        }
        out
    }

    /// Every file in its media folders named for `name` (any kind of file), for a rename or a
    /// removal.
    pub fn files_named(&self, name: &str) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        let audio = ["Audio", "AudioLaunch"].into_iter().filter_map(audio_folder).map(|f| self.front.media.join(f));
        let dirs: Vec<PathBuf> = self.folders.iter().map(|(_, d, _)| d.clone()).chain(audio).collect();
        for dir in dirs {
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let f = e.file_name().to_string_lossy().into_owned();
                if e.path().is_file() && number_of(&f, name).is_some() && !out.contains(&e.path()) {
                    out.push(e.path());
                }
            }
        }
        out.sort();
        out
    }
}

/// Every front end found for the library, read.
pub fn read_all(layout: &Layout) -> Vec<Found> {
    find(layout).iter().map(FrontEnd::read).collect()
}

/// The front end a drop's media go in: the one the user chose, while it's there; else PinUP
/// Popper's folders where they're there, else PinballX's, else PinballY's. None: none is there.
pub fn media_target(layout: &Layout) -> Option<Target> {
    let others = find(layout);
    let popper = crate::installed::pop_media(layout);
    let pick = |front: Front| -> Option<Target> {
        match front {
            Front::Popper => popper.clone().map(Target::Popper),
            other => others.iter().find(|f| f.front == other).cloned().map(Target::Other),
        }
    };
    layout.media_front.and_then(pick).or_else(|| [Front::Popper, Front::PinballX, Front::PinballY].into_iter().find_map(pick))
}

/// Where a drop's front-end media go.
#[derive(Clone, Debug)]
pub enum Target {
    /// PinUP Popper's media folder for VPX (`POPMedia\Visual Pinball X`).
    Popper(PathBuf),
    Other(FrontEnd),
}

impl Target {
    pub fn front(&self) -> Front {
        match self {
            Target::Popper(_) => Front::Popper,
            Target::Other(f) => f.front,
        }
    }
}

/// The front ends there for the library, PinUP Popper's first: what Settings › Media offers to put
/// media in.
pub fn there(layout: &Layout) -> Vec<(Front, PathBuf)> {
    let mut out: Vec<(Front, PathBuf)> = crate::installed::pop_media(layout).map(|p| (Front::Popper, p)).into_iter().collect();
    out.extend(find(layout).into_iter().map(|f| (f.front, f.media)));
    out
}

/// Where the list's `<game name="…">` for the game `name` is: its tag's start, its name's
/// (`name="…"`) start and length, and the quote it's written in; None where it doesn't hold it.
fn game_at(text: &str, name: &str) -> Option<(usize, usize, usize, char)> {
    let lower = text.to_ascii_lowercase();
    for quote in ['"', '\''] {
        for written in [escape(name), name.to_string()] {
            let needle = format!("name={quote}{written}{quote}").to_ascii_lowercase();
            let mut at = 0;
            while let Some(i) = lower[at..].find(&needle) {
                let start = at + i;
                // only a game's own name: inside its `<game …>` tag
                if let Some(open) = lower[..start].rfind('<')
                    && lower[open..start].starts_with("<game")
                    && !lower[open..start].contains('>')
                {
                    return Some((open, start, needle.len(), quote));
                }
                at = start + needle.len();
            }
        }
    }
    None
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A list written back as it was written — UTF-16's and UTF-8's marks kept, else in the encoding it
/// was read in — the list as it was kept beside it first (`<list>.vpi-before`).
fn write_list(list: &Path, read: &[u8], text: &str) -> Result<(), String> {
    let out: Vec<u8> = match read {
        [0xFF, 0xFE, ..] => [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(u16::to_le_bytes)).collect(),
        [0xFE, 0xFF, ..] => [0xFE, 0xFF].into_iter().chain(text.encode_utf16().flat_map(u16::to_be_bytes)).collect(),
        [0xEF, 0xBB, 0xBF, ..] => [0xEF, 0xBB, 0xBF].into_iter().chain(text.bytes()).collect(),
        _ if std::str::from_utf8(read).is_ok() => text.as_bytes().to_vec(),
        _ => encoding_rs::WINDOWS_1252.encode(text).0.into_owned(),
    };
    let before = PathBuf::from(format!("{}.vpi-before", list.display()));
    std::fs::copy(list, &before).map_err(|e| format!("{} couldn't be kept before changing it: {e}", list.display()))?;
    let passing = PathBuf::from(format!("{}.vpi-part", list.display()));
    std::fs::write(&passing, out).map_err(|e| format!("{}: {e}", passing.display()))?;
    std::fs::rename(&passing, list).map_err(|e| format!("{}: {e}", list.display()))
}

/// A game list's game renamed for its table's new file name (`<game name="…">`), and given the
/// description `description` where one's given (the name its media go under), the list changed in
/// place, only those; nothing done where the list doesn't hold the game.
pub fn rename_game(list: &Path, from: &str, to: &str, description: Option<&str>) -> Result<bool, String> {
    let bytes = std::fs::read(list).map_err(|e| format!("{}: {e}", list.display()))?;
    let text = decode(&bytes);
    let Some((open, start, len, quote)) = game_at(&text, from) else { return Ok(false) };
    let mut changed = format!("{}name={quote}{}{quote}{}", &text[..start], escape(to), &text[start + len..]);
    if let Some(d) = description {
        let lower = changed.to_ascii_lowercase();
        let end = lower[open..].find("</game>").map_or(lower.len(), |i| open + i);
        if let Some(i) = lower[open..end].find("<description>") {
            let at = open + i + "<description>".len();
            if let Some(j) = lower[at..end].find("</description>") {
                let text_only = d.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                changed = format!("{}{text_only}{}", &changed[..at], &changed[at + j..]);
            }
        }
    }
    write_list(list, &bytes, &changed)?;
    Ok(true)
}

/// A game list's game taken out of it (`<game name="…">` to its `</game>`, with the line it
/// stands on where it stands alone), the rest as it was; nothing done where the list doesn't hold
/// it.
pub fn remove_game(list: &Path, name: &str) -> Result<bool, String> {
    let bytes = std::fs::read(list).map_err(|e| format!("{}: {e}", list.display()))?;
    let text = decode(&bytes);
    let Some((open, _, _, _)) = game_at(&text, name) else { return Ok(false) };
    let lower = text.to_ascii_lowercase();
    let end = match lower[open..].find("</game>") {
        Some(i) => open + i + "</game>".len(),
        // written `<game … />`
        None => open + lower[open..].find("/>").ok_or("That game list's game has no end.")? + 2,
    };
    // the whitespace before it on its line, and the line's end after it, go with it
    let line_start = text[..open].rfind('\n').map_or(0, |i| i + 1);
    let from = if text[line_start..open].trim().is_empty() { line_start } else { open };
    let rest = &text[end..];
    let to = end + if rest.starts_with("\r\n") { 2 } else if rest.starts_with('\n') { 1 } else { 0 };
    let changed = format!("{}{}", &text[..from], &text[to..]);
    write_list(list, &bytes, &changed)?;
    Ok(true)
}

/// A game list's game given `stars` as its `<rating>` (none: 0, which the lists take for none), the
/// element added where it has none, lined up with the game's others; the rest as it was. Nothing
/// written where the list doesn't hold the game or already says so; whether the list holds it.
pub fn set_rating(list: &Path, name: &str, stars: Option<u8>) -> Result<bool, String> {
    let bytes = std::fs::read(list).map_err(|e| format!("{}: {e}", list.display()))?;
    let text = decode(&bytes);
    let Some((open, _, _, _)) = game_at(&text, name) else { return Ok(false) };
    let lower = text.to_ascii_lowercase();
    // written `<game … />`: nothing to put a rating in
    let Some(end) = lower[open..].find("</game>").map(|i| open + i) else { return Ok(false) };
    let value = stars.unwrap_or(0).to_string();
    let changed = match lower[open..end].find("<rating>").map(|i| open + i + "<rating>".len()) {
        Some(at) => {
            let Some(close) = lower[at..end].find("</rating>").map(|i| at + i) else { return Ok(false) };
            if text[at..close].trim() == value {
                return Ok(true);
            }
            format!("{}{value}{}", &text[..at], &text[close..])
        }
        None => {
            let line_start = text[..end].rfind('\n').map_or(0, |i| i + 1);
            if text[line_start..end].trim().is_empty() {
                // on a line of its own, indented as the game's first element is
                let tag_end = open + lower[open..end].find('>').map_or(0, |i| i + 1);
                let first = lower[tag_end..end].find('<').map(|i| tag_end + i);
                let indent = first.map(|f| {
                    let ls = text[..f].rfind('\n').map_or(0, |i| i + 1);
                    let lead = &text[ls..f];
                    if lead.trim().is_empty() { lead.to_string() } else { String::new() }
                });
                let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
                format!("{}{}<rating>{value}</rating>{newline}{}", &text[..line_start], indent.unwrap_or_default(), &text[line_start..])
            } else {
                format!("{}<rating>{value}</rating>{}", &text[..end], &text[end..])
            }
        }
    };
    write_list(list, &bytes, &changed)?;
    Ok(true)
}

impl FrontEnd {
    /// The ratings it keeps for its games, by each game's name in its lists: PinballY's stats over
    /// its game lists' (PinballY's own order); whole stars, one to five, those rated.
    pub fn ratings(&self) -> Vec<(String, u8)> {
        let stats = if self.front == Front::PinballY { pinbally_stats_ratings(&self.folder) } else { HashMap::new() };
        self.games()
            .into_iter()
            .filter_map(|g| {
                let ids = id_in_pinbally(&g, &self.system);
                let stars = match stats.get(&ids[0]).or_else(|| stats.get(&ids[1])) {
                    // PinballY's own: -1 none, 0 a rating of no stars, halves allowed
                    Some(r) if *r < 0.0 => None,
                    Some(r) => Some(stars_of(r.max(1.0))?),
                    None => stars_of(g.rating),
                };
                Some((g.name, stars?))
            })
            .collect()
    }

    /// The game named `name` in its lists given `stars`, in each list holding it, and in PinballY's
    /// stats where they rate it; whether its lists hold it (so it now keeps that rating).
    pub fn set_rating(&self, name: &str, stars: Option<u8>) -> Result<bool, String> {
        let mut listed = false;
        let name = name.strip_suffix(".vpx").or_else(|| name.strip_suffix(".VPX")).unwrap_or(name);
        let with_ext = format!("{name}.vpx");
        for g in self.games().into_iter().filter(|g| g.name.eq_ignore_ascii_case(name) || g.name.eq_ignore_ascii_case(&with_ext)) {
            listed |= set_rating(&g.list, &g.name, stars)?;
            if self.front == Front::PinballY {
                set_pinbally_stats_rating(&self.folder, &id_in_pinbally(&g, &self.system), stars)?;
            }
        }
        Ok(listed)
    }
}

/// A rating as the front ends keep one, made whole stars from one to five: none at 0 or below,
/// half stars rounded up, as PinballY rounds them for PinballX's lists.
pub fn stars_of(value: f64) -> Option<u8> {
    (value > 0.0 && value.is_finite()).then(|| value.ceil().min(5.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chosen_put_in_front_their_own_kept_numbered_as_they_number_them() {
        let dir = std::env::temp_dir().join(format!("vpi-front-put-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let wheel = dir.join("Wheel Images");
        std::fs::create_dir_all(&wheel).unwrap();
        let ours = dir.join("ours.png");
        std::fs::write(&ours, "ours").unwrap();
        std::fs::write(wheel.join("Black Knight (Williams 1980).png"), "theirs").unwrap();
        std::fs::write(wheel.join("Black Knight (Williams 1980) 1.png"), "their second").unwrap();
        assert!(put_in_front(&wheel, "Black Knight (Williams 1980)", &ours, &|_| false, &|p| Err(format!("{} sent away", p.display()))).unwrap().is_some());
        let read = |n: &str| std::fs::read_to_string(wheel.join(n)).unwrap();
        let got = (read("Black Knight (Williams 1980).png"), read("Black Knight (Williams 1980) 2.png"), read("Black Knight (Williams 1980) 1.png"));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, ("ours".to_string(), "theirs".to_string(), "their second".to_string()));
    }

    fn utf16(text: &str) -> Vec<u8> {
        [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(u16::to_le_bytes)).collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vpi-frontends-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn media_names_and_their_numbers() {
        assert_eq!(clean_media_name("Attack from Mars (Bally 1995)"), "Attack from Mars (Bally 1995)");
        assert_eq!(clean_media_name("AC/DC: Let There Be Rock? <LE>"), "AC;DC Let There Be Rock (LE)");
        assert_eq!(number_of("Goldeneye (Sega 1996).png", "Goldeneye (Sega 1996)"), Some(0));
        assert_eq!(number_of("goldeneye (sega 1996) 2.jpg", "Goldeneye (Sega 1996)"), Some(2));
        assert_eq!(number_of("Goldeneye (Sega 1996)2.jpg", "Goldeneye (Sega 1996)"), None);
        assert_eq!(number_of("Goldeneye (Sega 1996) alt.png", "Goldeneye (Sega 1996)"), None);
        assert_eq!(folder_for(Front::PinballX, "PlayField", true), Some("Table Videos"));
        assert_eq!(folder_for(Front::PinballX, "PlayField", false), Some("Table Images"));
        assert_eq!(folder_for(Front::PinballX, "Wheel", true), None);
        assert_eq!(folder_for(Front::PinballX, "Menu", true), Some("FullDMD Videos"));
        assert_eq!(folder_for(Front::PinballY, "Menu", true), None);
        assert_eq!(folder_for(Front::PinballY, "GameHelp", false), Some("../Instruction Cards"));
    }

    #[test]
    fn pinballx_found_for_the_library_its_media_read_and_a_game_renamed() {
        let d = scratch("x");
        let tables = d.join("Visual Pinball").join("Tables");
        std::fs::create_dir_all(&tables).unwrap();
        let px = d.join("PinballX");
        // PinballX's settings in UTF-16, its own Visual Pinball system playing another folder and a
        // second system playing the library's
        let ini = format!(
            "[Display]\r\nMonitor=0\r\n[VisualPinball]\r\nEnabled=True\r\nTablePath=C:\\Elsewhere\\Tables\r\n[System_1]\r\nName=VPX Tables\r\nEnabled=True\r\nSystemType=1\r\nTablePath={}\\\r\n",
            tables.display()
        );
        put(&px.join("Config").join("PinballX.ini"), &utf16(&ini));
        let list = px.join("Databases").join("VPX Tables").join("VPX Tables.xml");
        put(&list, "<?xml version=\"1.0\"?>\r\n<menu>\r\n  <game name=\"afm_vpw\">\r\n    <description>Attack from Mars (Bally 1995)</description>\r\n  </game>\r\n  <game name=\"Taxi\">\r\n    <description></description>\r\n  </game>\r\n</menu>\r\n".as_bytes());
        let media = px.join("Media").join("VPX Tables");
        put(&media.join("Wheel Images").join("Attack from Mars (Bally 1995).png"), b"w");
        put(&media.join("Table Videos").join("Attack from Mars (Bally 1995).f4v"), b"v");
        put(&media.join("Table Videos").join("Attack from Mars (Bally 1995).wmv"), b"v");
        put(&media.join("Backglass Images").join("Attack from Mars (Bally 1995) 1.jpg"), b"b");
        put(&px.join("Media").join("Instruction Cards").join("Attack from Mars (Bally 1995).png"), b"c");
        put(&media.join("Wheel Images").join("Taxi.png"), b"t");
        put(&media.join("Table Audio").join("Taxi.mp3"), b"a");
        let mut layout = Layout::under(&d.join("Visual Pinball"));
        layout.tables = tables.clone();
        layout.front_ends = vec![px.clone()];
        let found = find(&layout);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].front, found[0].system.as_str(), found[0].media.clone()), (Front::PinballX, "VPX Tables", media.clone()));
        let read = found[0].read();
        // named for its description, else its file's name
        assert_eq!(read.media_name("afm_vpw.vpx"), "Attack from Mars (Bally 1995)");
        assert_eq!(read.media_name("Taxi.vpx"), "Taxi");
        let got: Vec<(String, String, bool)> = read.media_for("afm_vpw.vpx").iter().map(|m| (m.kind.clone(), m.path.file_name().unwrap().to_string_lossy().into_owned(), m.video)).collect();
        assert_eq!(
            got,
            [
                ("Wheel".to_string(), "Attack from Mars (Bally 1995).png".to_string(), false),
                ("PlayField".into(), "Attack from Mars (Bally 1995).f4v".into(), true),
                ("BackGlass".into(), "Attack from Mars (Bally 1995) 1.jpg".into(), false),
                ("GameHelp".into(), "Attack from Mars (Bally 1995).png".into(), false),
            ]
        );
        // every file named for it, those the app doesn't show and its audio too
        assert_eq!(read.files_named("Taxi").len(), 2);
        assert_eq!(read.files_named("Attack from Mars (Bally 1995)").len(), 5);
        // where a drop's go
        assert_eq!(found[0].place("PlayField", false), Some(media.join("Table Images")));
        assert_eq!(found[0].place("GameInfo", false), Some(px.join("Media").join("Flyer Images").join("Front")));
        assert_eq!(found[0].place("AudioLaunch", false), Some(media.join("Launch Audio")));
        // a game renamed for its table's new name, its description kept, the list before kept
        assert!(rename_game(&list, "afm_vpw", "Attack from Mars (Bally 1995)", None).unwrap());
        let games = found[0].games();
        assert_eq!(games[0].name, "Attack from Mars (Bally 1995)");
        assert_eq!(games[0].media_name.as_deref(), Some("Attack from Mars (Bally 1995)"));
        assert!(PathBuf::from(format!("{}.vpi-before", list.display())).is_file());
        assert!(!rename_game(&list, "Not There", "x", None).unwrap());
        // a description following its new name
        assert!(rename_game(&list, "Attack from Mars (Bally 1995)", "AFM", Some("AFM & Co")).unwrap());
        assert_eq!(found[0].games()[0].media_name.as_deref(), Some("AFM & Co"));
        // a game taken out, the line it stood on with it, the other kept as it was
        assert!(remove_game(&list, "AFM").unwrap());
        let left = std::fs::read_to_string(&list).unwrap();
        assert_eq!(left, "<?xml version=\"1.0\"?>\r\n<menu>\r\n  <game name=\"Taxi\">\r\n    <description></description>\r\n  </game>\r\n</menu>\r\n");
        // not found where no system plays the library's tables
        let mut elsewhere = layout.clone();
        elsewhere.tables = d.join("Other").join("Tables");
        assert!(find(&elsewhere).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn pinbally_found_with_its_media_in_pinballxs_folder() {
        let d = scratch("y");
        let vpx = d.join("Visual Pinball");
        let tables = vpx.join("Tables");
        std::fs::create_dir_all(&tables).unwrap();
        let px = d.join("PinballX");
        put(&px.join("Config").join("PinballX.ini"), &utf16("[VisualPinball]\r\nTablePath=C:\\Nowhere\r\n"));
        let py = d.join("PinballY");
        let settings = "# PinballY settings\r\nMediaPath = [PinballX]\\Media\r\nTableDatabasePath = Databases\r\nSystem1 = Visual Pinball 9\r\nSystem1.Class = VP\r\nSystem1.TablePath = Tables\r\nSystem3 = Visual Pinball X\r\nSystem3.Class = VPX\r\nSystem3.Enabled = 1\r\nSystem3.TablePath = Tables\r\nSystem3.DefExt = .vpx\r\n";
        put(&py.join("Settings.txt"), settings.as_bytes());
        put(&py.join("Databases").join("Visual Pinball X").join("Visual Pinball X.xml"), b"<menu><game name=\"Taxi\"><description>Taxi (Williams 1988)</description></game></menu>");
        put(&px.join("Media").join("Visual Pinball X").join("Topper Videos").join("Taxi (Williams 1988).mp4"), b"t");
        let mut layout = Layout::under(&vpx);
        layout.tables = tables.clone();
        layout.front_ends = vec![px.clone(), py.clone()];
        let found = find(&layout);
        // PinballX plays no folder of the library's; PinballY's VPX system plays `Tables` beside VPX
        assert_eq!(found.iter().map(|f| f.front).collect::<Vec<_>>(), [Front::PinballY]);
        assert_eq!(found[0].media, px.join("Media").join("Visual Pinball X"));
        let read = found[0].read();
        let got: Vec<String> = read.media_for("Taxi.vpx").iter().map(|m| format!("{} {}", m.kind, m.path.file_name().unwrap().to_string_lossy())).collect();
        assert_eq!(got, ["Topper Taxi (Williams 1988).mp4"]);
        assert_eq!(found[0].place("Menu", true), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
