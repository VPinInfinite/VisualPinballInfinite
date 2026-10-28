// Its rules follow VPin Studio's UploaderAnalysis (https://github.com/syd711/vpin-studio, its main
// branch): which files make a PinUP pack, an AltSound pack, a colouring, a DMD
// folder, music, a ROM, and the rest. Copyright (c) 2022 syd711. MIT licence
// (LICENSE-VPinStudio.txt beside this crate's Cargo.toml); this work is offered under the same terms.

//! What each file in a drop is, as VPin Studio tells them apart: a PinUP pack
//! by its marker files or its many folders of videos, an AltSound pack by its
//! table of sounds, a colouring by its file's kind, a DMD folder by its name,
//! music by its sound files outside those, and the rest by their kind of file.
//! Then the ROM the drop is for, from what names it. Where ours differs from
//! VPin Studio's it says so.

use crate::archive::{Entry, Listing};

/// What one file of a drop is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A VPX table.
    Table,
    /// A Visual Pinball 9 table (`.vpt`), which ours doesn't play.
    OldTable,
    /// A Future Pinball table or library (`.fpt`, `.fpl`), which ours doesn't play.
    FuturePinball,
    Backglass,
    /// B2S Server's window places for the table (`.res`).
    BackglassPlaces,
    /// The table's views (`.pov`).
    Pov,
    /// The table's physics, as VPX's editor exports them (`.vpp`; ours, VPin Studio leaves them out).
    Physics,
    /// The table's own settings (`.ini`).
    TableSettings,
    /// A script in place of the table's own (`.vbs`).
    Script,
    /// A script file the table's script loads (`KISSCore.vbs`), found so by the drop's plan.
    HelperScript,
    /// A VPU patch (`.dif`).
    Patch,
    /// A ROM set (a zip inside the drop, or the drop itself).
    Rom,
    PupPack,
    AltSound,
    Colouring(Colouring),
    /// An UltraDMD or FlexDMD folder of the table's own pictures.
    DmdFolder,
    Music,
    /// The machine's memory (`.nv`).
    Nvram,
    /// PinMAME's settings for the game (`.cfg`).
    PinmameSettings,
    /// Front-end media: wheel, backglass and playfield pictures and videos.
    Media,
    /// A picture named as one of its table's own (`backdrop2.png` for its picture `backdrop2`), which
    /// the player reads in the table's own's place, the table file left as it is: found so by the
    /// drop, which knows the table (ours).
    TablePicture,
    /// A manual or rule sheet to read (`.pdf`), kept beside its table, named for it (ours).
    Manual,
    /// A read-me: a text file outside the packs, or a picture of the instructions beside the music
    /// (ours: a media pack's note to put its music folder in place, written on a picture).
    Readme,
    /// An archive inside the drop, not a ROM: to be opened by itself.
    InnerArchive,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colouring {
    /// Serum (`.cRZ`, `.cROMc`).
    Serum,
    /// PIN2DMD's older files (`.pal`, `.vni`).
    Pin2dmd,
    /// PIN2DMD's encrypted files (`.pac`).
    Pin2dmdPac,
}

impl Kind {
    pub fn describe(self) -> &'static str {
        match self {
            Kind::Table => "table",
            Kind::OldTable => "Visual Pinball 9 table (not played)",
            Kind::FuturePinball => "Future Pinball file (not played)",
            Kind::Backglass => "backglass",
            Kind::BackglassPlaces => "backglass window places",
            Kind::Pov => "table views (POV)",
            Kind::Physics => "table physics (VPP)",
            Kind::TableSettings => "table settings",
            Kind::Script => "script replacing the table's",
            Kind::HelperScript => "script its table loads",
            Kind::Patch => "VPU patch",
            Kind::Rom => "ROM",
            Kind::PupPack => "PinUP pack",
            Kind::AltSound => "AltSound pack",
            Kind::Colouring(Colouring::Serum) => "Serum coloring",
            Kind::Colouring(Colouring::Pin2dmd) => "PIN2DMD coloring",
            Kind::Colouring(Colouring::Pin2dmdPac) => "PIN2DMD coloring (.pac)",
            Kind::DmdFolder => "UltraDMD or FlexDMD folder",
            Kind::Music => "music",
            Kind::Nvram => "machine memory (NVRAM)",
            Kind::PinmameSettings => "PinMAME settings",
            Kind::Media => "front-end media",
            Kind::TablePicture => "picture for the table",
            Kind::Manual => "manual",
            Kind::Readme => "read-me",
            Kind::InnerArchive => "archive inside (open it by itself)",
            Kind::Other => "other",
        }
    }
}

