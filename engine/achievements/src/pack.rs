//! An achievement pack as its `achievements.json` says, the format's first draft: the pack itself
//! (an ID kept through its versions, its name, version, authors, what changed), what it's made for
//! (a game, the tables and ROM sets it was tried on) and its achievements, each with what's shown
//! (name, what to do, points, a picture, secret or not), the pack version its rules date from
//! (`since`) and when it's earned (`when`, read by `cond`). Read from a pack's folder, and checked
//! as VPIDB checks an upload. Ours.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What the file says it is.
pub const FORMAT: &str = "vpi-achievements";
/// The draft of the format this reads.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub format: String,
    pub format_version: u32,
    pub pack: About,
    #[serde(rename = "for")]
    pub made_for: For,
    pub achievements: Vec<Achievement>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct About {
    /// Made once, a UUID, kept through every version.
    pub id: String,
    pub name: String,
    /// `major.minor`.
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<String>,
    /// Its picture, a path inside the pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The sound played as one of its achievements is announced, a path inside the pack (WAV,
    /// FLAC, MP3 or Ogg Vorbis); none: the player's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct For {
    pub game: Game,
    /// The tables it was made and tried on; none: any table of the game.
    #[serde(default)]
    pub tables: Vec<TableRef>,
    /// A ROM machine's sets it was made for.
    #[serde(default)]
    pub roms: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    /// VPS's ID for the game, where VPS lists it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vps: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TableRef {
    /// VPS's ID for the table file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vps: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    /// Lower case, unique in the pack, never changed once published.
    pub id: String,
    pub name: String,
    /// What to do, in words.
    pub about: String,
    /// 1 to 100.
    pub points: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Its name and words hidden until it's earned.
    #[serde(default)]
    pub secret: bool,
    /// The pack version its rules date from.
    pub since: String,
    /// When it's earned: a condition (`cond::Cond`).
    pub when: serde_json::Value,
    /// Its own sound as it's announced, a path inside the pack, over the pack's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
}

