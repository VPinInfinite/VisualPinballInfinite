//! When an achievement is earned: a condition over what the game does, the format's first draft,
//! modelled on RetroAchievements' condition engine in purpose (counts, windows, sequences) and
//! written over named signals rather than memory addresses. A condition is either a state, true
//! while it lasts (a score reached, balls in play), or an event, true at the moment it happens (a
//! part hit, a lamp turning on, a ball begun); `all` is met when all of its are at once — or, given a
//! `within`, when each has been in any order inside it — `any` when one is, `then` when each has been
//! in turn (inside its `within` where it has one, and with `strict`, anything of another step out of
//! turn starting it over), `count` when its own has been met so many times within a ball, a game,
//! some seconds or every game played (its count kept between plays); and any of them with `unless`
//! starts over whenever that is met, as RetroAchievements' ResetIf. Ours.

use std::collections::VecDeque;

use serde_json::Value;

/// What the game does, as the player tells it.
#[derive(Clone, Debug, PartialEq)]
pub enum Signal {
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
    Tilt(bool),
    /// The balls on the table now.
    BallsInPlay(u32),
    /// A part of the table, by its name: a hit, a spin, a kick, a ball entering or leaving.
    Part { name: String, does: String },
    Lamp { number: u32, on: bool },
    Switch { number: u32, on: bool },
    Solenoid { number: u32, on: bool },
    /// A table written for achievements telling its own (`Achievement.Event "name"`).
    Script(String),
}

/// How long a count runs before it starts again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Window {
    Ball,
    /// One ball, by its number: counted on that ball alone.
    OnBall(u32),
    Game,
    Seconds(f64),
    /// Every game played, from one play to the next: the count is kept with what's earned.
    Games,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cond {
    /// All at once; or, with a window, each at some moment inside it, in any order.
    All { of: Vec<Cond>, within: Option<Window> },
    Any(Vec<Cond>),
    /// Each in turn, the whole inside its window where it has one; `strict`: anything another step
    /// counts, out of turn, starts it over.
    Then { of: Vec<Cond>, within: Option<Window>, strict: bool },
    Count { of: Box<Cond>, times: u32, within: Window },
    /// Its condition, started over whenever `reset` is met.
    Unless { cond: Box<Cond>, reset: Box<Cond> },
    Score(u64),
    BallsInPlay(u32),
    Credits(u32),
    GameStarted,
    GameOver,
    Ball(u32),
    Tilt,
    ExtraBall,
    /// A part's event, by any of its names (`|` between them), `any` for every event.
    Part { names: Vec<String>, does: String },
    Lamp { number: u32, on: bool },
    Switch { number: u32, on: bool },
    Solenoid { number: u32, on: bool },
    Script(String),
}

fn at_least(v: &Value, what: &str) -> Result<u64, String> {
    v.get("atLeast").and_then(Value::as_u64).ok_or_else(|| format!("{what} needs \"atLeast\" with a whole number"))
}

fn number(v: &Value, what: &str) -> Result<u32, String> {
    v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| format!("{what} needs a number"))
}

fn turns(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_str).is_none_or(|s| !s.eq_ignore_ascii_case("off"))
}

/// A window as the format writes it: `"ball"`, `{ "ball": n }`, `"game"`, `"games"` (where
/// `games` is allowed) or `{ "seconds": n }`.
fn window(w: &Value, games: bool) -> Result<Window, String> {
    if let Some(n) = w.get("ball") {
        return match n.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n >= 1) {
            Some(n) => Ok(Window::OnBall(n)),
            None => Err("\"within\": { \"ball\": n } needs a ball's number, 1 or more".into()),
        };
    }
    match w {
        Value::String(s) if s == "ball" => Ok(Window::Ball),
        Value::String(s) if s == "game" => Ok(Window::Game),
        Value::String(s) if s == "games" && games => Ok(Window::Games),
        w => match w.get("seconds").and_then(Value::as_f64) {
            Some(s) if s > 0.0 => Ok(Window::Seconds(s)),
            _ if games => Err("\"within\" is \"ball\", { \"ball\": n }, \"game\", \"games\" or { \"seconds\": n }".into()),
            _ => Err("a group's \"within\" is \"ball\", { \"ball\": n }, \"game\" or { \"seconds\": n }".into()),
        },
    }
}

