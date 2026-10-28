//! A ROM machine's game as it goes — over or on, the ball in play, the player up, each player's
//! score, the credits, extra balls, a tilt — read from its memory while it plays through the Pinball
//! Memory Maps' `game_state` (`pinmame-nvram`, MIT; the maps ODbL, as the high scores are read): the
//! first of the signals achievements are to be read from. A thread of its own asks PinMAME for the
//! game's memory twice a second, as the high scores' watcher does, and tells what changed; the game
//! thread never waits on it. It runs while a table's achievements are followed, each change a signal
//! to them, and where `VPI_GAME_STATE` is set, printing each change (the headless runner's check of a
//! machine's map). Ours.

use std::collections::HashMap;
use std::sync::Weak;
use std::time::{Duration, Instant};

use pinmame_link::Link;
use serde_json::Value;

/// How often the machine's memory is read.
const EVERY: Duration = Duration::from_millis(500);

/// The values of a map's game state read here (`GameState::from_map`).
const READ: [&str; 8] = ["game_over", "current_ball", "current_player", "player_count", "scores", "credits", "extra_balls", "tilted"];

/// How a machine's game is read from its map, as the map means it: the map reader reads a game's
/// values whole and as stored, so the map is cut to the values read here — one kept in a way the
/// reader can't read (a System 6 machine's game over as a list of names, Doctor Who's raw bytes)
/// stops its whole reading — a one-byte value sharing its byte with others (a Bally's game over
/// beside its tilt, a Data East's beside its players) is read here with its mask, and each value
/// read takes the number its map adds (players counted from 0 in the memory) and loses the values
/// its map says mean none.
#[derive(Default)]
struct Reading {
    /// Whether the map holds a game state at all.
    any: bool,
    /// The values read here with their masks: each name and its map's description.
    masked: Vec<(String, Value)>,
    /// Where the map's addresses start in the memory read, and which half of each byte holds a
    /// value where the memory keeps one in each byte's half.
    base: u64,
    nibble: Option<String>,
    /// Each value's number added and values meaning none.
    adjust: HashMap<String, (i64, Vec<i64>)>,
}

/// A number in a map: written out, or in hex as `0x20C`.
fn number(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => match s.trim().strip_prefix("0x").or_else(|| s.trim().strip_prefix("0X")) {
            Some(hex) => u64::from_str_radix(hex, 16).ok(),
            None => s.trim().parse().ok(),
        },
        _ => None,
    }
}