/// A version as `major.minor`, compared as numbers (1.10 after 1.9).
pub fn version_key(v: &str) -> Option<(u32, u32)> {
    let (major, minor) = v.trim().split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

impl Pack {
    /// The pack in `dir` (its `achievements.json`).
    pub fn read(dir: &Path) -> Result<Pack, String> {
        let file = dir.join("achievements.json");
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        Pack::parse(&text).map_err(|e| format!("{}: {e}", file.display()))
    }

    pub fn parse(text: &str) -> Result<Pack, String> {
        let pack: Pack = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
        if pack.format != FORMAT {
            return Err(format!("not an achievement pack (its format is \"{}\")", pack.format));
        }
        Ok(pack)
    }

    /// What's wrong with it, as VPIDB checks an upload: none for a pack that's right.
    pub fn problems(&self, dir: Option<&Path>) -> Vec<String> {
        let mut out = Vec::new();
        if self.format_version != FORMAT_VERSION {
            out.push(format!("its format's draft is {}, and this reads draft {FORMAT_VERSION}", self.format_version));
        }
        if !looks_like_uuid(&self.pack.id) {
            out.push(format!("its ID, \"{}\", isn't a UUID", self.pack.id));
        }
        let Some(version) = version_key(&self.pack.version) else {
            out.push(format!("its version, \"{}\", isn't major.minor", self.pack.version));
            return out;
        };
        if self.pack.name.trim().is_empty() {
            out.push("it has no name".into());
        }
        if self.made_for.game.name.trim().is_empty() {
            out.push("it names no game".into());
        }
        let mut seen = std::collections::HashSet::new();
        for a in &self.achievements {
            let who = if a.id.is_empty() { format!("\"{}\"", a.name) } else { a.id.clone() };
            if a.id.is_empty() || a.id != a.id.to_lowercase() || a.id.chars().any(char::is_whitespace) {
                out.push(format!("{who}: its ID must be lower case, with no spaces"));
            }
            if !seen.insert(a.id.clone()) {
                out.push(format!("{who}: its ID is used twice"));
            }
            if !(1..=100).contains(&a.points) {
                out.push(format!("{who}: its points must be 1 to 100"));
            }
            match version_key(&a.since) {
                None => out.push(format!("{who}: its since, \"{}\", isn't major.minor", a.since)),
                Some(s) if s > version => out.push(format!("{who}: its since is later than the pack's version")),
                _ => {}
            }
            if let Err(e) = crate::cond::Cond::parse(&a.when) {
                out.push(format!("{who}: {e}"));
            }
            if let (Some(icon), Some(dir)) = (&a.icon, dir)
                && !icon_there(dir, icon)
            {
                out.push(format!("{who}: its picture, {icon}, isn't in the pack"));
            }
            if let Some(p) = a.sound.as_deref().and_then(|s| sound_problem(s, dir)) {
                out.push(format!("{who}: {p}"));
            }
        }
        if let (Some(icon), Some(dir)) = (&self.pack.icon, dir)
            && !icon_there(dir, icon)
        {
            out.push(format!("its picture, {icon}, isn't in the pack"));
        }
        if let Some(p) = self.pack.sound.as_deref().and_then(|s| sound_problem(s, dir)) {
            out.push(p);
        }
        out
    }

    /// An achievement's picture, else the pack's, as a file in `dir`.
    pub fn icon_of(&self, dir: &Path, achievement: &Achievement) -> Option<PathBuf> {
        achievement.icon.as_ref().or(self.pack.icon.as_ref()).map(|i| dir.join(i)).filter(|p| p.is_file())
    }

    /// An achievement's sound, else the pack's, as a file in `dir`; none: the player's own.
    pub fn sound_of(&self, dir: &Path, achievement: &Achievement) -> Option<PathBuf> {
        achievement.sound.as_ref().or(self.pack.sound.as_ref()).filter(|s| !s.contains("..")).map(|s| dir.join(s)).filter(|p| p.is_file())
    }
}

fn icon_there(dir: &Path, icon: &str) -> bool {
    !icon.contains("..") && dir.join(icon).is_file()
}

/// The most a sound may weigh, as VPIDB takes one.
pub const SOUND_MOST: u64 = 5 << 20;

/// What's wrong with a sound a pack names, as VPIDB checks it: its path, and in `dir`, its file.
fn sound_problem(sound: &str, dir: Option<&Path>) -> Option<String> {
    if let Some(p) = sound_path_problem(sound) {
        return Some(p);
    }
    let dir = dir?;
    if !icon_there(dir, sound) {
        return Some(format!("its sound, {sound}, isn't in the pack"));
    }
    sound_file_problem(&dir.join(sound)).map(|p| format!("its sound: {p}"))
}

/// What's wrong with a sound's path inside a pack, as VPIDB checks it: none for a path inside the
/// pack to a WAV, Ogg Vorbis, MP3 or FLAC file.
pub fn sound_path_problem(path: &str) -> Option<String> {
    if path.contains("..") || path.starts_with(['/', '\\']) {
        return Some(format!("its sound, {path}, must be a path inside the pack"));
    }
    let kind = Path::new(path).extension().map(|e| e.to_string_lossy().to_lowercase());
    if !matches!(kind.as_deref(), Some("wav" | "ogg" | "mp3" | "flac")) {
        return Some(format!("its sound, {path}, must be a WAV, Ogg Vorbis, MP3 or FLAC file"));
    }
    None
}

/// What's wrong with a sound's file, as VPIDB checks it: none for one of 5 MB at most whose bytes
/// are a WAV, an Ogg Vorbis (not Opus), an MP3 or a FLAC file.
pub fn sound_file_problem(file: &Path) -> Option<String> {
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Ok(size) = std::fs::metadata(file).map(|m| m.len()) else {
        return Some(format!("{name} isn't there"));
    };
    if size > SOUND_MOST {
        return Some(format!("{name} is over 5 MB: a second or two is plenty"));
    }
    let mut head = vec![0u8; 4096];
    let read = std::fs::File::open(file).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
    head.truncate(read);
    if sound_kind(&head).is_none() {
        return Some(format!("{name} isn't a WAV, Ogg Vorbis, MP3 or FLAC sound inside"));
    }
    None
}

/// A sound's kind by its first bytes: a WAV, an Ogg Vorbis (Opus isn't one), an MP3 (an ID3 tag,
/// or an MPEG layer III frame), a FLAC.
fn sound_kind(b: &[u8]) -> Option<&'static str> {
    if b.len() >= 12 && b.starts_with(b"RIFF") && &b[8..12] == b"WAVE" {
        return Some("wav");
    }
    if b.starts_with(b"fLaC") {
        return Some("flac");
    }
    if b.starts_with(b"OggS") {
        // Vorbis's identification header, in the first page
        return b.windows(7).any(|w| w == b"\x01vorbis").then_some("ogg");
    }
    if b.starts_with(b"ID3") || (b.len() >= 2 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0 && (b[1] >> 1) & 0x03 == 0x01) {
        return Some("mp3");
    }
    None
}

