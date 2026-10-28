//! A game followed for the achievements of the packs a table plays: each signal the player tells
//! moves the game on (`cond::Game`) and every achievement not yet earned is checked against it —
//! only while a game is on, so what a machine shows between games (the last game's scores) earns
//! nothing — an achievement met recorded at once (`Earned`), hardcore or practice as the table
//! plays, with a mastery where it completes one; a count over every game starts where the last
//! play left it, and where it stands is kept with what's earned as it moves. Ours.

use crate::cond::{Game, Measure, Node, Signal};
use crate::earned::{Earned, Mastered, Record};
use crate::pack::{Achievement, Pack};

/// Where one of the packs' achievements stands, for the pause menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub pack: String,
    pub name: String,
    pub secret: bool,
    pub how: How,
}

#[derive(Clone, Debug, PartialEq)]
pub enum How {
    /// Earned, before this play or during it.
    Earned,
    /// Still to earn: how far it's come, where it counts.
    OnItsWay(Option<(u64, u64, Measure)>),
    /// Its condition doesn't read, so it can't be earned.
    Unread,
}

/// An achievement just earned, to announce.
#[derive(Clone, Debug)]
pub struct Earn {
    pub pack_id: String,
    pub pack_name: String,
    pub achievement: Achievement,
    pub record: Record,
    /// The pack's version mastered with it.
    pub mastered: Option<Mastered>,
}

struct Watched {
    pack: Pack,
    /// The achievements still to earn, by their place in the pack, each followed.
    nodes: Vec<(usize, Node)>,
}

pub struct Watcher {
    packs: Vec<Watched>,
    pub game: Game,
    /// `hardcore` or `practice`.
    mode: String,
    table: Option<String>,
    /// A count over every game moved since this was last asked (`take_progress_moved`).
    progress_moved: bool,
}

