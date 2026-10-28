//! PinUP Popper's game list (`PUPDatabase.db` beside PinUP's `PUPVideos`, SQLite): its games for
//! VPX's tables — the emulator whose games' folder is the tables folder — each by its table file's
//! name, with Popper's own name for it (which its media are named for) and the name it shows. Read
//! for a table's Popper name, and changed when a table is renamed to VPS's name for it ("Overwrite
//! for all players"), in one change, the list kept first. Ours.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};

use crate::plan::Layout;

/// One of Popper's games for VPX's tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub id: i64,
    /// Popper's own name for it: its media are named for it (`GameName`).
    pub name: String,
    /// Its table file's name, beside the emulator's games folder (`GameFileName`).
    pub file: String,
    /// The name Popper shows (`GameDisplay`).
    pub display: String,
}

/// Popper's game list for the layout's tables, if Popper is there.
pub fn database(layout: &Layout) -> Option<PathBuf> {
    let db = layout.pup_videos.parent()?.join("PUPDatabase.db");
    db.is_file().then_some(db)
}

fn fail(e: rusqlite::Error) -> String {
    format!("PinUP Popper's game list: {e}")
}

/// The emulators whose games' folder is `tables` (case aside, a trailing slash aside).
fn emulators(c: &Connection, tables: &Path) -> Result<Vec<i64>, String> {
    let want = tables.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    let mut q = c.prepare("SELECT EMUID, DirGames FROM Emulators").map_err(fail)?;
    let rows = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))).map_err(fail)?;
    Ok(rows.flatten().filter(|(_, dir)| dir.as_deref().is_some_and(|d| d.trim().trim_end_matches(['\\', '/']).to_lowercase() == want)).map(|(id, _)| id).collect())
}

/// Popper's games for the tables in `tables`, read only.
pub fn games(db: &Path, tables: &Path) -> Result<Vec<Game>, String> {
    let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(fail)?;
    let mut out = Vec::new();
    for emu in emulators(&c, tables)? {
        let mut q = c.prepare("SELECT GameID, GameName, GameFileName, GameDisplay FROM Games WHERE EMUID = ?1").map_err(fail)?;
        let rows = q
            .query_map(params![emu], |r| {
                Ok(Game {
                    id: r.get(0)?,
                    name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    file: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    display: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                })
            })
            .map_err(fail)?;
        out.extend(rows.flatten());
    }
    Ok(out)
}

/// The game for the table file `file` (its name alone), case aside.
pub fn game_for(db: &Path, tables: &Path, file: &str) -> Result<Option<Game>, String> {
    Ok(games(db, tables)?.into_iter().find(|g| g.file.eq_ignore_ascii_case(file)))
}

/// The name Popper's media for a table are named for: Popper's own name for its game, else the
/// table file's name (`stem`, without its extension).
pub fn media_name(layout: &Layout, stem: &str) -> String {
    database(layout)
        .and_then(|db| game_for(&db, &layout.tables, &format!("{stem}.vpx")).ok().flatten())
        .map(|g| g.name)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| stem.to_string())
}

/// The name Popper's media in its media `folder` (`POPMedia\<emulator>\<screen>`) are named
/// for, for the table file `file`: Popper's own name for its game, from its list beside its media
/// (any emulator's: a `.vpx` is VPX's), else `stem`.
pub fn media_name_in(folder: &Path, file: &str, stem: &str) -> String {
    let db = folder.parent().and_then(Path::parent).and_then(Path::parent).map(|p| p.join("PUPDatabase.db")).filter(|p| p.is_file());
    let found = db.and_then(|db| {
        let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).ok()?;
        c.query_row("SELECT GameName FROM Games WHERE GameFileName = ?1 COLLATE NOCASE", params![file], |r| r.get::<_, Option<String>>(0)).ok().flatten()
    });
    found.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| stem.to_string())
}