fn looks_like_uuid(id: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 5 && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// The example pack the tests read.
#[cfg(test)]
pub(crate) fn tests_example() -> &'static str {
    tests::EXAMPLE
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const EXAMPLE: &str = r#"{
      "format": "vpi-achievements",
      "formatVersion": 1,
      "pack": { "id": "8d6c2f0e-4c9a-4b8e-9f51-3a7d2b1c9e40", "name": "Attack from Mars", "version": "1.1", "authors": ["VPW"] },
      "for": { "game": { "name": "Attack from Mars", "maker": "Bally", "year": 1995 }, "roms": ["afm_113b"] },
      "achievements": [
        { "id": "billionaire", "name": "Billionaire", "about": "Score a billion", "points": 25, "since": "1.0", "when": { "score": { "atLeast": 1000000000 } } },
        { "id": "strobe", "name": "Strobe Light", "about": "Start Strobe Multiball", "points": 10, "secret": true, "since": "1.1", "when": { "ballsInPlay": { "atLeast": 4 } } }
      ]
    }"#;

    #[test]
    fn a_pack_read_and_checked() {
        let p = Pack::parse(EXAMPLE).unwrap();
        assert_eq!((p.pack.name.as_str(), p.achievements.len(), p.achievements[1].secret), ("Attack from Mars", 2, true));
        assert!(p.problems(None).is_empty(), "{:?}", p.problems(None));
        assert!(version_key("1.10") > version_key("1.9"));
        // what VPIDB refuses, refused here too
        let mut bad = p.clone();
        bad.pack.id = "afm".into();
        bad.achievements[0].points = 0;
        bad.achievements[1].since = "2.0".into();
        bad.achievements[1].id = "Strobe".into();
        let said = bad.problems(None).join("; ");
        assert!(said.contains("isn't a UUID") && said.contains("1 to 100") && said.contains("later than") && said.contains("lower case"), "{said}");
        assert!(Pack::parse(r#"{"format":"other","formatVersion":1,"pack":{"id":"x","name":"x","version":"1.0"},"for":{"game":{"name":"x"}},"achievements":[]}"#).is_err());
    }

    #[test]
    fn sounds_checked_as_vpidb_checks_them() {
        assert_eq!(sound_kind(b"RIFF\x24\x00\x00\x00WAVEfmt "), Some("wav"));
        assert_eq!(sound_kind(b"fLaC\x00\x00\x00\x22"), Some("flac"));
        assert_eq!(sound_kind(b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x01vorbis"), Some("ogg"));
        assert_eq!(sound_kind(b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00OpusHead"), None);
        assert_eq!(sound_kind(b"ID3\x04\x00"), Some("mp3"));
        assert_eq!(sound_kind(&[0xFF, 0xFB, 0x90, 0x64]), Some("mp3"));
        // an MPEG layer II frame isn't an MP3
        assert_eq!(sound_kind(&[0xFF, 0xFD, 0x90, 0x64]), None);
        assert_eq!(sound_kind(b"just words"), None);
        assert!(sound_path_problem("sounds/pack.WAV").is_none());
        assert!(sound_path_problem("../pack.wav").is_some() && sound_path_problem("/pack.wav").is_some());
        assert!(sound_path_problem("sounds/pack.aiff").is_some());
        let dir = std::env::temp_dir().join(format!("vpi-sound-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("words.wav"), "not a sound").unwrap();
        std::fs::write(dir.join("ok.wav"), b"RIFF\x24\x00\x00\x00WAVEfmt ").unwrap();
        assert!(sound_file_problem(&dir.join("words.wav")).is_some_and(|p| p.contains("isn't a WAV")));
        assert!(sound_file_problem(&dir.join("ok.wav")).is_none());
        assert!(sound_problem("missing.wav", Some(&dir)).is_some_and(|p| p.contains("isn't in the pack")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_packs_tools_achievements_makes_read_and_check_clean() {
        // its example and sample packs, where the repository holds them
        let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/achievements");
        for set in ["examples", "samples"] {
            let dirs = std::fs::read_dir(tools.join(set)).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("achievements.json").is_file());
            for dir in dirs {
                let pack = Pack::read(&dir).unwrap();
                let problems = pack.problems(Some(&dir));
                assert!(problems.is_empty(), "{}: {problems:?}", dir.display());
            }
        }
    }
}