impl Cond {
    /// A condition from the pack's JSON.
    pub fn parse(v: &Value) -> Result<Cond, String> {
        let obj = v.as_object().ok_or("a condition is an object")?;
        // what starts it over, beside it
        if let Some(reset) = obj.get("unless") {
            let mut rest = obj.clone();
            rest.remove("unless");
            return Ok(Cond::Unless { cond: Box::new(Cond::parse(&Value::Object(rest))?), reset: Box::new(Cond::parse(reset)?) });
        }
        let list = |key: &str| -> Result<Vec<Cond>, String> {
            let items = obj[key].as_array().ok_or_else(|| format!("\"{key}\" needs a list of conditions"))?;
            if items.is_empty() {
                return Err(format!("\"{key}\" has no conditions"));
            }
            items.iter().map(Cond::parse).collect()
        };
        if obj.contains_key("count") {
            let of = Box::new(Cond::parse(&obj["count"])?);
            let times = obj.get("times").map(|t| number(t, "\"times\"")).transpose()?.unwrap_or(1).max(1);
            let within = obj.get("within").map(|w| window(w, true)).transpose()?.unwrap_or(Window::Game);
            return Ok(Cond::Count { of, times, within });
        }
        // one thing named, with what's said of it beside it (a part's "does", a lamp's "turns", a
        // group's "within" and "strict")
        let named: Vec<(&String, &Value)> = obj.iter().filter(|(k, _)| !["does", "turns", "fires", "within", "strict"].contains(&k.as_str())).collect();
        if named.len() != 1 {
            return Err(format!("a condition names one thing, not {}", named.len()));
        }
        let (key, val) = named[0];
        let group_within = || obj.get("within").map(|w| window(w, false)).transpose();
        if obj.contains_key("within") && !["all", "then"].contains(&key.as_str()) {
            return Err(format!("\"within\" goes with \"count\", \"all\" or \"then\", not \"{key}\""));
        }
        if obj.contains_key("strict") && key != "then" {
            return Err("\"strict\" goes with \"then\"".into());
        }
        Ok(match key.as_str() {
            "all" => Cond::All { of: list("all")?, within: group_within()? },
            "any" => Cond::Any(list("any")?),
            "then" => Cond::Then { of: list("then")?, within: group_within()?, strict: obj.get("strict").and_then(Value::as_bool).unwrap_or(false) },
            "score" => Cond::Score(at_least(val, "\"score\"")?),
            "ballsInPlay" => Cond::BallsInPlay(at_least(val, "\"ballsInPlay\"")? as u32),
            "credits" => Cond::Credits(at_least(val, "\"credits\"")? as u32),
            "game" => match val.as_str() {
                Some("started") => Cond::GameStarted,
                Some("over") => Cond::GameOver,
                _ => return Err("\"game\" is \"started\" or \"over\"".into()),
            },
            "ball" => Cond::Ball(number(val, "\"ball\"")?),
            "tilt" => Cond::Tilt,
            "extraBall" => Cond::ExtraBall,
            "part" => {
                let names: Vec<String> = val.as_str().ok_or("\"part\" needs the part's name")?.split('|').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                let does = obj.get("does").and_then(Value::as_str).unwrap_or("any").to_lowercase();
                Cond::Part { names, does }
            }
            "lamp" => Cond::Lamp { number: number(val, "\"lamp\"")?, on: turns(v, "turns") },
            "switch" => Cond::Switch { number: number(val, "\"switch\"")?, on: turns(v, "turns") },
            "solenoid" => Cond::Solenoid { number: number(val, "\"solenoid\"")?, on: v.get("fires").and_then(Value::as_bool).unwrap_or(true) },
            "script" => Cond::Script(val.as_str().ok_or("\"script\" needs the event's name")?.to_string()),
            other => return Err(format!("\"{other}\" isn't something a condition can read")),
        })
    }
}