impl Reading {
    /// The reading of `nv`'s game, its map cut to what's read here.
    fn of(nv: &mut pinmame_nvram::Nvram) -> Reading {
        let platform = serde_json::to_value(&nv.platform).unwrap_or_default();
        let layout = platform
            .get("memory_layout")
            .and_then(Value::as_array)
            .and_then(|l| l.iter().find(|m| m.get("type").and_then(Value::as_str) == Some("nvram")))
            .cloned()
            .unwrap_or_default();
        let mut r = Reading {
            any: nv.map.game_state.is_some(),
            masked: Vec::new(),
            base: layout.get("address").and_then(number).unwrap_or(0),
            nibble: layout.get("nibble").and_then(Value::as_str).filter(|n| *n != "both").map(str::to_string),
            adjust: HashMap::new(),
        };
        if let Some(state) = nv.map.game_state.as_mut() {
            state.retain(|key, v| {
                if !READ.contains(&key.as_str()) {
                    return false;
                }
                let Ok(d) = serde_json::to_value(&*v) else { return false };
                let each: Vec<&Value> = match &d {
                    Value::Array(list) => list.iter().collect(),
                    one => vec![one],
                };
                let encoding = |e: &Value| e.get("encoding").and_then(Value::as_str).map(str::to_string);
                if !each.iter().all(|e| matches!(encoding(e).as_deref(), Some("int" | "bcd" | "bool" | "ch"))) {
                    return false;
                }
                let [one] = each.as_slice() else { return true };
                let add = one.get("offset").and_then(Value::as_i64).unwrap_or(0);
                let none: Vec<i64> = one
                    .get("special_values")
                    .and_then(Value::as_object)
                    .map(|m| m.iter().filter(|(_, v)| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case("n/a"))).filter_map(|(k, _)| k.parse().ok()).collect())
                    .unwrap_or_default();
                if add != 0 || !none.is_empty() {
                    r.adjust.insert(key.clone(), (add, none));
                }
                if one.get("mask").is_none() {
                    return true;
                }
                // read here, never whole: one byte at its address, as a yes or no or a number
                let one_byte = one.get("start").is_some() && one.get("offsets").is_none() && one.get("length").and_then(Value::as_u64).unwrap_or(1) == 1;
                if one_byte && matches!(encoding(one).as_deref(), Some("int" | "bcd" | "bool")) {
                    r.masked.push((key.clone(), (*one).clone()));
                }
                false
            });
        }
        r
    }

    /// The game's values now, from its memory just read (`bytes`, which `nv`'s file holds): None
    /// where the map holds no game state or its reading fails.
    fn values(&self, nv: &mut pinmame_nvram::Nvram, bytes: &[u8]) -> Option<HashMap<String, String>> {
        if !self.any {
            return None;
        }
        let mut values = nv.read_game_state().ok().flatten()?;
        for (key, d) in &self.masked {
            if let Some(v) = self.masked_value(bytes, d) {
                values.insert(key.clone(), v);
            }
        }
        self.adjusted(&mut values);
        Some(values)
    }

    /// Each value read with the number its map adds, or gone where its map says it means none.
    fn adjusted(&self, values: &mut HashMap<String, String>) {
        for (key, (add, none)) in &self.adjust {
            let Some(n) = values.get(key).and_then(|v| v.trim().parse::<i64>().ok()) else { continue };
            let n = n + add;
            if none.contains(&n) {
                values.remove(key);
            } else {
                values.insert(key.clone(), n.to_string());
            }
        }
    }

    /// A one-byte value read with its mask, as the map reader reads a byte: its half where the
    /// memory keeps a value in each half (the value's own half where its map names one), then the
    /// mask's bits — a yes or no (`invert`ed where the map says), a number, or a BCD digit pair.
    fn masked_value(&self, bytes: &[u8], d: &Value) -> Option<String> {
        let at = usize::try_from(d.get("start").and_then(number)?.checked_sub(self.base)?).ok()?;
        let byte = *bytes.get(at)?;
        let byte = match d.get("nibble").and_then(Value::as_str).or(self.nibble.as_deref()) {
            Some("high") => byte >> 4,
            Some("low") => byte & 0x0F,
            _ => byte,
        };
        let mask = u8::try_from(d.get("mask").and_then(number)?).ok().filter(|&m| m != 0)?;
        let v = (byte & mask) >> mask.trailing_zeros();
        Some(match d.get("encoding").and_then(Value::as_str)? {
            "bool" => ((v != 0) != d.get("invert").and_then(Value::as_bool).unwrap_or(false)).to_string(),
            "bcd" => ((v >> 4) * 10 + (v & 0x0F)).to_string(),
            _ => v.to_string(),
        })
    }
}

/// A game's state as its machine's map reads it; None for what the map doesn't hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameState {
    pub game_over: Option<bool>,
    pub ball: Option<u32>,
    pub player: Option<u32>,
    pub players: Option<u32>,
    pub scores: Vec<u64>,
    pub credits: Option<u32>,
    pub extra_balls: Option<u32>,
    /// The player up has tilted their ball.
    pub tilted: Option<bool>,
}

/// What changed from one reading to the next, as achievements will hear it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    GameStarted,
    GameOver,
    /// A ball begun, by its number.
    Ball(u32),
    /// The player up.
    Player(u32),
    /// A player's score, players counted from 1.
    Score { player: u32, score: u64 },
    Credits(u32),
    ExtraBalls(u32),
    /// Tilted, or the tilt over: a machine's from its memory where its map tells it, a script
    /// table's as it tells its backglass.
    Tilt(bool),
    /// The balls on the table now, the physics' count: two or more a multiball.
    BallsInPlay(u32),
}

impl GameState {
    /// The state from the maps' readings (`read_game_state`: its names, each value as text).
    pub fn from_map(values: &HashMap<String, String>) -> GameState {
        let int = |k: &str| values.get(k).and_then(|v| v.trim().parse::<u32>().ok());
        let yes = |k: &str| values.get(k).map(|v| v.trim().eq_ignore_ascii_case("true"));
        let mut scores: Vec<(usize, u64)> = values
            .iter()
            .filter_map(|(k, v)| Some((k.strip_prefix("scores.")?.parse::<usize>().ok()?, v.trim().parse::<u64>().ok()?)))
            .collect();
        scores.sort();
        GameState {
            game_over: yes("game_over"),
            ball: int("current_ball"),
            player: int("current_player"),
            players: int("player_count"),
            scores: scores.into_iter().map(|(_, s)| s).collect(),
            credits: int("credits"),
            extra_balls: int("extra_balls"),
            tilted: yes("tilted"),
        }
    }

