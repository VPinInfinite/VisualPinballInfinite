//! A table's rating, one to five stars, as the front ends keep theirs — PinUP Popper's game list
//! (`GameRating`), PinballX's and PinballY's game lists (`<rating>`) and PinballY's stats over them;
//! VPin Studio keeps its ratings in the front end it runs beside — read for the library, the first
//! that rates a table taken (PinUP Popper's, then PinballX's, then PinballY's), and written to each
//! that lists it on the user's word. VPX itself keeps none. Ours.

use std::collections::HashMap;
use std::path::Path;

use crate::frontends::{Found, Front};
use crate::plan::Layout;

/// A rating as a front end keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rating {
    pub stars: u8,
    pub from: Front,
}

/// A table's name for its ratings, the same for every front end: its file below the tables folder
/// (`listed_name`), or a game's name in a list, lower case, without `.vpx`.
pub fn key(listed: &str) -> String {
    let low = listed.replace('/', "\\").to_lowercase();
    low.strip_suffix(".vpx").map(str::to_string).unwrap_or(low)
}

/// Every rating the front ends keep for the library's tables, by `key`: PinUP Popper's first, then
/// PinballX's, then PinballY's, as `fronts` were found.
pub fn read_all(layout: &Layout, fronts: &[Found]) -> HashMap<String, Rating> {
    let mut out: HashMap<String, Rating> = HashMap::new();
    if let Some(db) = crate::popper::database(layout)
        && let Ok(rated) = crate::popper::ratings(&db, &layout.tables)
    {
        for (file, stars) in rated {
            out.entry(key(&file)).or_insert(Rating { stars, from: Front::Popper });
        }
    }
    for f in [Front::PinballX, Front::PinballY] {
        for found in fronts.iter().filter(|x| x.front.front == f) {
            for (name, stars) in found.front.ratings() {
                out.entry(key(&name)).or_insert(Rating { stars, from: f });
            }
        }
    }
    out
}

/// The front ends `write` would change for the library: PinUP Popper where its list is there, and
/// PinballX and PinballY where they play the library's tables.
pub fn fronts_there(layout: &Layout, fronts: &[Found]) -> Vec<Front> {
    let mut out: Vec<Front> = crate::popper::database(layout).map(|_| Front::Popper).into_iter().collect();
    for f in [Front::PinballX, Front::PinballY] {
        if fronts.iter().any(|x| x.front.front == f && x.front.databases.is_some()) {
            out.push(f);
        }
    }
    out
}

/// The table listed as `listed` (its file below the tables folder, as `listed_name` gives it) given
/// `stars` (none: no rating) in each front end's list that holds it, those in `skip` (running, so
/// they'd keep their own) left alone; PinUP Popper's list kept in `keep` first where one's named
/// (`popper::keep_copy`: the folder and a stamp). Each front end's outcome: changed, not changed
/// (not listed there, or already so), or what went wrong.
pub fn write(layout: &Layout, fronts: &[Found], listed: &str, stars: Option<u8>, skip: &[Front], keep: Option<(&Path, &str)>) -> Vec<(Front, Result<bool, String>)> {
    let mut out = Vec::new();
    if !skip.contains(&Front::Popper)
        && let Some(db) = crate::popper::database(layout)
    {
        let kept = match keep {
            Some((dir, stamp)) => crate::popper::keep_copy(&db, dir, stamp).map(|_| ()),
            None => Ok(()),
        };
        out.push((Front::Popper, kept.and_then(|()| crate::popper::set_rating(&db, &layout.tables, listed, stars))));
    }
    let name = listed.strip_suffix(".vpx").or_else(|| listed.strip_suffix(".VPX")).unwrap_or(listed);
    for found in fronts.iter().filter(|x| matches!(x.front.front, Front::PinballX | Front::PinballY) && !skip.contains(&x.front.front)) {
        out.push((found.front.front, found.front.set_rating(name, stars)));
    }
    out
}

