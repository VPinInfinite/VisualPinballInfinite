//! Achievements while a table plays: the packs the app hands the player for it — `VPI_ACHIEVEMENTS`
//! names a file saying which packs' folders, hardcore or practice, the table, and where what's
//! earned is kept — followed through the game's signals as they come from any thread (the
//! machine's game from its memory, a script table's from its backglass calls, the balls in play,
//! the table's parts by their names, the machine's lamps, switches and solenoids from the event
//! stream). Each achievement earned is recorded at once and announced, one after another, a few
//! seconds each (`notice`), with a sound as each shows — its pack's own, else ours
//! (`take_sound`); and as play begins, whether it plays hardcore or softcore. A count over every
//! game is saved as it moves. Ours.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use vpi_achievements::cond::{Measure, Signal};
use vpi_achievements::earned::Earned;
use vpi_achievements::pack::Pack;
use vpi_achievements::watch::{How, Watcher};

use crate::pause_menu::ControlLine;

/// What the app hands over: `mode`, `hardcore` or `practice` (softcore, as the player says it);
/// `why`, what makes it softcore, in words; `table`, the table file's name, kept with each record;
/// `packs`, each pack's folder (its `achievements.json` in it); `earned`, where what's earned is
/// kept (`Achievements\earned.json` in the data folder); `trying`, a pack being made, tried by
/// playing, what's earned kept in a passing file.
struct Asked {
    mode: String,
    why: Option<String>,
    table: Option<String>,
    packs: Vec<PathBuf>,
    earned: PathBuf,
    trying: bool,
}

impl Asked {
    fn read(bytes: &[u8]) -> Result<Asked, String> {
        let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let text = |k: &str| v.get(k).and_then(serde_json::Value::as_str).map(str::to_string);
        Ok(Asked {
            mode: text("mode").unwrap_or_else(|| "practice".into()),
            why: text("why"),
            table: text("table"),
            packs: v.get("packs").and_then(serde_json::Value::as_array).map(|a| a.iter().filter_map(|p| p.as_str().map(PathBuf::from)).collect()).unwrap_or_default(),
            earned: text("earned").map(PathBuf::from).ok_or("it names no file for what's earned")?,
            trying: v.get("trying").and_then(serde_json::Value::as_bool).unwrap_or(false),
        })
    }
}

struct Following {
    watcher: Watcher,
    earned: Earned,
    file: PathBuf,
    started: Instant,
    /// The machine's lamps, solenoids and switches as last seen, for their changes.
    outputs: HashMap<(char, i32), i32>,
    /// `hardcore` or `practice`, and whether it's a pack being tried.
    mode: String,
    trying: bool,
    /// Each pack's folder, by the pack's ID: where its own sound is.
    dirs: HashMap<String, PathBuf>,
}

/// The sound an announcement plays as it shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Chime {
    None,
    /// The player's own (`assets/vpi/sounds/achievement.wav`).
    Ours,
    /// A pack's own, a file in it.
    File(PathBuf),
}

static FOLLOWING: Mutex<Option<Following>> = Mutex::new(None);
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// What's to be announced, each with its sound, and when the one showing began.
static ANNOUNCED: Mutex<(VecDeque<(String, Chime)>, Option<Instant>)> = Mutex::new((VecDeque::new(), None));
/// The sound of the announcement that just began showing, for the game thread to play.
static CHIME_DUE: Mutex<Option<Chime>> = Mutex::new(None);
const SHOWN_FOR: Duration = Duration::from_secs(5);
/// Our chime, built in.
const OUR_CHIME: &[u8] = include_bytes!("../assets/vpi/sounds/achievement.wav");