    /// What changed from `before` to this.
    pub fn changes_from(&self, before: &GameState) -> Vec<Change> {
        let mut out = Vec::new();
        // a game starting says its first ball and player too, whatever its counters held before
        // (the last game's ball can read 1 already)
        let started = matches!((before.game_over, self.game_over), (Some(true), Some(false)));
        match (before.game_over, self.game_over) {
            (Some(true), Some(false)) => out.push(Change::GameStarted),
            (Some(false), Some(true)) => out.push(Change::GameOver),
            _ => {}
        }
        let playing = self.game_over != Some(true);
        if playing && let Some(b) = self.ball.filter(|&b| (started || Some(b) != before.ball) && b > 0) {
            out.push(Change::Ball(b));
        }
        if playing && let Some(p) = self.player.filter(|&p| (started || Some(p) != before.player) && p > 0) {
            out.push(Change::Player(p));
        }
        for (i, &s) in self.scores.iter().enumerate() {
            if before.scores.get(i) != Some(&s) && (s > 0 || before.scores.get(i).is_some_and(|&b| b > 0)) {
                out.push(Change::Score { player: i as u32 + 1, score: s });
            }
        }
        if let Some(c) = self.credits.filter(|&c| Some(c) != before.credits) {
            out.push(Change::Credits(c));
        }
        if let Some(e) = self.extra_balls.filter(|&e| Some(e) != before.extra_balls) {
            out.push(Change::ExtraBalls(e));
        }
        if let (Some(was), Some(t)) = (before.tilted, self.tilted)
            && was != t
        {
            out.push(Change::Tilt(t));
        }
        out
    }
}

/// A script table's game as its script tells its backglass, B2S's way (`B2SSetGameOver` and the
/// rest, as the event stream carries them): game over (B2S's 35: 1 over, 0 on), the ball in play
/// (32), the player up (30), credits (29), each player's score (`C`); None for what says nothing
/// of the game.
pub fn from_b2s(kind: char, number: i32, value: i32, playing: &mut Option<bool>) -> Option<Change> {
    let count = |v: i32| u32::try_from(v).ok();
    match (kind, number) {
        // `playing`: whether a game is on, as last told
        ('E', 35) => {
            let on = value == 0;
            match (playing.replace(on), on) {
                (Some(false) | None, true) => Some(Change::GameStarted),
                (Some(true), false) => Some(Change::GameOver),
                _ => None,
            }
        }
        ('E', 32) => count(value).filter(|&b| b > 0).map(Change::Ball),
        ('E', 30) => count(value).filter(|&p| p > 0).map(Change::Player),
        ('B', 29) => count(value).map(Change::Credits),
        ('E', 33) => Some(Change::Tilt(value != 0)),
        ('C', player) => Some(Change::Score { player: count(player)?, score: u64::try_from(value).ok()? }),
        _ => None,
    }
}

/// How long a ball still counts as in play while it sits still: a ball cradled on a flipper, but
/// not one parked in a lock or a trough.
const STILL_IN_PLAY_MS: u64 = 10_000;

/// How far a ball travels from where it last rested before it counts as in play: a trough's balls
/// shuffling along by a ball's width never do, a ball plunged or ejected into play does.
const AWAY_FROM_REST: f32 = 150.0;

/// A ball as `in_play` follows it.
struct Followed {
    /// Where it last rested: held in a kicker, or still.
    rest: (f32, f32),
    /// When it last moved.
    moved: u64,
    /// In play: it's travelled away from where it rested since, and isn't parked.
    active: bool,
}