/// A drop's files sorted out.
#[derive(Clone, Debug)]
pub struct Contents {
    pub files: Vec<(Entry, Kind)>,
    /// The PinUP pack's top folder (with its `/`), and its name: the ROM or game it's for.
    pub pup_root: Option<String>,
    pub altsound_folder: Option<String>,
    pub dmd_folder: Option<String>,
    pub music_folder: Option<String>,
    /// The read-me's text, the first one found.
    pub readme: Option<String>,
    /// The drop is itself a ROM set, a zip named for it (ours).
    pub rom_set: Option<String>,
}

const ROM_SUFFIXES: [&str; 10] = ["bin", "rom", "cpu", "snd", "dat", "s2", "l1", "ic1", "ic2", "ic3"];
/// VPin Studio's, and PinballX's older videos (`.f4v`, ours).
const MEDIA_SUFFIXES: [&str; 6] = ["mp3", "png", "apng", "jpg", "mp4", "f4v"];
const MUSIC_SUFFIXES: [&str; 3] = ["mp3", "ogg", "wav"];
const VIDEO_SUFFIXES: [&str; 2] = ["mp4", "mkv"];
pub(crate) const PICTURE_SUFFIXES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "bmp", "gif"];
/// Words in a front-end media file's path naming the screen it's for: VPin Studio's screens, and
/// ours on top — what media packs name the files for PinUP Popper's game info and game help
/// screens (a `Flyer` folder, a `Credits` one, an `Instructions` one), and a table's logos, its
/// wheel's pictures, which `plan::media_folder` places there; and PinballX's and PinballY's names
/// for a playfield's (`Table Videos`, `Table Images`).
const MEDIA_SCREENS: [&str; 21] = [
    "wheel", "backglass", "playfield", "topper", "dmd", "fulldmd", "apron", "loading", "audio", "gameinfo", "gamehelp", "menu", "game info", "game help", "flyer", "credits", "instruction", "rules", "logo", "table video", "table image",
];
/// A PinUP pack's root has at least this many folders, with this many videos under them, when
/// it has none of a pack's own files (VPin Studio's fallback).
const PUP_MIN_FOLDERS: usize = 10;
const PUP_MIN_VIDEOS: usize = 10;

