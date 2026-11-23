//! A table's achievements followed while VPX plays it: the packs the app hands over (the file
//! `VPI_ACHIEVEMENTS` names: which packs' folders, hardcore or practice, the table, where what's
//! earned is kept, where to say how it went), each signal of the game moved through them, what's
//! earned recorded at once and announced. The signals: a ROM machine's game as its memory map reads
//! it (the game over or on, the ball, the player up, each player's score, the credits, extra
//! balls, a tilt — the `game_state` values VPX's PinMAME add-on shares once the map is in its
//! `memmaps` folder) and the machine's lamps, switches and solenoids it shares; a script table's
//! game as its script tells its backglass (B2S's calls, which VPX's B2S add-ons share). Each one
//! earned with a sound — its pack's own, else ours — and where the game stands said on asking (the
//! game paused). Nothing here knows VPX: `lib.rs` hands the values in, and shows and plays what
//! comes out.

use std::collections::HashMap;
use std::path::PathBuf;

use vpi_achievements::cond::{FROM_A_BACKGLASS, FROM_A_MACHINE, FROM_THE_BALLS, FROM_THE_LIGHTS, LightState, Signal};
use vpi_achievements::earned::Earned;
use vpi_achievements::cond::Measure;
use vpi_achievements::pack::Pack;
use vpi_achievements::watch::{How, Standing, Watcher};

/// A line of the achievements' page in VPX's own menu: a pack's name with how many of its
/// achievements are earned (where several packs are followed), or one achievement — its name, its
/// points and how it stands, then what to do — as the menu's markdown.
#[derive(Clone, Debug, PartialEq)]
pub enum PageLine {
    Heading(String),
    Achievement(String),
}

/// The page: a line summing it up for the page's foot, and its lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub summary: String,
    pub lines: Vec<PageLine>,
}

/// The page for where the achievements stand, `mode` the word for how this play counts them.
pub fn page_of(all: &[Standing], mode: &str) -> Page {
    let earned = |s: &&Standing| s.how == How::Earned;
    let points: u32 = all.iter().map(|s| s.points).sum();
    let points_earned: u32 = all.iter().filter(earned).map(|s| s.points).sum();
    let summary = format!("{mode} · {} of {} earned · {points_earned} of {points} points", all.iter().filter(earned).count(), all.len());
    let mut packs: Vec<&str> = Vec::new();
    for s in all {
        if !packs.contains(&s.pack.as_str()) {
            packs.push(&s.pack);
        }
    }
    let mut lines = Vec::new();
    for pack in &packs {
        let of_pack: Vec<&Standing> = all.iter().filter(|s| s.pack == *pack).collect();
        if packs.len() > 1 {
            lines.push(PageLine::Heading(format!("{} · {} of {} earned", plain(pack), of_pack.iter().filter(|s| s.how == How::Earned).count(), of_pack.len())));
        }
        lines.extend(of_pack.into_iter().map(|s| PageLine::Achievement(line_of(s))));
    }
    Page { summary, lines }
}

/// One achievement's lines: a secret one not yet earned unnamed.
fn line_of(s: &Standing) -> String {
    let points = if s.points == 1 { "1 point".to_string() } else { format!("{} points", s.points) };
    let hidden = s.secret && s.how != How::Earned;
    let (name, about) = if hidden { ("A secret achievement".to_string(), String::new()) } else { (plain(&s.name), plain(&s.about)) };
    let how = match &s.how {
        How::Earned => "Earned".to_string(),
        How::OnItsWay(Some((done, of, measure))) if *done > 0 && *of > 0 => match measure {
            Measure::Times => format!("{done} of {of}"),
            Measure::Score => format!("{} of {}", grouped(*done), grouped(*of)),
            Measure::Steps => format!("{done} of {of} in turn"),
        },
        How::OnItsWay(_) => "Not yet".to_string(),
        How::Unread => "Can't be earned here".to_string(),
    };
    if about.is_empty() { format!("**{name}** · {points} · {how}") } else { format!("**{name}** · {points} · {how}\n{about}") }
}

/// A pack's own words, without what the menu's markdown would take as formatting.
fn plain(text: &str) -> String {
    text.replace(['*', '_', '`', '#'], "").replace('\n', " ")
}

/// What the app hands over, as its file says it: `mode`, `hardcore` or `practice`; `why`, what makes
/// it practice, in words; `table`, the table file's name, kept with each record; `packs`, each
/// pack's folder; `earned`, where what's earned is kept; `trying`, a pack being made, tried by
/// playing; `result`, where to say how the following went, for the app once VPX has quit; `sound`,
/// whether a sound plays as each is earned (`on`) and how loud (`gain`, 1 as it comes).
#[derive(Clone, Debug)]
pub struct Asked {
    pub mode: String,
    pub why: Option<String>,
    pub table: Option<String>,
    pub packs: Vec<PathBuf>,
    pub earned: PathBuf,
    pub trying: bool,
    pub result: Option<PathBuf>,
    pub sound_on: bool,
    pub gain: f64,
    /// The card each one earned shows (`popup`: where and how big), where the app asks for one.
    pub popup: Option<crate::popup::Settings>,
    /// The settings files a hardcore game plays with (`hardcoreFiles`), watched for what makes a
    /// game softcore being changed during it (`hardcore::Watch`).
    pub hardcore_files: Vec<PathBuf>,
}

impl Default for Asked {
    fn default() -> Self {
        Asked { mode: String::new(), why: None, table: None, packs: Vec::new(), earned: PathBuf::new(), trying: false, result: None, sound_on: true, gain: 1.0, popup: None, hardcore_files: Vec::new() }
    }
}