/// The balls in play among `balls` (each its ID, whether it's moving now, whether a kicker holds
/// it, its x and y) at `now` on the game's clock: those that travelled well away from where they
/// last rested, until a kicker holds them or they've sat still ten seconds — the physics' count
/// holds the balls resting in a trough too.
pub fn in_play(balls: impl Iterator<Item = (u32, bool, bool, f32, f32)>, now: u64) -> usize {
    static FOLLOWED: std::sync::Mutex<Option<HashMap<u32, Followed>>> = std::sync::Mutex::new(None);
    let mut followed = FOLLOWED.lock().unwrap_or_else(|e| e.into_inner());
    let followed = followed.get_or_insert_with(HashMap::new);
    let mut seen = Vec::new();
    let mut count = 0;
    for (id, moving, held, x, y) in balls {
        seen.push(id);
        let f = followed.entry(id).or_insert(Followed { rest: (x, y), moved: now, active: false });
        if moving {
            f.moved = now;
        }
        let parked = held || now.saturating_sub(f.moved) > STILL_IN_PLAY_MS;
        if parked || (!moving && !f.active) {
            // resting: where it rests now is where it's measured from
            f.rest = (x, y);
            f.active = false;
        } else if !f.active && ((x - f.rest.0).powi(2) + (y - f.rest.1).powi(2)).sqrt() > AWAY_FROM_REST {
            f.active = true;
        }
        count += usize::from(f.active);
    }
    followed.retain(|id, _| seen.contains(id));
    count
}

/// A change as achievements hear it.
pub fn signal_of(change: &Change) -> vpi_achievements::cond::Signal {
    use vpi_achievements::cond::Signal;
    match change {
        Change::GameStarted => Signal::GameStarted,
        Change::GameOver => Signal::GameOver,
        Change::Ball(n) => Signal::Ball(*n),
        Change::Player(p) => Signal::Player(*p),
        Change::Score { player, score } => Signal::Score { player: *player, score: *score },
        Change::Credits(c) => Signal::Credits(*c),
        Change::ExtraBalls(n) => Signal::ExtraBalls(*n),
        Change::Tilt(on) => Signal::Tilt(*on),
        Change::BallsInPlay(n) => Signal::BallsInPlay(*n),
    }
}

/// A change told: to achievements, and printed where a check asks.
fn tell(change: Change, said: impl FnOnce(&Change)) {
    if asked() {
        said(&change);
    }
    crate::achievements::signal(signal_of(&change));
}

/// The balls on the table, each frame: a change told, as achievements hear a multiball start and end.
pub fn balls_in_play(count: usize) {
    static LAST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);
    if wanted() && LAST.swap(count, std::sync::atomic::Ordering::Relaxed) != count {
        tell(Change::BallsInPlay(count as u32), |c| println!("game state: {c:?}"));
    }
}

/// Whether the game's signals are wanted: for a table's achievements, or a check's.
pub fn wanted() -> bool {
    asked() || crate::achievements::active()
}

/// Whether a check asks for the game's signals (`VPI_GAME_STATE`).
pub fn asked() -> bool {
    static ASKED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ASKED.get_or_init(|| std::env::var_os("VPI_GAME_STATE").is_some())
}

