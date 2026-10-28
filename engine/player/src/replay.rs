//! Replays (determinism): the game thread writes down, in order,
//! each key pressed or released, each time the physics caught up with the
//! clock (counted in physics steps) and each frame prepared. Played back in
//! simulated time, that list repeats the game exactly — as long as the
//! table's script doesn't read the clock: `Randomize`, `Timer` and `Now`
//! are answered by the script engine from Windows' clock, which a replay
//! can't repeat yet.
//!
//! The file is plain text: a header, then one event per line.
//!
//! ```text
//! vpi-replay 1
//! table C:\vPinball\VisualPinball\Tables\Hang Glider (Bally 1976) VPW v1.2.vpx
//! i 2a d      a key: its DirectInput code in hex, d(own) or u(p)
//! a 1234      the physics caught up: steps done since play started
//! f           a frame prepared
//! ```

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::game::Game;
use crate::input::InputEvent;

const HEADER: &str = "vpi-replay 1";

/// One recorded event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Input { dik: u8, down: bool },
    /// The physics caught up to this many steps since play started.
    Advance { steps: u64 },
    Frame,
}

/// Writes a replay as the game runs.
pub struct Recorder {
    out: BufWriter<File>,
}

impl Recorder {
    pub fn create(path: &Path, table: &Path) -> std::io::Result<Recorder> {
        let mut out = BufWriter::new(File::create(path)?);
        writeln!(out, "{HEADER}")?;
        writeln!(out, "table {}", table.display())?;
        Ok(Recorder { out })
    }

    /// Writes one event. A failed write stops nothing: the game goes on.
    pub fn record(&mut self, e: Event) {
        let _ = match e {
            Event::Input { dik, down } => writeln!(self.out, "i {dik:02x} {}", if down { 'd' } else { 'u' }),
            Event::Advance { steps } => writeln!(self.out, "a {steps}"),
            Event::Frame => writeln!(self.out, "f"),
        };
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

/// A replay read back: the table it was played on, and its events.
pub struct Replay {
    pub table: PathBuf,
    pub events: Vec<Event>,
}

pub fn read(path: &Path) -> Result<Replay, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut lines = BufReader::new(file).lines();
    let mut next = |what: &str| lines.next().and_then(Result::ok).ok_or_else(|| format!("{}: {what} missing", path.display()));
    if next("header")? != HEADER {
        return Err(format!("{} is not a replay (version 1)", path.display()));
    }
    let table = next("table")?.strip_prefix("table ").map(PathBuf::from).ok_or_else(|| format!("{}: no table line", path.display()))?;
    let mut events = Vec::new();
    for (n, line) in lines.enumerate() {
        let line = line.map_err(|e| format!("{}: {e}", path.display()))?;
        let bad = || format!("{}: line {} not understood: {line}", path.display(), n + 3);
        let mut f = line.split(' ');
        events.push(match (f.next(), f.next(), f.next()) {
            (Some("i"), Some(k), Some(d)) => Event::Input { dik: u8::from_str_radix(k, 16).map_err(|_| bad())?, down: d == "d" },
            (Some("a"), Some(s), None) => Event::Advance { steps: s.parse().map_err(|_| bad())? },
            (Some("f"), None, None) => Event::Frame,
            (Some(""), None, None) | (None, ..) => continue,
            _ => return Err(bad()),
        });
    }
    Ok(Replay { table, events })
}

/// Plays a replay's events on a game started at simulated time 0, calling
/// `after` after each one.
pub fn play(game: &mut Game, events: &[Event], mut after: impl FnMut(&Game, &Event)) {
    game.start(0);
    for (n, e) in events.iter().enumerate() {
        match *e {
            Event::Input { dik, down } => game.input(InputEvent { id: n as u64 + 1, time_ns: 0, dik, down }),
            // the physics ran until `steps` were done: the next step ends after them
            Event::Advance { steps } => {
                game.advance((steps + 1) * 1000);
            }
            Event::Frame => game.prepare_frame(),
        }
        after(game, e);
        if game.quit_requested() {
            break;
        }
    }
}