impl Asked {
    pub fn read(bytes: &[u8]) -> Result<Asked, String> {
        let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let text = |k: &str| v.get(k).and_then(serde_json::Value::as_str).map(str::to_string);
        Ok(Asked {
            mode: text("mode").unwrap_or_else(|| "practice".into()),
            why: text("why"),
            table: text("table"),
            packs: v.get("packs").and_then(serde_json::Value::as_array).map(|a| a.iter().filter_map(|p| p.as_str().map(PathBuf::from)).collect()).unwrap_or_default(),
            earned: text("earned").map(PathBuf::from).ok_or("it names no file for what's earned")?,
            trying: v.get("trying").and_then(serde_json::Value::as_bool).unwrap_or(false),
            result: text("result").map(PathBuf::from),
            sound_on: v.pointer("/sound/on").and_then(serde_json::Value::as_bool).unwrap_or(true),
            gain: v.pointer("/sound/gain").and_then(serde_json::Value::as_f64).filter(|g| (0.0..=2.0).contains(g)).unwrap_or(1.0),
            popup: v.get("popup").and_then(crate::popup::Settings::from_json),
            hardcore_files: v.get("hardcoreFiles").and_then(serde_json::Value::as_array).map(|a| a.iter().filter_map(|p| p.as_str().map(PathBuf::from)).collect()).unwrap_or_default(),
        })
    }
}

/// A machine's game as its memory map reads it; None for what the map doesn't hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameState {
    pub game_over: Option<bool>,
    pub ball: Option<u32>,
    pub player: Option<u32>,
    pub players: Option<u32>,
    pub scores: Vec<u64>,
    pub credits: Option<u32>,
    pub extra_balls: Option<u32>,
    pub tilted: Option<bool>,
}

/// One of the map's game state values as PinMAME's add-on shares it: its key in the map's
/// `game_state` (`scores`, `current_ball`, …), its place among the values of that key (each
/// player's score), and its number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    pub key: String,
    pub index: usize,
    pub number: i64,
}

impl GameState {
    /// The state from the values read.
    pub fn from_values(values: &[Value]) -> GameState {
        let one = |k: &str| values.iter().find(|v| v.key == k).map(|v| v.number);
        let count = |k: &str| one(k).and_then(|n| u32::try_from(n).ok());
        let mut scores: Vec<(usize, u64)> = values.iter().filter(|v| v.key == "scores").filter_map(|v| Some((v.index, u64::try_from(v.number).ok()?))).collect();
        scores.sort_unstable();
        GameState {
            game_over: one("game_over").map(|n| n != 0),
            ball: count("current_ball"),
            player: count("current_player"),
            players: count("player_count"),
            scores: scores.into_iter().map(|(_, s)| s).collect(),
            credits: count("credits"),
            extra_balls: count("extra_balls"),
            tilted: one("tilted").map(|n| n != 0),
        }
    }

    /// The signals from `before` to this: a game starting says its first ball and player too,
    /// whatever its counters held before (the last game's ball can read 1 already).
    pub fn signals_from(&self, before: &GameState) -> Vec<Signal> {
        let mut out = Vec::new();
        let started = matches!((before.game_over, self.game_over), (Some(true), Some(false)));
        match (before.game_over, self.game_over) {
            (Some(true), Some(false)) => out.push(Signal::GameStarted),
            (Some(false), Some(true)) => out.push(Signal::GameOver),
            _ => {}
        }
        let playing = self.game_over != Some(true);
        if playing && let Some(b) = self.ball.filter(|&b| (started || Some(b) != before.ball) && b > 0) {
            out.push(Signal::Ball(b));
        }
        if playing && let Some(p) = self.player.filter(|&p| (started || Some(p) != before.player) && p > 0) {
            out.push(Signal::Player(p));
        }
        for (i, &s) in self.scores.iter().enumerate() {
            if before.scores.get(i) != Some(&s) && (s > 0 || before.scores.get(i).is_some_and(|&b| b > 0)) {
                out.push(Signal::Score { player: i as u32 + 1, score: s });
            }
        }
        if let Some(c) = self.credits.filter(|&c| Some(c) != before.credits) {
            out.push(Signal::Credits(c));
        }
        if let Some(e) = self.extra_balls.filter(|&e| Some(e) != before.extra_balls) {
            out.push(Signal::ExtraBalls(e));
        }
        if let (Some(was), Some(t)) = (before.tilted, self.tilted)
            && was != t
        {
            out.push(Signal::Tilt(t));
        }
        out
    }
}

/// A script table's game as its script tells its backglass, B2S's way: game over (B2S's 35: 1 over,
/// 0 on), the ball in play (32), the player up (30), credits (29), a tilt (33), each player's score
/// (`C`). `playing`: whether a game is on, as last told.
pub fn from_b2s(kind: char, number: i32, value: i32, playing: &mut Option<bool>) -> Option<Signal> {
    let count = |v: i32| u32::try_from(v).ok();
    match (kind, number) {
        ('E', 35) => {
            let on = value == 0;
            match (playing.replace(on), on) {
                (Some(false) | None, true) => Some(Signal::GameStarted),
                (Some(true), false) => Some(Signal::GameOver),
                _ => None,
            }
        }
        ('E', 32) => count(value).filter(|&b| b > 0).map(Signal::Ball),
        ('E', 30) => count(value).filter(|&p| p > 0).map(Signal::Player),
        ('B', 29) | ('E', 29) => count(value).map(Signal::Credits),
        ('E', 33) => Some(Signal::Tilt(value != 0)),
        ('C', player) => Some(Signal::Score { player: count(player)?, score: u64::try_from(value).ok()? }),
        _ => None,
    }
}