impl Cond {
    /// The names of the parts it counts, into `out`: the player watches them where the table has
    /// their hits off.
    pub fn parts(&self, out: &mut Vec<String>) {
        match self {
            Cond::All { of: l, .. } | Cond::Any(l) | Cond::Then { of: l, .. } => l.iter().for_each(|c| c.parts(out)),
            Cond::Count { of, .. } => of.parts(out),
            Cond::Unless { cond, reset } => {
                cond.parts(out);
                reset.parts(out);
            }
            Cond::Part { names, .. } => out.extend(names.iter().cloned()),
            _ => {}
        }
    }

    /// Whether `signal` is one of the events it counts (never a state): what a strict `then` starts
    /// over on when it belongs to another step. Nothing's moved on by asking.
    pub fn touches(&self, signal: &Signal) -> bool {
        match self {
            Cond::All { of, .. } | Cond::Any(of) | Cond::Then { of, .. } => of.iter().any(|c| c.touches(signal)),
            Cond::Count { of, .. } => of.touches(signal),
            Cond::Unless { cond, .. } => cond.touches(signal),
            Cond::Score(_) | Cond::BallsInPlay(_) | Cond::Credits(_) => false,
            Cond::GameStarted => matches!(signal, Signal::GameStarted),
            Cond::GameOver => matches!(signal, Signal::GameOver),
            Cond::Ball(n) => matches!(signal, Signal::Ball(b) if b == n),
            Cond::Tilt => matches!(signal, Signal::Tilt(true)),
            Cond::ExtraBall => matches!(signal, Signal::ExtraBalls(n) if *n > 0),
            Cond::Part { names, does } => match signal {
                Signal::Part { name, does: d } => names.iter().any(|n| n.eq_ignore_ascii_case(name)) && (does == "any" || does.eq_ignore_ascii_case(d)),
                _ => false,
            },
            Cond::Lamp { number, on } => matches!(signal, Signal::Lamp { number: n, on: o } if n == number && o == on),
            Cond::Switch { number, on } => matches!(signal, Signal::Switch { number: n, on: o } if n == number && o == on),
            Cond::Solenoid { number, on } => matches!(signal, Signal::Solenoid { number: n, on: o } if n == number && o == on),
            Cond::Script(name) => matches!(signal, Signal::Script(s) if s.eq_ignore_ascii_case(name)),
        }
    }
}

/// The game as the signals have told it: what a state is read against.
#[derive(Clone, Debug, Default)]
pub struct Game {
    pub playing: bool,
    pub ball: u32,
    pub player: u32,
    pub scores: Vec<u64>,
    pub balls_in_play: u32,
    pub credits: u32,
    pub extra_balls: u32,
    /// The time now, in milliseconds.
    pub now: u64,
}

impl Game {
    /// The player up's score.
    pub fn score(&self) -> u64 {
        self.scores.get(self.player.max(1) as usize - 1).copied().unwrap_or(0)
    }
}

/// A condition followed: its own and its parts' counts and steps.
#[derive(Clone, Debug)]
pub struct Node {
    pub cond: Cond,
    children: Vec<Node>,
    count: u32,
    step: usize,
    times: VecDeque<u64>,
    /// Whether a state child was met at the last signal (a count counts it as it becomes met).
    was: bool,
    /// An `all` with a window: when each of its own was last met.
    latched: Vec<Option<u64>>,
    /// A `then` with a window: when its first step was met.
    started: Option<u64>,
}

impl Node {
    pub fn new(cond: Cond) -> Node {
        let children: Vec<Node> = match &cond {
            Cond::All { of: l, .. } | Cond::Any(l) | Cond::Then { of: l, .. } => l.iter().cloned().map(Node::new).collect(),
            Cond::Count { of, .. } => vec![Node::new((**of).clone())],
            Cond::Unless { cond, reset } => vec![Node::new((**cond).clone()), Node::new((**reset).clone())],
            _ => Vec::new(),
        };
        let latched = vec![None; children.len()];
        Node { cond, children, count: 0, step: 0, times: VecDeque::new(), was: false, latched, started: None }
    }

