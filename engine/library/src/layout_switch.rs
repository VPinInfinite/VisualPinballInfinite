//! The switch to one folder per game (VPX 10.8.1's layout), planned: each table in the shared
//! tables folder given the folder its game would get — named `Name (Maker Year)` as VPS names the
//! game, else for the table's file — with every file of its that would go there: itself and what's
//! named for it beside it, its ROM (with the set it's a clone of) into the folder's `pinmame\roms`,
//! the machine's memory and settings VPX keeps copied into `pinmame\nvram` and `pinmame\cfg`, its
//! AltSound pack, colouring, PinUP pack, the music its script plays, the scripts it loads beside it
//! and its DMD's folders. What tables of more than one game use stays where it is — a ROM is copied
//! into each instead, as PinMAME plays a folder's tables from its `pinmame` alone once it has one —
//! VPX's own scripts always stay, and the physics and views every table beside them shares
//! (`autovpp.vpp`, `autopov.pov`) are copied into each folder. A plan only: nothing moves. Ours.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::installed::Holds;
use crate::plan::Layout;

/// A table, what it holds, and the name of the folder its game would get.
pub struct Table<'a> {
    pub path: &'a Path,
    pub holds: &'a Holds,
    pub folder: String,
}

/// One file or folder moved, or copied, into a game's folder.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Move {
    pub from: PathBuf,
    pub to: PathBuf,
    /// Copied, the original kept: what other tables still read where it is.
    pub copy: bool,
    pub bytes: u64,
    pub files: u64,
}

/// What switching would do.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// The games' folders made, each with the tables that would go in it.
    pub folders: BTreeMap<PathBuf, Vec<PathBuf>>,
    pub moves: Vec<Move>,
    /// What's moved and copied, in bytes and files.
    pub moved: (u64, u64),
    pub copied: (u64, u64),
    /// What stays where it is, used by tables of more than one game, with their folders' names.
    pub shared: Vec<(PathBuf, Vec<String>)>,
    /// What can't be done, and why.
    pub problems: Vec<String>,
}

/// A folder's name made safe for Windows: what it doesn't allow taken out.
pub fn folder_name(name: &str) -> String {
    let safe: String = name.chars().filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()).collect();
    safe.trim().trim_end_matches('.').trim().to_string()
}

/// A file's or a folder's size and count of files.
fn size_of(p: &Path) -> (u64, u64) {
    match std::fs::metadata(p) {
        Ok(m) if m.is_file() => (m.len(), 1),
        Ok(m) if m.is_dir() => std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| size_of(&e.path())).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1)),
        _ => (0, 0),
    }
}

fn key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