/// The sound an achievement earned plays: its pack's own (a file in the pack), else ours.
#[derive(Clone, Debug, PartialEq)]
pub enum Chime {
    Ours,
    File(PathBuf),
}

pub struct Following {
    watcher: Watcher,
    /// Each pack's folder, by the pack's ID: where its sounds are.
    dirs: HashMap<String, PathBuf>,
    /// The sound due, and how loud, where sounds are on.
    pub chime_due: Option<Chime>,
    pub sound_on: bool,
    pub gain: f64,
    earned: Earned,
    file: PathBuf,
    result: Option<PathBuf>,
    mode: String,
    trying: bool,
    /// The machine's game as last read, and when it was first read (the game's clock).
    game: Option<GameState>,
    first_read: Option<u64>,
    /// The machine's lamps, switches and solenoids as last seen.
    outputs: HashMap<(char, u32), bool>,
    /// The table's lights as last seen, by name in lower case.
    lights: HashMap<String, LightState>,
    /// The backglass's game: on or over as last told, and each value as last told.
    b2s_playing: Option<bool>,
    b2s_told: HashMap<(char, i32), i32>,
    /// Whether a machine's map or a backglass has been heard, and whether its tilt can be.
    pub machine: bool,
    pub backglass: bool,
    pub tilt: bool,
    /// Whether the balls in play have been heard (`balls`).
    pub balls: bool,
    /// What's been earned in this play, by name.
    pub earned_names: Vec<String>,
    /// Each signal of the game said, where a check asks (`telling`), taken with `take_told`.
    pub telling: bool,
    told: Vec<String>,
    /// A card for each notice of what's earned, the same moments, taken with `take_cards`.
    cards: Vec<crate::popup::Card>,
}

impl Following {
    /// The packs `asked` names taken up; with the card to show as play begins, where there's
    /// anything to earn.
    pub fn start(asked: Asked) -> (Following, Option<String>) {
        let read: Vec<(Pack, PathBuf)> = asked.packs.iter().filter_map(|dir| Pack::read(dir).ok().map(|p| (p, dir.clone()))).collect();
        let dirs: HashMap<String, PathBuf> = read.iter().map(|(p, d)| (p.pack.id.clone(), d.clone())).collect();
        let packs: Vec<Pack> = read.into_iter().map(|(p, _)| p).collect();
        let earned = Earned::load(&asked.earned);
        let watcher = Watcher::new(packs, &asked.mode, asked.table.clone(), &earned);
        let hardcore = asked.mode == "hardcore";
        let left = watcher.left();
        let card = (left > 0).then(|| {
            if asked.trying {
                format!("Achievements: trying the pack · {left} to earn, nothing kept")
            } else if hardcore {
                format!("Achievements: hardcore · {left} to earn")
            } else {
                format!("Achievements: softcore · {}", asked.why.as_deref().unwrap_or("kept for you, not counted for masteries"))
            }
        });
        let following = Following {
            watcher,
            dirs,
            chime_due: None,
            sound_on: asked.sound_on && asked.gain > 0.0,
            gain: asked.gain,
            earned,
            file: asked.earned,
            result: asked.result,
            mode: if hardcore { "hardcore" } else { "practice" }.into(),
            trying: asked.trying,
            game: None,
            first_read: None,
            outputs: HashMap::new(),
            lights: HashMap::new(),
            b2s_playing: None,
            b2s_told: HashMap::new(),
            machine: false,
            backglass: false,
            tilt: false,
            balls: false,
            earned_names: Vec::new(),
            telling: false,
            told: Vec::new(),
            cards: Vec::new(),
        };
        (following, card)
    }

    /// The machine's lamps, switches and solenoids the achievements still to earn count.
    pub fn outputs_wanted(&self) -> Vec<(char, u32)> {
        self.watcher.outputs()
    }

    /// The table's lights the achievements still to earn read, by name.
    pub fn lights_wanted(&self) -> Vec<String> {
        self.watcher.light_names()
    }

    /// One of the table's lights as VPX keeps its state now (0 off to 1 on, 2 blinking): a signal
    /// where it changed; its first sight is where it starts, which a light's state is read against.
    pub fn light(&mut self, name: &str, level: f64, now: u64) -> Vec<String> {
        let state = LightState::from_level(level);
        match self.lights.insert(name.to_lowercase(), state) {
            None => {
                self.watcher.light_seen(name, state);
                Vec::new()
            }
            Some(was) if was == state => Vec::new(),
            Some(_) => self.signal(Signal::Light { name: name.to_string(), state }, now),
        }
    }

    /// Whether the table's lights have been heard (`light`).
    pub fn lights_heard(&self) -> bool {
        !self.lights.is_empty()
    }

    /// Whether the achievements still to earn count the balls in play.
    pub fn wants_balls(&self) -> bool {
        self.watcher.needs().contains(&"ballsInPlay")
    }

    /// The balls in play at `now`, as counted (`balls::InPlay`): the notices to show.
    pub fn balls_in_play(&mut self, count: u32, now: u64) -> Vec<String> {
        self.balls = true;
        self.signal(Signal::BallsInPlay(count), now)
    }

    /// Whether anything's left to earn.
    pub fn is_empty(&self) -> bool {
        self.watcher.is_empty()
    }

    /// Play goes on softcore from now: something that makes a game softcore was changed while it
    /// played (`why`, in words). The notice to show; none where the game wasn't hardcore.
    pub fn turn_softcore(&mut self, why: &str) -> Option<String> {
        if self.mode != "hardcore" || self.trying {
            return None;
        }
        self.mode = "practice".into();
        self.watcher.set_practice();
        self.write_result(false);
        Some(format!("Achievements: softcore from now on · {why}"))
    }

