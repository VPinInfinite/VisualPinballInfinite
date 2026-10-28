//! A table taken out of the library, and what goes with it: its own files beside it, its ROM and
//! what's kept for the ROM's set (its AltSound pack, its colouring, the machine's memory our player
//! keeps), its PinUP pack, its music, the scripts and folders it loads, the media PinUP Popper shows
//! for it and its game in Popper's list, and the values its script keeps — each offered ticked
//! unless another table uses it too. The ticked files and folders go to the Recycle Bin; Popper's
//! game is taken out of its list with its rows in Popper's playlists and play counts, as VPin
//! Studio takes one out, the list kept first; a script's values are taken out of our player's
//! `VPReg.ini`, the file kept first. Nothing is done while a program that holds them runs. Ours.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::installed::{Holds, Read};
use crate::plan::Layout;
use crate::popper::{self, Game};

/// A table as a removal sees it: its name, its file, what it says of itself and what it holds.
pub struct Seen<'a> {
    pub name: &'a str,
    pub path: &'a Path,
    pub read: &'a Read,
    pub holds: &'a Holds,
}

/// What an item does when it's ticked.
#[derive(Clone, Debug)]
pub enum Act {
    /// Its files and folders, to the Recycle Bin.
    Recycle(Vec<PathBuf>),
    /// The values kept under these names in our player's `VPReg.ini`, taken out of it.
    Values(PathBuf, Vec<String>),
    /// PinUP Popper's game for it, taken out of Popper's list (`PUPDatabase.db`).
    PopperGame(PathBuf, Game),
    /// PinballX's or PinballY's game for it, by its name, taken out of that game list (`.xml`).
    FrontGame(PathBuf, String),
    /// The caller's own (the app's record of its plays): done by the caller.
    Own,
}

/// One thing a removal can take.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// What it is, the same from one plan to the next: `table`, `backglass`, `rom`, `helper:<path>`…
    pub key: String,
    /// What it is to the table, in words: "The table", "Its ROM".
    pub what: String,
    /// Its name: a file's or a folder's, else what it holds.
    pub name: String,
    pub bytes: u64,
    /// How many files it is: a folder's, PinUP Popper's media; none for what isn't a file.
    pub files: usize,
    /// The other tables that use it too, by name.
    pub shared: Vec<String>,
    /// Ticked to start with: nothing else uses it.
    pub ticked: bool,
    #[serde(skip)]
    pub act: Act,
}

impl Item {
    /// An item that's the caller's own to do (`Act::Own`).
    pub fn own(key: &str, what: &str, name: String, ticked: bool) -> Item {
        Item { key: key.into(), what: what.into(), name, bytes: 0, files: 0, shared: Vec::new(), ticked, act: Act::Own }
    }

    /// Files and folders, to the Recycle Bin: those that are there; none, no item.
    pub fn recycle(key: impl Into<String>, what: &str, paths: Vec<PathBuf>, shared: Vec<String>) -> Option<Item> {
        let paths: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
        if paths.is_empty() {
            return None;
        }
        let (bytes, files) = paths.iter().map(|p| size_of(p)).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let name = if paths.len() == 1 { file_name(&paths[0]) } else { format!("{} files", paths.len()) };
        Some(Item { key: key.into(), what: what.into(), name, bytes, files: files.max(paths.len()), ticked: shared.is_empty(), shared, act: Act::Recycle(paths) })
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string_lossy().into_owned())
}

fn lower(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

fn same(a: &Path, b: &Path) -> bool {
    lower(a) == lower(b)
}

/// A file's size; a folder's files', and how many.
fn size_of(path: &Path) -> (u64, usize) {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => std::fs::read_dir(path).map_or((0, 0), |entries| {
            entries.flatten().map(|e| size_of(&e.path())).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
        }),
        Ok(m) => (m.len(), 1),
        Err(_) => (0, 0),
    }
}

