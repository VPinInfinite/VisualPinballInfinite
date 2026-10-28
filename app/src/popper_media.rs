//! The media a table uses put where PinUP Popper shows them, a setting off unless it's turned on
//! (`prefs.popperMedia`, Tools › Media): the one of each kind chosen on a table's page — VPS's
//! picture, one our player made, or one of Popper's own — put in front in Popper's folder for its
//! screen, named for Popper's own name for the game (`library::popper::put_in_front`: Popper's one
//! before it kept, numbered). And the same in PinballX's and PinballY's folders, where either plays
//! the library's tables (`prefs.frontsMedia`): named as each names the game's media, its own one
//! before kept numbered as it numbers them (`library::frontends::put_in_front`); a copy of VPS's
//! or ours put in before sent to the Recycle Bin instead (`User::put_in`). When a choice changes,
//! for that kind; on a word from the settings, for every table.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::library::{Shared, State, Table};

/// Whether it's on for PinUP Popper.
pub fn on(st: &State) -> bool {
    st.user.prefs.get("popperMedia").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// Whether it's on for PinballX and PinballY.
pub fn fronts_on(st: &State) -> bool {
    st.user.prefs.get("frontsMedia").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// One kind's chosen put in: from where (a file, or VPS's picture to fetch first), into which
/// folder, under which name; the table, the kind and the choice, to remember it by.
pub struct Job {
    from: From,
    folder: PathBuf,
    name: String,
    table: String,
    kind: String,
    pick: String,
    /// Into PinballX's or PinballY's folder, not Popper's.
    other: Option<::library::frontends::Front>,
}

enum From {
    File(PathBuf),
    Vps(String),
}

/// What putting a table's chosen media in would do, in Popper's folders where that's on (or
/// `asked`) and PinballX's and PinballY's where that's on.
pub fn jobs(st: &State, t: &Table, only: Option<&str>, asked: bool) -> Vec<Job> {
    let mut out = if asked || on(st) { popper_jobs(st, t, only, asked) } else { Vec::new() };
    if fronts_on(st) {
        out.extend(front_jobs(st, t, only));
    }
    out
}

/// Where a choice comes from: a file, or VPS's picture to fetch first.
fn source(st: &State, t: &Table, pick: &str) -> Option<From> {
    if let Some(id) = pick.strip_prefix("vps:") {
        Some(From::Vps(st.vps_pictures.get(id)?.clone()))
    } else if let Some(kind) = pick.strip_prefix("ours:") {
        Some(From::File(crate::own_pictures::made(&t.id, kind)?.0))
    } else if let Some(file) = pick.strip_prefix("file:") {
        Some(From::File(t.holds.media.iter().find(|x| x.path.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == file))?.path.clone()))
    } else {
        None
    }
}

/// The chosen of each kind (or `only` that kind) put in front in PinballX's and PinballY's folders,
/// where either plays the library's tables: but one each shows already.
fn front_jobs(st: &State, t: &Table, only: Option<&str>) -> Vec<Job> {
    let file_name = t.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let view = crate::library::view(st, t);
    let mut out = Vec::new();
    for found in st.fronts.iter().filter(|f| f.front.front != ::library::frontends::Front::Popper) {
        let name = found.media_name(&file_name);
        let key = found.front.front.key();
        for m in view.media.iter().filter(|m| m.chosen && only.is_none_or(|k| k == m.kind) && !(m.front && m.from == key)) {
            let Some(folder) = ::library::popper::screen_folder(&m.kind).and_then(|k| found.front.place(k, m.kind.ends_with("-video"))) else { continue };
            let Some(from) = source(st, t, &m.pick) else { continue };
            out.push(Job { from, folder, name: name.clone(), table: crate::library::key(&t.path), kind: m.kind.clone(), pick: m.pick.clone(), other: Some(found.front.front) });
        }
    }
    out
}

/// The chosen of each kind (or `only` that kind) put in front in Popper's folders, but those
/// Popper shows already; none without Popper's media folder. `asked`: the user's word for that one
/// kind, whatever was chosen for it put in.
fn popper_jobs(st: &State, t: &Table, only: Option<&str>, asked: bool) -> Vec<Job> {
    let Some(layout) = st.layout.as_ref() else { return Vec::new() };
    let Some(root) = ::library::installed::pop_media(layout) else { return Vec::new() };
    let file_name = t.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let name = ::library::popper::database(layout)
        .and_then(|db| ::library::popper::game_for(&db, &layout.tables, &file_name).ok().flatten())
        .map(|g| g.name)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| t.stem.clone());
    let view = crate::library::view(st, t);
    // only what the user chose for a kind, or what fills a screen PinUP Popper shows nothing on: a
    // kind's first by default (VPS's playfield picture, say) never puts aside what Popper shows
    // there already
    let explicit = st.user.chosen.get(&crate::library::key(&t.path));
    let shows = |kind: &str| view.media.iter().any(|x| x.front && x.from == "Popper" && ::library::popper::screen_folder(&x.kind) == ::library::popper::screen_folder(kind));
    view.media
        .iter()
        .filter(|m| m.chosen && !(m.front && m.from == "Popper") && only.is_none_or(|k| k == m.kind))
        .filter(|m| asked || explicit.is_some_and(|c| c.contains_key(&m.kind)) || !shows(&m.kind))
        .filter_map(|m| {
            let folder = root.join(::library::popper::screen_folder(&m.kind)?);
            let from = source(st, t, &m.pick)?;
            Some(Job { from, folder, name: name.clone(), table: crate::library::key(&t.path), kind: m.kind.clone(), pick: m.pick.clone(), other: None })
        })
        .collect()
}