/// A zip named as PinMAME names a ROM set: up to 16 letters, digits and underscores (`afm_113b.zip`).
pub(crate) fn looks_like_rom_set(name: &str) -> bool {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    (2..=16).contains(&stem.len()) && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn ext(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn folder(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(d, _)| d)
}

/// `path` lies under `folder` (given with or without its `/`).
fn below(folder: &str, path: &str) -> bool {
    let f = folder.trim_end_matches('/');
    !f.is_empty() && path.len() > f.len() && path.starts_with(f) && path.as_bytes()[f.len()] == b'/'
}

/// Sorts out a drop's files (VPin Studio's `UploaderAnalysis`).
/// Sound files loose in a drop for a table already in the library — dropped on it, or found its by
/// their names — are its music, as beside a table file (ours: found adding Tom and Jerry's songs
/// to it on its own).
pub fn loose_music_for_a_table(contents: &mut Contents) {
    if contents.altsound_folder.is_some() {
        return;
    }
    for (e, kind) in &mut contents.files {
        if *kind == Kind::Other && !e.path.contains('/') && MUSIC_SUFFIXES.contains(&ext(&e.path).as_str()) {
            *kind = Kind::Music;
        }
    }
}

pub fn analyse(listing: &Listing) -> Contents {
    // macOS's leftovers and Scorbit's files are passed over, as VPin Studio does
    let entries: Vec<Entry> = listing.entries.iter().filter(|e| !e.path.contains("_MACOSX") && !e.path.to_lowercase().contains("scorbit")).cloned().collect();
    let names: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let folders: Vec<&str> = {
        let mut f: Vec<&str> = names.iter().filter_map(|n| folder(n)).collect();
        f.sort();
        f.dedup();
        f
    };
    let pup_root = pup_root(&names, &folders);
    let altsound_folder = names.iter().find(|n| n.contains("altsound.csv") || n.contains("g-sound.csv")).and_then(|n| folder(n)).map(str::to_string);
    let dmd_folder = dmd_folder(&folders, pup_root.as_deref());
    let music_folder = if altsound_folder.is_some() { None } else { music_folder(&names, pup_root.as_deref(), dmd_folder.as_deref()) };
    // sound files loose beside a table are its music, which VPX and the player play from the
    // `music` folder beside the table (ours: VPin Studio's music needs a folder of its own)
    let loose_music = altsound_folder.is_none() && names.iter().any(|n| ext(n) == "vpx");
    // a drop of ROM files alone, with no folders, is a ROM set (VPin Studio's `isRom`)
    let rom_set = folders.is_empty()
        && names.iter().any(|n| {
            let e = ext(n);
            ROM_SUFFIXES.contains(&e.as_str()) || (e.len() == 2 && e.as_bytes()[1].is_ascii_digit()) || e.parse::<u32>().is_ok()
        })
        && !names.iter().any(|n| matches!(ext(n).as_str(), "vpx" | "vpt" | "fpt" | "directb2s"));

    let mut readme = None;
    let mut files = Vec::new();
    for e in &entries {
        let p = e.path.as_str();
        let x = ext(p);
        let lower = p.to_lowercase();
        let kind = if pup_root.as_deref().is_some_and(|r| below(r, p)) {
            Kind::PupPack
        } else if altsound_folder.as_deref().is_some_and(|f| below(f, p)) {
            Kind::AltSound
        } else if dmd_folder.as_deref().is_some_and(|f| below(f, p)) {
            Kind::DmdFolder
        } else if rom_set {
            Kind::Rom
        } else {
            match x.as_str() {
                "vpx" => Kind::Table,
                "vpt" => Kind::OldTable,
                "fpt" | "fpl" => Kind::FuturePinball,
                "directb2s" => Kind::Backglass,
                "res" => Kind::BackglassPlaces,
                "pov" => Kind::Pov,
                "vpp" => Kind::Physics,
                "ini" if !lower.ends_with("altsound.ini") && !lower.contains("pinup") => Kind::TableSettings,
                "vbs" => Kind::Script,
                "dif" => Kind::Patch,
                "nv" => Kind::Nvram,
                "cfg" => Kind::PinmameSettings,
                "crz" | "cromc" => Kind::Colouring(Colouring::Serum),
                "pal" | "vni" => Kind::Colouring(Colouring::Pin2dmd),
                "pac" => Kind::Colouring(Colouring::Pin2dmdPac),
                // a ROM's zip: under a `roms` folder, or named as PinMAME names its sets (ours: VPin
                // Studio takes the first zip it meets)
                "zip" if lower.contains("rom") || looks_like_rom_set(file_name(p)) => Kind::Rom,
                "zip" | "7z" | "rar" => Kind::InnerArchive,
                // every text file outside the packs is something to read (ours: VPin Studio's read-me
                // is one named so; a media pack's `INSTALLATION INSTRUCTIONS.txt` went unsorted)
                "txt" => Kind::Readme,
                "pdf" => Kind::Manual,
                _ if music_folder.as_deref().is_some_and(|f| below(f, p)) && MUSIC_SUFFIXES.contains(&x.as_str()) => Kind::Music,
                _ if loose_music && !p.contains('/') && MUSIC_SUFFIXES.contains(&x.as_str()) => Kind::Music,
                // a picture beside the music, in its folder or the one holding it, that names no screen:
                // the pack's own note on where its music goes, not a picture to install
                _ if PICTURE_SUFFIXES.contains(&x.as_str())
                    && music_folder.as_deref().is_some_and(|f| folder(p) == Some(f) || folder(f).is_some_and(|holding| folder(p) == Some(holding)))
                    && !MEDIA_SCREENS.iter().any(|s| lower.contains(s)) =>
                {
                    Kind::Readme
                }
                _ if MEDIA_SUFFIXES.contains(&x.as_str()) && !lower.contains("screenshot") && MEDIA_SCREENS.iter().any(|s| lower.contains(s)) => Kind::Media,
                _ => Kind::Other,
            }
        };
        if kind == Kind::Readme && readme.is_none() && x == "txt" {
            readme = listing.read_text(p, 64 * 1024);
        }
        files.push((e.clone(), kind));
    }
    let rom_set = match &listing.source {
        crate::archive::Source::Archive(a) if rom_set => Some(a.name()),
        _ => None,
    };
    Contents { files, pup_root, altsound_folder, dmd_folder, music_folder, readme, rom_set }
}

/// A PinUP pack's top folder (VPin Studio's `getPupPackRootDirectory`): the shortest folder
/// holding one of a pack's own files, unless its name has a space; else, VPin Studio's fallback, a
/// folder of many folders full of videos.
fn pup_root(names: &[&str], folders: &[&str]) -> Option<String> {
    let mut best: Option<String> = None;
    for n in names {
        let lower = n.to_lowercase();
        let marker = n.contains("screens.pup")
            || (lower.contains("option") && lower.ends_with(".bat"))
            || n.contains("EditThisPuPPack.bat")
            || (lower.contains("screen") && lower.ends_with(".bat"))
            || n.contains("scriptonly.txt");
        if marker && let Some(d) = folder(n) {
            let path = format!("{d}/");
            if best.as_ref().is_none_or(|b| b.len() > path.len()) {
                best = Some(path);
            }
        }
    }
    if let Some(b) = &best {
        let name = b.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        if name.contains(' ') {
            best = None;
        }
    }
    best.or_else(|| {
        let mut by_parent: std::collections::BTreeMap<&str, std::collections::BTreeSet<&str>> = Default::default();
        for f in folders {
            if let Some((parent, sub)) = f.rsplit_once('/') {
                by_parent.entry(parent).or_default().insert(sub);
            }
        }
        let mut found: Option<(String, usize)> = None;
        for (parent, subs) in by_parent {
            if subs.len() < PUP_MIN_FOLDERS || found.as_ref().is_some_and(|(_, n)| subs.len() <= *n) {
                continue;
            }
            let videos = names.iter().filter(|n| below(parent, n) && VIDEO_SUFFIXES.contains(&ext(n).as_str())).count();
            if videos >= PUP_MIN_VIDEOS {
                found = Some((format!("{parent}/"), subs.len()));
            }
        }
        found.map(|(p, _)| p)
    })
}

/// An UltraDMD or FlexDMD folder (VPin Studio's `getDMDPath`).
fn dmd_folder(folders: &[&str], pup_root: Option<&str>) -> Option<String> {
    for f in folders {
        if pup_root.is_some_and(|r| below(r, f)) {
            continue;
        }
        for segment in f.split('/') {
            if segment.contains("UltraDMD") || segment.contains("FlexDMD") {
                let end = f.find(segment).map_or(f.len(), |p| p + segment.len());
                return Some(f[..end].to_string());
            }
            if (segment.ends_with("DMD") || segment.to_lowercase().ends_with(".flex"))
                && !segment.eq_ignore_ascii_case("DMD")
                && !segment.eq_ignore_ascii_case("FullDMD")
                && !segment.contains(' ')
            {
                return Some(f.to_string());
            }
        }
    }
    None
}

/// The folder music is in (VPin Studio's `getMusicFolder`): the first sound file in a folder,
/// outside a PinUP pack and a DMD folder, unless videos lie there too.
fn music_folder(names: &[&str], pup_root: Option<&str>, dmd: Option<&str>) -> Option<String> {
    let found = names.iter().find(|n| {
        MUSIC_SUFFIXES.contains(&ext(n).as_str()) && !pup_root.is_some_and(|r| below(r, n)) && !dmd.is_some_and(|d| n.contains(d)) && n.contains('/')
    })?;
    let path = folder(found)?.to_string();
    let videos = names.iter().any(|n| VIDEO_SUFFIXES.contains(&ext(n).as_str()) && below(&path, n));
    (!videos).then_some(path)
}

impl Contents {
    pub fn of(&self, kind: Kind) -> impl Iterator<Item = &Entry> {
        self.files.iter().filter(move |(_, k)| *k == kind).map(|(e, _)| e)
    }

    /// The tables in the drop.
    pub fn tables(&self) -> Vec<&Entry> {
        self.of(Kind::Table).collect()
    }

    /// The ROM names the drop gives: its own, a ROM set's; its ROM zips', its PinUP pack's folder,
    /// its AltSound pack's, its colouring's and machine memory's names; each once, in that order.
    pub fn rom_hints(&self) -> Vec<String> {
        let stem = |p: &str| file_name(p).rsplit_once('.').map_or(file_name(p), |(s, _)| s).to_string();
        let mut out: Vec<String> = Vec::new();
        let mut add = |s: String| {
            if !s.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&s)) {
                out.push(s);
            }
        };
        if let Some(r) = &self.rom_set {
            add(r.clone());
        }
        for e in self.of(Kind::Rom) {
            if ext(&e.path) == "zip" {
                add(stem(&e.path));
            }
        }
        if let Some(r) = &self.pup_root {
            add(r.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string());
        }
        if let Some(a) = &self.altsound_folder {
            add(a.rsplit('/').next().unwrap_or("").to_string());
        }
        for (e, k) in &self.files {
            if matches!(k, Kind::Colouring(_) | Kind::Nvram) {
                add(stem(&e.path));
            }
        }
        out
    }

    /// A count of each kind of file, for a summary.
    pub fn summary(&self) -> Vec<(Kind, usize)> {
        let mut out: Vec<(Kind, usize)> = Vec::new();
        for (_, k) in &self.files {
            match out.iter_mut().find(|(o, _)| o == k) {
                Some((_, n)) => *n += 1,
                None => out.push((*k, 1)),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Entry, Listing, Source};

    fn listing(paths: &[&str]) -> Listing {
        Listing { source: Source::Folder("x".into()), entries: paths.iter().map(|p| Entry { path: p.to_string(), size: 1, modified: None }).collect() }
    }

    #[test]
    fn a_table_download_sorted_out() {
        let c = analyse(&listing(&[
            "Attack from Mars (Bally 1995) 6.0.vpx",
            "Attack from Mars (Bally 1995) 6.0.directb2s",
            "Attack from Mars (Bally 1995) 6.0.ini",
            "roms/afm_113b.zip",
            "afm_113b/screens.pup",
            "afm_113b/Backglass/intro.mp4",
            "altsound/afm_113b/altsound.csv",
            "altsound/afm_113b/jingle/a.ogg",
            "altcolor/afm_113b/afm_113b.cRZ",
            "Read Me.txt",
            "__MACOSX/._x",
        ]));
        let kind = |p: &str| c.files.iter().find(|(e, _)| e.path == p).map(|(_, k)| *k);
        assert_eq!(kind("Attack from Mars (Bally 1995) 6.0.vpx"), Some(Kind::Table));
        assert_eq!(kind("Attack from Mars (Bally 1995) 6.0.directb2s"), Some(Kind::Backglass));
        assert_eq!(kind("roms/afm_113b.zip"), Some(Kind::Rom));
        assert_eq!(kind("afm_113b/Backglass/intro.mp4"), Some(Kind::PupPack));
        assert_eq!(kind("altsound/afm_113b/jingle/a.ogg"), Some(Kind::AltSound));
        assert_eq!(kind("altcolor/afm_113b/afm_113b.cRZ"), Some(Kind::Colouring(Colouring::Serum)));
        assert_eq!(kind("Read Me.txt"), Some(Kind::Readme));
        assert_eq!(kind("__MACOSX/._x"), None);
        assert_eq!(c.pup_root.as_deref(), Some("afm_113b/"));
        assert_eq!(c.rom_hints(), ["afm_113b"]);
    }

    #[test]
    fn a_media_packs_flyer_and_instruction_card_are_front_end_media() {
        let c = analyse(&listing(&["Flyer/flyer1.png", "Instructions/full instructionscard.png", "Credits/Credits.png", "Animated Topper/topper.mp4"]));
        let kind = |p: &str| c.files.iter().find(|(e, _)| e.path == p).map(|(_, k)| *k);
        assert_eq!(kind("Flyer/flyer1.png"), Some(Kind::Media));
        assert_eq!(kind("Instructions/full instructionscard.png"), Some(Kind::Media));
        assert_eq!(kind("Animated Topper/topper.mp4"), Some(Kind::Media));
        assert_eq!(kind("Credits/Credits.png"), Some(Kind::Media));
        assert_eq!(crate::plan::media_folder("Credits/Credits.png"), Some("GameInfo"));
        assert_eq!(crate::plan::media_folder("Flyer/flyer1.png"), Some("GameInfo"));
        assert_eq!(crate::plan::media_folder("Instructions/full instructionscard.png"), Some("GameHelp"));
    }

    #[test]
    fn a_media_packs_logos_manual_instructions_and_note_on_its_music() {
        // as Blizzard of Ozz's media pack lays them out
        let c = analyse(&listing(&[
            "Media Pack/Addable Assets/Logos/Logo.png",
            "Media Pack/INSTALLATION INSTRUCTIONS.txt",
            "Media Pack/Instruction Manual/Manual.pdf",
            "Media Pack/Music Files/BLIZZARD/01 - Song.mp3",
            "Media Pack/Music Files/MUSIC.png",
        ]));
        let kind = |p: &str| c.files.iter().find(|(e, _)| e.path == p).map(|(_, k)| *k);
        assert_eq!(kind("Media Pack/Addable Assets/Logos/Logo.png"), Some(Kind::Media));
        assert_eq!(crate::plan::media_folder("Media Pack/Addable Assets/Logos/Logo.png"), Some("Wheel"));
        assert_eq!(kind("Media Pack/INSTALLATION INSTRUCTIONS.txt"), Some(Kind::Readme));
        assert_eq!(kind("Media Pack/Instruction Manual/Manual.pdf"), Some(Kind::Manual));
        assert_eq!(kind("Media Pack/Music Files/BLIZZARD/01 - Song.mp3"), Some(Kind::Music));
        assert_eq!(kind("Media Pack/Music Files/MUSIC.png"), Some(Kind::Readme));
        // its music below a folder named as its own, its manual named for the table
        let layout = crate::plan::Layout::under(std::path::Path::new("V"));
        let steps = crate::plan::plan(&c, &Source::Folder("x".into()), Some("Blizzard"), None, None, &[], &layout);
        let to = |p: &str| steps.iter().find(|s| s.from == p).and_then(|s| s.to.clone());
        assert_eq!(to("Media Pack/Music Files/BLIZZARD/01 - Song.mp3"), Some(layout.tables.join("music").join("BLIZZARD/01 - Song.mp3")));
        assert_eq!(to("Media Pack/Instruction Manual/Manual.pdf"), Some(layout.tables.join("Blizzard.pdf")));
        assert_eq!(to("Media Pack/Music Files/MUSIC.png"), None);
    }

    #[test]
    fn a_tables_two_backglasses_both_kept_the_alternative_numbered() {
        let c = analyse(&listing(&["B2s/AlternateBLIZZARDOFOZZ.directb2s", "B2s/BlizzardofOZZ.directb2s"]));
        let layout = crate::plan::Layout::under(std::path::Path::new("V-no-such-folder"));
        let steps = crate::plan::plan(&c, &Source::Folder("x".into()), Some("Blizzard"), None, None, &[], &layout);
        let to = |p: &str| steps.iter().find(|s| s.from == p).and_then(|s| s.to.clone());
        assert_eq!(to("B2s/BlizzardofOZZ.directb2s"), Some(layout.tables.join("Blizzard.directb2s")));
        assert_eq!(to("B2s/AlternateBLIZZARDOFOZZ.directb2s"), Some(layout.tables.join("Blizzard (1).directb2s")));
    }

    #[test]
    fn a_physics_file_goes_beside_its_table_named_for_it() {
        let c = analyse(&listing(&["nFozzy physics.vpp"]));
        assert_eq!(c.files[0].1, Kind::Physics);
        let layout = crate::plan::Layout::under(std::path::Path::new("V"));
        let steps = crate::plan::plan(&c, &Source::Folder("x".into()), Some("Attack from Mars"), None, None, &[], &layout);
        assert_eq!(steps[0].to, Some(layout.tables.join("Attack from Mars.vpp")));
    }

    #[test]
    fn sound_files_beside_a_table_are_its_music() {
        let c = analyse(&listing(&["Tom and Jerry.vpx", "bgout_T&Jmultiballmadness.mp3", "roms/tomjerry.zip"]));
        let kind = |p: &str| c.files.iter().find(|(e, _)| e.path == p).map(|(_, k)| *k);
        assert_eq!(kind("bgout_T&Jmultiballmadness.mp3"), Some(Kind::Music));
        // alone, with no table, they're not
        let mut c = analyse(&listing(&["a.mp3", "a.txt"]));
        assert_eq!(c.files[0].1, Kind::Other);
        // until they're for a table in the library
        loose_music_for_a_table(&mut c);
        assert_eq!((c.files[0].1, c.files[1].1), (Kind::Music, Kind::Readme));
    }

    #[test]
    fn a_rom_set_alone() {
        let c = analyse(&listing(&["afm_113b.bin", "afm_s2.l1", "afm_s3.l1"]));
        assert!(c.files.iter().all(|(_, k)| *k == Kind::Rom));
    }

    #[test]
    fn a_pack_without_its_own_files() {
        let mut paths: Vec<String> = (0..10).map(|i| format!("pack/screen{i}/v.mp4")).collect();
        paths.push("pack/screen0/w.mp4".into());
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        assert_eq!(analyse(&listing(&refs)).pup_root.as_deref(), Some("pack/"));
    }
}