/// What removing `table` could take, the other tables installed beside it (`others`) deciding
/// what's ticked: anything one of them uses too isn't.
pub fn plan(layout: &Layout, table: &Seen, others: &[Seen]) -> Vec<Item> {
    let h = table.holds;
    let users = |uses: &dyn Fn(&Seen) -> bool| -> Vec<String> {
        let mut names: Vec<String> = others.iter().filter(|o| uses(o)).map(|o| o.name.to_string()).collect();
        names.dedup();
        names
    };
    let mut out = Vec::new();
    out.extend(Item::recycle("table", "The table", vec![table.path.to_path_buf()], Vec::new()));
    // its own files beside it, named for it
    for (key, what, path) in [("backglass", "Its backglass", &h.backglass), ("settings", "Its settings", &h.settings), ("pov", "Its POV", &h.pov), ("script", "Its script", &h.script)] {
        out.extend(path.clone().and_then(|p| Item::recycle(key, what, vec![p], Vec::new())));
    }
    // its other backglasses, kept beside it to choose from, and the pictures kept for it
    out.extend(Item::recycle("backglasses", "Its other backglasses", h.backglasses.clone(), Vec::new()));
    if let Some(folder) = h.pictures.first().and_then(|p| p.parent()) {
        out.extend(Item::recycle("pictures", "Pictures played in place of its own", vec![folder.to_path_buf()], Vec::new()));
    }
    // its own physics, not the folder's `autovpp.vpp`, which every table beside it reads
    if let Some(p) = h.physics.clone().filter(|p| !file_name(p).eq_ignore_ascii_case("autovpp.vpp")) {
        out.extend(Item::recycle("physics", "Its physics", vec![p], Vec::new()));
    }
    // its ROM, and what's kept for the ROM's set, which every table playing the same set shares
    if let Some(set) = h.rom_set.as_deref() {
        let plays_it = users(&|o| o.holds.rom_set.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(set)));
        out.extend(h.rom.clone().and_then(|rom| Item::recycle("rom", "Its ROM", vec![rom], plays_it.clone())));
        // the set a clone takes files from, kept where another table plays it or a clone of it
        let parents: HashMap<String, String> = crate::roms::all_sets()
            .map(|all| all.into_iter().filter_map(|k| Some((k.set.to_lowercase(), k.clone_of.filter(|p| !p.is_empty())?.to_lowercase()))).collect())
            .unwrap_or_default();
        if let Some(parent) = parents.get(&set.to_lowercase()) {
            let needs = users(&|o| o.holds.rom_set.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(parent) || parents.get(&s.to_lowercase()) == Some(parent)));
            out.extend(Item::recycle("parent-rom", "The ROM its ROM is based on", vec![layout.vpinmame.join("roms").join(format!("{parent}.zip"))], needs));
        }
        out.extend(h.altsound.clone().and_then(|p| Item::recycle("altsound", "Its AltSound pack", vec![p], plays_it.clone())));
        out.extend(h.colour.as_ref().and_then(|c| Item::recycle("colour", "Its color DMD files", vec![c.folder.clone()], plays_it.clone())));
        // the machine's memory our player keeps, its high scores in it
        let memory = layout.data.join("PinMAME");
        let scores = Item::recycle("scores", "Its high scores", vec![memory.join("nvram").join(format!("{set}.nv")), memory.join("hi").join(format!("{set}.hi"))], plays_it);
        out.extend(scores.map(|s| Item { name: format!("The machine's memory, {}", s.name), ..s }));
    } else if !table.read.saved_under.is_empty() {
        // the values its script keeps, its high scores among them
        let file = layout.data.join("VPReg.ini");
        let text = std::fs::read(&file).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let sections: BTreeSet<String> = vpi_settings::parse_ini(&text).into_iter().map(|(s, _, _)| s.to_lowercase()).collect();
        let kept: Vec<String> = table.read.saved_under.iter().filter(|n| sections.contains(&n.to_lowercase())).cloned().collect();
        if !kept.is_empty() {
            let shared = users(&|o| o.read.saved_under.iter().any(|n| kept.iter().any(|k| k.eq_ignore_ascii_case(n))));
            out.push(Item {
                key: "scores".into(),
                what: "Its high scores".into(),
                name: format!("What its script keeps, under {}", kept.join(", ")),
                bytes: 0,
                files: 0,
                ticked: shared.is_empty(),
                shared,
                act: Act::Values(file, kept),
            });
        }
    }
    // its PinUP pack, under the name its script asks for or another
    for (key, path) in [("pup", &h.pup_pack), ("pup-misnamed", &h.pup_misnamed)] {
        if let Some(p) = path {
            let shared = users(&|o| [&o.holds.pup_pack, &o.holds.pup_misnamed].into_iter().flatten().any(|q| same(q, p)));
            out.extend(Item::recycle(key, "Its PinUP pack", vec![p.clone()], shared));
        }
    }
    if !h.music.is_empty() {
        let shared = users(&|o| o.holds.music.iter().any(|m| h.music.iter().any(|n| same(m, n))));
        out.extend(Item::recycle("music", "Its music", h.music.clone(), shared));
    }
    // the scripts it loads that aren't VPX's own: one with VPX's scripts kept to start with, for the
    // tables still to come that load it
    for helper in &h.helpers {
        if let Some(p) = &helper.path {
            let shared = users(&|o| o.holds.helpers.iter().any(|x| x.path.as_deref().is_some_and(|q| same(q, p))));
            let shelved = lower(p).starts_with(&lower(&layout.scripts));
            out.extend(Item::recycle(format!("helper:{}", lower(p)), "A script it loads", vec![p.clone()], shared).map(|i| Item { ticked: i.ticked && !shelved, ..i }));
        }
    }
    for folder in &h.folders {
        let shared = users(&|o| o.holds.folders.iter().any(|f| same(f, folder)));
        out.extend(Item::recycle(format!("folder:{}", lower(folder)), "Its DMD's pictures", vec![folder.clone()], shared));
    }
    // other files beside it named for it, which nothing reads
    for b in &h.beside {
        out.extend(Item::recycle(format!("beside:{}", lower(b)), "A file named for it", vec![b.clone()], Vec::new()));
    }
    // PinUP Popper's: the media it shows for it, and its game in Popper's list
    let popper_media = h.media.iter().filter(|m| m.from == crate::frontends::Front::Popper).map(|m| m.path.clone()).collect();
    out.extend(Item::recycle("popper-media", "Its media in PinUP Popper's folders", popper_media, Vec::new()));
    if let Some(db) = popper::database(layout)
        && let Ok(Some(game)) = popper::game_for(&db, &layout.tables, &crate::installed::listed_name(&layout.tables, table.path))
    {
        let name = if game.display.trim().is_empty() { game.name.clone() } else { game.display.clone() };
        out.push(Item { key: "popper-game".into(), what: "Its game in PinUP Popper's list".into(), name, bytes: 0, files: 0, shared: Vec::new(), ticked: true, act: Act::PopperGame(db, game) });
    }
    // PinballX's and PinballY's: every file of its media named for it (its sounds and the videos
    // the app doesn't play too), and its game in their game lists
    let file = file_name(table.path);
    let stem = table.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    for found in crate::frontends::read_all(layout) {
        let front = found.front.front;
        let what = format!("Its media in {}'s folders", front.name());
        out.extend(Item::recycle(format!("{}-media", front.key().to_lowercase()), &what, found.files_named(&found.media_name(&file)), Vec::new()));
        for g in found.front.games().into_iter().filter(|g| g.name.eq_ignore_ascii_case(&stem) || g.name.eq_ignore_ascii_case(&file)) {
            let name = g.media_name.clone().unwrap_or_else(|| g.name.clone());
            let what = format!("Its game in {}'s list", front.name());
            out.push(Item { key: format!("{}-game", front.key().to_lowercase()), what, name, bytes: 0, files: 0, shared: Vec::new(), ticked: true, act: Act::FrontGame(g.list, g.name) });
        }
    }
    out
}

