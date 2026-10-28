//! A table renamed to VPS's name for its game, `Name (Maker Year)`, for every program ("Overwrite
//! for all players (VPX, PinMAME, VPin Studio)", PinUP Popper's game list changing with it): the table file and every file beside it named
//! as it (its backglass, POV, settings, script, physics…), PinUP Popper's media named for it — by
//! its file's name, or by Popper's own name for it — and Popper's game (its table file's, its own
//! and its shown name), the list kept first. Nothing is done while a program that holds them is
//! running, or when a name is taken; a step that fails puts back the ones before it; and what was
//! done is kept, to put back later. Ours.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::plan::Layout;
use crate::popper::{self, Game};

/// What a rename did, to put it back.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Renamed {
    /// The table file's name before, without its extension, and after.
    pub from: String,
    pub to: String,
    /// Each file moved: from, to.
    pub files: Vec<(PathBuf, PathBuf)>,
    /// Popper's game before and after, if it has one.
    pub popper_before: Option<PopperGame>,
    pub popper_after: Option<PopperGame>,
    /// Popper's game list as it was before, kept.
    pub popper_kept: Option<PathBuf>,
    /// PinballX's and PinballY's games renamed in their game lists.
    pub fronts: Vec<FrontGame>,
}

/// A game of PinballX's or PinballY's game lists, as kept with a rename: its list, its name before
/// and after (its table file's), and its description before and after where that followed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FrontGame {
    pub list: PathBuf,
    pub from: String,
    pub to: String,
    pub description: Option<(String, String)>,
}

/// Popper's game, as kept with a rename.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PopperGame {
    pub id: i64,
    pub name: String,
    pub file: String,
    pub display: String,
}

impl From<&Game> for PopperGame {
    fn from(g: &Game) -> Self {
        PopperGame { id: g.id, name: g.name.clone(), file: g.file.clone(), display: g.display.clone() }
    }
}

impl PopperGame {
    fn game(&self) -> Game {
        Game { id: self.id, name: self.name.clone(), file: self.file.clone(), display: self.display.clone() }
    }
}

/// VPS's name for a game as a file's name: `Name (Maker Year)`, what Windows won't have taken out.
pub fn vps_name(name: &str, maker: Option<&str>, year: Option<i32>) -> String {
    let maker_year = [maker.map(str::trim).filter(|m| !m.is_empty()).map(str::to_string), year.map(|y| y.to_string())].into_iter().flatten().collect::<Vec<_>>().join(" ");
    let name = if maker_year.is_empty() { name.to_string() } else { format!("{name} ({maker_year})") };
    let name: String = name.chars().filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()).collect();
    name.trim().trim_end_matches('.').trim().to_string()
}

/// Those running now of the programs that hold what a rename moves: PinUP Popper (its game list,
/// which it keeps and writes back), VPX and our player (a table open).
pub fn holders_running() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for n in crate::processes::running() {
        let n = n.to_ascii_lowercase();
        let what = if n.starts_with("pinupmenu") {
            "PinUP Popper"
        } else if n == "pinballx.exe" {
            "PinballX"
        } else if n == "pinbally.exe" {
            "PinballY"
        } else if n.starts_with("vpinballx") {
            "VPX"
        } else if n == "vpi-player.exe" {
            "our player"
        } else {
            continue;
        };
        if !out.contains(&what) {
            out.push(what);
        }
    }
    out
}

/// A name with `from` at its start (case aside) given `to` there instead: `from` itself, or `from`
/// and two digits (Popper's second and later media of a kind: `Name01.png`).
fn renamed_stem(stem: &str, from: &str, to: &str) -> Option<String> {
    if stem.len() < from.len() || !stem.is_char_boundary(from.len()) || !stem[..from.len()].eq_ignore_ascii_case(from) {
        return None;
    }
    let rest = &stem[from.len()..];
    (rest.is_empty() || (rest.len() == 2 && rest.chars().all(|c| c.is_ascii_digit()))).then(|| format!("{to}{rest}"))
}

