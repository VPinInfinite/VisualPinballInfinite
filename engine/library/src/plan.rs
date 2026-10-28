//! Where each file of a drop would go, in the layout the player reads today —
//! VPX's own: tables and what's named for them in the tables folder, ROMs,
//! AltSound packs and colourings in VPinMAME's folder, PinUP packs in PinUP's
//! `PUPVideos`, the machine's memory in the player's data folder. Only a plan:
//! nothing is copied by it. The app's own layout, and how a drop is shown and
//! confirmed, are the app's. Ours.

use std::path::{Path, PathBuf};

use crate::analysis::{Contents, Kind};
use crate::archive::{Format, Source};

/// The folders a drop's files go into.
#[derive(Clone, Debug)]
pub struct Layout {
    pub tables: PathBuf,
    /// VPX's scripts (`core.vbs` and the rest), where tables load theirs from.
    pub scripts: PathBuf,
    pub vpinmame: PathBuf,
    pub pup_videos: PathBuf,
    /// The player's data folder (`%APPDATA%\Visual Pinball Infinite`), for the machine's memory.
    pub data: PathBuf,
    /// The folders PinballX and PinballY are looked for in, for their media (`frontends::find`).
    pub front_ends: Vec<PathBuf>,
    /// The front end whose folders a drop's media go in, where the user chose one
    /// (`frontends::media_target`).
    pub media_front: Option<crate::frontends::Front>,
    /// Tables kept in a folder for each game: a table being added goes into its game's.
    pub per_game: bool,
}

impl Layout {
    /// VPX's layout under `root` (`Tables`, `Scripts`, `VPinMAME`, `PUPVideos`), for checks in a
    /// scratch folder.
    pub fn under(root: &Path) -> Layout {
        Layout {
            tables: root.join("Tables"),
            scripts: root.join("Scripts"),
            vpinmame: root.join("VPinMAME"),
            pup_videos: root.join("PUPVideos"),
            data: root.join("data"),
            // only those inside it, never this PC's own
            front_ends: vec![root.join("PinballX"), root.join("PinballY")],
            media_front: None,
            per_game: false,
        }
    }

    /// The library the settings name (first setup): its tables', scripts', PinMAME's and
    /// PinUP packs' folders, those not named beside the tables' as VPX lays them out (`Scripts`,
    /// `VPinMAME`, and `PinUPSystem\PUPVideos` above them).
    pub fn from_folders(f: &vpi_settings::LibraryFolders) -> Layout {
        let root = f.tables.parent().map(Path::to_path_buf).unwrap_or_else(|| f.tables.clone());
        Layout {
            tables: f.tables.clone(),
            scripts: f.scripts.clone().unwrap_or_else(|| root.join("Scripts")),
            vpinmame: f.pinmame.clone().unwrap_or_else(|| root.join("VPinMAME")),
            pup_videos: f.pinup.clone().unwrap_or_else(|| root.parent().unwrap_or(&root).join("PinUPSystem").join("PUPVideos")),
            data: std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Visual Pinball Infinite")).unwrap_or_default(),
            front_ends: f.pinballx.iter().chain(f.pinbally.iter()).cloned().chain(crate::frontends::usual_places(&f.tables)).collect(),
            media_front: None,
            per_game: f.per_game,
        }
    }

    /// The reference install's layout (`C:\vPinball`), for showing a plan while developing.
    pub fn reference() -> Layout {
        Layout {
            tables: PathBuf::from(r"C:\vPinball\VisualPinball\Tables"),
            scripts: PathBuf::from(r"C:\vPinball\VisualPinball\Scripts"),
            vpinmame: PathBuf::from(r"C:\vPinball\VisualPinball\VPinMAME"),
            pup_videos: PathBuf::from(r"C:\vPinball\PinUPSystem\PUPVideos"),
            data: std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Visual Pinball Infinite")).unwrap_or_default(),
            front_ends: crate::frontends::usual_places(Path::new(r"C:\vPinball\VisualPinball\Tables")),
            media_front: None,
            per_game: false,
        }
    }
}

/// One file's place.
#[derive(Clone, Debug)]
pub struct Step {
    /// The file in the drop (the drop itself for a ROM set dropped whole).
    pub from: String,
    pub kind: Kind,
    /// Its size, in bytes.
    pub size: u64,
    /// Where it goes; None when it doesn't.
    pub to: Option<PathBuf>,
    /// Why not, or what to know.
    pub note: Option<String>,
}