/// What switching every table in `layout`'s shared tables folder to a folder of its game's own
/// would move and copy; tables already in a folder of their own stay as they are.
pub fn plan(layout: &Layout, tables: &[Table]) -> Plan {
    let mut out = Plan::default();
    let same = |a: &Path, b: &Path| key(a) == key(b);
    let shared_tables: Vec<&Table> = tables.iter().filter(|t| t.path.parent().is_some_and(|d| same(d, &layout.tables))).collect();
    // each wanted file or folder: where it goes, by the game folders wanting it
    struct Want {
        from: PathBuf,
        /// Where it goes, below the game's folder.
        below: PathBuf,
        /// Copied even when one game wants it (what the shared folder's other tables read too).
        copy: bool,
        /// Copied into each folder when several want it, else left where it is.
        copy_when_shared: bool,
    }
    let mut wants: Vec<(String, Want)> = Vec::new();
    for t in &shared_tables {
        let name = folder_name(&t.folder);
        if name.is_empty() {
            out.problems.push(format!("{}: no name for its folder", t.path.display()));
            continue;
        }
        let dir = layout.tables.join(&name);
        out.folders.entry(dir.clone()).or_default().push(t.path.to_path_buf());
        let h = t.holds;
        let beside = |p: &Path| p.file_name().map(PathBuf::from).unwrap_or_default();
        let mut add = |from: &Path, below: PathBuf, copy: bool, copy_when_shared: bool| {
            if from.exists() {
                wants.push((name.clone(), Want { from: from.to_path_buf(), below, copy, copy_when_shared }));
            }
        };
        // the table and its own files beside it
        add(t.path, beside(t.path), false, false);
        for p in [&h.backglass, &h.pov, &h.settings, &h.script].into_iter().flatten().chain(&h.backglasses).chain(&h.beside) {
            add(p, beside(p), false, false);
        }
        // its physics: its own file moves, the folder's shared one is copied
        if let Some(p) = &h.physics {
            let shared_file = p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("autovpp.vpp"));
            add(p, beside(p), shared_file, shared_file);
        }
        let autopov = layout.tables.join("autopov.pov");
        if h.pov.is_none() {
            add(&autopov, PathBuf::from("autopov.pov"), true, true);
        }
        if let Some(first) = h.pictures.first().and_then(|p| p.parent()) {
            add(first, beside(first), false, false);
        }
        // its ROM, and the set it's a clone of, played from the folder's own `pinmame` once it has
        // one; the memory and settings VPX keeps for it copied in beside them
        if let (Some(zip), Some(set)) = (&h.rom, &h.rom_set) {
            add(zip, PathBuf::from("pinmame").join("roms").join(beside(zip)), false, true);
            if let Some(parent) = crate::roms::all_sets().ok().and_then(|all| all.into_iter().find(|k| k.set.eq_ignore_ascii_case(set)).and_then(|k| k.clone_of)).filter(|p| !p.is_empty()) {
                let pz = zip.with_file_name(format!("{parent}.zip"));
                add(&pz, PathBuf::from("pinmame").join("roms").join(beside(&pz)), false, true);
            }
            for (folder, ext) in [("nvram", "nv"), ("cfg", "cfg")] {
                let f = layout.vpinmame.join(folder).join(format!("{set}.{ext}"));
                add(&f, PathBuf::from("pinmame").join(folder).join(beside(&f)), true, true);
            }
        }
        if let Some(p) = h.altsound.as_deref().filter(|p| p.starts_with(&layout.vpinmame)) {
            add(p, PathBuf::from("altsound").join(beside(p)), false, false);
        }
        if let Some(c) = h.colour.as_ref().filter(|c| c.folder.starts_with(&layout.vpinmame)) {
            let kind = if c.kind == "Serum" { "serum" } else { "vni" };
            add(&c.folder, PathBuf::from(kind).join(beside(&c.folder)), false, false);
        }
        if let Some(p) = h.pup_pack.as_deref().filter(|p| p.starts_with(&layout.pup_videos)) {
            add(p, PathBuf::from("pupvideos").join(beside(p)), false, false);
        }
        // its music, below the music folder as its script names it
        for m in &h.music {
            let below = [layout.tables.join("music"), layout.tables.parent().map(|v| v.join("Music")).unwrap_or_default()]
                .iter()
                .find_map(|music| m.strip_prefix(music).ok().map(Path::to_path_buf))
                .unwrap_or_else(|| beside(m));
            add(m, PathBuf::from("music").join(below), false, false);
        }
        // the scripts it loads beside it (VPX's own and those in VPX's scripts folder stay), and
        // its DMD's folders
        for helper in h.helpers.iter().filter_map(|x| x.path.as_deref()).filter(|p| p.parent().is_some_and(|d| same(d, &layout.tables))) {
            add(helper, beside(helper), false, false);
        }
        for f in &h.folders {
            add(f, beside(f), false, false);
        }
    }
    // each thing wanted, by the game folders wanting it
    let mut by_thing: BTreeMap<String, (BTreeSet<String>, &Want)> = BTreeMap::new();
    for (folder, w) in &wants {
        by_thing.entry(key(&w.from)).or_insert_with(|| (BTreeSet::new(), w)).0.insert(folder.clone());
    }
    for (folders, w) in by_thing.values() {
        let several = folders.len() > 1;
        if several && !w.copy_when_shared {
            out.shared.push((w.from.clone(), folders.iter().cloned().collect()));
            continue;
        }
        let copy = w.copy || several;
        let (bytes, files) = size_of(&w.from);
        for folder in folders {
            let to = layout.tables.join(folder).join(&w.below);
            if to.exists() && !same(&to, &w.from) {
                out.problems.push(format!("{} is there already", to.display()));
                continue;
            }
            if copy {
                out.copied = (out.copied.0 + bytes, out.copied.1 + files);
            } else {
                out.moved = (out.moved.0 + bytes, out.moved.1 + files);
            }
            out.moves.push(Move { from: w.from.clone(), to, copy, bytes, files });
        }
    }
    out.moves.sort_by(|a, b| a.to.cmp(&b.to));
    out
}