/// Which of the front ends are running now, so their lists are left alone (each keeps its own while
/// it runs and writes it back).
pub fn running() -> Vec<Front> {
    crate::rename::holders_running()
        .into_iter()
        .filter_map(|n| match n {
            "PinUP Popper" => Some(Front::Popper),
            "PinballX" => Some(Front::PinballX),
            "PinballY" => Some(Front::PinballY),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontends::{FrontEnd, id_in_pinbally};

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vpi-ratings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_tables_key_the_same_from_every_front_end() {
        assert_eq!(key("Attack from Mars.vpx"), "attack from mars");
        assert_eq!(key("AFM/Attack From Mars.VPX"), "afm\\attack from mars");
        assert_eq!(key("Taxi"), "taxi");
    }

    #[test]
    fn popper_ratings_read_and_written_for_the_library_s_emulator_alone() {
        let d = scratch("popper");
        let tables = d.join("Visual Pinball").join("Tables");
        std::fs::create_dir_all(&tables).unwrap();
        let pup = d.join("PinUPSystem");
        std::fs::create_dir_all(pup.join("PUPVideos")).unwrap();
        let db = pup.join("PUPDatabase.db");
        {
            let c = rusqlite::Connection::open(&db).unwrap();
            c.execute_batch(&format!(
                "CREATE TABLE Emulators (EMUID INTEGER, DirGames TEXT);
                 INSERT INTO Emulators VALUES (1, '{}'), (2, 'C:\\Elsewhere');
                 CREATE TABLE Games (GameID INTEGER, EMUID INTEGER, GameName TEXT, GameFileName TEXT, GameDisplay TEXT, GameRating INTEGER);
                 INSERT INTO Games VALUES (1, 1, 'AFM', 'Attack from Mars.vpx', 'AFM', 5), (2, 1, 'Taxi', 'Taxi.vpx', 'Taxi', 0),
                   (3, 1, 'MM', 'MM\\Medieval Madness.vpx', 'MM', NULL), (4, 2, 'Other', 'Taxi.vpx', 'Other', 3);",
                tables.display()
            ))
            .unwrap();
        }
        let mut layout = Layout::under(&d.join("Visual Pinball"));
        layout.tables = tables.clone();
        layout.pup_videos = pup.join("PUPVideos");
        let read = read_all(&layout, &[]);
        // 0 and none aren't ratings; the other emulator's game isn't the library's
        assert_eq!(read.len(), 1);
        assert_eq!(read.get("attack from mars"), Some(&Rating { stars: 5, from: Front::Popper }));
        assert_eq!(fronts_there(&layout, &[]), [Front::Popper]);
        // written for the table in its own folder, the list kept first
        let kept = d.join("kept");
        let done = write(&layout, &[], "MM\\Medieval Madness.vpx", Some(4), &[], Some((&kept, "2025-01-02 03-04-05")));
        assert_eq!(done, [(Front::Popper, Ok(true))]);
        assert!(kept.join("PUPDatabase 2025-01-02 03-04-05.db").is_file());
        assert_eq!(read_all(&layout, &[]).get("mm\\medieval madness").map(|r| r.stars), Some(4));
        // cleared, the other emulator's untouched; a table it doesn't list changes nothing
        assert_eq!(write(&layout, &[], "Taxi.vpx", None, &[], None), [(Front::Popper, Ok(true))]);
        assert_eq!(write(&layout, &[], "Not There.vpx", Some(2), &[], None), [(Front::Popper, Ok(false))]);
        let c = rusqlite::Connection::open(&db).unwrap();
        let other: Option<i64> = c.query_row("SELECT GameRating FROM Games WHERE GameID = 4", [], |r| r.get(0)).unwrap();
        assert_eq!(other, Some(3));
        // a running Popper is left alone
        assert!(write(&layout, &[], "Taxi.vpx", Some(1), &[Front::Popper], None).is_empty());
        drop(c);
        let _ = std::fs::remove_dir_all(&d);
    }

    fn pinballx_at(d: &Path, list: &str) -> FrontEnd {
        let px = d.join("PinballX");
        let dbs = px.join("Databases").join("Visual Pinball");
        std::fs::create_dir_all(&dbs).unwrap();
        std::fs::write(dbs.join("Visual Pinball.xml"), list).unwrap();
        FrontEnd { front: Front::PinballX, folder: px.clone(), system: "Visual Pinball".into(), media_root: px.join("Media"), media: px.join("Media").join("Visual Pinball"), databases: Some(dbs) }
    }

    #[test]
    fn pinballx_ratings_read_and_written_in_its_game_list() {
        let d = scratch("x");
        let list ="<?xml version=\"1.0\"?>\r\n<menu>\r\n  <game name=\"Taxi\">\r\n    <description>Taxi (Williams 1988)</description>\r\n    <rating>3</rating>\r\n  </game>\r\n  <game name=\"AFM\">\r\n    <description>Attack from Mars (Bally 1995)</description>\r\n  </game>\r\n  <game name=\"Gorgar\"><description>Gorgar</description><rating>0</rating></game>\r\n</menu>\r\n";
        let fe = pinballx_at(&d, list);
        assert_eq!(fe.ratings(), [("Taxi".to_string(), 3)]);
        // a rating added on a line of its own, lined up; one changed in place; one already none,
        // still listed; one it doesn't list
        assert!(fe.set_rating("AFM", Some(5)).unwrap());
        assert!(fe.set_rating("Taxi.vpx", Some(4)).unwrap());
        assert!(fe.set_rating("Gorgar", None).unwrap());
        assert!(!fe.set_rating("Not There", Some(1)).unwrap());
        let file = fe.databases.as_ref().unwrap().join("Visual Pinball.xml");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("    <description>Attack from Mars (Bally 1995)</description>\r\n    <rating>5</rating>\r\n  </game>"), "{text}");
        assert!(text.contains("<rating>4</rating>"));
        let mut got = fe.ratings();
        got.sort();
        assert_eq!(got, [("AFM".to_string(), 5), ("Taxi".to_string(), 4)]);
        assert!(std::path::PathBuf::from(format!("{}.vpi-before", file.display())).is_file());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn pinbally_names_a_game_as_its_stats_do() {
        let game = |desc: &str, title: Option<&str>, maker: Option<&str>, year: Option<i32>| crate::frontends::Game {
            name: "x".into(),
            media_name: None,
            list: Default::default(),
            rating: 0.0,
            description: desc.into(),
            title: title.map(str::to_string),
            manufacturer: maker.map(str::to_string),
            year,
        };
        let vpx = "Visual Pinball X";
        assert_eq!(id_in_pinbally(&game("Attack from Mars (Bally 1995)", None, None, None), vpx)[0], "Attack from Mars (Bally 1995).Visual Pinball X");
        // the list's own maker and year over the description's
        assert_eq!(id_in_pinbally(&game("Taxi (Williams 1988)", None, Some("Midway"), None), vpx)[0], "Taxi (Midway 1988).Visual Pinball X");
        assert_eq!(id_in_pinbally(&game("Gorgar (1979)", None, None, None), vpx)[0], "Gorgar (1979).Visual Pinball X");
        assert_eq!(id_in_pinbally(&game("Black Knight (Williams)", None, None, None), vpx)[0], "Black Knight (Williams).Visual Pinball X");
        assert_eq!(id_in_pinbally(&game("Spirit (of 76) (Gottlieb 1976)", None, None, None), vpx)[0], "Spirit (of 76) (Gottlieb 1976).Visual Pinball X");
        assert_eq!(id_in_pinbally(&game("Plain", Some("A Title"), None, Some(1990)), vpx), ["A Title (1990).Visual Pinball X".to_string(), "A Title.Visual Pinball X".to_string()]);
        assert_eq!(id_in_pinbally(&game("Plain", None, None, None), vpx)[0], "Plain.Visual Pinball X");
    }

    #[test]
    fn pinballys_stats_over_its_list_and_changed_in_place() {
        let d = scratch("y");
        let py = d.join("PinballY");
        let dbs = py.join("Databases").join("Visual Pinball X");
        std::fs::create_dir_all(&dbs).unwrap();
        std::fs::write(dbs.join("Visual Pinball X.xml"), "<menu><game name=\"Taxi\"><description>Taxi (Williams 1988)</description><rating>2</rating></game><game name=\"AFM\"><description>Attack from Mars (Bally 1995)</description><rating>4</rating></game></menu>").unwrap();
        // written as PinballY writes it: UTF-16 with its mark, fields quoted only where needed
        let stats = "Game,Last Played,Play Count,Rating,Categories\r\nTaxi (Williams 1988).Visual Pinball X,,3,4.5,\"Fun, Classic\"\r\nAttack from Mars (Bally 1995).Visual Pinball X,,1,,\r\n";
        let utf16: Vec<u8> = [0xFF, 0xFE].into_iter().chain(stats.encode_utf16().flat_map(u16::to_le_bytes)).collect();
        std::fs::write(py.join("GameStats.csv"), utf16).unwrap();
        let fe = FrontEnd { front: Front::PinballY, folder: py.clone(), system: "Visual Pinball X".into(), media_root: py.join("Media"), media: py.join("Media"), databases: Some(dbs.clone()) };
        let mut got = fe.ratings();
        got.sort();
        // Taxi's stats (4.5, rounded up) over its list's 2; AFM's left empty there, so its list's
        assert_eq!(got, [("AFM".to_string(), 4), ("Taxi".to_string(), 5)]);
        assert!(fe.set_rating("Taxi", Some(2)).unwrap());
        let bytes = std::fs::read(py.join("GameStats.csv")).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let text = String::from_utf16(&units).unwrap();
        assert_eq!(text, "Game,Last Played,Play Count,Rating,Categories\r\nTaxi (Williams 1988).Visual Pinball X,,3,2,\"Fun, Classic\"\r\nAttack from Mars (Bally 1995).Visual Pinball X,,1,,\r\n");
        let mut got = fe.ratings();
        got.sort();
        assert_eq!(got, [("AFM".to_string(), 4), ("Taxi".to_string(), 2)]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