/// What's at a step's place already.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum There {
    Nothing,
    /// A file of the same size: most likely the same file.
    SameSize,
    /// A different file, which this one would replace.
    Different,
    /// A file of that name, which sizes can't be compared with: this one is to be made anew (a ROM
    /// set from another kind of archive made a zip).
    Named,
}

/// Where a drop's files go, the table's own files named for the table (`table`, its file name's
/// stem), the machine's for its ROM (`rom`), a PinUP pack for the name the table's script hands
/// PinUP (`pup`: often its ROM, not always — The Goonies' script asks for `TheGoonies`, its ROM
/// `goonies`), else its ROM; and a script file the table's script
/// loads (`helpers`) with VPX's scripts, under its own name.
/// PinUP Popper's media folder for a front-end media file, by the words in its own name, else in
/// its folder's (VPin Studio's screens: Menu is its Full DMD and apron).
pub fn media_folder(path: &str) -> Option<&'static str> {
    let parts: Vec<String> = path.rsplit('/').take(2).map(str::to_lowercase).collect();
    parts.iter().find_map(|low| {
        let has = |w: &str| low.contains(w);
        Some(if has("audiolaunch") || has("launch audio") || has("launchaudio") {
            "AudioLaunch"
        } else if has("fulldmd") || has("full dmd") || has("apron") || has("menu") {
            "Menu"
        } else if has("backglass") || has("back glass") {
            "BackGlass"
        } else if has("playfield") || has("play field") || has("table video") || has("table image") {
            "PlayField"
        } else if has("wheel") || has("logo") {
            "Wheel"
        } else if has("topper") {
            "Topper"
        } else if has("loading") {
            "Loading"
        } else if has("gameinfo") || has("game info") || has("flyer") || has("credits") {
            "GameInfo"
        } else if has("gamehelp") || has("game help") || has("rules") || has("instruction") {
            "GameHelp"
        } else if has("gameselect") || has("game select") {
            "GameSelect"
        } else if has("dmd") {
            "DMD"
        } else if has("audio") {
            "Audio"
        } else {
            return None;
        })
    })
}

/// A Popper media folder in words.
pub fn media_label(folder: &str) -> &'static str {
    match folder {
        "AudioLaunch" => "launch audio",
        "Menu" => "Full DMD",
        "BackGlass" => "backglass",
        "PlayField" => "playfield",
        "Wheel" => "wheel",
        "Topper" => "topper",
        "Loading" => "loading",
        "GameInfo" => "game info",
        "GameHelp" => "game help",
        "GameSelect" => "game select",
        "DMD" => "DMD",
        "Audio" => "audio",
        _ => "other",
    }
}

/// PinUP Popper's media folders the user can put a file in, with each one's words.
pub const MEDIA_FOLDERS: [(&str, &str, &str); 12] = [
    ("media:Wheel", "PinUP Popper: wheel", "Wheel"),
    ("media:PlayField", "PinUP Popper: playfield", "PlayField"),
    ("media:BackGlass", "PinUP Popper: backglass", "BackGlass"),
    ("media:Topper", "PinUP Popper: topper", "Topper"),
    ("media:DMD", "PinUP Popper: DMD", "DMD"),
    ("media:Menu", "PinUP Popper: Full DMD", "Menu"),
    ("media:Loading", "PinUP Popper: loading", "Loading"),
    ("media:GameInfo", "PinUP Popper: game info (flyers, credits)", "GameInfo"),
    ("media:GameHelp", "PinUP Popper: game help (instruction cards, rules)", "GameHelp"),
    ("media:GameSelect", "PinUP Popper: game select", "GameSelect"),
    ("media:Audio", "PinUP Popper: audio", "Audio"),
    ("media:AudioLaunch", "PinUP Popper: launch audio", "AudioLaunch"),
];

/// A backglass named as an alternative to the table's own (`AlternateBLIZZARDOFOZZ.directb2s`).
fn alternative(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    name.starts_with("alt") || name.contains("alternat") || [" alt", "_alt", "-alt", "(alt"].iter().any(|w| name.contains(w))
}

/// A further version's place beside its table, as VPin Studio numbers a backglass's versions
/// (`<table> (1).directb2s` and on): the first number neither given already in this plan nor
/// holding another file — one holding a file of the same size is most likely this one.
fn version_place(dir: &Path, table: &str, ext: &str, size: u64, given: &mut Vec<u32>) -> PathBuf {
    let place = |n: u32| dir.join(format!("{table} ({n}).{ext}"));
    let n = (1..).find(|n| !given.contains(n) && std::fs::metadata(place(*n)).map_or(true, |m| m.len() == size)).expect("a free number");
    given.push(n);
    place(n)
}