/// The packs `VPI_ACHIEVEMENTS` names taken up, once, as play begins.
pub fn start() {
    let Some(file) = std::env::var_os("VPI_ACHIEVEMENTS").filter(|f| !f.is_empty()) else { return };
    let asked = match std::fs::read(&file).map_err(|e| e.to_string()).and_then(|b| Asked::read(&b)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("achievements: {}: {e}", PathBuf::from(&file).display());
            return;
        }
    };
    let packs: Vec<Pack> = asked
        .packs
        .iter()
        .filter_map(|dir| match Pack::read(dir) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("achievements: {e}");
                None
            }
        })
        .collect();
    let dirs: HashMap<String, PathBuf> = packs.iter().zip(&asked.packs).map(|(p, d)| (p.pack.id.clone(), d.clone())).collect();
    let earned = Earned::load(&asked.earned);
    let watcher = Watcher::new(packs, &asked.mode, asked.table, &earned);
    let hardcore = asked.mode == "hardcore";
    println!("achievements: {} to earn, {}{}", watcher.left(), if hardcore { "hardcore" } else { "softcore" }, if asked.trying { ", a pack tried" } else { "" });
    // as play begins, how it counts: its card, without a sound
    let left = watcher.left();
    if left > 0 {
        let card = if asked.trying {
            format!("Achievements: Trying the pack · {left} to earn, nothing kept")
        } else if hardcore {
            format!("Achievements: Hardcore · {left} to earn")
        } else {
            format!("Achievements: Softcore · {}", asked.why.as_deref().unwrap_or("kept for you, not counted for masteries"))
        };
        ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner()).0.push_back((card, Chime::None));
    }
    let mode = if hardcore { "hardcore" } else { "practice" }.to_string();
    *FOLLOWING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Following { watcher, earned, file: asked.earned, started: Instant::now(), outputs: HashMap::new(), mode, trying: asked.trying, dirs });
    ACTIVE.store(true, Ordering::Release);
}

/// Something that changes how the table plays was changed while playing (one of its options, in
/// the pause menu): the rest of this play counts as softcore, and a card says so.
pub fn to_softcore(why: &str) {
    {
        let mut guard = FOLLOWING.lock().unwrap_or_else(|e| e.into_inner());
        let Some(f) = guard.as_mut() else { return };
        if f.mode != "hardcore" || f.trying {
            return;
        }
        f.mode = "practice".into();
        f.watcher.set_practice();
    }
    println!("achievements: softcore from here: {why}");
    ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner()).0.push_back((format!("Achievements: Softcore · {why}"), Chime::None));
}

/// The sound of the announcement that just began showing, once: the game thread plays it.
pub fn take_sound() -> Option<Chime> {
    CHIME_DUE.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// Our chime as a file the sound player can read (written into the temp folder once).
pub fn our_chime() -> Option<PathBuf> {
    let file = std::env::temp_dir().join("vpi-achievement-chime.wav");
    let same = std::fs::metadata(&file).is_ok_and(|m| m.len() == OUR_CHIME.len() as u64);
    if !same {
        std::fs::write(&file, OUR_CHIME).ok()?;
    }
    Some(file)
}

/// How many of the table's achievements are earned, of how many, for the pause menu's row.
pub fn tally() -> Option<(usize, usize)> {
    let guard = FOLLOWING.lock().unwrap_or_else(|e| e.into_inner());
    let standing = guard.as_ref()?.watcher.standing();
    Some((standing.iter().filter(|s| s.how == How::Earned).count(), standing.len()))
}

/// The pause menu's Achievements page: a line for each achievement under its pack's name —
/// earned, how far it's come where it counts, else not yet; a secret one unnamed, and how far it's
/// come unsaid, until it's earned — and how what's earned is kept.
pub fn page() -> (Vec<ControlLine>, String) {
    let guard = FOLLOWING.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map(page_of).unwrap_or_default()
}

fn page_of(f: &Following) -> (Vec<ControlLine>, String) {
    let mut lines = Vec::new();
    let mut pack: Option<String> = None;
    for s in f.watcher.standing() {
        let section = (pack.as_deref() != Some(s.pack.as_str())).then(|| s.pack.clone());
        pack = Some(s.pack.clone());
        let hidden = s.secret && s.how != How::Earned;
        let label = if hidden { "A secret achievement".to_string() } else { s.name };
        let keys = match s.how {
            How::Earned => "Earned".to_string(),
            How::OnItsWay(_) if hidden => "Not yet".to_string(),
            How::OnItsWay(Some((done, of, Measure::Times))) => format!("{done} of {of}"),
            How::OnItsWay(Some((done, of, Measure::Score))) => format!("{} of {}", grouped(done), grouped(of)),
            How::OnItsWay(Some((done, of, Measure::Steps))) => format!("{done} of {of} in turn"),
            How::OnItsWay(None) => "Not yet".to_string(),
            How::Unread => "Can\u{2019}t be read".to_string(),
        };
        lines.push(ControlLine { section, label, keys });
    }
    let kept = if f.trying {
        "Trying the pack: nothing is kept"
    } else if f.mode == "hardcore" {
        "Hardcore: the table plays as its author made it"
    } else {
        "Softcore: kept for you, never counted for masteries"
    };
    (lines, kept.to_string())
}

/// A number with its thousands apart: 250,000,000.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The parts the table's achievements count, by name: watched where the table has their hits off
/// (`physics_compat::env::PhysicsEnv::watching`).
pub fn watched_parts() -> Vec<String> {
    FOLLOWING.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|f| f.watcher.part_names()).unwrap_or_default()
}