    /// A signal at `now` (milliseconds of the game's clock): what's earned by it recorded at once,
    /// and the notices to show.
    pub fn signal(&mut self, signal: Signal, now: u64) -> Vec<String> {
        if self.telling {
            self.told.push(format!("{now} ms: {signal:?}"));
        }
        let earns = self.watcher.signal(signal, now, &mut self.earned);
        let moved = self.watcher.take_progress_moved();
        if earns.is_empty() && !moved {
            return Vec::new();
        }
        let _ = self.earned.save(&self.file);
        let mut notices = Vec::new();
        for e in earns {
            let a = &e.achievement;
            let points = if a.points == 1 { "1 point".to_string() } else { format!("{} points", a.points) };
            let practice = e.record.mode == "practice";
            let softcore = if practice { ", softcore" } else { "" };
            notices.push(format!("Achievement: {} · {points}{softcore}", a.name));
            let pack = self.dirs.get(&e.pack_id).and_then(|dir| Pack::read(dir).ok().map(|p| (p, dir.clone())));
            // its card: its picture (else the pack's), the line above saying what it's worth
            let picture = pack.as_ref().and_then(|(p, dir)| p.icon_of(dir, a));
            self.cards.push(crate::popup::Card { kicker: format!("Achievement earned · {points}{}", if practice { " · softcore" } else { "" }), name: a.name.clone(), picture });
            // its sound: the first earned at once is the one heard
            if self.sound_on && self.chime_due.is_none() {
                let own = pack.as_ref().and_then(|(p, dir)| p.sound_of(dir, a));
                self.chime_due = Some(own.map_or(Chime::Ours, Chime::File));
            }
            if let Some(m) = &e.mastered {
                notices.push(format!("Mastered: {} {}", e.pack_name, m.pack_version));
                let picture = pack.as_ref().and_then(|(p, dir)| p.pack.icon.as_ref().map(|i| dir.join(i)).filter(|f| f.is_file()));
                self.cards.push(crate::popup::Card { kicker: "Pack mastered".into(), name: format!("{} {}", e.pack_name, m.pack_version), picture });
            }
            self.earned_names.push(a.name.clone());
        }
        if !notices.is_empty() {
            self.write_result(false);
        }
        notices
    }

    fn signals(&mut self, signals: Vec<Signal>, now: u64) -> Vec<String> {
        signals.into_iter().flat_map(|s| self.signal(s, now)).collect()
    }

    /// The machine's game read again from its map's values: the signals of what changed.
    pub fn game_values(&mut self, values: &[Value], now: u64) -> Vec<String> {
        let state = GameState::from_values(values);
        if !self.machine {
            self.machine = true;
            self.tilt = state.tilted.is_some();
            self.write_result(false);
        }
        let first = *self.first_read.get_or_insert(now);
        // a machine starting up reads as it likes (a game over flag flipping as its memory is set
        // up): its first moments only set where the game stands, no game being started that soon
        let signals = match &self.game {
            Some(before) if now >= first + SETTLING_MS => state.signals_from(before),
            _ => Vec::new(),
        };
        self.game = Some(state);
        self.signals(signals, now)
    }

    /// One of the machine's lamps (`L`), switches (`W`) or solenoids (`S`) as it is now: a signal
    /// where it changed (the first sight of each is where it starts, not a change).
    pub fn output(&mut self, kind: char, number: u32, on: bool, now: u64) -> Vec<String> {
        let Some(was) = self.outputs.insert((kind, number), on) else { return Vec::new() };
        if was == on {
            return Vec::new();
        }
        let signal = match kind {
            'L' => Signal::Lamp { number, on },
            'S' => Signal::Solenoid { number, on },
            _ => Signal::Switch { number, on },
        };
        self.signal(signal, now)
    }

    /// One of the backglass's values as the script tells it: a signal where it says something new
    /// of the game (many scripts tell every score each frame).
    pub fn b2s(&mut self, kind: char, number: i32, value: i32, now: u64) -> Vec<String> {
        if self.b2s_told.insert((kind, number), value) == Some(value) {
            return Vec::new();
        }
        let Some(signal) = from_b2s(kind, number, value, &mut self.b2s_playing) else { return Vec::new() };
        if !self.backglass {
            self.backglass = true;
            self.write_result(false);
        }
        self.signal(signal, now)
    }

    /// Where the achievements stand, for a notice while the game's paused: how many are earned of
    /// how many, then those furthest on their way (a secret one unnamed), at most `most` of them.
    pub fn standing(&self, most: usize) -> String {
        let all = self.watcher.standing();
        let earned = all.iter().filter(|s| s.how == How::Earned).count();
        let mut on_the_way: Vec<(f64, String)> = all
            .iter()
            .filter_map(|s| match &s.how {
                How::OnItsWay(Some((done, of, measure))) if *done > 0 && *of > 0 => {
                    let name = if s.secret { "A secret achievement".to_string() } else { s.name.clone() };
                    let said = match measure {
                        Measure::Times => format!("{name}: {done} of {of}"),
                        Measure::Score => format!("{name}: {} of {}", grouped(*done), grouped(*of)),
                        Measure::Steps => format!("{name}: {done} of {of} in turn"),
                    };
                    Some((*done as f64 / *of as f64, said))
                }
                _ => None,
            })
            .collect();
        on_the_way.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mode = if self.trying { "trying the pack" } else if self.mode == "hardcore" { "hardcore" } else { "softcore" };
        let mut lines = vec![format!("Achievements: {earned} of {} earned · {mode}", all.len())];
        lines.extend(on_the_way.into_iter().take(most).map(|(_, s)| s));
        lines.join("\n")
    }