/// A copy of the list kept in `dir` before it's changed, named for when; its path.
pub fn keep_copy(db: &Path, dir: &Path, stamp: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let to = dir.join(format!("PUPDatabase {stamp}.db"));
    std::fs::copy(db, &to).map_err(|e| format!("PinUP Popper's game list couldn't be kept before changing it: {e}"))?;
    Ok(to)
}

/// Game `id` given its table file's, its own and its shown name, in one change, only if it's
/// still as `was` says (else nothing is changed).
pub fn rename(db: &Path, was: &Game, file: &str, name: &str, display: &str) -> Result<(), String> {
    let mut c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(fail)?;
    c.busy_timeout(std::time::Duration::from_secs(5)).map_err(fail)?;
    let t = c.transaction().map_err(fail)?;
    let changed = t
        .execute(
            "UPDATE Games SET GameFileName = ?1, GameName = ?2, GameDisplay = ?3 WHERE GameID = ?4 AND GameFileName = ?5 AND GameName = ?6",
            params![file, name, display, was.id, was.file, was.name],
        )
        .map_err(fail)?;
    if changed != 1 {
        return Err("PinUP Popper's game list changed meanwhile; nothing was renamed in it.".into());
    }
    t.commit().map_err(fail)
}

/// The ratings Popper keeps for the tables in `tables` (`GameRating`, which VPin Studio sets too),
/// by each game's table file as the list names it: whole stars, one to five, those rated (0 none).
pub fn ratings(db: &Path, tables: &Path) -> Result<Vec<(String, u8)>, String> {
    let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(fail)?;
    let mut out = Vec::new();
    for emu in emulators(&c, tables)? {
        let mut q = c.prepare("SELECT GameFileName, GameRating FROM Games WHERE EMUID = ?1").map_err(fail)?;
        let rows = q.query_map(params![emu], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<f64>>(1)?))).map_err(fail)?;
        out.extend(rows.flatten().filter_map(|(file, rating)| Some((file?, crate::frontends::stars_of(rating?)?))));
    }
    Ok(out)
}

/// The table file `file`'s game (as the list names it) given `stars` (none: no rating), in one
/// change; whether the list holds it.
pub fn set_rating(db: &Path, tables: &Path, file: &str, stars: Option<u8>) -> Result<bool, String> {
    let mut c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(fail)?;
    c.busy_timeout(std::time::Duration::from_secs(5)).map_err(fail)?;
    let emus = emulators(&c, tables)?;
    let t = c.transaction().map_err(fail)?;
    let mut changed = 0;
    for emu in emus {
        changed += t.execute("UPDATE Games SET GameRating = ?1 WHERE EMUID = ?2 AND GameFileName = ?3 COLLATE NOCASE", params![stars.map(i64::from), emu, file]).map_err(fail)?;
    }
    t.commit().map_err(fail)?;
    Ok(changed > 0)
}

/// Game `was` taken out of the list, with its rows in Popper's playlists and play counts, as VPin
/// Studio takes one out (`PinUPConnector.deleteGame`), in one change, only if it's still as read
/// (else nothing is changed).
pub fn remove(db: &Path, was: &Game) -> Result<(), String> {
    let mut c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(fail)?;
    c.busy_timeout(std::time::Duration::from_secs(5)).map_err(fail)?;
    let t = c.transaction().map_err(fail)?;
    let gone = t
        .execute("DELETE FROM Games WHERE GameID = ?1 AND IFNULL(GameFileName, '') = ?2 AND IFNULL(GameName, '') = ?3", params![was.id, was.file, was.name])
        .map_err(fail)?;
    if gone != 1 {
        return Err("PinUP Popper's game list changed meanwhile; nothing was taken out of it.".into());
    }
    for rows in ["PlayListDetails", "GamesStats"] {
        let there: bool = t.query_row("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1", params![rows], |r| r.get(0)).map_err(fail)?;
        if there {
            t.execute(&format!("DELETE FROM {rows} WHERE GameID = ?1"), params![was.id]).map_err(fail)?;
        }
    }
    t.commit().map_err(fail)
}