    /// Everything it's counted and every step it's taken, its own's too, back to nothing.
    pub fn reset(&mut self) {
        self.count = 0;
        self.step = 0;
        self.times.clear();
        self.was = false;
        self.latched.iter_mut().for_each(|l| *l = None);
        self.started = None;
        self.children.iter_mut().for_each(Node::reset);
    }

    /// True while it lasts (a state), rather than at a moment (an event).
    pub fn is_state(&self) -> bool {
        match &self.cond {
            Cond::Score(_) | Cond::BallsInPlay(_) | Cond::Credits(_) => true,
            Cond::All { within: None, .. } | Cond::Any(_) => self.children.iter().all(Node::is_state),
            Cond::Unless { .. } => self.children[0].is_state(),
            _ => false,
        }
    }

    /// Whether it's met by `signal`, the game as it is after it; its counts and steps moved on.
    pub fn met(&mut self, signal: &Signal, game: &Game) -> bool {
        // a new game, or ball, starts what runs over one again
        let new_game = matches!(signal, Signal::GameStarted);
        let new_ball = matches!(signal, Signal::Ball(_));
        match &self.cond {
            Cond::All { within: None, .. } => {
                let met: Vec<bool> = self.children.iter_mut().map(|c| c.met(signal, game)).collect();
                met.iter().all(|m| *m)
            }
            Cond::All { within: Some(within), .. } => {
                let within = *within;
                if new_game || (new_ball && matches!(within, Window::Ball | Window::OnBall(_))) {
                    self.latched.iter_mut().for_each(|l| *l = None);
                }
                let off_ball = matches!(within, Window::OnBall(n) if game.ball != n);
                if let Window::Seconds(s) = within {
                    let from = game.now.saturating_sub((s * 1000.0) as u64);
                    self.latched.iter_mut().filter(|l| l.is_some_and(|t| t < from)).for_each(|l| *l = None);
                }
                let met: Vec<bool> = self.children.iter_mut().map(|c| c.met(signal, game)).collect();
                for (k, m) in met.into_iter().enumerate() {
                    if m && !off_ball {
                        self.latched[k] = Some(game.now);
                    }
                }
                if self.latched.iter().all(Option::is_some) {
                    self.latched.iter_mut().for_each(|l| *l = None);
                    return true;
                }
                false
            }
            Cond::Any(_) => {
                let met: Vec<bool> = self.children.iter_mut().map(|c| c.met(signal, game)).collect();
                met.iter().any(|m| *m)
            }
            Cond::Then { within, strict, .. } => {
                let (within, strict) = (*within, *strict);
                let late = matches!(within, Some(Window::Seconds(s)) if self.step > 0 && self.started.is_some_and(|t| game.now.saturating_sub(t) > (s * 1000.0) as u64));
                if new_game || (new_ball && matches!(within, Some(Window::Ball | Window::OnBall(_)))) || late {
                    self.reset();
                }
                if matches!(within, Some(Window::OnBall(n)) if game.ball != n) {
                    return false;
                }
                // strict: something another step counts, out of turn, starts it over
                if strict && self.step > 0 {
                    let step = self.step;
                    let other = self.children.iter().enumerate().any(|(k, c)| k != step && c.cond.touches(signal));
                    if other && !self.children[step].cond.touches(signal) {
                        self.reset();
                    }
                }
                let step = self.step;
                if self.children[step].met(signal, game) {
                    if step == 0 {
                        self.started = Some(game.now);
                    }
                    self.step += 1;
                    if self.step == self.children.len() {
                        self.reset();
                        return true;
                    }
                }
                false
            }
            Cond::Unless { .. } => {
                if self.children[1].met(signal, game) {
                    self.children[0].reset();
                    return false;
                }
                self.children[0].met(signal, game)
            }
            Cond::Count { times, within, .. } => {
                let (times, within) = (*times, *within);
                if (new_game && within != Window::Games) || (new_ball && matches!(within, Window::Ball | Window::OnBall(_))) {
                    self.count = 0;
                    self.times.clear();
                }
                let state = self.children[0].is_state();
                let met = self.children[0].met(signal, game);
                let counts = met && !(state && self.was) && !matches!(within, Window::OnBall(n) if game.ball != n);
                self.was = met;
                if counts {
                    match within {
                        Window::Seconds(s) => {
                            self.times.push_back(game.now);
                            let from = game.now.saturating_sub((s * 1000.0) as u64);
                            while self.times.front().is_some_and(|t| *t < from) {
                                self.times.pop_front();
                            }
                            self.count = self.times.len() as u32;
                        }
                        _ => self.count += 1,
                    }
                    if self.count >= times {
                        self.count = 0;
                        self.times.clear();
                        return true;
                    }
                }
                false
            }
            Cond::Score(n) => game.score() >= *n,
            Cond::BallsInPlay(n) => game.balls_in_play >= *n,
            Cond::Credits(n) => game.credits >= *n,
            Cond::GameStarted => new_game,
            Cond::GameOver => matches!(signal, Signal::GameOver),
            Cond::Ball(n) => matches!(signal, Signal::Ball(b) if b == n),
            Cond::Tilt => matches!(signal, Signal::Tilt(true)),
            Cond::ExtraBall => matches!(signal, Signal::ExtraBalls(n) if *n > 0),
            Cond::Part { names, does } => match signal {
                Signal::Part { name, does: d } => names.iter().any(|n| n.eq_ignore_ascii_case(name)) && (does == "any" || does.eq_ignore_ascii_case(d)),
                _ => false,
            },
            Cond::Lamp { number, on } => matches!(signal, Signal::Lamp { number: n, on: o } if n == number && o == on),
            Cond::Switch { number, on } => matches!(signal, Signal::Switch { number: n, on: o } if n == number && o == on),
            Cond::Solenoid { number, on } => matches!(signal, Signal::Solenoid { number: n, on: o } if n == number && o == on),
            Cond::Script(name) => matches!(signal, Signal::Script(s) if s.eq_ignore_ascii_case(name)),
        }
    }