/// What a removal did.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Done {
    /// The files and folders moved to the Recycle Bin.
    pub recycled: usize,
    /// What couldn't be done, in words.
    pub problems: Vec<String>,
    /// The table file itself went.
    pub table_gone: bool,
}

/// The ticked items of a plan carried out (`Act::Own` left to the caller): the table first — if it
/// can't be moved, nothing else is done — then the rest, PinUP Popper's list and our player's
/// `VPReg.ini` kept in `keep_in` first, named for `stamp`.
pub fn carry_out(items: &[Item], ticked: &BTreeSet<String>, keep_in: &Path, stamp: &str) -> Result<Done, String> {
    let running = crate::rename::holders_running();
    if !running.is_empty() {
        let they = if running.len() == 1 { "it holds" } else { "they hold" };
        return Err(format!("Close {} first: {they} what's being removed.", running.join(" and ")));
    }
    let chosen: Vec<&Item> = items.iter().filter(|i| ticked.contains(&i.key)).collect();
    let mut done = Done::default();
    if let Some(Act::Recycle(paths)) = chosen.iter().find(|i| i.key == "table").map(|i| &i.act) {
        for p in paths {
            crate::install::to_recycle_bin(p).map_err(|e| format!("The table couldn't be moved to the Recycle Bin, so nothing was removed: {e}"))?;
            done.recycled += 1;
        }
        done.table_gone = true;
    }
    for item in chosen.iter().filter(|i| i.key != "table") {
        match &item.act {
            Act::Recycle(paths) => {
                for p in paths {
                    match crate::install::to_recycle_bin(p) {
                        Ok(()) => done.recycled += 1,
                        Err(e) => done.problems.push(format!("{} couldn't be moved to the Recycle Bin: {e}", file_name(p))),
                    }
                }
            }
            Act::Values(file, names) => {
                if let Err(e) = take_values(file, names, keep_in, stamp) {
                    done.problems.push(e);
                }
            }
            Act::PopperGame(db, game) => {
                if let Err(e) = popper::keep_copy(db, keep_in, stamp).and_then(|_| popper::remove(db, game)) {
                    done.problems.push(e);
                }
            }
            Act::FrontGame(list, name) => {
                if let Err(e) = crate::frontends::remove_game(list, name) {
                    done.problems.push(e);
                }
            }
            Act::Own => {}
        }
    }
    Ok(done)
}