/// Popper's folder for a kind of a table's media, as the app names its kinds (`playfield`,
/// `playfield-video`, `wheel`…).
pub fn screen_folder(kind: &str) -> Option<&'static str> {
    Some(match kind.strip_suffix("-video").unwrap_or(kind) {
        "playfield" => "PlayField",
        "backglass" => "BackGlass",
        "dmd" => "DMD",
        "topper" => "Topper",
        "wheel" => "Wheel",
        "gameinfo" => "GameInfo",
        "gamehelp" => "GameHelp",
        "loading" => "Loading",
        "menu" => "Menu",
        "gameselect" => "GameSelect",
        _ => return None,
    })
}

const PICTURES: [&str; 5] = ["png", "jpg", "jpeg", "apng", "gif"];
const VIDEOS: [&str; 5] = ["mp4", "m4v", "webm", "mov", "f4v"];

fn family(p: &Path) -> Option<bool> {
    let ext = p.extension()?.to_string_lossy().to_lowercase();
    if PICTURES.contains(&ext.as_str()) {
        Some(false)
    } else if VIDEOS.contains(&ext.as_str()) {
        Some(true)
    } else {
        None
    }
}

/// Two files the same byte for byte, their sizes compared first.
fn same_bytes(a: &Path, b: &Path) -> bool {
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
    size(a).is_some() && size(a) == size(b) && std::fs::read(a).ok().is_some_and(|x| std::fs::read(b).ok().is_some_and(|y| x == y))
}

/// Pictures Popper can't show that are turned into PNGs on their way into its folders: VPS's pictures
/// are WebP, which it refuses.
const CONVERTED: [&str; 3] = ["webp", "bmp", "tga"];

