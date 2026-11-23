//! A ROM machine's game making the high scores, for the screenshot taken then (`[VPI]`
//! `ScreenshotAtHighScore`): while that's on, a thread
//! of its own asks PinMAME for the game's memory every few seconds — as VPX's PinMAME plugin's
//! `GetNVRAM` gives it, what PinMAME writes to the game's `.nv` file — reads its high scores with
//! the Pinball Memory Maps (`pinmame-nvram`, MIT; the maps ODbL, as the app reads them), and
//! asks for a screenshot when a score appears that the table didn't hold before. The game thread
//! never waits on it. Ours.

use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use pinmame_link::Link;

/// How long the machine runs before its scores are first read (past its start-up and, the first
/// time, its factory settings), and how often after.
const FIRST_LOOK: Duration = Duration::from_secs(10);
const EVERY: Duration = Duration::from_secs(3);

/// A high score as the maps read it: who, and how much.
type Entry = (String, u64);

/// Whether `now` holds a score the table didn't hold in `before`: a new one — its initials entered
/// after it count for nothing more — not one of the factory's settings coming in over memory that
/// held none (`before` with no score at all).
pub fn made_the_scores(before: &[Entry], now: &[Entry]) -> bool {
    before.iter().any(|(_, s)| *s > 0) && now.iter().any(|(_, s)| *s > 0 && !before.iter().any(|(_, b)| b == s))
}

/// Watches the game `rom` runs over `link` while it lives, asking for a screenshot each time it
/// makes the high scores.
pub fn watch(link: Weak<Link>, rom: String) {
    let spawned = std::thread::Builder::new().name("vpi-machine-scores".into()).spawn(move || {
        let folder = std::env::temp_dir().join(format!("vpi-machine-scores-{}", std::process::id()));
        if std::fs::create_dir_all(&folder).is_err() {
            return;
        }
        // named as PinMAME names its sets, whatever case the script wrote: the map is found by it
        let file = folder.join(format!("{}.nv", rom.to_ascii_lowercase()));
        let started = Instant::now();
        let mut before: Option<Vec<Entry>> = None;
        let mut said = false;
        loop {
            std::thread::sleep(if before.is_none() { FIRST_LOOK.saturating_sub(started.elapsed()).max(EVERY) } else { EVERY });
            let Some(link) = link.upgrade() else { break };
            let Some(now) = scores(&link, &file) else {
                if before.is_none() && started.elapsed() > Duration::from_secs(60) {
                    // no map for the game, or no memory to read: nothing to watch
                    break;
                }
                continue;
            };
            if !said {
                eprintln!("vpi-player: the high scores watched, for a screenshot when a game makes them: {} read from {rom}'s memory", now.len());
                said = true;
            }
            match &before {
                Some(b) if made_the_scores(b, &now) => {
                    eprintln!("vpi-player: {rom}'s game made the high scores");
                    crate::screenshot::at_new_high_score();
                    before = Some(now);
                }
                Some(_) => before = Some(now),
                None if now.iter().any(|(_, s)| *s > 0) => before = Some(now),
                None => {}
            }
        }
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&folder);
    });
    if let Err(e) = spawned {
        eprintln!("vpi-player: the high scores aren't watched: {e}");
    }
}

/// The game's high scores now: its memory asked of PinMAME, written as its `.nv` file would be,
/// and read with the maps. None when PinMAME doesn't answer or the maps don't know the game.
fn scores(link: &Arc<Link>, file: &PathBuf) -> Option<Vec<Entry>> {
    let shared = link.shared();
    let request = shared.request_nvram();
    let asked = Instant::now();
    let bytes = loop {
        if let Some(answer) = shared.nvram_answer_since(request) {
            break answer?;
        }
        if asked.elapsed() > Duration::from_secs(1) {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    std::fs::write(file, &bytes).ok()?;
    let mut nv = pinmame_nvram::Nvram::open(file).ok()??;
    Some(nv.read_highscores().ok()?.into_iter().map(|h| (h.initials.trim().to_string(), h.score)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(scores: &[(&str, u64)]) -> Vec<Entry> {
        scores.iter().map(|&(i, s)| (i.to_string(), s)).collect()
    }

    #[test]
    fn a_new_score_in_the_table_is_a_game_making_it() {
        let before = list(&[("GCH", 8_000_000_000), ("JPS", 6_000_000_000), ("ABC", 5_000_000_000)]);
        // nothing changed
        assert!(!made_the_scores(&before, &before));
        // a new score pushed the last one out
        assert!(made_the_scores(&before, &list(&[("GCH", 8_000_000_000), ("VPI", 7_000_000_000), ("JPS", 6_000_000_000)])));
        // its initials entered after: the same game, no second screenshot
        let entered = list(&[("GCH", 8_000_000_000), ("VPI", 7_000_000_000), ("JPS", 6_000_000_000)]);
        assert!(!made_the_scores(&list(&[("GCH", 8_000_000_000), ("", 7_000_000_000), ("JPS", 6_000_000_000)]), &entered));
        // the factory's scores coming in over memory that held none
        assert!(!made_the_scores(&list(&[("", 0), ("", 0)]), &before));
    }
}