    /// The page of where the achievements stand, for VPX's own menu.
    pub fn page(&self) -> Page {
        let mode = if self.trying { "Trying the pack" } else if self.mode == "hardcore" { "Hardcore" } else { "Softcore" };
        page_of(&self.watcher.standing(), mode)
    }

    /// The signals said since this was last asked, where `telling`.
    pub fn take_told(&mut self) -> Vec<String> {
        std::mem::take(&mut self.told)
    }

    /// The cards of what's been earned since this was last asked, a notice's each.
    pub fn take_cards(&mut self) -> Vec<crate::popup::Card> {
        std::mem::take(&mut self.cards)
    }

    /// A card of a pack's first achievement, as if earned: for checking how cards show.
    pub fn sample_card(&self) -> Option<crate::popup::Card> {
        let mut dirs: Vec<&PathBuf> = self.dirs.values().collect();
        dirs.sort();
        let dir = dirs.first()?;
        let pack = Pack::read(dir).ok()?;
        let a = pack.achievements.first()?;
        let points = if a.points == 1 { "1 point".to_string() } else { format!("{} points", a.points) };
        Some(crate::popup::Card { kicker: format!("Achievement earned · {points}"), name: a.name.clone(), picture: pack.icon_of(dir, a) })
    }

    /// The kinds of signal heard so far.
    pub fn heard(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        if self.machine {
            out.extend(FROM_A_MACHINE);
            if self.tilt {
                out.push("tilt");
            }
        }
        if self.backglass {
            out.extend(FROM_A_BACKGLASS);
        }
        if self.balls {
            out.extend(FROM_THE_BALLS);
        }
        if self.lights_heard() {
            out.extend(FROM_THE_LIGHTS);
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Play over: what's earned saved, and how it went said for the app.
    pub fn finish(&mut self) {
        let _ = self.earned.save(&self.file);
        self.write_result(true);
    }

    /// How the following went, for the app once VPX has quit: whether the game was heard at all
    /// (if not, the app counts what it can from what the game left), what was, and what's been
    /// earned; written as soon as something's heard, so a VPX that stops on its way out still says.
    fn write_result(&self, over: bool) {
        let Some(file) = &self.result else { return };
        let v = serde_json::json!({
            "followed": self.machine || self.backglass,
            "machine": self.machine,
            "backglass": self.backglass,
            "heard": self.heard(),
            "earned": self.earned_names,
            "mode": self.mode,
            "over": over,
            "trying": self.trying,
        });
        if let Ok(text) = serde_json::to_vec_pretty(&v) {
            let new = file.with_extension("json.new");
            if std::fs::write(&new, text).is_ok() {
                let _ = std::fs::rename(&new, file);
            }
        }
    }
}

/// A number with its thousands apart: 250,000,000.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A pack's sound as a WAV: a WAV as it comes; an Ogg Vorbis, MP3 or FLAC decoded (Symphonia) into
/// 16-bit PCM at its own rate, in one or two channels (a sound of more channels keeps its first
/// two). None where it isn't a sound that decodes.
pub fn as_wav(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Some(bytes.to_vec());
    }
    let (rate, channels, samples) = decoded(bytes.to_vec())?;
    let kept = channels.min(2);
    let frames = samples.len() / channels;
    let mut data = Vec::with_capacity(frames * kept * 2);
    for frame in samples.chunks_exact(channels) {
        for s in &frame[..kept] {
            let v = (f64::from(*s).clamp(-1.0, 1.0) * f64::from(i16::MAX)).round() as i16;
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    let (kept, block) = (kept as u16, kept as u16 * 2);
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&kept.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    wav.extend_from_slice(&block.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    Some(wav)
}

/// A whole sound file decoded: its rate, its channels, and its samples interleaved, -1 to 1. A
/// damaged packet is passed over, as players do.
fn decoded(bytes: Vec<u8>) -> Option<(u32, usize, Vec<f32>)> {
    use symphonia::core::codecs::CodecParameters;
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::errors::Error;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
    use symphonia::core::meta::MetadataOptions;
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), MediaSourceStreamOptions::default());
    let mut format = symphonia::default::get_probe().probe(&Hint::new(), mss, FormatOptions::default(), MetadataOptions::default()).ok()?;
    let track = format.default_track(TrackType::Audio)?;
    let id = track.id;
    let Some(CodecParameters::Audio(params)) = track.codec_params.clone() else { return None };
    let mut decoder = symphonia::default::get_codecs().make_audio_decoder(&params, &AudioDecoderOptions::default()).ok()?;
    let (mut rate, mut channels, mut samples, mut chunk) = (0u32, 0usize, Vec::new(), Vec::<f32>::new());
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(_) => return None,
        };
        if packet.track_id != id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buffer) => {
                (rate, channels) = (buffer.spec().rate(), buffer.spec().channels().count());
                buffer.copy_to_vec_interleaved(&mut chunk);
                samples.extend_from_slice(&chunk);
            }
            Err(Error::DecodeError(_)) => continue,
            Err(_) => return None,
        }
    }
    (rate > 0 && channels > 0 && !samples.is_empty()).then_some((rate, channels, samples))
}