pub fn plan(contents: &Contents, source: &Source, table: Option<&str>, rom: Option<&str>, pup: Option<&str>, helpers: &[String], layout: &Layout) -> Vec<Step> {
    let mut steps = Vec::new();
    // a table's backglasses: the one that plays named for it, the first not named as an alternative;
    // the others kept beside it as VPin Studio keeps a backglass's versions, to choose from on its page
    let backglasses: Vec<&str> = contents.files.iter().filter(|(_, k)| *k == Kind::Backglass).map(|(e, _)| e.path.as_str()).collect();
    let playing = backglasses.iter().copied().find(|p| !alternative(p)).or(backglasses.first().copied());
    let mut versions: Vec<u32> = Vec::new();
    // manuals named for the table, the first `<table>.pdf`, the rest numbered
    let mut manuals: Vec<u32> = Vec::new();
    let mut first_manual = true;
    let named = |path: &str| -> String { path.rsplit('/').next().unwrap_or(path).to_string() };
    let ext = |path: &str| path.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
    // the front end a drop's media go in, and the name they go under in PinballX's or PinballY's
    let target = if contents.files.iter().any(|(_, k)| *k == Kind::Media) { crate::frontends::media_target(layout) } else { None };
    let other_name = match (&target, table) {
        (Some(crate::frontends::Target::Other(fe)), Some(t)) => Some(fe.media_names().get(&t.to_lowercase()).cloned().unwrap_or_else(|| t.to_string())),
        _ => None,
    };
    // front-end media named for the table in each of the front end's folders: the first
    // `<table>.png`, the next `<table>01.png` and on, as PinUP Popper numbers them, or `<name> 1.png`
    // in PinballX's and PinballY's, which show one of a kind (whatever its file's kind); in the
    // download's order — but a table's logos after its wheels, so a wheel of its own is the one shown
    let mut media_number: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    {
        let mut order: Vec<&str> = contents.files.iter().filter(|(_, k)| *k == Kind::Media).map(|(e, _)| e.path.as_str()).collect();
        order.sort_by_key(|p| p.to_lowercase().contains("logo"));
        let mut count: std::collections::HashMap<(&str, String), usize> = std::collections::HashMap::new();
        for p in order {
            if let Some(folder) = media_folder(p) {
                let family = match &target {
                    Some(crate::frontends::Target::Other(_)) => crate::frontends::VIDEOS.contains(&ext(p).to_lowercase().as_str()).to_string(),
                    _ => ext(p).to_lowercase(),
                };
                let n = count.entry((folder, family)).or_insert(0);
                media_number.insert(p, *n);
                *n += 1;
            }
        }
    }
    let below = |folder: &str, path: &str| path.strip_prefix(folder.trim_end_matches('/')).map(|r| r.trim_start_matches('/').to_string()).unwrap_or_default();
    // a ROM set dropped whole goes as it is, a zip; in another kind of archive, made a zip, the
    // only kind PinMAME reads
    if let Source::Archive(a) = source
        && !contents.files.is_empty()
        && contents.files.iter().all(|(_, k)| *k == Kind::Rom)
    {
        let from = named(&a.path().to_string_lossy().replace('\\', "/"));
        let size = a.parts.iter().map(|p| std::fs::metadata(p).map_or(0, |m| m.len())).sum();
        let (to, note) = match a.format {
            Format::Zip => (from.clone(), None),
            other => (format!("{}.zip", a.name()), Some(format!("from a {} into a zip, the only kind PinMAME reads", other.describe()))),
        };
        steps.push(Step { from, kind: Kind::Rom, size, to: Some(layout.vpinmame.join("roms").join(to)), note });
        return steps;
    }
    let rom_or = |fallback: &str| rom.map(str::to_string).unwrap_or_else(|| fallback.to_string());
    for (entry, kind) in &contents.files {
        let p = entry.path.as_str();
        let (to, note): (Option<PathBuf>, Option<String>) = match kind {
            Kind::Table => (Some(layout.tables.join(named(p))), None),
            // a script its table loads goes with VPX's scripts under its own name; one named like a
            // table's file replaces its table's own, named for it; another, named like a script
            // file (`KISSCore.vbs`), is a script tables load too — never renamed for the table, which
            // would play it in the table's own script's place (KISS's downloads did that)
            Kind::Script if helpers.iter().any(|h| h.eq_ignore_ascii_case(&named(p))) => (Some(layout.scripts.join(named(p))), Some("a script its table loads, put with VPX's scripts".into())),
            Kind::HelperScript => (Some(layout.scripts.join(named(p))), Some("a script its table loads, put with VPX's scripts".into())),
            Kind::Script if !table.is_some_and(|t| named(p).eq_ignore_ascii_case(&format!("{t}.vbs"))) && !named(p).contains([' ', '(', ')']) => {
                (Some(layout.scripts.join(named(p))), Some("a script file tables load, put with VPX's scripts".into()))
            }
            Kind::Backglass if table.is_some() && backglasses.len() > 1 && playing != Some(p) => {
                let to = version_place(&layout.tables, table.unwrap_or_default(), &ext(p), entry.size, &mut versions);
                let kept = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                (Some(to), Some(format!("another backglass for it, kept beside it as “{kept}”: which one plays is chosen on its page")))
            }
            Kind::Backglass | Kind::Pov | Kind::Physics | Kind::TableSettings | Kind::Script | Kind::BackglassPlaces => match table {
                Some(t) => {
                    let to = layout.tables.join(format!("{t}.{}", ext(p)));
                    let mut said = Vec::new();
                    if named(p) != format!("{t}.{}", ext(p)) {
                        said.push("named for its table, as VPX and B2S look for it".to_string());
                    }
                    if *kind == Kind::Backglass && backglasses.len() > 1 {
                        said.push("the one that plays; the others are kept beside it".to_string());
                    }
                    (Some(to), (!said.is_empty()).then(|| said.join(": ")))
                }
                None => (Some(layout.tables.join(named(p))), Some("which table it's for: to choose".into())),
            },
            Kind::Manual => match table {
                Some(t) => {
                    let to = if std::mem::take(&mut first_manual) { layout.tables.join(format!("{t}.{}", ext(p))) } else { version_place(&layout.tables, t, &ext(p), entry.size, &mut manuals) };
                    (Some(to), Some("to read, kept beside its table, named for it: its page shows it".into()))
                }
                None => (None, Some("which table it's for: to choose".into())),
            },
            Kind::TablePicture => match table {
                Some(t) => (Some(layout.tables.join(format!("{t} pictures")).join(named(p))), None),
                None => (None, Some("which table it's for: to choose".into())),
            },
            Kind::Rom => (Some(layout.vpinmame.join("roms").join(named(p))), None),
            Kind::PupPack => {
                let root = contents.pup_root.as_deref().unwrap_or("");
                let pack = root.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string();
                let name = pup.map(str::to_string).unwrap_or_else(|| rom_or(&pack));
                let note = (!pack.is_empty() && !pack.eq_ignore_ascii_case(&name)).then(|| match pup {
                    Some(_) => format!("the pack's folder `{pack}` named `{name}`, as its table's script asks PinUP for it"),
                    None => format!("the pack's folder `{pack}` named for the game, `{name}`"),
                });
                (Some(layout.pup_videos.join(&name).join(below(root, p))), note)
            }
            Kind::AltSound => {
                let folder = contents.altsound_folder.as_deref().unwrap_or("");
                let pack = folder.rsplit('/').next().unwrap_or("").to_string();
                (Some(layout.vpinmame.join("altsound").join(rom_or(&pack)).join(below(folder, p))), None)
            }
            Kind::Colouring(_) => {
                let stem = named(p).rsplit_once('.').map_or(named(p), |(s, _)| s.to_string());
                (Some(layout.vpinmame.join("altcolor").join(rom_or(&stem)).join(named(p))), None)
            }
            Kind::DmdFolder => {
                let folder = contents.dmd_folder.as_deref().unwrap_or("");
                let name = folder.rsplit('/').next().unwrap_or(folder);
                (Some(layout.tables.join(name).join(below(folder, p))), None)
            }
            Kind::Music => {
                // as the script names it, below a `music` folder; else in a folder named as the
                // download's music folder is (`Music Files/BLIZZARD/…` as `BLIZZARD/…`), the drop
                // putting each where its table's script plays it from when the script says
                // (`drop::music_as_its_script_plays`)
                let lower = p.to_lowercase();
                let rel = match (lower.find("music/"), contents.music_folder.as_deref()) {
                    (Some(at), _) => p[at + 6..].to_string(),
                    (None, Some(f)) if p.starts_with(&format!("{f}/")) => format!("{}/{}", f.rsplit('/').next().unwrap_or(f), below(f, p)),
                    (None, _) => p.to_string(),
                };
                (Some(layout.tables.join("music").join(rel)), None)
            }
            Kind::Nvram => (Some(layout.data.join("PinMAME").join("nvram").join(named(p))), Some("the machine's memory: its scores and settings replaced".into())),
            Kind::PinmameSettings => (Some(layout.data.join("PinMAME").join("cfg").join(named(p))), None),
            // a VPU patch makes the modded table beside its original, which stays as it is
            Kind::Patch => match table {
                Some(t) => {
                    let mod_name = named(p).rsplit_once('.').map_or(named(p), |(s, _)| s.to_string());
                    (Some(layout.tables.join(format!("{t} - {mod_name}.vpx"))), Some(format!("makes a new table, “{t} - {mod_name}”, from {t}.vpx, which stays as it is; its backglass, settings and POV copied for the new one")))
                }
                None => (None, Some("which table it patches: to choose".into())),
            },
            // front-end media into the front end's folder for its screen: PinUP Popper's named for its
            // table as Popper names them, for its own name for the game where it has one; PinballX's
            // and PinballY's for the table's description in their game lists; what can't be told
            // apart is the user's to place
            Kind::Media => match (table, &target, media_folder(p)) {
                (Some(t), Some(crate::frontends::Target::Popper(root)), Some(folder)) => {
                    let t = crate::popper::media_name(layout, t);
                    let n = media_number.get(p).copied().unwrap_or(0);
                    let name = if n == 0 { format!("{t}.{}", ext(p)) } else { format!("{t}{n:02}.{}", ext(p)) };
                    (Some(root.join(folder).join(name)), Some(format!("PinUP Popper's {} media, named for its table", media_label(folder))))
                }
                (Some(t), Some(crate::frontends::Target::Other(fe)), Some(folder)) => {
                    let video = crate::frontends::VIDEOS.contains(&ext(p).to_lowercase().as_str());
                    match fe.place(folder, video) {
                        Some(dir) => {
                            let name = other_name.clone().unwrap_or_else(|| t.to_string());
                            let n = media_number.get(p).copied().unwrap_or(0);
                            let file = if n == 0 { format!("{name}.{}", ext(p)) } else { format!("{name} {n}.{}", ext(p)) };
                            let shown = if n == 0 { "named for its table" } else { "kept numbered beside the one it shows" };
                            (Some(dir.join(file)), Some(format!("{}'s {} media, {shown}", fe.front.name(), media_label(folder))))
                        }
                        None => (None, Some(format!("front-end media: {} has no folder for {} {}", fe.front.name(), media_label(folder), if video { "videos" } else { "pictures" }))),
                    }
                }
                (None, _, _) => (None, Some("front-end media: which table it's for, to choose".into())),
                (_, None, _) => (None, Some("front-end media: no front end's media folder is there (PinUP Popper's, PinballX's or PinballY's)".into())),
                (_, _, None) => (None, Some("front-end media: which screen it's for, to choose".into())),
            },
            Kind::OldTable | Kind::FuturePinball => (None, Some("not a table ours plays".into())),
            Kind::InnerArchive => (None, Some("an archive inside: drop it by itself".into())),
            Kind::Readme if crate::analysis::PICTURE_SUFFIXES.contains(&ext(p).to_lowercase().as_str()) => (None, Some("a picture of its instructions, not installed".into())),
            Kind::Readme => (None, Some("shown, not installed".into())),
            Kind::Other => (None, None),
        };
        // a script going with VPX's scripts is one its table loads
        let kind = if *kind == Kind::Script && to.as_deref().is_some_and(|t| t.starts_with(&layout.scripts)) { Kind::HelperScript } else { *kind };
        steps.push(Step { from: p.to_string(), kind, size: entry.size, to, note });
    }
    steps
}

/// What's at the place a file would land.
pub fn there(step: &Step) -> There {
    step.to.as_deref().map_or(There::Nothing, |to| there_at(to, &step.from, step.size))
}

/// What's at `to` for a file of the drop (`from`, `size` bytes) put there.
pub fn there_at(to: &Path, from: &str, size: u64) -> There {
    let ext = |p: &str| p.rsplit_once('.').map(|(_, e)| e.to_lowercase());
    let remade = ext(&to.to_string_lossy()) != ext(from);
    match std::fs::metadata(to) {
        Ok(_) if remade => There::Named,
        Ok(m) if m.len() == size => There::SameSize,
        Ok(_) => There::Different,
        Err(_) => There::Nothing,
    }
}

/// The folder a pack's files go into: the deepest folder all their places share.
pub fn common_folder<'a>(places: impl IntoIterator<Item = &'a Path>) -> Option<PathBuf> {
    let mut common: Option<PathBuf> = None;
    for p in places {
        let parent = p.parent()?;
        common = Some(match common {
            None => parent.to_path_buf(),
            Some(c) => c.ancestors().find(|a| parent.starts_with(a)).map(Path::to_path_buf).unwrap_or_default(),
        });
    }
    common
}