/// Whether the table plays hardcore whatever its own settings say (`VPI_HARDCORE=1`, the app's
/// Play Hardcore): its options and difficulty in its settings file, a script or physics file left
/// beside it, VPX's physics sets as the settings change them and PinMAME's cheat mode left out,
/// so it plays as its author made it.
pub fn hardcore_play() -> bool {
    std::env::var_os("VPI_HARDCORE").is_some_and(|v| v == "1")
}

/// Whether a table's achievements are being followed.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// Whether an achievement's sound plays as it's announced (`[VPI]` `AchievementSound`).
static SOUND_ON: AtomicBool = AtomicBool::new(true);

pub fn set_sound(on: bool) {
    SOUND_ON.store(on, Ordering::Release);
}

pub fn sound_on() -> bool {
    SOUND_ON.load(Ordering::Acquire)
}

/// A signal from the game, from any thread.
pub fn signal(signal: Signal) {
    if !active() {
        return;
    }
    let mut guard = FOLLOWING.lock().unwrap_or_else(|e| e.into_inner());
    let Some(f) = guard.as_mut() else { return };
    let now = f.started.elapsed().as_millis() as u64;
    let earns = f.watcher.signal(signal, now, &mut f.earned);
    let moved = f.watcher.take_progress_moved();
    if earns.is_empty() && !moved {
        return;
    }
    if let Err(e) = f.earned.save(&f.file) {
        eprintln!("achievements: {e}");
    }
    let mut shown = ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner());
    for e in earns {
        let a = &e.achievement;
        let points = if a.points == 1 { "1 point".to_string() } else { format!("{} points", a.points) };
        let mode = if e.record.mode == "practice" { "softcore" } else { "hardcore" };
        println!("achievement: {} — {} ({points}, {mode})", e.pack_name, a.name);
        let softcore = if mode == "softcore" { ", softcore" } else { "" };
        let chime = f.dirs.get(&e.pack_id).and_then(|dir| pack_sound(dir, a)).map_or(Chime::Ours, Chime::File);
        shown.0.push_back((format!("Achievement: {} · {points}{softcore}", a.name), chime.clone()));
        if let Some(m) = &e.mastered {
            println!("achievement: {} {} mastered", e.pack_name, m.pack_version);
            shown.0.push_back((format!("Mastered: {}", e.pack_name), chime));
        }
    }
}

/// An achievement's sound, else its pack's, as a file in the pack's folder.
fn pack_sound(dir: &std::path::Path, a: &vpi_achievements::pack::Achievement) -> Option<PathBuf> {
    let pack = Pack::read(dir).ok()?;
    pack.sound_of(dir, a)
}