/// A WAV made louder or quieter: its 16-bit samples scaled by `gain`, clipped; any other kind as it
/// comes. None where it isn't a WAV.
pub fn scaled_wav(bytes: &[u8], gain: f64) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut out = bytes.to_vec();
    if (gain - 1.0).abs() < 1e-6 {
        return Some(out);
    }
    let (mut at, mut sixteen_bit_pcm) = (12usize, false);
    while at + 8 <= out.len() {
        let id = [out[at], out[at + 1], out[at + 2], out[at + 3]];
        let size = u32::from_le_bytes([out[at + 4], out[at + 5], out[at + 6], out[at + 7]]) as usize;
        let body = at + 8;
        let end = body.saturating_add(size).min(out.len());
        if &id == b"fmt " && end >= body + 16 {
            let format = u16::from_le_bytes([out[body], out[body + 1]]);
            let bits = u16::from_le_bytes([out[body + 14], out[body + 15]]);
            sixteen_bit_pcm = (format == 1 || format == 0xFFFE) && bits == 16;
        } else if &id == b"data" && sixteen_bit_pcm {
            let (samples, _) = out[body..end].as_chunks_mut::<2>();
            for s in samples {
                let v = f64::from(i16::from_le_bytes(*s)) * gain;
                let v = v.round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
                *s = v.to_le_bytes();
            }
        }
        at = body + size + (size & 1);
    }
    Some(out)
}

/// A WAV's samples as VPX's add-on sound takes them: mono or stereo, 16-bit or 32-bit float.
#[derive(Debug, PartialEq)]
pub struct Pcm<'a> {
    pub channels: u16,
    pub rate: u32,
    pub float: bool,
    pub samples: &'a [u8],
}

/// A WAV's samples, where they're a kind VPX takes from add-ons (PCM 16-bit, or 32-bit float, in
/// one or two channels); None otherwise, for Windows to play instead.
pub fn pcm_of(bytes: &[u8]) -> Option<Pcm<'_>> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let (mut at, mut format) = (12usize, None);
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]) as usize;
        let body = at + 8;
        let end = body.saturating_add(size).min(bytes.len());
        if id == b"fmt " && end >= body + 16 {
            let tag = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
            let channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
            let rate = u32::from_le_bytes([bytes[body + 4], bytes[body + 5], bytes[body + 6], bytes[body + 7]]);
            let bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
            // an extensible format's kind in its sub-format's first two bytes
            let kind = if tag == 0xFFFE && end >= body + 26 { u16::from_le_bytes([bytes[body + 24], bytes[body + 25]]) } else { tag };
            format = match (kind, bits) {
                (1, 16) => Some((channels, rate, false)),
                (3, 32) => Some((channels, rate, true)),
                _ => return None,
            };
        } else if id == b"data" {
            let (channels, rate, float) = format?;
            if !(1..=2).contains(&channels) || !(8000..=192_000).contains(&rate) {
                return None;
            }
            let frame = usize::from(channels) * if float { 4 } else { 2 };
            let len = (end - body) / frame * frame;
            return (len > 0).then(|| Pcm { channels, rate, float, samples: &bytes[body..body + len] });
        }
        at = body + size + (size & 1);
    }
    None
}

/// How long a machine's game is read before what changes is told: its start-up.
pub const SETTLING_MS: u64 = 3000;