/// One move or copy carried out, as kept to put it back.
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Done {
    pub from: PathBuf,
    pub to: PathBuf,
    pub copy: bool,
}

/// What a switch did, kept to put it back.
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Switched {
    /// Each move and copy, in the order done.
    pub done: Vec<Done>,
    /// Each table file's place before and after, for what's kept by its path (the app's own
    /// records of it).
    pub tables: Vec<(PathBuf, PathBuf)>,
    /// PinUP Popper's games given their tables' new places, before and after.
    pub popper: Vec<(crate::rename::PopperGame, crate::rename::PopperGame)>,
    /// Popper's game list as it was before, kept.
    pub popper_kept: Option<PathBuf>,
    /// When it was done (`2025-01-02 18-04-05`, local time).
    pub at: String,
}

/// The drive a path is on, for telling a rename from a copy across drives.
fn drive(p: &Path) -> String {
    p.components().next().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// A file or folder copied whole.
fn copy_all(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            copy_all(&e.path(), &to.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// Folders left empty below `dir` taken away, and `dir` itself where it's left empty.
fn remove_empty(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                remove_empty(&e.path());
            }
        }
    }
    let _ = std::fs::remove_dir(dir);
}

/// The plan carried out: nothing while a program holding the files runs, while PinballX or
/// PinballY plays the tables (whether they follow a table into a folder of its own is unchecked),
/// or while the plan has a problem; then every move and copy, PinUP Popper's list kept in `keep_in`
/// first and each of its games given its table's new place (`<folder>\<file>`). A step that fails
/// puts back the ones before it. `progress` is told as each game's folder is begun: how many
/// begun, of how many, and its name.
pub fn carry_out(layout: &Layout, plan: &Plan, keep_in: &Path, stamp: &str, progress: &dyn Fn(usize, usize, &str)) -> Result<Switched, String> {
    let running = crate::rename::holders_running();
    if !running.is_empty() {
        return Err(format!("Close {} first: {} the files being moved.", running.join(" and "), if running.len() == 1 { "it holds" } else { "they hold" }));
    }
    if let Some(f) = crate::frontends::find(layout).first() {
        return Err(format!("{} plays these tables, and whether it follows a table into a folder of its own isn't known yet: nothing was moved.", f.front.name()));
    }
    if let Some(p) = plan.problems.first() {
        return Err(format!("{p}: nothing was moved."));
    }
    for m in &plan.moves {
        if !m.from.exists() {
            return Err(format!("{} isn't there any more: nothing was moved.", m.from.display()));
        }
        if m.to.exists() {
            return Err(format!("{} is there already: nothing was moved.", m.to.display()));
        }
        if !m.copy && drive(&m.from) != drive(&m.to) {
            return Err(format!("{} is on another drive than the tables: nothing was moved.", m.from.display()));
        }
    }
    // the tables moved, and Popper's games for them
    let tables: Vec<(PathBuf, PathBuf)> = plan
        .moves
        .iter()
        .filter(|m| !m.copy && m.from.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx")) && m.from.parent().is_some_and(|d| key(d) == key(&layout.tables)))
        .map(|m| (m.from.clone(), m.to.clone()))
        .collect();
    let db = crate::popper::database(layout);
    let mut popper: Vec<(crate::popper::Game, crate::popper::Game)> = Vec::new();
    if let Some(db) = &db {
        let games = crate::popper::games(db, &layout.tables)?;
        for (from, to) in &tables {
            let was = crate::installed::listed_name(&layout.tables, from);
            if let Some(g) = games.iter().find(|g| g.file.eq_ignore_ascii_case(&was)) {
                let file = crate::installed::listed_name(&layout.tables, to);
                popper.push((g.clone(), crate::popper::Game { file, ..g.clone() }));
            }
        }
    }
    let popper_kept = match (&db, popper.is_empty()) {
        (Some(db), false) => Some(crate::popper::keep_copy(db, keep_in, stamp)?),
        _ => None,
    };
    let mut done: Vec<Done> = Vec::new();
    let undo = |done: &[Done]| {
        for d in done.iter().rev() {
            if d.copy {
                let _ = if d.to.is_dir() { std::fs::remove_dir_all(&d.to) } else { std::fs::remove_file(&d.to) };
            } else {
                let _ = std::fs::rename(&d.to, &d.from);
            }
        }
        for folder in plan.folders.keys() {
            remove_empty(folder);
        }
    };
    let mut begun: Vec<&Path> = Vec::new();
    for m in &plan.moves {
        if let Some(folder) = plan.folders.keys().find(|f| m.to.starts_with(f))
            && !begun.contains(&folder.as_path())
        {
            begun.push(folder);
            progress(begun.len(), plan.folders.len(), &folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        }
        let result = m.to.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| if m.copy { copy_all(&m.from, &m.to) } else { std::fs::rename(&m.from, &m.to) });
        if let Err(e) = result {
            undo(&done);
            return Err(format!("{} couldn't be {} ({e}); everything was put back.", m.from.display(), if m.copy { "copied" } else { "moved" }));
        }
        done.push(Done { from: m.from.clone(), to: m.to.clone(), copy: m.copy });
    }
    if let Some(db) = &db {
        for (n, (before, after)) in popper.iter().enumerate() {
            if let Err(e) = crate::popper::rename(db, before, &after.file, &before.name, &before.display) {
                for (b, a) in popper[..n].iter().rev() {
                    let _ = crate::popper::rename(db, a, &b.file, &b.name, &b.display);
                }
                undo(&done);
                return Err(format!("{e} The files were put back as they were."));
            }
        }
    }
    Ok(Switched { done, tables, popper: popper.iter().map(|(b, a)| (b.into(), a.into())).collect(), popper_kept, at: stamp.to_string() })
}

/// A switch put back: every move undone, and of every copy the newer kept — a machine's memory VPX
/// played on in the table's folder goes back over the one in VPinMAME's, the one it replaces to
/// `recycle` (the Recycle Bin) — then the folders left empty taken away and Popper's games given
/// their old places, each only while it's as the switch left it.
pub fn put_back(layout: &Layout, s: &Switched, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<(), String> {
    let running = crate::rename::holders_running();
    if !running.is_empty() {
        return Err(format!("Close {} first.", running.join(" and ")));
    }
    for d in s.done.iter().filter(|d| !d.copy) {
        if d.from.exists() && d.to.exists() {
            return Err(format!("{} is there again, so {} can't go back: nothing was changed.", d.from.display(), d.to.display()));
        }
    }
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    for d in s.done.iter().rev() {
        if !d.to.exists() {
            continue;
        }
        if !d.copy {
            std::fs::rename(&d.to, &d.from).map_err(|e| format!("{} couldn't go back ({e})", d.to.display()))?;
        } else if d.to.is_file() && modified(&d.to) > modified(&d.from) {
            if d.from.exists() {
                recycle(&d.from)?;
            }
            std::fs::rename(&d.to, &d.from).map_err(|e| format!("{} couldn't go back ({e})", d.to.display()))?;
        } else {
            recycle(&d.to)?;
        }
    }
    let mut folders: Vec<PathBuf> = s.done.iter().filter_map(|d| d.to.strip_prefix(&layout.tables).ok()?.components().next().map(|c| layout.tables.join(c))).collect();
    folders.sort();
    folders.dedup();
    for f in &folders {
        remove_empty(f);
    }
    if let Some(db) = crate::popper::database(layout) {
        for (before, after) in &s.popper {
            let (b, a) = (crate::popper::Game { id: before.id, name: before.name.clone(), file: before.file.clone(), display: before.display.clone() }, crate::popper::Game { id: after.id, name: after.name.clone(), file: after.file.clone(), display: after.display.clone() });
            crate::popper::rename(&db, &a, &b.file, &b.name, &b.display).map_err(|e| format!("The files are back, but a game in PinUP Popper's list couldn't be: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_switched_to_folders_of_its_games_and_put_back() {
        let root = std::env::temp_dir().join(format!("vpi-layout-carry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::under(&root);
        let touch = |p: &Path, text: &str| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let (a, b) = (layout.tables.join("AFM JP.vpx"), layout.tables.join("AFM VPW.vpx"));
        touch(&a, "a");
        touch(&b, "b");
        touch(&layout.tables.join("AFM JP.directb2s"), "glass");
        let rom = layout.vpinmame.join("roms").join("afm_113b.zip");
        touch(&rom, "rom");
        let nv = layout.vpinmame.join("nvram").join("afm_113b.nv");
        touch(&nv, "memory");
        let pack = layout.pup_videos.join("afm_113b");
        touch(&pack.join("screens.pup"), "pack");
        // PinUP Popper's list, its games in the tables folder
        let db = layout.pup_videos.parent().unwrap().join("PUPDatabase.db");
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch(&format!(
            "CREATE TABLE Emulators (EMUID INTEGER PRIMARY KEY, EmuName TEXT, DirGames TEXT);
             CREATE TABLE Games (GameID INTEGER PRIMARY KEY, EMUID INTEGER, GameName TEXT, GameFileName TEXT, GameDisplay TEXT);
             INSERT INTO Emulators VALUES (1, 'Visual Pinball X', '{}');
             INSERT INTO Games VALUES (3, 1, 'AFM JP', 'AFM JP.vpx', 'Attack from Mars');",
            layout.tables.display()
        ))
        .unwrap();
        drop(c);
        let holds = |glass: bool| Holds {
            backglass: glass.then(|| layout.tables.join("AFM JP.directb2s")),
            rom: Some(rom.clone()),
            rom_set: Some("afm_113b".into()),
            pup_pack: Some(pack.clone()),
            ..Default::default()
        };
        let (ha, hb) = (holds(true), holds(false));
        let name = "Attack from Mars (Bally 1995)";
        let tables = [Table { path: &a, holds: &ha, folder: name.into() }, Table { path: &b, holds: &hb, folder: name.into() }];
        let p = plan(&layout, &tables);
        let told = std::cell::RefCell::new(Vec::new());
        let s = carry_out(&layout, &p, &root.join("kept"), "test", &|n, of, name| told.borrow_mut().push((n, of, name.to_string()))).unwrap();
        assert_eq!(told.borrow().as_slice(), [(1, 1, "Attack from Mars (Bally 1995)".to_string())]);
        let folder = layout.tables.join(name);
        let moved = (
            folder.join("AFM JP.vpx").is_file() && folder.join("AFM VPW.vpx").is_file() && !a.exists(),
            folder.join("pinmame").join("roms").join("afm_113b.zip").is_file() && !rom.exists(),
            folder.join("pinmame").join("nvram").join("afm_113b.nv").is_file() && nv.is_file(),
            folder.join("pupvideos").join("afm_113b").join("screens.pup").is_file(),
            crate::popper::games(&db, &layout.tables).unwrap()[0].file.clone(),
        );
        // VPX plays on in the table's folder: its newer memory goes back over the old
        std::thread::sleep(std::time::Duration::from_millis(20));
        touch(&folder.join("pinmame").join("nvram").join("afm_113b.nv"), "memory after a game");
        let recycled = std::cell::RefCell::new(Vec::new());
        let recycle = |p: &Path| {
            recycled.borrow_mut().push(p.to_path_buf());
            if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) }.map_err(|e| e.to_string())
        };
        put_back(&layout, &s, &recycle).unwrap();
        let back = (
            a.is_file() && b.is_file() && rom.is_file() && pack.join("screens.pup").is_file(),
            std::fs::read_to_string(&nv).unwrap(),
            folder.exists(),
            crate::popper::games(&db, &layout.tables).unwrap()[0].file.clone(),
        );
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(moved, (true, true, true, true, format!("{name}\\AFM JP.vpx")));
        assert_eq!(s.tables.len(), 2);
        assert_eq!(back, (true, "memory after a game".to_string(), false, "AFM JP.vpx".to_string()));
        assert_eq!(recycled.borrow().len(), 1);
    }

    #[test]
    fn two_versions_of_a_game_share_its_folder_and_what_other_games_use_stays() {
        let root = std::env::temp_dir().join(format!("vpi-layout-switch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::under(&root);
        let touch = |p: &Path, bytes: usize| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, vec![0u8; bytes]).unwrap();
        };
        let (a, b, c) = (layout.tables.join("AFM JP.vpx"), layout.tables.join("AFM VPW.vpx"), layout.tables.join("Taxi.vpx"));
        for p in [&a, &b, &c] {
            touch(p, 100);
        }
        touch(&layout.tables.join("AFM JP.directb2s"), 50);
        touch(&layout.tables.join("autovpp.vpp"), 5);
        let rom = layout.vpinmame.join("roms").join("afm_113b.zip");
        touch(&rom, 10);
        let nv = layout.vpinmame.join("nvram").join("afm_113b.nv");
        touch(&nv, 3);
        let pack = layout.pup_videos.join("afm_113b");
        touch(&pack.join("screens.pup"), 7);
        let shared_song = layout.tables.join("music").join("theme.mp3");
        touch(&shared_song, 20);
        let holds_afm = |backglass: Option<PathBuf>| Holds {
            backglass,
            rom: Some(rom.clone()),
            rom_set: Some("afm_113b".into()),
            pup_pack: Some(pack.clone()),
            physics: Some(layout.tables.join("autovpp.vpp")),
            music: vec![shared_song.clone()],
            ..Default::default()
        };
        let (ha, hb) = (holds_afm(Some(layout.tables.join("AFM JP.directb2s"))), holds_afm(None));
        let hc = Holds { music: vec![shared_song.clone()], ..Default::default() };
        let tables = [
            Table { path: &a, holds: &ha, folder: "Attack from Mars (Bally 1995)".into() },
            Table { path: &b, holds: &hb, folder: "Attack from Mars (Bally 1995)".into() },
            Table { path: &c, holds: &hc, folder: "Taxi (Williams 1988)".into() },
        ];
        let p = plan(&layout, &tables);
        let _ = std::fs::remove_dir_all(&root);
        let afm = layout.tables.join("Attack from Mars (Bally 1995)");
        assert_eq!(p.folders.len(), 2);
        assert_eq!(p.folders[&afm].len(), 2);
        let to = |from: &Path| p.moves.iter().filter(|m| m.from == from).map(|m| (m.to.clone(), m.copy)).collect::<Vec<_>>();
        // both versions in one folder, the ROM and the pack once, moved; the memory copied
        assert_eq!(to(&a), [(afm.join("AFM JP.vpx"), false)]);
        assert_eq!(to(&rom), [(afm.join("pinmame").join("roms").join("afm_113b.zip"), false)]);
        assert_eq!(to(&pack), [(afm.join("pupvideos").join("afm_113b"), false)]);
        assert_eq!(to(&nv), [(afm.join("pinmame").join("nvram").join("afm_113b.nv"), true)]);
        // the shared physics copied into each folder; a song two games play stays
        assert_eq!(to(&layout.tables.join("autovpp.vpp")).len(), 1);
        assert!(to(&layout.tables.join("autovpp.vpp")).iter().all(|(_, copy)| *copy));
        assert_eq!(p.shared.len(), 1);
        assert_eq!(p.shared[0].0, shared_song);
        assert!(p.problems.is_empty(), "{:?}", p.problems);
    }
}