/// Each part's events by its name and the event, counted where a check asks: what achievements
/// can name (`gate:LeftRampExit`, a spinner's spins), listed when play ends.
static PARTS: std::sync::Mutex<std::collections::BTreeMap<(String, &'static str), u32>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// A part's event, by the part's name in the table: a ball through a gate or over a trigger, a
/// spinner's spin, a slingshot's kick.
pub fn on_part(name: &str, event: &'static str) {
    if asked() {
        *PARTS.lock().unwrap_or_else(|e| e.into_inner()).entry((name.to_string(), event)).or_insert(0) += 1;
    }
    crate::achievements::signal(vpi_achievements::cond::Signal::Part { name: name.to_string(), does: event.to_lowercase() });
}

/// The parts' events counted, most first, printed where a check asks.
pub fn print_parts() {
    if !asked() {
        return;
    }
    let parts = PARTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<(&(String, &str), &u32)> = parts.iter().collect();
    list.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let shown: Vec<String> = list.iter().take(24).map(|((name, event), n)| format!("{name} {event} x{n}")).collect();
    println!("game state: parts: {} kinds of event; {}", list.len(), shown.join(", "));
}

/// A script table's game told as its script tells the backglass: to achievements, and printed
/// where `VPI_GAME_STATE` asks (the headless runner's check).
pub fn on_b2s(kind: char, number: i32, value: i32) {
    static TOLD: std::sync::Mutex<(Option<bool>, Option<HashMap<(char, i32), i32>>)> = std::sync::Mutex::new((None, None));
    if !wanted() {
        return;
    }
    let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
    // a script telling the same again (many set every score each frame) tells nothing new
    if told.1.get_or_insert_with(HashMap::new).insert((kind, number), value) == Some(value) {
        return;
    }
    if let Some(change) = from_b2s(kind, number, value, &mut told.0) {
        drop(told);
        tell(change, |c| println!("game state: from its script {c:?}"));
    }
}

/// Watches the game `rom` runs over `link` while it lives, for achievements or where
/// `VPI_GAME_STATE` asks, telling each change (printed with its time since the machine started).
pub fn watch_if_asked(link: Weak<Link>, rom: String) {
    if !wanted() {
        return;
    }
    let spawned = std::thread::Builder::new().name("vpi-game-state".into()).spawn(move || {
        let folder = std::env::temp_dir().join(format!("vpi-game-state-{}", std::process::id()));
        if std::fs::create_dir_all(&folder).is_err() {
            return;
        }
        // named as PinMAME names its sets, whatever case the script wrote: the map is found by it
        let file = folder.join(format!("{}.nv", rom.to_ascii_lowercase()));
        let started = Instant::now();
        let mut before: Option<GameState> = None;
        let mut nv: Option<pinmame_nvram::Nvram> = None;
        let mut reading = Reading::default();
        loop {
            std::thread::sleep(EVERY);
            let Some(link) = link.upgrade() else { break };
            let Some(bytes) = memory(&link) else { continue };
            if std::fs::write(&file, &bytes).is_err() {
                break;
            }
            // the map found once, by the game's name; read again each time from the file written
            if nv.is_none() {
                match pinmame_nvram::Nvram::open(&file) {
                    Ok(Some(mut n)) => {
                        reading = Reading::of(&mut n);
                        nv = Some(n);
                    }
                    _ => {
                        if asked() {
                            println!("game state: no memory map for {rom}");
                        }
                        break;
                    }
                }
            }
            let Some(values) = nv.as_mut().and_then(|n| reading.values(n, &bytes)) else {
                if asked() {
                    println!("game state: {rom}'s map holds no game state");
                }
                break;
            };
            let now = GameState::from_map(&values);
            match &before {
                None if asked() => println!("game state: {rom} read from its memory: {now:?}"),
                None => {}
                Some(b) => {
                    for change in now.changes_from(b) {
                        let at = started.elapsed().as_secs_f64();
                        tell(change, |c| println!("game state: {at:.1} s {c:?}"));
                    }
                }
            }
            before = Some(now);
        }
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&folder);
    });
    if let Err(e) = spawned {
        eprintln!("vpi-player: the game's state isn't watched: {e}");
    }
}