/// A map's game state key from the group PinMAME's add-on gives one of its values: `game_state\
/// <key>\<label>` (or without the label where the map gives none); None for a value outside the
/// game state (its audits, adjustments, high scores).
pub fn game_state_key(group: &str) -> Option<&str> {
    let mut parts = group.split('\\');
    if parts.next()? != "game_state" {
        return None;
    }
    parts.next().filter(|k| !k.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(dir: &std::path::Path) -> PathBuf {
        let text = r#"{ "format": "vpi-achievements", "formatVersion": 1,
          "pack": { "id": "4f0e8c2a-1d3b-4e5f-8a9b-0c1d2e3f4a5b", "name": "Test", "version": "1.0" },
          "for": { "game": { "name": "Test" } },
          "achievements": [
            { "id": "start", "name": "Off We Go", "about": "a", "points": 1, "since": "1.0", "when": { "game": "started" } },
            { "id": "million", "name": "A Million", "about": "b", "points": 5, "since": "1.0", "when": { "score": { "atLeast": 1000000 } } },
            { "id": "lock", "name": "Locked", "about": "c", "points": 5, "since": "1.0", "when": { "count": { "solenoid": 7 }, "times": 3, "within": "game" } },
            { "id": "pops", "name": "Pops", "about": "d", "points": 5, "since": "1.0", "when": { "count": { "part": "Bumper1", "does": "hit" }, "times": 3, "within": "game" } }
          ] }"#;
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("achievements.json"), text).unwrap();
        dir.to_path_buf()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vpi-addon-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn values(game_over: i64, ball: i64, player: i64, scores: &[i64]) -> Vec<Value> {
        let mut out = vec![
            Value { key: "game_over".into(), index: 0, number: game_over },
            Value { key: "current_ball".into(), index: 0, number: ball },
            Value { key: "current_player".into(), index: 0, number: player },
        ];
        out.extend(scores.iter().enumerate().map(|(i, &s)| Value { key: "scores".into(), index: i, number: s }));
        out
    }

    #[test]
    fn a_machines_game_and_its_solenoids_earn_as_it_plays() {
        let dir = scratch("machine");
        let asked = Asked { mode: "hardcore".into(), packs: vec![pack(&dir.join("pack"))], earned: dir.join("earned.json"), result: Some(dir.join("result.json")), ..Asked::default() };
        let (mut f, card) = Following::start(asked);
        assert_eq!(card.as_deref(), Some("Achievements: hardcore · 4 to earn"));
        assert_eq!(f.outputs_wanted(), [('S', 7)]);
        // starting up: its memory read as a game on, then over, tells nothing
        assert!(f.game_values(&values(0, 0, 0, &[0]), 0).is_empty());
        assert!(f.game_values(&values(1, 1, 1, &[750_000]), 1600).is_empty());
        // attract mode, then a game begun
        assert!(f.game_values(&values(1, 1, 1, &[750_000]), 3000).is_empty());
        assert_eq!(f.game_values(&values(0, 1, 1, &[0]), 3100), ["Achievement: Off We Go · 1 point"]);
        // the lock's solenoid, seen first off, fired three times
        assert!(f.output('S', 7, false, 3150).is_empty());
        let mut said = Vec::new();
        for t in 0..3 {
            said.extend(f.output('S', 7, true, 3200 + t * 100));
            said.extend(f.output('S', 7, false, 3250 + t * 100));
        }
        assert_eq!(said, ["Achievement: Locked · 5 points"]);
        assert_eq!(f.game_values(&values(0, 1, 1, &[1_250_000]), 3900), ["Achievement: A Million · 5 points"]);
        f.finish();
        let result: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("result.json")).unwrap()).unwrap();
        assert_eq!(result["followed"], true);
        assert_eq!(result["earned"], serde_json::json!(["Off We Go", "Locked", "A Million"]));
        assert_eq!(result["over"], true);
        // what's earned kept: a second play has only the bumpers' left
        let again = Asked { mode: "hardcore".into(), packs: vec![dir.join("pack")], earned: dir.join("earned.json"), ..Asked::default() };
        assert_eq!(Following::start(again).1.as_deref(), Some("Achievements: hardcore · 1 to earn"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_script_tables_backglass_calls_are_its_game() {
        let dir = scratch("backglass");
        let asked = Asked { mode: "practice".into(), packs: vec![pack(&dir.join("pack"))], earned: dir.join("earned.json"), ..Asked::default() };
        let (mut f, card) = Following::start(asked);
        assert_eq!(card.as_deref(), Some("Achievements: softcore · kept for you, not counted for masteries"));
        assert_eq!(f.b2s('E', 35, 0, 10), ["Achievement: Off We Go · 1 point, softcore"]);
        // its card, the same moment: no picture in this pack
        assert_eq!(f.take_cards(), [crate::popup::Card { kicker: "Achievement earned · 1 point · softcore".into(), name: "Off We Go".into(), picture: None }]);
        // told again each frame: nothing new
        assert!(f.b2s('C', 1, 500_000, 20).is_empty());
        assert!(f.b2s('C', 1, 500_000, 30).is_empty());
        assert!(f.take_cards().is_empty());
        assert_eq!(f.b2s('C', 1, 1_000_000, 40), ["Achievement: A Million · 5 points, softcore"]);
        assert_eq!(f.take_cards().len(), 1);
        assert_eq!(f.heard(), ["ball", "credits", "game", "score", "tilt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_achievement_earned_brings_its_packs_sound_else_ours_and_the_standing_is_said() {
        let dir = scratch("chime");
        let pack_dir = pack(&dir.join("pack"));
        let asked = Asked { mode: "hardcore".into(), packs: vec![pack_dir.clone()], earned: dir.join("earned.json"), ..Asked::default() };
        let (mut f, _) = Following::start(asked);
        f.b2s('E', 35, 0, 10);
        assert_eq!(f.chime_due.take(), Some(Chime::Ours));
        f.b2s('C', 1, 400_000, 20);
        assert_eq!(f.standing(3), "Achievements: 1 of 4 earned · hardcore\nA Million: 400,000 of 1,000,000");
        // a pack with its own sound: that one
        let text = std::fs::read_to_string(pack_dir.join("achievements.json")).unwrap().replace("\"name\": \"Test\", \"version\": \"1.0\"", "\"name\": \"Test\", \"version\": \"1.0\", \"sound\": \"ding.wav\", \"icon\": \"badge.png\"");
        std::fs::write(pack_dir.join("achievements.json"), text).unwrap();
        std::fs::write(pack_dir.join("ding.wav"), b"RIFF").unwrap();
        std::fs::write(pack_dir.join("badge.png"), b"PNG").unwrap();
        let again = Asked { mode: "hardcore".into(), packs: vec![pack_dir.clone()], earned: dir.join("other.json"), sound_on: true, ..Asked::default() };
        let (mut g, _) = Following::start(again);
        g.b2s('E', 35, 0, 10);
        assert_eq!(g.chime_due, Some(Chime::File(pack_dir.join("ding.wav"))));
        // and its picture, the pack's, on the card
        assert_eq!(g.take_cards()[0].picture, Some(pack_dir.join("badge.png")));
        // sounds off: none
        let quiet = Asked { mode: "hardcore".into(), packs: vec![pack_dir], earned: dir.join("third.json"), sound_on: false, ..Asked::default() };
        let (mut h, _) = Following::start(quiet);
        h.b2s('E', 35, 0, 10);
        assert_eq!(h.chime_due, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wavs_samples_scaled_and_clipped() {
        let mut wav = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes()); // PCM
        wav.extend(1u16.to_le_bytes()); // one channel
        wav.extend(44_100u32.to_le_bytes());
        wav.extend(88_200u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(4u32.to_le_bytes());
        wav.extend(1000i16.to_le_bytes());
        wav.extend(30_000i16.to_le_bytes());
        let louder = scaled_wav(&wav, 2.0).unwrap();
        let n = louder.len();
        assert_eq!(i16::from_le_bytes([louder[n - 4], louder[n - 3]]), 2000);
        assert_eq!(i16::from_le_bytes([louder[n - 2], louder[n - 1]]), i16::MAX);
        assert_eq!(scaled_wav(b"OggS....", 1.0), None);
        // its samples as VPX takes them from add-ons
        assert_eq!(pcm_of(&louder), Some(Pcm { channels: 1, rate: 44_100, float: false, samples: &louder[n - 4..] }));
        // 8-bit: Windows plays it instead
        let mut eight = wav.clone();
        eight[34] = 8;
        assert_eq!(pcm_of(&eight), None);
        assert_eq!(pcm_of(b"OggS...."), None);
        // a WAV is a WAV as it comes; what isn't a sound, nothing
        assert_eq!(as_wav(&wav), Some(wav.clone()));
        assert_eq!(as_wav(b"OggS.... not a sound"), None);
    }

    /// An Ogg Vorbis sound decoded into a WAV VPX takes: one of VPX's own, from the newer build's
    /// example table in the repository's `vpx-build`, where it's there.
    #[test]
    fn an_ogg_sound_decoded_into_a_wav() {
        let ogg = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vpx-build/vpx-10.8.1-6758/assets/blankTable.vpz/sounds/drain.ogg");
        let Ok(bytes) = std::fs::read(&ogg) else {
            eprintln!("skipped: {} isn't there", ogg.display());
            return;
        };
        let wav = as_wav(&bytes).expect("decoded");
        let pcm = pcm_of(&wav).expect("as VPX takes it");
        assert!((1..=2).contains(&pcm.channels) && (8000..=192_000).contains(&pcm.rate) && pcm.samples.len() > 1000 && !pcm.float);
        assert!(scaled_wav(&wav, 0.5).is_some());
    }

    #[test]
    fn our_chime_is_a_kind_vpx_takes() {
        let ours = include_bytes!("../../player/assets/vpi/sounds/achievement.wav");
        let pcm = pcm_of(ours).expect("our chime as samples");
        assert!(pcm.samples.len() > 1000 && (1..=2).contains(&pcm.channels));
    }

    #[test]
    fn the_menus_page_says_where_each_achievement_stands() {
        let s = |pack: &str, name: &str, points: u32, secret: bool, how: How| Standing { pack: pack.into(), name: name.into(), about: format!("Do the *{name}* thing"), points, secret, how };
        let all = vec![
            s("Medieval Madness", "Arise", 5, false, How::Earned),
            s("Medieval Madness", "Load the Catapult", 30, false, How::OnItsWay(Some((2, 3, Measure::Times)))),
            s("Medieval Madness", "Hidden", 50, true, How::OnItsWay(None)),
            s("Castle", "King of the Realm", 1, false, How::OnItsWay(Some((25_000_000, 100_000_000, Measure::Score)))),
        ];
        let page = page_of(&all, "Hardcore");
        assert_eq!(page.summary, "Hardcore · 1 of 4 earned · 5 of 86 points");
        assert_eq!(
            page.lines,
            vec![
                PageLine::Heading("Medieval Madness · 1 of 3 earned".into()),
                PageLine::Achievement("**Arise** · 5 points · Earned\nDo the Arise thing".into()),
                PageLine::Achievement("**Load the Catapult** · 30 points · 2 of 3\nDo the Load the Catapult thing".into()),
                PageLine::Achievement("**A secret achievement** · 50 points · Not yet".into()),
                PageLine::Heading("Castle · 0 of 1 earned".into()),
                PageLine::Achievement("**King of the Realm** · 1 point · 25,000,000 of 100,000,000\nDo the King of the Realm thing".into()),
            ]
        );
        // one pack: no heading
        assert!(!page_of(&all[..2], "Softcore").lines.iter().any(|l| matches!(l, PageLine::Heading(_))));
    }

    #[test]
    fn a_maps_groups_name_their_game_state_keys() {
        assert_eq!(game_state_key("game_state\\scores\\Player 1"), Some("scores"));
        assert_eq!(game_state_key("game_state\\current_ball"), Some("current_ball"));
        assert_eq!(game_state_key("audits\\Standard Audits\\Total Plays"), None);
        assert_eq!(game_state_key("game_state"), None);
    }

    #[test]
    fn a_tables_lights_earn_as_it_plays() {
        let dir = scratch("lights");
        let packs = dir.join("pack");
        std::fs::create_dir_all(&packs).unwrap();
        let text = r#"{ "format": "vpi-achievements", "formatVersion": 1,
          "pack": { "id": "5a1e9d3b-2c4f-4a6e-9b8c-1d2e3f4a5b6c", "name": "Lights", "version": "1.0" },
          "for": { "game": { "name": "Test" } },
          "achievements": [
            { "id": "rainbow", "name": "Rainbow", "about": "a", "points": 10, "since": "1.0", "when": { "all": [{ "light": "rain1", "is": "on" }, { "light": "rain2", "is": "on" }] } },
            { "id": "again", "name": "Again", "about": "b", "points": 5, "since": "1.0", "when": { "light": "LightShootAgain", "turns": "blinking" } }
          ] }"#;
        std::fs::write(packs.join("achievements.json"), text).unwrap();
        let asked = Asked { mode: "hardcore".into(), packs: vec![packs], earned: dir.join("earned.json"), ..Asked::default() };
        let (mut f, _) = Following::start(asked);
        assert_eq!(f.lights_wanted(), ["LightShootAgain", "rain1", "rain2"]);
        assert!(!f.lights_heard());
        // first seen as the table starts: where each starts, nothing said
        assert!(f.light("rain1", 1.0, 0).is_empty());
        assert!(f.light("rain2", 0.0, 0).is_empty());
        assert!(f.light("LightShootAgain", 0.0, 0).is_empty());
        assert!(f.heard().contains(&"light"));
        // a game begun as the script tells its backglass, then the lights as VPX keeps them
        assert!(f.b2s('E', 35, 0, 100).is_empty());
        assert!(f.light("rain2", 0.3, 200).is_empty(), "dimmed below half is still off");
        assert_eq!(f.light("rain2", 1.0, 300), ["Achievement: Rainbow · 10 points"]);
        assert!(f.light("LightShootAgain", 1.0, 400).is_empty());
        assert_eq!(f.light("LightShootAgain", 2.0, 500), ["Achievement: Again · 5 points", "Mastered: Lights 1.0"]);
    }
}