    /// The counts that run over every game, by their place in the condition (`0`, `0.1`, ...),
    /// into `out`: kept with what's earned between plays.
    pub fn games_counts(&self, at: &str, out: &mut Vec<(String, u32)>) {
        if let Cond::Count { within: Window::Games, .. } = &self.cond {
            out.push((at.to_string(), self.count));
        }
        for (k, c) in self.children.iter().enumerate() {
            c.games_counts(&format!("{at}.{k}"), out);
        }
    }

    /// A count that runs over every game, set to where the last play left it.
    pub fn set_games_count(&mut self, at: &str, count: u32) {
        let mut node = self;
        let mut places = at.split('.');
        if places.next() != Some("0") {
            return;
        }
        for place in places {
            let Some(next) = place.parse::<usize>().ok().and_then(|k| node.children.get_mut(k)) else { return };
            node = next;
        }
        if let Cond::Count { within: Window::Games, times, .. } = &node.cond {
            node.count = count.min(times.saturating_sub(1));
        }
    }

    /// How far it's come, where it counts or reaches for a score: done, of.
    pub fn progress(&self, game: &Game) -> Option<(u64, u64)> {
        self.progress_of(game).map(|(done, of, _)| (done, of))
    }

    /// How far it's come, as `progress`, and what's counted.
    pub fn progress_of(&self, game: &Game) -> Option<(u64, u64, Measure)> {
        match &self.cond {
            Cond::Count { times, .. } => Some((self.count as u64, *times as u64, Measure::Times)),
            Cond::Score(n) if game.playing => Some((game.score().min(*n), *n, Measure::Score)),
            Cond::Then { of, .. } => Some((self.step as u64, of.len() as u64, Measure::Steps)),
            Cond::All { of, within: Some(_) } => Some((self.latched.iter().filter(|l| l.is_some()).count() as u64, of.len() as u64, Measure::Steps)),
            Cond::Unless { .. } => self.children[0].progress_of(game),
            _ => self.children.iter().find_map(|c| c.progress_of(game)),
        }
    }
}