fn with_stem(p: &Path, stem: &str) -> PathBuf {
    p.with_file_name(match p.extension() {
        Some(x) => format!("{stem}.{}", x.to_string_lossy()),
        None => stem.to_string(),
    })
}

/// What renaming the table at `table` to `to` would move and change, nothing done: each file, and
/// Popper's game with its new names.
pub struct Plan {
    pub files: Vec<(PathBuf, PathBuf)>,
    pub popper: Option<(Game, Game)>,
    pub db: Option<PathBuf>,
    /// PinballX's and PinballY's games to rename in their game lists.
    pub fronts: Vec<FrontGame>,
}

pub fn plan(layout: &Layout, table: &Path, to: &str) -> Result<Plan, String> {
    let from = table.file_stem().ok_or("That table has no name.")?.to_string_lossy().into_owned();
    let to = to.trim();
    if to.is_empty() {
        return Err("VPS gives no name to rename it to.".into());
    }
    let dir = table.parent().ok_or("That table has no folder.")?;
    let mut files = Vec::new();
    // the table and every file beside it named as it, and the script kept by a ROM's change
    // (`<table>.vbs.before-rom-change`, `rom_change::change`)
    let kept_script = format!("{from}.vbs.before-rom-change");
    let entries: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().map(|e| e.path()).collect();
    // the tables beside it, by their file's stem: a table's numbered companions aren't another's
    let tables: Vec<String> = entries.iter().filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx"))).filter_map(|p| Some(p.file_stem()?.to_string_lossy().to_lowercase())).collect();
    for p in entries {
        // the pictures the player reads in place of its own, in their folder named for it
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&format!("{from} pictures"))) {
                files.push((p.clone(), p.with_file_name(format!("{to} pictures"))));
            }
            continue;
        }
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if stem.eq_ignore_ascii_case(&from) {
            files.push((p.clone(), with_stem(&p, to)));
        } else if p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&kept_script)) {
            files.push((p.clone(), p.with_file_name(format!("{to}.vbs.before-rom-change"))));
        } else if let Some(n) = crate::installed::number_after(&from.to_lowercase(), &stem.to_lowercase())
            && !tables.contains(&stem.to_lowercase())
        {
            // its backglass's versions and other companions numbered as VPin Studio numbers them
            files.push((p.clone(), with_stem(&p, &format!("{to} ({n})"))));
        }
    }
    // Popper's game, and its media, named for its file's name or for Popper's own name for it
    let db = popper::database(layout);
    // as Popper's list names it: a table in a folder of its own by `<folder>\<file>`
    let file_name = crate::installed::listed_name(&layout.tables, table);
    let game = match &db {
        Some(db) => popper::game_for(db, &layout.tables, &file_name)?,
        None => None,
    };
    let mut names = vec![from.clone()];
    if let Some(g) = &game
        && !g.name.is_empty()
        && !names.iter().any(|n| n.eq_ignore_ascii_case(&g.name))
    {
        names.push(g.name.clone());
    }
    if let Some(media) = crate::installed::pop_media(layout) {
        for kind in std::fs::read_dir(&media).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            for e in std::fs::read_dir(&kind).into_iter().flatten().flatten() {
                let p = e.path();
                let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
                if p.is_file()
                    && let Some(new) = names.iter().find_map(|n| renamed_stem(&stem, n, to))
                    && !files.iter().any(|(f, _)| f == &p)
                {
                    files.push((p.clone(), with_stem(&p, &new)));
                }
            }
        }
    }
    // PinballX's and PinballY's: the game named for its file in their game lists given the new
    // name, its description too where that was only its file's name; their media follow where
    // they're named for the file's name (a game they don't list, or one whose description it was)
    let mut fronts: Vec<FrontGame> = Vec::new();
    for found in crate::frontends::read_all(layout) {
        let games: Vec<crate::frontends::Game> = found.front.games().into_iter().filter(|g| g.name.eq_ignore_ascii_case(&from) || g.name.eq_ignore_ascii_case(&format!("{from}.vpx"))).collect();
        let mut follows = games.is_empty();
        for g in games {
            let with_ext = g.name.to_lowercase().ends_with(".vpx");
            let description = match &g.media_name {
                Some(d) if d.eq_ignore_ascii_case(&from) => {
                    follows = true;
                    Some((d.clone(), to.to_string()))
                }
                Some(_) => None,
                None => {
                    follows = true;
                    None
                }
            };
            fronts.push(FrontGame { list: g.list.clone(), from: g.name.clone(), to: if with_ext { format!("{to}.vpx") } else { to.to_string() }, description });
        }
        if follows {
            for p in found.files_named(&from) {
                let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let n = crate::frontends::number_of(&p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), &from);
                let new = match n {
                    Some(0) => to.to_string(),
                    Some(n) => format!("{to} {n}"),
                    None => stem,
                };
                if !files.iter().any(|(f, _)| f == &p) {
                    files.push((p.clone(), with_stem(&p, &new)));
                }
            }
        }
    }
    let new_file = format!("{to}.{}", table.extension().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| "vpx".into()));
    // in its folder, where it has one of its own
    let new_file = match file_name.rsplit_once('\\') {
        Some((folder, _)) => format!("{folder}\\{new_file}"),
        None => new_file,
    };
    let popper = game.map(|g| {
        // a shown name that was only its file's or its own name follows the new name; one of the user's own stays
        let display = if g.display.eq_ignore_ascii_case(&from) || g.display.eq_ignore_ascii_case(&g.name) { to.to_string() } else { g.display.clone() };
        let after = Game { id: g.id, name: to.to_string(), file: new_file.clone(), display };
        (g, after)
    });
    // a name taken: by another file, or by another of Popper's games
    for (f, t) in &files {
        let same = f.to_string_lossy().eq_ignore_ascii_case(&t.to_string_lossy());
        if !same && t.exists() {
            return Err(format!("{} is there already, so {} can't take its name.", t.display(), f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
        }
    }
    if let (Some(db), Some((before, _))) = (&db, &popper)
        && popper::games(db, &layout.tables)?.iter().any(|g| g.id != before.id && g.file.eq_ignore_ascii_case(&new_file))
    {
        return Err(format!("PinUP Popper has another game for {new_file} already."));
    }
    Ok(Plan { files: files.into_iter().filter(|(f, t)| f != t).collect(), popper, db, fronts })
}

/// A file moved, a name differing only in case moved by way of a passing name (Windows keeps the
/// old case otherwise).
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.to_string_lossy().eq_ignore_ascii_case(&to.to_string_lossy()) {
        let passing = with_stem(from, &format!("{}.vpi-renaming", from.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()));
        std::fs::rename(from, &passing)?;
        return std::fs::rename(&passing, to);
    }
    std::fs::rename(from, to)
}