impl Watcher {
    /// The packs' achievements not yet earned in `mode` followed (one earned in hardcore is done for
    /// both); those whose conditions don't read are left out.
    pub fn new(packs: Vec<Pack>, mode: &str, table: Option<String>, earned: &Earned) -> Watcher {
        let mode = if mode == "hardcore" { "hardcore" } else { "practice" }.to_string();
        let packs = packs
            .into_iter()
            .map(|pack| {
                let nodes = pack
                    .achievements
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| !earned.has(&pack.pack.id, &a.id, &mode))
                    .filter_map(|(i, a)| {
                        let mut node = Node::new(crate::cond::parse(&a.when).ok()?);
                        for (at, count) in earned.progress_of(&pack.pack.id, &a.id, &mode) {
                            node.set_games_count(&at, count);
                        }
                        Some((i, node))
                    })
                    .collect();
                Watched { pack, nodes }
            })
            .collect();
        Watcher { packs, game: Game::default(), mode, table, progress_moved: false }
    }

    /// Play goes on in practice from now (something that changes how the table plays was changed
    /// while playing): what's earned from here is recorded so.
    pub fn set_practice(&mut self) {
        self.mode = "practice".into();
    }

    /// Whether a count over every game moved since the last time this was asked: what's earned is
    /// to be saved.
    pub fn take_progress_moved(&mut self) -> bool {
        std::mem::take(&mut self.progress_moved)
    }

    /// The parts the achievements still to earn count, by name, each once.
    pub fn part_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for w in &self.packs {
            for (_, node) in &w.nodes {
                node.cond.parts(&mut out);
            }
        }
        out.sort_by_key(|n| n.to_lowercase());
        out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        out
    }

    /// How many achievements are left to earn.
    pub fn left(&self) -> usize {
        self.packs.iter().map(|p| p.nodes.len()).sum()
    }

    /// Whether there's anything left to earn.
    pub fn is_empty(&self) -> bool {
        self.packs.iter().all(|p| p.nodes.is_empty())
    }

    /// A signal at `now` (milliseconds): the game moved on, and what's earned by it recorded in
    /// `earned` and returned.
    pub fn signal(&mut self, signal: Signal, now: u64, earned: &mut Earned) -> Vec<Earn> {
        let g = &mut self.game;
        g.now = now;
        match &signal {
            Signal::GameStarted => {
                g.playing = true;
                g.ball = 1;
                g.player = 1;
                g.scores.clear();
                g.extra_balls = 0;
            }
            Signal::GameOver => g.playing = false,
            Signal::Ball(n) => {
                g.ball = *n;
                // a machine whose map tells no game's start: its first ball starts one
                if *n >= 1 && !g.playing {
                    g.playing = true;
                }
            }
            Signal::Player(p) => g.player = *p,
            Signal::Score { player, score } => {
                let at = player.max(&1) - 1;
                if g.scores.len() <= at as usize {
                    g.scores.resize(at as usize + 1, 0);
                }
                g.scores[at as usize] = *score;
            }
            Signal::Credits(c) => g.credits = *c,
            Signal::ExtraBalls(n) => g.extra_balls = *n,
            Signal::BallsInPlay(n) => g.balls_in_play = *n,
            _ => {}
        }
        // between games nothing counts but a game's start and end themselves
        if !self.game.playing && !matches!(signal, Signal::GameOver) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for w in &mut self.packs {
            let mut done = Vec::new();
            for (k, (i, node)) in w.nodes.iter_mut().enumerate() {
                if node.met(&signal, &self.game) {
                    done.push((k, *i));
                }
                let mut counts = Vec::new();
                node.games_counts("0", &mut counts);
                if !counts.is_empty() && earned.set_progress(&w.pack.pack.id, &w.pack.achievements[*i].id, &self.mode, &counts) {
                    self.progress_moved = true;
                }
            }
            for (k, i) in done.into_iter().rev() {
                w.nodes.remove(k);
                let a = w.pack.achievements[i].clone();
                // earned: its counts over every game are done with
                if earned.set_progress(&w.pack.pack.id, &a.id, &self.mode, &[]) {
                    self.progress_moved = true;
                }
                let record = Record { at: crate::earned::now(), mode: self.mode.clone(), pack_version: w.pack.pack.version.clone(), table: self.table.clone() };
                if earned.add(&w.pack.pack.id, &a.id, record.clone()) {
                    let mastered = earned.note_mastery(&w.pack).then(|| earned.masters(&w.pack)).flatten();
                    out.push(Earn { pack_id: w.pack.pack.id.clone(), pack_name: w.pack.pack.name.clone(), achievement: a, record, mastered });
                }
            }
        }
        out
    }

    /// How far each achievement still to earn has come, where it counts: the pack's ID, the
    /// achievement's, done, of.
    pub fn progress(&self) -> Vec<(String, String, u64, u64)> {
        self.packs
            .iter()
            .flat_map(|w| w.nodes.iter().filter_map(|(i, n)| n.progress(&self.game).map(|(d, o)| (w.pack.pack.id.clone(), w.pack.achievements[*i].id.clone(), d, o))))
            .collect()
    }

    /// Where every achievement of the packs stands, in the packs' order.
    pub fn standing(&self) -> Vec<Standing> {
        self.packs
            .iter()
            .flat_map(|w| {
                w.pack.achievements.iter().enumerate().map(move |(i, a)| {
                    let how = match w.nodes.iter().find(|(k, _)| *k == i) {
                        Some((_, node)) => How::OnItsWay(node.progress_of(&self.game)),
                        None if crate::cond::parse(&a.when).is_err() => How::Unread,
                        None => How::Earned,
                    };
                    Standing { pack: w.pack.pack.name.clone(), name: a.name.clone(), secret: a.secret, how }
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earned_only_while_a_game_is_on_once_and_mastered() {
        let pack = Pack::parse(crate::pack::tests_example()).unwrap();
        let mut earned = Earned::default();
        let mut w = Watcher::new(vec![pack.clone()], "hardcore", Some("afm.vpx".into()), &earned);
        // the last game's score, shown between games: nothing
        assert!(w.signal(Signal::Score { player: 1, score: 2_000_000_000 }, 0, &mut earned).is_empty());
        assert!(w.signal(Signal::GameStarted, 10, &mut earned).is_empty());
        assert!(w.signal(Signal::Score { player: 1, score: 900_000_000 }, 20, &mut earned).is_empty());
        let got = w.signal(Signal::Score { player: 1, score: 1_000_000_001 }, 30, &mut earned);
        assert_eq!(got.iter().map(|e| e.achievement.id.as_str()).collect::<Vec<_>>(), ["billionaire"]);
        assert!(got[0].mastered.is_none());
        // earned once
        assert!(w.signal(Signal::Score { player: 1, score: 1_100_000_000 }, 40, &mut earned).is_empty());
        let got = w.signal(Signal::BallsInPlay(4), 50, &mut earned);
        assert_eq!(got[0].achievement.id, "strobe");
        assert_eq!(got[0].mastered.as_ref().map(|m| m.pack_version.as_str()), Some("1.1"));
        assert!(w.is_empty());
        // a new watcher on the same records has nothing left in hardcore; practice neither
        assert!(Watcher::new(vec![pack.clone()], "hardcore", None, &earned).is_empty());
        assert!(Watcher::new(vec![pack], "practice", None, &earned).is_empty());
    }

    #[test]
    fn where_each_stands_counts_scores_and_what_doesnt_read() {
        let mut pack = Pack::parse(crate::pack::tests_example()).unwrap();
        let mut pops = pack.achievements[0].clone();
        pops.id = "pops".into();
        pops.when = serde_json::json!({ "count": { "part": "Bumper1|Bumper2", "does": "hit" }, "times": 25, "within": "ball" });
        let mut wrong = pack.achievements[0].clone();
        wrong.id = "wrong".into();
        wrong.when = serde_json::json!({ "colour": "red" });
        pack.achievements.extend([pops, wrong]);
        let mut earned = Earned::default();
        let mut w = Watcher::new(vec![pack], "practice", None, &earned);
        w.signal(Signal::GameStarted, 0, &mut earned);
        w.signal(Signal::Score { player: 1, score: 250_000_000 }, 10, &mut earned);
        for at in 0..12 {
            w.signal(Signal::Part { name: "bumper2".into(), does: "hit".into() }, 20 + at, &mut earned);
        }
        w.signal(Signal::BallsInPlay(4), 40, &mut earned);
        let how: Vec<How> = w.standing().into_iter().map(|s| s.how).collect();
        assert_eq!(
            how,
            [
                How::OnItsWay(Some((250_000_000, 1_000_000_000, Measure::Score))),
                How::Earned,
                How::OnItsWay(Some((12, 25, Measure::Times))),
                How::Unread,
            ]
        );
        assert!(w.standing()[1].secret);
    }
}