/// What a condition's progress counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// Times something's happened.
    Times,
    /// Points towards a score.
    Score,
    /// Steps taken, one after another.
    Steps,
}

/// A condition from the pack's JSON.
pub fn parse(v: &Value) -> Result<Cond, String> {
    Cond::parse(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(v: Value) -> Node {
        Node::new(parse(&v).unwrap())
    }

    fn playing() -> Game {
        Game { playing: true, ball: 1, player: 1, scores: vec![0], balls_in_play: 1, ..Default::default() }
    }

    #[test]
    fn the_parts_a_condition_counts() {
        let c = parse(&json!({ "then": [{ "part": "Bumper1|Bumper2", "does": "hit" }, { "count": { "part": "sw37" }, "times": 3 }, { "score": { "atLeast": 5 } }] })).unwrap();
        let mut got = Vec::new();
        c.parts(&mut got);
        assert_eq!(got, ["Bumper1", "Bumper2", "sw37"]);
    }

    #[test]
    fn conditions_read_as_the_format_writes_them() {
        assert_eq!(parse(&json!({ "score": { "atLeast": 1000 } })).unwrap(), Cond::Score(1000));
        assert_eq!(parse(&json!({ "part": "LeftRamp|RightRamp", "does": "hit" })).unwrap(), Cond::Part { names: vec!["LeftRamp".into(), "RightRamp".into()], does: "hit".into() });
        assert_eq!(parse(&json!({ "lamp": 28, "turns": "on" })).unwrap(), Cond::Lamp { number: 28, on: true });
        assert!(matches!(parse(&json!({ "count": { "part": "Martian1" }, "times": 5, "within": "ball" })).unwrap(), Cond::Count { times: 5, within: Window::Ball, .. }));
        assert!(parse(&json!({ "score": 5 })).is_err());
        assert!(parse(&json!({ "colour": "red" })).is_err());
        assert!(parse(&json!({ "count": { "ball": 1 }, "within": "year" })).is_err());
        assert!(matches!(parse(&json!({ "count": { "game": "started" }, "times": 10, "within": "games" })).unwrap(), Cond::Count { times: 10, within: Window::Games, .. }));
    }

    #[test]
    fn a_count_over_every_game_goes_on_from_game_to_game_and_play_to_play() {
        // a score of a million, ten times over every game played
        let mut n = node(json!({ "count": { "score": { "atLeast": 1000000 } }, "times": 3, "within": "games" }));
        let mut g = playing();
        let game_of = |n: &mut Node, g: &mut Game| {
            g.scores = vec![0];
            n.met(&Signal::GameStarted, g);
            g.scores = vec![1_500_000];
            n.met(&Signal::Score { player: 1, score: 1_500_000 }, g)
        };
        assert!(!game_of(&mut n, &mut g));
        assert!(!game_of(&mut n, &mut g));
        let mut kept = Vec::new();
        n.games_counts("0", &mut kept);
        assert_eq!(kept, [("0".to_string(), 2)]);
        // the next play starts from where this one left it
        let mut next = node(json!({ "count": { "score": { "atLeast": 1000000 } }, "times": 3, "within": "games" }));
        next.set_games_count("0", 2);
        assert!(game_of(&mut next, &mut g));
    }

    #[test]
    fn a_count_within_a_ball_starts_again_with_the_next() {
        let mut n = node(json!({ "count": { "part": "Martian1|Martian2", "does": "hit" }, "times": 3, "within": "ball" }));
        let g = playing();
        let hit = |name: &str| Signal::Part { name: name.into(), does: "hit".into() };
        assert!(!n.met(&hit("Martian1"), &g));
        assert!(!n.met(&hit("martian2"), &g));
        assert_eq!(n.progress(&g), Some((2, 3)));
        // a new ball: from nothing again
        assert!(!n.met(&Signal::Ball(2), &g));
        assert!(!n.met(&hit("Martian1"), &g));
        assert!(!n.met(&hit("Martian1"), &g));
        assert!(n.met(&hit("Martian1"), &g));
    }

    #[test]
    fn a_state_counts_as_it_becomes_true_and_all_wants_both_at_once() {
        // three multiballs in a game: each counted as it starts, not every signal during it
        let mut n = node(json!({ "count": { "ballsInPlay": { "atLeast": 2 } }, "times": 2 }));
        let mut g = playing();
        g.balls_in_play = 2;
        assert!(!n.met(&Signal::BallsInPlay(2), &g));
        assert!(!n.met(&Signal::Part { name: "x".into(), does: "hit".into() }, &g));
        g.balls_in_play = 1;
        assert!(!n.met(&Signal::BallsInPlay(1), &g));
        g.balls_in_play = 3;
        assert!(n.met(&Signal::BallsInPlay(3), &g));
        // a ramp made during a multiball
        let mut both = node(json!({ "all": [{ "ballsInPlay": { "atLeast": 2 } }, { "part": "LeftRamp", "does": "hit" }] }));
        g.balls_in_play = 1;
        assert!(!both.met(&Signal::Part { name: "LeftRamp".into(), does: "hit".into() }, &g));
        g.balls_in_play = 2;
        assert!(both.met(&Signal::Part { name: "LeftRamp".into(), does: "hit".into() }, &g));
    }

    fn hit(name: &str) -> Signal {
        Signal::Part { name: name.into(), does: "hit".into() }
    }

    #[test]
    fn counts_one_after_another_inside_a_minute() {
        // each bumper three times, in turn, the whole inside sixty seconds
        let three = |n: &str| json!({ "count": { "part": n, "does": "hit" }, "times": 3 });
        let mut n = node(json!({ "then": [three("B1"), three("B2"), three("B3")], "within": { "seconds": 60 } }));
        let mut g = playing();
        let at = |n: &mut Node, g: &mut Game, ms: u64, name: &str| {
            g.now = ms;
            n.met(&hit(name), g)
        };
        for (ms, name) in [(0, "B1"), (1000, "B1"), (2000, "B2"), (3000, "B1")] {
            assert!(!at(&mut n, &mut g, ms, name));
        }
        // B2 before B1's third didn't count; now B1's third, then B2 and B3 three times each
        assert_eq!(n.progress(&g), Some((1, 3)));
        for (ms, name) in [(4000, "B2"), (5000, "B2"), (6000, "B2"), (7000, "B3"), (8000, "B3")] {
            assert!(!at(&mut n, &mut g, ms, name));
        }
        assert!(at(&mut n, &mut g, 9000, "B3"));
        // too slow: past the minute it starts over
        for (ms, name) in [(10_000, "B1"), (10_100, "B1"), (10_200, "B1"), (11_000, "B2"), (11_100, "B2"), (11_200, "B2"), (80_000, "B3"), (80_100, "B3")] {
            assert!(!at(&mut n, &mut g, ms, name));
        }
        assert!(!at(&mut n, &mut g, 80_200, "B3"));
    }

    #[test]
    fn strict_order_starts_over_on_a_step_out_of_turn() {
        let mut n = node(json!({ "then": [{ "part": "A" }, { "part": "B" }, { "part": "C" }], "strict": true }));
        let g = playing();
        // A, C (out of turn: over), B, C: not in order
        for name in ["A", "C", "B", "C"] {
            assert!(!n.met(&hit(name), &g));
        }
        // A, A (starts it again at A), B, then something no step counts, then C
        for name in ["A", "A", "B", "X"] {
            assert!(!n.met(&hit(name), &g));
        }
        assert!(n.met(&hit("C"), &g));
        // without strict, A, C, B, C is in order
        let mut loose = node(json!({ "then": [{ "part": "A" }, { "part": "B" }, { "part": "C" }] }));
        for name in ["A", "C", "B"] {
            assert!(!loose.met(&hit(name), &g));
        }
        assert!(loose.met(&hit("C"), &g));
    }

    #[test]
    fn all_in_any_order_inside_its_window_and_unless_starting_over() {
        let mut n = node(json!({ "all": [{ "part": "A" }, { "part": "B" }], "within": { "seconds": 10 } }));
        let mut g = playing();
        g.now = 0;
        assert!(!n.met(&hit("B"), &g));
        g.now = 11_000;
        // B was too long ago
        assert!(!n.met(&hit("A"), &g));
        g.now = 12_000;
        assert!(n.met(&hit("B"), &g));
        // fifty hits in a ball, unless the ball drains first
        let mut u = node(json!({ "count": { "part": "Bumper1" }, "times": 3, "within": "ball", "unless": { "part": "Drain" } }));
        let g = playing();
        assert!(!u.met(&hit("Bumper1"), &g));
        assert!(!u.met(&hit("Bumper1"), &g));
        assert!(!u.met(&hit("Drain"), &g));
        assert_eq!(u.progress(&g), Some((0, 3)));
        for _ in 0..2 {
            assert!(!u.met(&hit("Bumper1"), &g));
        }
        assert!(u.met(&hit("Bumper1"), &g));
        let mut parts = Vec::new();
        parse(&json!({ "count": { "part": "Bumper1" }, "unless": { "part": "Drain" } })).unwrap().parts(&mut parts);
        assert_eq!(parts, ["Bumper1", "Drain"]);
        // what isn't the format refused
        assert!(parse(&json!({ "any": [{ "part": "A" }], "within": "ball" })).is_err());
        assert!(parse(&json!({ "all": [{ "part": "A" }], "strict": true })).is_err());
        assert!(parse(&json!({ "then": [{ "part": "A" }], "within": "games" })).is_err());
    }

    #[test]
    fn a_count_on_one_ball_by_its_number() {
        let mut n = node(json!({ "count": { "part": "A" }, "times": 2, "within": { "ball": 3 } }));
        let mut g = playing();
        // ball 1: nothing counts
        assert!(!n.met(&hit("A"), &g));
        assert!(!n.met(&hit("A"), &g));
        g.ball = 3;
        assert!(!n.met(&Signal::Ball(3), &g));
        assert!(!n.met(&hit("A"), &g));
        assert!(n.met(&hit("A"), &g));
        // a score on ball 1 alone
        let mut s = node(json!({ "count": { "score": { "atLeast": 100 } }, "within": { "ball": 1 } }));
        let mut g = playing();
        g.ball = 2;
        g.scores = vec![150];
        assert!(!s.met(&Signal::Score { player: 1, score: 150 }, &g));
        assert!(parse(&json!({ "count": { "part": "A" }, "within": { "ball": 0 } })).is_err());
        assert!(parse(&json!({ "then": [{ "part": "A" }, { "part": "B" }], "within": { "ball": 1 } })).is_ok());
    }

    #[test]
    fn then_goes_in_turn_and_seconds_keep_only_the_latest() {
        let mut n = node(json!({ "then": [{ "part": "A" }, { "part": "B" }] }));
        let g = playing();
        let p = |name: &str| Signal::Part { name: name.into(), does: "hit".into() };
        assert!(!n.met(&p("B"), &g));
        assert!(!n.met(&p("A"), &g));
        assert!(n.met(&p("B"), &g));
        let mut quick = node(json!({ "count": { "part": "Spinner", "does": "spin" }, "times": 3, "within": { "seconds": 2 } }));
        let mut g = playing();
        let spin = Signal::Part { name: "Spinner".into(), does: "spin".into() };
        for (at, want) in [(0, false), (500, false), (3000, false), (3500, false), (3900, true)] {
            g.now = at;
            assert_eq!(quick.met(&spin, &g), want, "at {at}");
        }
    }
}