/// The plan carried out: Popper's game list kept in `keep_in` first, the files moved, Popper's game
/// renamed; a step that fails puts back the ones before it.
pub fn carry_out(plan: &Plan, keep_in: &Path, stamp: &str, from: &str, to: &str) -> Result<Renamed, String> {
    let running = holders_running();
    if !running.is_empty() {
        return Err(format!("Close {} first: {} the files or the game list being renamed.", running.join(" and "), if running.len() == 1 { "it holds" } else { "they hold" }));
    }
    let kept = match (&plan.db, &plan.popper) {
        (Some(db), Some(_)) => Some(popper::keep_copy(db, keep_in, stamp)?),
        _ => None,
    };
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    let put_back = |done: &[(PathBuf, PathBuf)]| {
        for (f, t) in done.iter().rev() {
            let _ = move_file(t, f);
        }
    };
    for (f, t) in &plan.files {
        if let Err(e) = move_file(f, t) {
            put_back(&done);
            return Err(format!("{} couldn't be renamed ({e}); nothing was changed.", f.display()));
        }
        done.push((f.clone(), t.clone()));
    }
    // PinballX's and PinballY's game lists, each kept beside itself first; one that fails puts the
    // ones before it back
    let mut lists: Vec<&FrontGame> = Vec::new();
    for g in &plan.fronts {
        if let Err(e) = crate::frontends::rename_game(&g.list, &g.from, &g.to, g.description.as_ref().map(|d| d.1.as_str())) {
            for b in lists.iter().rev() {
                let _ = crate::frontends::rename_game(&b.list, &b.to, &b.from, b.description.as_ref().map(|d| d.0.as_str()));
            }
            put_back(&done);
            return Err(format!("{e} The files were put back as they were."));
        }
        lists.push(g);
    }
    if let (Some(db), Some((before, after))) = (&plan.db, &plan.popper)
        && let Err(e) = popper::rename(db, before, &after.file, &after.name, &after.display)
    {
        for b in lists.iter().rev() {
            let _ = crate::frontends::rename_game(&b.list, &b.to, &b.from, b.description.as_ref().map(|d| d.0.as_str()));
        }
        put_back(&done);
        return Err(format!("{e} The files were put back as they were."));
    }
    Ok(Renamed {
        from: from.to_string(),
        to: to.to_string(),
        files: done,
        popper_before: plan.popper.as_ref().map(|(b, _)| b.into()),
        popper_after: plan.popper.as_ref().map(|(_, a)| a.into()),
        popper_kept: kept,
        fronts: plan.fronts.clone(),
    })
}