/// The game's memory now, as PinMAME writes its `.nv` file; None when it doesn't answer in a second.
fn memory(link: &Link) -> Option<Vec<u8>> {
    let shared = link.shared();
    let request = shared.request_nvram();
    let asked = Instant::now();
    loop {
        if let Some(answer) = shared.nvram_answer_since(request) {
            return answer;
        }
        if asked.elapsed() > Duration::from_secs(1) {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(pairs: &[(&str, &str)]) -> GameState {
        GameState::from_map(&pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    #[test]
    fn a_game_starting_its_balls_and_scores_told() {
        let attract = state(&[("game_over", "true"), ("current_ball", "1"), ("current_player", "1"), ("scores.0", "1250000"), ("credits", "2")]);
        let started = state(&[("game_over", "false"), ("current_ball", "1"), ("current_player", "1"), ("scores.0", "0"), ("credits", "1")]);
        // its first ball and player said, though the counters held them already
        assert_eq!(started.changes_from(&attract), [Change::GameStarted, Change::Ball(1), Change::Player(1), Change::Score { player: 1, score: 0 }, Change::Credits(1)]);
        let scored = state(&[("game_over", "false"), ("current_ball", "2"), ("current_player", "1"), ("scores.0", "52340"), ("credits", "1")]);
        assert_eq!(scored.changes_from(&started), [Change::Ball(2), Change::Score { player: 1, score: 52340 }]);
        let over = state(&[("game_over", "true"), ("current_ball", "2"), ("current_player", "1"), ("scores.0", "52340"), ("credits", "1")]);
        assert_eq!(over.changes_from(&scored), [Change::GameOver]);
    }

    /// A map's reading for `rom`, from a file of its memory all at zero but `set` (file offset, byte).
    fn read_as(rom: &str, set: &[(usize, u8)]) -> (Reading, Option<HashMap<String, String>>) {
        let folder = std::env::temp_dir().join(format!("vpi-reading-{rom}-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join(format!("{rom}.nv"));
        let mut bytes = vec![0u8; 0x10000];
        for &(at, b) in set {
            bytes[at] = b;
        }
        std::fs::write(&file, &bytes).unwrap();
        let mut nv = pinmame_nvram::Nvram::open(&file).unwrap().unwrap();
        let reading = Reading::of(&mut nv);
        let values = reading.values(&mut nv, &bytes);
        let _ = std::fs::remove_dir_all(&folder);
        (reading, values)
    }

    #[test]
    fn a_map_read_as_it_means_it() {
        // Doctor Who's raw bytes and a System 6 machine's game over as names, which the map reader
        // stops on, left out
        assert!(read_as("dw_l2", &[]).1.is_some());
        let (r, values) = read_as("algar_l1", &[]);
        assert!(values.is_some_and(|v| !v.contains_key("game_over")));
        assert!(r.masked.iter().all(|(k, _)| k != "game_over"));
        // a Bally's game over and tilt in one half byte, each with its mask, inverted: the game on
        // (its bit set) and tilted (its bit clear)
        let (r, values) = read_as("centaur", &[(0x0C, 0x40)]);
        assert_eq!(r.nibble.as_deref(), Some("high"));
        let values = values.unwrap();
        assert_eq!((values.get("game_over").map(String::as_str), values.get("tilted").map(String::as_str)), (Some("false"), Some("true")));
        // a System 11 machine's players counted from 0 in its memory: player 1 is 1
        let (r, values) = read_as("bcats_l5", &[]);
        assert_eq!(r.adjust.get("current_player").map(|a| a.0), Some(1));
        assert_eq!(values.unwrap().get("current_player").map(String::as_str), Some("1"));
    }

    #[test]
    fn a_masked_byte_read_as_its_map_says() {
        let r = Reading { any: true, base: 0x200, nibble: Some("high".into()), ..Default::default() };
        let d = |v: Value| v;
        let bytes = [0u8, 0xC0];
        // the half byte's top two bits set: 0x4 inverted reads false, 0x8 and 0xC as a number
        assert_eq!(r.masked_value(&bytes, &d(serde_json::json!({"start": "0x201", "encoding": "bool", "mask": "0x4", "invert": true}))).as_deref(), Some("false"));
        assert_eq!(r.masked_value(&bytes, &d(serde_json::json!({"start": "0x201", "encoding": "int", "mask": "0xC"}))).as_deref(), Some("3"));
        let flat = Reading { any: true, ..Default::default() };
        // a Data East's game over (its top bit) beside its players (the rest)
        let bytes = [0x82u8];
        assert_eq!(flat.masked_value(&bytes, &d(serde_json::json!({"start": 0, "encoding": "bool", "mask": "0x80"}))).as_deref(), Some("true"));
        assert_eq!(flat.masked_value(&bytes, &d(serde_json::json!({"start": 0, "encoding": "int", "mask": "0x7F"}))).as_deref(), Some("2"));
        // outside the memory read: nothing
        assert!(flat.masked_value(&bytes, &d(serde_json::json!({"start": 9, "encoding": "int", "mask": "0x7F"}))).is_none());
    }

    #[test]
    fn a_machine_tilting_told_from_its_memory() {
        let playing = state(&[("game_over", "false"), ("current_ball", "2"), ("tilted", "false")]);
        let tilted = state(&[("game_over", "false"), ("current_ball", "2"), ("tilted", "true")]);
        let next = state(&[("game_over", "false"), ("current_ball", "3"), ("tilted", "false")]);
        assert_eq!(tilted.changes_from(&playing), [Change::Tilt(true)]);
        assert_eq!(next.changes_from(&tilted), [Change::Ball(3), Change::Tilt(false)]);
        // a map without it tells none
        let plain = state(&[("game_over", "false"), ("current_ball", "2")]);
        assert!(plain.changes_from(&plain).is_empty());
    }

    #[test]
    fn a_script_tables_game_told_as_it_tells_its_backglass() {
        let mut playing = None;
        // game over shown in attract mode: nothing new; a game started, its ball, its score; over
        let told: Vec<Option<Change>> = [('E', 35, 1), ('E', 35, 0), ('E', 32, 1), ('C', 1, 5000), ('E', 30, 2), ('E', 35, 1), ('E', 99, 1)]
            .into_iter()
            .map(|(k, n, v)| from_b2s(k, n, v, &mut playing))
            .collect();
        assert_eq!(
            told,
            [None, Some(Change::GameStarted), Some(Change::Ball(1)), Some(Change::Score { player: 1, score: 5000 }), Some(Change::Player(2)), Some(Change::GameOver), None]
        );
    }
}