/// The values kept under `names` taken out of `file` (our player's `VPReg.ini`), the file kept in
/// `keep_in` first.
fn take_values(file: &Path, names: &[String], keep_in: &Path, stamp: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    std::fs::create_dir_all(keep_in).map_err(|e| format!("{}: {e}", keep_in.display()))?;
    std::fs::copy(file, keep_in.join(format!("VPReg {stamp}.ini"))).map_err(|e| format!("The saved values couldn't be kept before changing them: {e}"))?;
    let part = file.with_extension("ini.part");
    std::fs::write(&part, without_sections(&text, names)).and_then(|_| std::fs::rename(&part, file)).map_err(|e| format!("{}: {e}", file.display()))
}

/// An ini's text without the sections named `names` (case aside), the rest as it was.
fn without_sections(text: &str, names: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut leaving = false;
    for line in text.split_inclusive('\n') {
        if let Some(name) = line.trim().strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            leaving = names.iter().any(|n| n.eq_ignore_ascii_case(name.trim()));
        }
        if !leaving {
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vpi-remove-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn layout(top: &Path) -> Layout {
        Layout { tables: top.join("Tables"), scripts: top.join("Scripts"), vpinmame: top.join("VPinMAME"), pup_videos: top.join("PUPVideos"), data: top.join("data"), front_ends: Vec::new(), media_front: None, per_game: false }
    }

    fn touch(p: &Path, bytes: usize) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, vec![b'x'; bytes]).unwrap();
    }

    #[test]
    fn whats_shared_with_another_table_isnt_ticked() {
        let top = scratch("shared");
        let l = layout(&top);
        let (a, b) = (l.tables.join("A.vpx"), l.tables.join("B.vpx"));
        touch(&a, 10);
        touch(&b, 10);
        touch(&l.tables.join("A.directb2s"), 5);
        touch(&l.vpinmame.join("roms").join("abc_10.zip"), 7);
        touch(&l.vpinmame.join("altsound").join("abc_10").join("altsound.csv"), 3);
        touch(&l.data.join("PinMAME").join("nvram").join("abc_10.nv"), 4);
        touch(&l.pup_videos.join("abc").join("1.mp4"), 6);
        touch(&l.pup_videos.join("abc").join("2.mp4"), 6);
        touch(&l.tables.join("autovpp.vpp"), 1);
        let holds_a = Holds {
            backglass: Some(l.tables.join("A.directb2s")),
            rom_set: Some("abc_10".into()),
            rom: Some(l.vpinmame.join("roms").join("abc_10.zip")),
            altsound: Some(l.vpinmame.join("altsound").join("abc_10")),
            pup_pack: Some(l.pup_videos.join("abc")),
            physics: Some(l.tables.join("autovpp.vpp")),
            ..Default::default()
        };
        let read = Read::default();
        let (ta, tb) = (Seen { name: "A", path: &a, read: &read, holds: &holds_a }, Seen { name: "B", path: &b, read: &read, holds: &Holds::default() });
        // alone: everything ticked; the folder's autovpp.vpp not offered
        let items = plan(&l, &ta, std::slice::from_ref(&tb));
        let keys: Vec<&str> = items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["table", "backglass", "rom", "altsound", "scores", "pup"]);
        assert!(items.iter().all(|i| i.ticked && i.shared.is_empty()));
        let pup = items.iter().find(|i| i.key == "pup").unwrap();
        assert_eq!((pup.files, pup.bytes), (2, 12));
        // another table playing the same ROM and the same pack: those, and what's kept for the
        // ROM, left unticked
        let holds_b = Holds { rom_set: Some("ABC_10".into()), pup_pack: Some(l.pup_videos.join("ABC")), ..Default::default() };
        let tb = Seen { name: "B", path: &b, read: &read, holds: &holds_b };
        let items = plan(&l, &ta, &[tb]);
        for i in &items {
            let shared = matches!(i.key.as_str(), "rom" | "altsound" | "scores" | "pup");
            assert_eq!((i.key.as_str(), i.ticked, i.shared.clone()), (i.key.as_str(), !shared, if shared { vec!["B".to_string()] } else { vec![] }));
        }
        let _ = std::fs::remove_dir_all(&top);
    }

    #[test]
    fn a_scripts_values_taken_out_of_the_saved_values() {
        let text = "[hpgof]\nHighScore1 = 100\n\n[Other]\nA = 1\n[HPGOF_extra]\nB = 2\n[HpGoF]\nC = 3\n";
        assert_eq!(without_sections(text, &["hpgof".into()]), "[Other]\nA = 1\n[HPGOF_extra]\nB = 2\n");
        let top = scratch("values");
        let l = layout(&top);
        let t = l.tables.join("T.vpx");
        touch(&t, 1);
        std::fs::create_dir_all(&l.data).unwrap();
        std::fs::write(l.data.join("VPReg.ini"), text).unwrap();
        let read = Read { saved_under: vec!["HPGOF".into(), "never".into()], ..Default::default() };
        let holds = Holds::default();
        let items = plan(&l, &Seen { name: "T", path: &t, read: &read, holds: &holds }, &[]);
        let scores = items.iter().find(|i| i.key == "scores").unwrap();
        assert!(matches!(&scores.act, Act::Values(_, names) if names == &["HPGOF".to_string()]));
        let _ = std::fs::remove_dir_all(&top);
    }
}