/// A rename put back: the files moved back, Popper's game given its names back (only if it's still
/// as the rename left it). What couldn't be put back, said.
pub fn put_back(layout: &Layout, r: &Renamed) -> Result<(), String> {
    let running = holders_running();
    if !running.is_empty() {
        return Err(format!("Close {} first.", running.join(" and ")));
    }
    for (f, t) in &r.files {
        if f.exists() && !f.to_string_lossy().eq_ignore_ascii_case(&t.to_string_lossy()) {
            return Err(format!("{} is there again, so {} can't go back to its name.", f.display(), t.display()));
        }
    }
    let mut back: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (f, t) in r.files.iter().rev() {
        if !t.exists() {
            continue;
        }
        if let Err(e) = move_file(t, f) {
            for (f2, t2) in back.iter().rev() {
                let _ = move_file(f2, t2);
            }
            return Err(format!("{} couldn't be renamed back ({e}); nothing was changed.", t.display()));
        }
        back.push((f.clone(), t.clone()));
    }
    for g in &r.fronts {
        if let Err(e) = crate::frontends::rename_game(&g.list, &g.to, &g.from, g.description.as_ref().map(|d| d.0.as_str())) {
            return Err(format!("The files have their names back, but a game in {} couldn't be: {e}", g.list.display()));
        }
    }
    if let (Some(db), Some(before), Some(after)) = (popper::database(layout), &r.popper_before, &r.popper_after)
        && let Err(e) = popper::rename(&db, &after.game(), &before.file, &before.name, &before.display)
    {
        return Err(format!("The files have their names back, but PinUP Popper's game couldn't be: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vps_names_as_file_names() {
        assert_eq!(vps_name("Attack from Mars", Some("Bally"), Some(1995)), "Attack from Mars (Bally 1995)");
        assert_eq!(vps_name("AC/DC (LUCI Premium)", Some("Stern"), Some(2013)), "ACDC (LUCI Premium) (Stern 2013)");
        assert_eq!(vps_name("Who Dunnit?", None, Some(1995)), "Who Dunnit (1995)");
        assert_eq!(vps_name("Andromeda", Some(""), None), "Andromeda");
    }

    #[test]
    fn popper_media_numbered_follow_their_name() {
        assert_eq!(renamed_stem("Blood Machines 2.0", "blood machines 2.0", "Blood Machines (VPW 2022)").as_deref(), Some("Blood Machines (VPW 2022)"));
        assert_eq!(renamed_stem("Blood Machines 2.001", "Blood Machines 2.0", "Blood Machines (VPW 2022)").as_deref(), Some("Blood Machines (VPW 2022)01"));
        assert_eq!(renamed_stem("Blood Machines 2.0 Remix", "Blood Machines 2.0", "X"), None);
        assert_eq!(renamed_stem("Blood", "Blood Machines 2.0", "X"), None);
    }

    #[test]
    fn a_table_its_files_and_poppers_game_renamed_and_put_back() {
        let root = std::env::temp_dir().join(format!("vpi-rename-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::under(&root);
        let popper_root = layout.pup_videos.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&layout.tables).unwrap();
        let media = popper_root.join("POPMedia").join("Visual Pinball X");
        std::fs::create_dir_all(media.join("Wheel")).unwrap();
        std::fs::create_dir_all(&layout.pup_videos).unwrap();
        let old = "Attack from Mars (Bally 1995) JPSalas 6.0.0";
        for ext in ["vpx", "directb2s", "ini"] {
            std::fs::write(layout.tables.join(format!("{old}.{ext}")), ext).unwrap();
        }
        std::fs::write(layout.tables.join("Another table.vpx"), "other").unwrap();
        std::fs::write(media.join("Wheel").join(format!("{old}.png")), "wheel").unwrap();
        std::fs::write(media.join("Wheel").join(format!("{old}01.png")), "wheel 2").unwrap();
        // Popper's game list, its VPX games in the tables folder
        let db = popper_root.join("PUPDatabase.db");
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch(&format!(
            "CREATE TABLE Emulators (EMUID INTEGER PRIMARY KEY, EmuName TEXT, DirGames TEXT);
             CREATE TABLE Games (GameID INTEGER PRIMARY KEY, EMUID INTEGER, GameName TEXT, GameFileName TEXT, GameDisplay TEXT);
             INSERT INTO Emulators VALUES (1, 'Visual Pinball X', '{}');
             INSERT INTO Games VALUES (3, 1, '{old}', '{old}.vpx', '{old}');",
            layout.tables.display()
        ))
        .unwrap();
        drop(c);
        let to = vps_name("Attack from Mars", Some("Bally"), Some(1995));
        let table = layout.tables.join(format!("{old}.vpx"));
        let p = plan(&layout, &table, &to).unwrap();
        assert_eq!(p.files.len(), 5, "{:?}", p.files);
        let done = carry_out(&p, &root.join("kept"), "2025-01-02 18-00-00", old, &to).unwrap();
        for ext in ["vpx", "directb2s", "ini"] {
            assert!(layout.tables.join(format!("{to}.{ext}")).is_file());
        }
        assert!(layout.tables.join("Another table.vpx").is_file());
        assert!(media.join("Wheel").join(format!("{to}01.png")).is_file());
        let g = popper::game_for(&db, &layout.tables, &format!("{to}.vpx")).unwrap().unwrap();
        assert_eq!((g.name.as_str(), g.display.as_str()), (to.as_str(), to.as_str()));
        assert!(done.popper_kept.as_ref().is_some_and(|k| k.is_file()));
        // a second table can't take the same name
        std::fs::write(layout.tables.join(format!("{old}.vpx")), "a second copy").unwrap();
        assert!(plan(&layout, &table, &to).is_err());
        std::fs::remove_file(&table).unwrap();
        // and back
        put_back(&layout, &done).unwrap();
        assert!(table.is_file() && layout.tables.join(format!("{old}.directb2s")).is_file());
        assert!(media.join("Wheel").join(format!("{old}01.png")).is_file());
        let g = popper::game_for(&db, &layout.tables, &format!("{old}.vpx")).unwrap().unwrap();
        assert_eq!(g.name, old);
        let _ = std::fs::remove_dir_all(&root);
    }
}
