//! A hardcore game turned softcore when something that makes a game softcore is changed during it
//! in VPX's own menu. VPX tells add-ons nothing of its menu, but it keeps such a change in the
//! settings files it plays with — the copies the app hands it for a hardcore game (`-Ini`,
//! `-TableIni`), else its own — so those are watched: each read again once it's written, and the
//! settings that matter compared with what they were as play began: the table's options and its
//! difficulty, VPX's physics sets, its PinMAME add-on's cheat mode.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// How often the files' dates are looked at.
const EVERY: Duration = Duration::from_millis(1000);

/// A settings file's settings that make a game softcore, by (section, name), both in lower case:
/// none where they're unset or empty.
pub fn that_matter(text: &str) -> BTreeMap<(String, String), String> {
    let mut out = BTreeMap::new();
    let mut section = String::new();
    for line in text.lines() {
        let l = line.trim().trim_start_matches('\u{feff}');
        if l.starts_with('[') && l.ends_with(']') {
            section = l[1..l.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if l.starts_with(';') || l.starts_with('#') {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
        let matters = match section.as_str() {
            "tableoption" => true,
            "tableoverride" => k == "difficulty",
            "player" => k.starts_with("tablephysics") || k.starts_with("flipperphysics"),
            "plugin.pinmame" => k == "cheat",
            _ => false,
        };
        if matters && !v.is_empty() {
            out.insert((section.clone(), k), v.to_string());
        }
    }
    out
}

/// What changed from `before` to `now`, in words: the first change found; none where nothing did.
pub fn what_changed(before: &BTreeMap<(String, String), String>, now: &BTreeMap<(String, String), String>) -> Option<&'static str> {
    let keys: BTreeSet<&(String, String)> = before.keys().chain(now.keys()).collect();
    keys.into_iter().find(|k| before.get(*k) != now.get(*k)).map(|(section, _)| match section.as_str() {
        "tableoption" => "a table option was changed",
        "tableoverride" => "the difficulty was changed",
        "player" => "a physics set was changed",
        _ => "cheat mode was changed",
    })
}

/// One file watched: its date when last read, and its settings that matter then.
struct Watched {
    file: PathBuf,
    date: Option<SystemTime>,
    read: BTreeMap<(String, String), String>,
}

fn date_of(file: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(file).and_then(|m| m.modified()).ok()
}

fn read(file: &PathBuf) -> BTreeMap<(String, String), String> {
    std::fs::read(file).map(|b| that_matter(&String::from_utf8_lossy(&b))).unwrap_or_default()
}

/// The settings files a hardcore game plays with, watched.
pub struct Watch {
    files: Vec<Watched>,
    next: Instant,
}

impl Watch {
    /// `files` as they stand as play begins.
    pub fn new(files: &[PathBuf]) -> Watch {
        let files = files.iter().map(|f| Watched { file: f.clone(), date: date_of(f), read: read(f) }).collect();
        Watch { files, next: Instant::now() + EVERY }
    }

    /// Looked at, at most once each `EVERY`: what changed in words, once a file written since says
    /// so; none otherwise.
    pub fn look(&mut self, now: Instant) -> Option<&'static str> {
        if now < self.next {
            return None;
        }
        self.next = now + EVERY;
        for w in &mut self.files {
            let date = date_of(&w.file);
            if date == w.date {
                continue;
            }
            w.date = date;
            let read = read(&w.file);
            if let Some(why) = what_changed(&w.read, &read) {
                return Some(why);
            }
            w.read = read;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_makes_a_game_softcore_is_read_and_its_changes_told() {
        let start = "[Player]\r\nSoundVolume = 65\r\nTablePhysicsGravityConstant0 = 1.762\r\n[TableOption]\r\n[TableOverride]\r\nDifficulty = \r\n[Plugin.PinMAME]\r\nCheat = 0\r\n";
        let a = that_matter(start);
        assert_eq!(a.len(), 2, "{a:?}");
        // the sound turned up: nothing that matters
        assert_eq!(what_changed(&a, &that_matter(&start.replace("SoundVolume = 65", "SoundVolume = 80"))), None);
        // a table option set in VPX's menu, the difficulty, cheat mode on
        assert_eq!(what_changed(&a, &that_matter(&start.replace("[TableOption]\r\n", "[TableOption]\r\nOutlane_Difficulty = 2\r\n"))), Some("a table option was changed"));
        assert_eq!(what_changed(&a, &that_matter(&start.replace("Difficulty = ", "Difficulty = 0.4"))), Some("the difficulty was changed"));
        assert_eq!(what_changed(&a, &that_matter(&start.replace("Cheat = 0", "Cheat = 1"))), Some("cheat mode was changed"));
    }

    #[test]
    fn a_file_is_read_again_once_its_written() {
        let dir = std::env::temp_dir().join(format!("vpi-addon-hardcore-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("A Table.ini");
        std::fs::write(&file, "[TableOption]\r\n").unwrap();
        let mut w = Watch::new(std::slice::from_ref(&file));
        let later = Instant::now() + Duration::from_secs(2);
        assert_eq!(w.look(later), None);
        std::thread::sleep(Duration::from_millis(30));
        std::fs::write(&file, "[TableOption]\r\nOutlane_Difficulty = 2\r\n").unwrap();
        // the date moved on Windows' clock by then, or the next look finds it
        let told = w.look(later + Duration::from_secs(2)).or_else(|| {
            std::thread::sleep(Duration::from_millis(1100));
            std::fs::write(&file, "[TableOption]\r\nOutlane_Difficulty = 3\r\n").unwrap();
            w.look(later + Duration::from_secs(4))
        });
        assert_eq!(told, Some("a table option was changed"));
        // a file not there as play began, made during it
        let none = dir.join("Another.ini");
        let _ = std::fs::remove_file(&none);
        let mut w = Watch::new(std::slice::from_ref(&none));
        std::fs::write(&none, "[Plugin.PinMAME]\r\nCheat = 1\r\n").unwrap();
        assert_eq!(w.look(later), Some("cheat mode was changed"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