/// `chosen` made the one Popper shows for a kind of a game's media: in Popper's `folder` for the kind, named for Popper's
/// own name for the game, `name` (`Name.png`); the one Popper showed there kept beside it, numbered
/// as Popper numbers a kind's others (`Name01.png`), never deleted; one of those numbered chosen
/// swapped with the one in front. Ours — VPS's or our player's — copied in, a picture Popper can't
/// show (VPS's WebP) as a PNG. A copy of VPS's or ours put in front before (`ours` knows them) is
/// sent away by `recycle` rather than kept numbered: what it copied is still VPS's or ours. The
/// front: where `chosen` is in front now, moved or copied there, or found there already the same
/// byte for byte; None when it was the one in front already.
pub fn put_in_front(folder: &Path, name: &str, chosen: &Path, ours: &dyn Fn(&Path) -> bool, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<Option<PathBuf>, String> {
    put_in_front_as(folder, name, chosen, &|name, n| format!("{name}{n:02}"), ours, recycle)
}

/// `put_in_front` for a front end numbering its others its own way (`numbered`: PinUP Popper's
/// `Name01`, PinballX's and PinballY's `Name 1`).
pub fn put_in_front_as(
    folder: &Path,
    name: &str,
    chosen: &Path,
    numbered: &dyn Fn(&str, u32) -> String,
    ours: &dyn Fn(&Path) -> bool,
    recycle: &dyn Fn(&Path) -> Result<(), String>,
) -> Result<Option<PathBuf>, String> {
    let ext = chosen.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
    let convert = CONVERTED.contains(&ext.as_str());
    let video = if convert { false } else { family(chosen).ok_or("That isn't a picture or a video Popper shows.")? };
    let ext = if convert { "png".to_string() } else { ext };
    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let files: Vec<PathBuf> = std::fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    let stem_is = |p: &Path, s: &str| p.file_stem().is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case(s));
    let fronts: Vec<&PathBuf> = files.iter().filter(|p| stem_is(p, name) && family(p) == Some(video)).collect();
    let same = |a: &Path, b: &Path| a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy());
    if fronts.iter().any(|f| same(f, chosen)) {
        return Ok(None);
    }
    let inside = chosen.parent().is_some_and(|d| same(d, folder));
    // a copy of it in front already, byte for byte (ours made again the same): nothing to move
    let ext_is = |p: &Path| p.extension().is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case(&ext));
    if let [one] = fronts.as_slice()
        && !inside
        && !convert
        && ext_is(one)
        && same_bytes(one, chosen)
    {
        return Ok(Some((*one).clone()));
    }
    // the next free number for one moved aside
    let taken = |n: u32| files.iter().any(|p| stem_is(p, &numbered(name, n)));
    let mut free = (1..100).filter(|&n| !taken(n));
    let chosen_stem = chosen.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let front = folder.join(format!("{name}.{ext}"));
    let mut moved = Vec::new();
    for f in &fronts {
        // a copy put in before goes, never numbered among the front end's own
        if ours(f) {
            recycle(f).map_err(|e| format!("{} couldn't go to the Recycle Bin: {e}", f.display()))?;
            continue;
        }
        // one of Popper's numbered chosen takes the front's place, the front takes its number
        let to = if inside && moved.is_empty() {
            f.with_file_name(format!("{chosen_stem}.{}", f.extension().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()))
        } else {
            let n = free.next().ok_or("The folder has a hundred of that kind for it already.")?;
            f.with_file_name(format!("{}.{}", numbered(name, n), f.extension().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()))
        };
        // the chosen one, numbered, moves first when its name is the one the front takes
        if inside && same(&to, chosen) {
            let passing = chosen.with_extension(format!("{ext}.vpi-swapping"));
            std::fs::rename(chosen, &passing).map_err(|e| format!("{}: {e}", chosen.display()))?;
            std::fs::rename(f, &to).map_err(|e| format!("{}: {e}", f.display()))?;
            std::fs::rename(&passing, &front).map_err(|e| format!("{}: {e}", passing.display()))?;
            return Ok(Some(front));
        }
        std::fs::rename(f, &to).map_err(|e| format!("{}: {e}", f.display()))?;
        moved.push(to);
    }
    if inside {
        std::fs::rename(chosen, &front).map_err(|e| format!("{}: {e}", chosen.display()))?;
    } else if convert {
        // written beside its place first, then moved there, so a failed write leaves nothing half done
        let part = front.with_extension("png.part");
        let picture = image::open(chosen).map_err(|e| format!("{} couldn't be read: {e}", chosen.display()))?;
        picture.save_with_format(&part, image::ImageFormat::Png).map_err(|e| format!("{} couldn't be written as a PNG: {e}", front.display()))?;
        std::fs::rename(&part, &front).map_err(|e| format!("{}: {e}", front.display()))?;
    } else {
        std::fs::copy(chosen, &front).map_err(|e| format!("{} couldn't be copied into {}: {e}", chosen.display(), folder.display()))?;
    }
    Ok(Some(front))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No file a copy put in before, and none to send away.
    const NONE: &dyn Fn(&Path) -> bool = &|_| false;
    const KEEP: &dyn Fn(&Path) -> Result<(), String> = &|p| Err(format!("{} sent away", p.display()));

    #[test]
    fn a_copy_put_in_before_goes_and_poppers_own_stays() {
        let dir = std::env::temp_dir().join(format!("vpi-popper-copies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let playfield = dir.join("PlayField");
        let bin = dir.join("bin");
        std::fs::create_dir_all(&playfield).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        // the Recycle Bin, a folder of the test's own
        let recycle = |p: &Path| std::fs::rename(p, bin.join(p.file_name().unwrap())).map_err(|e| e.to_string());
        let name = "Cirqus Voltaire (Bally 1997)";
        let read = |n: &str| std::fs::read_to_string(playfield.join(n)).unwrap();
        let binned = |n: &str| std::fs::read_to_string(bin.join(n)).unwrap();
        std::fs::write(playfield.join(format!("{name}.png")), "popper's").unwrap();
        // ours put in: Popper's own kept numbered
        let ours = dir.join("ours.png");
        std::fs::write(&ours, "ours, first").unwrap();
        let front = put_in_front(&playfield, name, &ours, NONE, &recycle).unwrap().unwrap();
        assert_eq!(read(&format!("{name}01.png")), "popper's");
        let copy = front.clone();
        let was_ours = move |p: &Path| p == copy;
        // ours made again and put in: the copy before it sent away, not numbered beside Popper's
        std::fs::write(&ours, "ours, again").unwrap();
        assert_eq!(put_in_front(&playfield, name, &ours, &was_ours, &recycle).unwrap(), Some(front.clone()));
        assert_eq!(read(&format!("{name}.png")), "ours, again");
        assert_eq!(read(&format!("{name}01.png")), "popper's");
        assert!(!playfield.join(format!("{name}02.png")).exists());
        assert_eq!(binned(&format!("{name}.png")), "ours, first");
        std::fs::remove_file(bin.join(format!("{name}.png"))).unwrap();
        // the same again byte for byte: found in front, nothing moved or sent away
        assert_eq!(put_in_front(&playfield, name, &ours, NONE, KEEP).unwrap(), Some(front.clone()));
        // Popper's own chosen again: the copy in front sent away, Popper's back in front
        assert!(put_in_front(&playfield, name, &playfield.join(format!("{name}01.png")), &was_ours, &recycle).unwrap().is_some());
        assert_eq!(read(&format!("{name}.png")), "popper's");
        assert!(!playfield.join(format!("{name}01.png")).exists());
        assert_eq!(binned(&format!("{name}.png")), "ours, again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_chosen_put_in_front_poppers_own_kept_numbered() {
        let dir = std::env::temp_dir().join(format!("vpi-popper-front-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let wheel = dir.join("Wheel");
        std::fs::create_dir_all(&wheel).unwrap();
        let ours = dir.join("ours.png");
        std::fs::write(&ours, "ours").unwrap();
        std::fs::write(wheel.join("Black Knight (Williams 1980).png"), "popper's").unwrap();
        std::fs::write(wheel.join("Black Knight (Williams 1980)01.png"), "popper's second").unwrap();
        // ours copied in front; Popper's moved to the next free number
        assert!(put_in_front(&wheel, "Black Knight (Williams 1980)", &ours, NONE, KEEP).unwrap().is_some());
        let read = |n: &str| std::fs::read_to_string(wheel.join(n)).unwrap();
        assert_eq!(read("Black Knight (Williams 1980).png"), "ours");
        assert_eq!(read("Black Knight (Williams 1980)02.png"), "popper's");
        assert_eq!(read("Black Knight (Williams 1980)01.png"), "popper's second");
        // Popper's own numbered one chosen: swapped with the one in front
        assert!(put_in_front(&wheel, "Black Knight (Williams 1980)", &wheel.join("Black Knight (Williams 1980)02.png"), NONE, KEEP).unwrap().is_some());
        assert_eq!(read("Black Knight (Williams 1980).png"), "popper's");
        assert_eq!(read("Black Knight (Williams 1980)02.png"), "ours");
        // the one in front chosen: nothing to do
        assert!(put_in_front(&wheel, "Black Knight (Williams 1980)", &wheel.join("Black Knight (Williams 1980).png"), NONE, KEEP).unwrap().is_none());
        // a video beside the pictures isn't touched
        std::fs::write(wheel.join("Black Knight (Williams 1980).mp4"), "video").unwrap();
        assert!(put_in_front(&wheel, "Black Knight (Williams 1980)", &ours, NONE, KEEP).unwrap().is_some());
        assert_eq!(read("Black Knight (Williams 1980).mp4"), "video");
        // VPS's WebP picture: in front as a PNG Popper shows
        let webp = dir.join("vps.webp");
        image::RgbaImage::from_pixel(4, 3, image::Rgba([200, 30, 30, 255])).save_with_format(&webp, image::ImageFormat::WebP).unwrap();
        let front = put_in_front(&wheel, "Black Knight (Williams 1980)", &webp, NONE, KEEP).unwrap().unwrap();
        assert_eq!(front.extension().unwrap(), "png");
        assert_eq!(image::open(&front).unwrap().to_rgba8().dimensions(), (4, 3));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn made(dir: &Path) -> PathBuf {
        let db = dir.join("PUPDatabase.db");
        let c = Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE Emulators (EMUID INTEGER PRIMARY KEY, EmuName TEXT, DirGames TEXT);
             CREATE TABLE Games (GameID INTEGER PRIMARY KEY, EMUID INTEGER, GameName TEXT, GameFileName TEXT, GameDisplay TEXT);
             INSERT INTO Emulators VALUES (1, 'Visual Pinball X', 'C:\\vPinball\\visualpinball\\Tables'), (4, 'Future Pinball', 'C:\\vPinball\\FuturePinball\\Tables');
             INSERT INTO Games VALUES (3, 1, 'Attack from Mars (Bally 1995)', 'Attack from Mars (Bally 1995) JPSalas 6.0.0.vpx', 'Attack from Mars (Bally 1995) JPSalas 6.0.0'),
                                      (7, 1, 'Black Knight (Williams 1980)', 'Black Knight (Williams 1980).vpx', 'Black Knight (Williams 1980)'),
                                      (9, 4, 'Retroflair', 'Retroflair.fpt', 'Retroflair');",
        )
        .unwrap();
        db
    }

    #[test]
    fn vpxs_games_read_and_one_renamed_only_while_it_is_as_read() {
        let dir = std::env::temp_dir().join(format!("vpi-popper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = made(&dir);
        let tables = Path::new(r"C:\vPinball\VisualPinball\Tables\");
        let all = games(&db, tables).unwrap();
        assert_eq!(all.len(), 2, "{all:?}");
        let afm = game_for(&db, tables, "attack from mars (bally 1995) jpsalas 6.0.0.vpx").unwrap().unwrap();
        assert_eq!((afm.id, afm.name.as_str()), (3, "Attack from Mars (Bally 1995)"));
        rename(&db, &afm, "Attack from Mars (Bally 1995).vpx", "Attack from Mars (Bally 1995)", "Attack from Mars (Bally 1995)").unwrap();
        let now = game_for(&db, tables, "Attack from Mars (Bally 1995).vpx").unwrap().unwrap();
        assert_eq!((now.id, now.display.as_str()), (3, "Attack from Mars (Bally 1995)"));
        // as read before: no longer so, and nothing changed
        assert!(rename(&db, &afm, "x.vpx", "x", "x").is_err());
        assert_eq!(game_for(&db, tables, "Attack from Mars (Bally 1995).vpx").unwrap().unwrap(), now);
        let kept = keep_copy(&db, &dir.join("kept"), "2025-01-02 18-00-00").unwrap();
        assert!(kept.is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_game_taken_out_with_its_playlists_and_play_counts_only_while_it_is_as_read() {
        let dir = std::env::temp_dir().join(format!("vpi-popper-remove-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = made(&dir);
        let c = Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE PlayListDetails (UniqueID INTEGER PRIMARY KEY, PlayListID INTEGER, GameID INTEGER);
             CREATE TABLE GamesStats (UniqueID INTEGER PRIMARY KEY, GameID INTEGER, NumberPlays INTEGER);
             INSERT INTO PlayListDetails VALUES (1, 1, 3), (2, 1, 7), (3, 2, 3);
             INSERT INTO GamesStats VALUES (1, 3, 12), (2, 7, 4);",
        )
        .unwrap();
        drop(c);
        let tables = Path::new(r"C:\vPinball\VisualPinball\Tables");
        let afm = game_for(&db, tables, "Attack from Mars (Bally 1995) JPSalas 6.0.0.vpx").unwrap().unwrap();
        // not as read: nothing taken out
        assert!(remove(&db, &Game { name: "another".into(), ..afm.clone() }).is_err());
        assert_eq!(games(&db, tables).unwrap().len(), 2);
        remove(&db, &afm).unwrap();
        assert_eq!(games(&db, tables).unwrap().iter().map(|g| g.id).collect::<Vec<_>>(), [7]);
        let c = Connection::open(&db).unwrap();
        let left = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(left("SELECT COUNT(*) FROM PlayListDetails"), 1);
        assert_eq!(left("SELECT COUNT(*) FROM GamesStats WHERE GameID = 3"), 0);
        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