/// What putting every table's in would do, before it's done: how many of each kind from where
/// (`VPS`, `ours`, `Popper` — one of its own numbered put in front), the kind in words.
pub fn plan(st: &State) -> Vec<(String, &'static str, usize)> {
    let mut out: Vec<(String, &'static str, usize)> = Vec::new();
    for t in &st.tables {
        for j in jobs(st, t, None, false) {
            let label = crate::library::kind_label(&j.kind);
            // a front end's own file: Popper's, or PinballX's or PinballY's put in Popper's folder
            let from = match &j.from {
                _ if j.pick.starts_with("vps:") => "VPS",
                _ if j.pick.starts_with("ours:") => "ours",
                From::File(p) => t.holds.media.iter().find(|m| &m.path == p).map_or("Popper", |m| m.from.key()),
                From::Vps(_) => "VPS",
            };
            match out.iter_mut().find(|(l, f, _)| *l == label && *f == from) {
                Some((_, _, n)) => *n += 1,
                None => out.push((label, from, 1)),
            }
        }
    }
    out
}

/// What a run did: how many put in, and what couldn't be, each reason once with how many.
#[derive(Default)]
pub struct Done {
    pub moved: usize,
    pub problems: std::collections::BTreeMap<String, usize>,
}

impl Done {
    /// In words, for a notice: none of the reasons more than once.
    pub fn said(&self) -> Result<usize, String> {
        if self.problems.is_empty() {
            return Ok(self.moved);
        }
        let reasons: Vec<String> = self.problems.iter().map(|(why, n)| if *n == 1 { why.clone() } else { format!("{why} ({n} of them)") }).collect();
        Err(format!("{} put in the front ends’ folders; {} couldn’t be: {}", self.moved, self.problems.values().sum::<usize>(), reasons.join("; ")))
    }
}

/// The jobs done, VPS's pictures fetched first where they aren't kept yet; each put in remembered:
/// the choice now Popper's copy in front, the source shown no more beside it, the copy known as
/// ours (`User::put_in`) — one put in before sent to the Recycle Bin when another takes its place.
pub fn run(app: &AppHandle, jobs: Vec<Job>, done: &mut Done) {
    let put_in = app.state::<Shared>().0.lock().unwrap().user.put_in.clone();
    let mut fronts: Vec<(Job, PathBuf)> = Vec::new();
    // the copies sent to the Recycle Bin, by table and path
    let gone: RefCell<Vec<(String, String)>> = RefCell::new(Vec::new());
    for j in jobs {
        let path = match &j.from {
            From::File(p) => Some(p.clone()),
            From::Vps(url) => crate::pictures::kept_vps(url),
        };
        let Some(path) = path else {
            *done.problems.entry("one of VPS’s pictures couldn’t be fetched".to_string()).or_default() += 1;
            continue;
        };
        let copies = put_in.get(&j.table);
        let ours = |f: &Path| copies.is_some_and(|c| c.contains_key(&f.to_string_lossy().to_lowercase()));
        let recycle = |f: &Path| {
            ::library::install::to_recycle_bin(f)?;
            gone.borrow_mut().push((j.table.clone(), f.to_string_lossy().to_lowercase()));
            Ok(())
        };
        let put = match j.other {
            None => ::library::popper::put_in_front(&j.folder, &j.name, &path, &ours, &recycle),
            Some(_) => ::library::frontends::put_in_front(&j.folder, &j.name, &path, &ours, &recycle),
        };
        match put {
            Ok(Some(front)) => {
                done.moved += 1;
                fronts.push((j, front));
            }
            Ok(None) => {}
            Err(e) => *done.problems.entry(e).or_default() += 1,
        }
    }
    let gone = gone.into_inner();
    if fronts.is_empty() && gone.is_empty() {
        return;
    }
    let shared = app.state::<Shared>();
    let mut st = shared.0.lock().unwrap();
    let user = &mut st.user;
    let tables = remember(user, &put_in, gone, fronts);
    // the copies gone from the folders forgotten
    for k in &tables {
        if let Some(copies) = user.put_in.get_mut(k) {
            copies.retain(|path, _| Path::new(path).exists());
        }
    }
    user.put_in.retain(|_, c| !c.is_empty());
    if let Err(e) = user.save() {
        eprintln!("PinUP Popper's media: {e}");
    }
}

/// What a run did, remembered (`before`: the copies known as it began; `gone`: those it sent to the
/// Recycle Bin, by table and path; `fronts`: each job done, with its file in front now). The tables
/// it touched.
fn remember(user: &mut crate::library::User, before: &BTreeMap<String, BTreeMap<String, String>>, gone: Vec<(String, String)>, fronts: Vec<(Job, PathBuf)>) -> BTreeSet<String> {
    // a copy sent away stands in for nothing now: what it copied shows again, unless it's what took
    // its place (its kind's folders hold files of one name, so it's known by its path)
    let mut tables = BTreeSet::new();
    for (k, path) in gone {
        if let Some(was) = before.get(&k).and_then(|c| c.get(&path))
            && let Some(copies) = user.in_popper.get_mut(&k)
        {
            copies.remove(was);
        }
        if let Some(c) = user.put_in.get_mut(&k) {
            c.remove(&path);
        }
        tables.insert(k);
    }
    // what went in front in Popper's folder is the choice from then on; one put only in PinballX's
    // or PinballY's, where Popper took none of its kind
    let popper_took: Vec<(String, String)> = fronts.iter().filter(|(j, _)| j.other.is_none()).map(|(j, _)| (j.table.clone(), j.kind.clone())).collect();
    for (j, front) in fronts {
        let name = front.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        let path = front.to_string_lossy().to_lowercase();
        // a front end's own in front is no copy of ours; VPS's or ours put in stands in for its source
        if j.pick.starts_with("file:") {
            if let Some(c) = user.put_in.get_mut(&j.table) {
                c.remove(&path);
            }
        } else {
            user.in_popper.entry(j.table.clone()).or_default().insert(j.pick.clone(), name.clone());
            user.put_in.entry(j.table.clone()).or_default().insert(path, j.pick.clone());
        }
        tables.insert(j.table.clone());
        if j.other.is_none() || !popper_took.contains(&(j.table.clone(), j.kind.clone())) {
            user.chosen.entry(j.table).or_default().insert(j.kind, format!("file:{name}"));
        }
    }
    user.in_popper.retain(|_, c| !c.is_empty());
    tables
}

/// A table's chosen of `kind` (or every kind) put in, if it's on — or `always`, on the user's word
/// for that kind — on a thread of its own; the library read again. After a choice, after making
/// media, after drawing a wheel.
pub fn after(app: &AppHandle, id: &str, kind: Option<&str>, always: bool) -> std::thread::JoinHandle<Done> {
    let jobs = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        match st.tables.iter().find(|t| t.id == id) {
            Some(t) if always || on(&st) || fronts_on(&st) => jobs(&st, t, kind, always && kind.is_some()),
            _ => Vec::new(),
        }
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let mut done = Done::default();
        if jobs.is_empty() {
            return done;
        }
        run(&app, jobs, &mut done);
        if let Err(e) = done.said() {
            eprintln!("PinUP Popper's media: {e}");
        }
        crate::library::start(app);
        done
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = r"c:\vpinball\visualpinball\tables\cirqus voltaire (bally 1997).vpx";
    const MEDIA: &str = r"c:\vpinball\pinupsystem\popmedia\visual pinball x";
    const NAME: &str = "cirqus voltaire (bally 1997).png";

    fn at(kind: &str) -> String {
        format!(r"{MEDIA}\{kind}\{NAME}")
    }

    fn job(kind: &str, pick: &str) -> Job {
        let folder = PathBuf::from(MEDIA).join(kind);
        Job { from: From::File(PathBuf::from("ours.png")), folder, name: "Cirqus Voltaire (Bally 1997)".into(), table: TABLE.into(), kind: kind.into(), pick: pick.into(), other: None }
    }

    /// Ours put in four of Popper's folders, every copy under the same name.
    fn copied_in() -> crate::library::User {
        let mut user = crate::library::User::default();
        for (folder, own) in [("playfield", "playfield"), ("backglass", "backglass"), ("dmd", "dmd"), ("wheel", "wheel")] {
            user.in_popper.entry(TABLE.into()).or_default().insert(format!("ours:{own}"), NAME.into());
            user.put_in.entry(TABLE.into()).or_default().insert(at(folder), format!("ours:{own}"));
        }
        user
    }

    #[test]
    fn a_copy_replaced_forgotten_by_its_path_the_other_kinds_kept() {
        // ours made again and put in front: the playfield's copy before it sent away
        let mut user = copied_in();
        let before = user.put_in.clone();
        let tables = remember(&mut user, &before, vec![(TABLE.into(), at("playfield"))], vec![(job("playfield", "ours:playfield"), PathBuf::from(at("playfield")))]);
        assert_eq!(tables.into_iter().collect::<Vec<_>>(), vec![TABLE.to_string()]);
        let copies = &user.in_popper[TABLE];
        assert_eq!(copies.len(), 4, "the backglass's, score display's and wheel's copies still stand in for ours: {copies:?}");
        assert_eq!(user.put_in[TABLE][&at("playfield")], "ours:playfield");
        assert_eq!(user.chosen[TABLE]["playfield"], format!("file:{NAME}"));

        // Popper's own numbered one chosen: the copy in front sent away, ours shown again beside it
        let mut user = copied_in();
        let before = user.put_in.clone();
        remember(&mut user, &before, vec![(TABLE.into(), at("playfield"))], vec![(job("playfield", "file:cirqus voltaire (bally 1997)01.png"), PathBuf::from(at("playfield")))]);
        let copies = &user.in_popper[TABLE];
        assert!(!copies.contains_key("ours:playfield"));
        assert_eq!(copies.len(), 3);
        assert!(!user.put_in[TABLE].contains_key(&at("playfield")));
        assert_eq!(user.put_in[TABLE].len(), 3);
    }
}
