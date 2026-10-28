//! The media a table uses put where PinUP Popper shows them, a setting off unless it's turned on
//! (`prefs.popperMedia`, Tools › Media): the one of each kind chosen on a table's page — VPS's
//! picture, one our player made, or one of Popper's own — put in front in Popper's folder for its
//! screen, named for Popper's own name for the game (`library::popper::put_in_front`: Popper's one
//! before it kept, numbered). And the same in PinballX's and PinballY's folders, where either plays
//! the library's tables (`prefs.frontsMedia`): named as each names the game's media, its own one
//! before kept numbered as it numbers them (`library::frontends::put_in_front`). When a choice
//! changes, for that kind; on a word from the settings, for every table.

use std::path::PathBuf;

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
/// the choice now Popper's copy in front, the source shown no more beside it.
pub fn run(app: &AppHandle, jobs: Vec<Job>, done: &mut Done) {
    let mut fronts: Vec<(Job, String)> = Vec::new();
    for j in jobs {
        let path = match &j.from {
            From::File(p) => Some(p.clone()),
            From::Vps(url) => crate::pictures::kept_vps(url),
        };
        let Some(path) = path else {
            *done.problems.entry("one of VPS’s pictures couldn’t be fetched".to_string()).or_default() += 1;
            continue;
        };
        let put = match j.other {
            None => ::library::popper::put_in_front(&j.folder, &j.name, &path),
            Some(_) => ::library::frontends::put_in_front(&j.folder, &j.name, &path),
        };
        match put {
            Ok(Some(front)) => {
                done.moved += 1;
                let name = front.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                fronts.push((j, name));
            }
            Ok(None) => {}
            Err(e) => *done.problems.entry(e).or_default() += 1,
        }
    }
    if fronts.is_empty() {
        return;
    }
    let shared = app.state::<Shared>();
    let mut st = shared.0.lock().unwrap();
    // what went in front in Popper's folder is the choice from then on; one put only in PinballX's
    // or PinballY's, where Popper took none of its kind
    let popper_took: Vec<(String, String)> = fronts.iter().filter(|(j, _)| j.other.is_none()).map(|(j, _)| (j.table.clone(), j.kind.clone())).collect();
    for (j, name) in fronts {
        if !j.pick.starts_with("file:") {
            st.user.in_popper.entry(j.table.clone()).or_default().insert(j.pick.clone(), name.clone());
        }
        if j.other.is_none() || !popper_took.contains(&(j.table.clone(), j.kind.clone())) {
            st.user.chosen.entry(j.table).or_default().insert(j.kind, format!("file:{name}"));
        }
    }
    if let Err(e) = st.user.save() {
        eprintln!("PinUP Popper's media: {e}");
    }
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