/// The machine's lamps, solenoids and the switches the script set, as the event stream polls them:
/// each change a signal.
pub fn machine_states(states: &[(char, i32, i32)]) {
    if !active() {
        return;
    }
    let changed: Vec<Signal> = {
        let mut guard = FOLLOWING.lock().unwrap_or_else(|e| e.into_inner());
        let Some(f) = guard.as_mut() else { return };
        states
            .iter()
            .filter(|(k, ..)| matches!(k, 'L' | 'S' | 'W'))
            .filter_map(|&(k, n, v)| {
                let was = f.outputs.insert((k, n), v);
                let on = v != 0;
                // the first sight of each is where it starts, not a change
                let before = was?;
                ((before != 0) != on).then(|| {
                    let number = n.max(0) as u32;
                    match k {
                        'L' => Signal::Lamp { number, on },
                        'S' => Signal::Solenoid { number, on },
                        _ => Signal::Switch { number, on },
                    }
                })
            })
            .collect()
    };
    for s in changed {
        signal(s);
    }
}

/// The announcement to show now, if any (the window thread): each in turn, a few seconds each,
/// its sound due as it begins.
pub fn notice() -> Option<String> {
    let mut shown = ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner());
    let (queue, since) = &mut *shown;
    match since {
        Some(t) if t.elapsed() < SHOWN_FOR => return queue.front().map(|(words, _)| words.clone()),
        Some(_) => {
            queue.pop_front();
            *since = None;
        }
        None => {}
    }
    let (words, chime) = queue.front()?.clone();
    *since = Some(Instant::now());
    if chime != Chime::None {
        *CHIME_DUE.lock().unwrap_or_else(|e| e.into_inner()) = Some(chime);
    }
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menus_page_says_how_far_each_has_come_and_keeps_a_secret() {
        let pack = Pack::parse(
            r#"{ "format": "vpi-achievements", "formatVersion": 1,
              "pack": { "id": "8d6c2f0e-4c9a-4b8e-9f51-3a7d2b1c9e40", "name": "Attack from Mars", "version": "1.1" },
              "for": { "game": { "name": "Attack from Mars" } },
              "achievements": [
                { "id": "pops", "name": "Pop Goes the Martian", "about": "a", "points": 10, "since": "1.1", "when": { "count": { "part": "Bumper1|Bumper2", "does": "hit" }, "times": 25, "within": "ball" } },
                { "id": "big", "name": "Martian Billionaire", "about": "b", "points": 20, "since": "1.0", "when": { "score": { "atLeast": 1000000000 } } },
                { "id": "conquered", "name": "Earth Conquered", "about": "c", "points": 30, "since": "1.0", "secret": true, "when": { "score": { "atLeast": 3000000000 } } },
                { "id": "start", "name": "The Invasion Begins", "about": "d", "points": 1, "since": "1.0", "when": { "game": "started" } }
              ] }"#,
        )
        .unwrap();
        let mut earned = Earned::default();
        let mut watcher = Watcher::new(vec![pack], "practice", None, &earned);
        watcher.signal(Signal::GameStarted, 0, &mut earned);
        watcher.signal(Signal::Score { player: 1, score: 250_000_000 }, 5, &mut earned);
        for at in 0..12 {
            watcher.signal(Signal::Part { name: "Bumper1".into(), does: "hit".into() }, 10 + at, &mut earned);
        }
        let f = Following { watcher, earned, file: PathBuf::new(), started: Instant::now(), outputs: HashMap::new(), mode: "practice".into(), trying: true, dirs: HashMap::new() };
        let (lines, kept) = page_of(&f);
        let said: Vec<(Option<&str>, &str, &str)> = lines.iter().map(|l| (l.section.as_deref(), l.label.as_str(), l.keys.as_str())).collect();
        assert_eq!(
            said,
            [
                (Some("Attack from Mars"), "Pop Goes the Martian", "12 of 25"),
                (None, "Martian Billionaire", "250,000,000 of 1,000,000,000"),
                (None, "A secret achievement", "Not yet"),
                (None, "The Invasion Begins", "Earned"),
            ]
        );
        assert_eq!(kept, "Trying the pack: nothing is kept");
    }

    #[test]
    fn a_score_said_with_its_thousands_apart() {
        assert_eq!([0, 999, 1_000, 25_000, 250_000_000, 1_000_000_000].map(grouped), ["0", "999", "1,000", "25,000", "250,000,000", "1,000,000,000"]);
    }
}
