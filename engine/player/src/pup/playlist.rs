// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its PinUp Player
// plugin's playlists (plugins/pup/PUPPlaylist.cpp), with the newer fixes matching PinUP Player (to commit
// a4a1e75ca: 108a65d69's defaults, 27a463451's shuffle).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A pack's playlist: a folder of media, played in order — shuffled once as
//! it's read, where it says so, as PinUP Player does — with a rest time
//! between plays (`playlists.pup`:
//! `ScreenNum,Folder,Des,AlphaSort,RestSeconds,Volume,Priority`; an empty
//! rest time is 30 seconds, an empty volume 100).

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};

use super::{find_case_insensitive, parse_csv_line, string_to_int};

/// What a playlist's folder is for, by its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    /// Media to play.
    Default,
    /// `PuPOverlays`: a picture over the screen.
    Overlays,
    /// `PuPFrames`: a picture behind it.
    Frames,
    /// `PuPAlphas`: a picture over it, as overlays.
    Alphas,
    /// `PuPShapes`: a mask for what the screen shows.
    Shapes,
}

#[derive(Clone, Debug)]
pub struct Playlist {
    /// The folder's name, as on disk.
    pub folder: String,
    pub description: String,
    pub randomize: bool,
    pub rest_seconds: i32,
    pub volume: f32,
    pub priority: i32,
    pub function: Function,
    /// The folder, or None where it isn't there.
    pub base: Option<PathBuf>,
    /// Its own files (not those in folders inside it), sorted, or shuffled once: what it plays in turn.
    pub files: Vec<String>,
    /// Every file, in folders inside it too, by its path in lower case: what can be asked for by name.
    file_map: HashMap<String, String>,
    last_index: usize,
    /// When it last played (the game's milliseconds).
    last_played: Option<u64>,
}

impl Playlist {
    /// A playlist of the folder `folder` in the pack at `pack`.
    pub fn new(pack: &Path, folder: &str, description: &str, randomize: bool, rest_seconds: i32, volume: f32, priority: i32) -> Playlist {
        let function = match folder.to_ascii_lowercase().as_str() {
            "pupoverlays" => Function::Overlays,
            "pupframes" => Function::Frames,
            "pupalphas" => Function::Alphas,
            "pupshapes" => Function::Shapes,
            _ => Function::Default,
        };
        let base = find_case_insensitive(&pack.join(folder), true);
        let mut files = Vec::new();
        let mut file_map = HashMap::new();
        if let Some(base) = &base {
            let mut stack = vec![PathBuf::new()];
            while let Some(rel) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(base.join(&rel)) else { continue };
                for e in entries.flatten() {
                    let name = rel.join(e.file_name());
                    let Ok(kind) = e.file_type() else { continue };
                    if kind.is_dir() {
                        stack.push(name);
                    } else if kind.is_file() {
                        let text = name.to_string_lossy().replace('/', "\\");
                        if rel.as_os_str().is_empty() {
                            files.push(text.clone());
                        }
                        file_map.insert(text.to_lowercase(), text);
                    }
                }
            }
            files.sort();
            if randomize {
                shuffle(&mut files);
            }
        }
        Playlist {
            folder: folder.to_string(),
            description: description.to_string(),
            randomize,
            rest_seconds,
            volume,
            priority,
            function,
            base,
            files,
            file_map,
            last_index: 0,
            last_played: None,
        }
    }

    /// A line of `playlists.pup` (VPX's `CreateFromCSV`); None for one that doesn't read or whose
    /// folder isn't there.
    pub fn from_csv(pack: &Path, line: &str) -> Option<Playlist> {
        let parts = parse_csv_line(line);
        if parts.len() != 7 {
            return None;
        }
        let folder = find_case_insensitive(&pack.join(&parts[1]), true)?;
        let name = folder.file_name()?.to_string_lossy().into_owned();
        Some(Playlist::new(
            pack,
            &name,
            &parts[2],
            string_to_int(&parts[3], 0) == 1,
            string_to_int(&parts[4], 30),
            string_to_int(&parts[5], 100) as f32,
            string_to_int(&parts[6], 0),
        ))
    }

    /// A file asked for by name, as it is on disk; None where the playlist has no such file.
    pub fn play_file(&self, name: &str) -> Option<String> {
        self.file_map.get(&name.replace('/', "\\").to_lowercase()).cloned()
    }

    /// The next file in turn, round and round (in the shuffled order where it was shuffled).
    fn next_file(&mut self) -> Option<String> {
        if self.files.is_empty() {
            return None;
        }
        let file = self.files[self.last_index].clone();
        self.last_index = (self.last_index + 1) % self.files.len();
        Some(file)
    }

    /// The file to play: the one named, or the next in turn when none is (VPX's `GetPlayFilePath`).
    pub fn play_file_path(&mut self, name: &str) -> Option<PathBuf> {
        let base = self.base.clone()?;
        if !name.is_empty() {
            return self.play_file(name).map(|f| base.join(f));
        }
        self.next_file().map(|f| base.join(f))
    }

    /// Resting: played less than its rest seconds ago.
    pub fn is_resting(&self, now: u64) -> bool {
        self.rest_seconds > 0 && self.last_played.is_some_and(|t| now.saturating_sub(t) < self.rest_seconds as u64 * 1000)
    }

    pub fn mark_played(&mut self, now: u64) {
        self.last_played = Some(now);
    }
}

/// A new order for a shuffled playlist, different on each run (VPX shuffles with a Mersenne
/// Twister seeded by `std::random_device`; ours is a small generator seeded by the standard
/// library's random hash keys).
fn shuffle(files: &mut [String]) {
    let mut seed = std::collections::hash_map::RandomState::new().build_hasher().finish() | 1;
    for i in (1..files.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        files.swap(i, (seed % (i as u64 + 1)) as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shuffled playlist plays every file once before any again, in the order it was shuffled
    /// to; an empty rest time is 30 seconds and an empty volume 100 (PinUP Player's defaults).
    #[test]
    fn a_shuffled_playlist_plays_each_file_once_a_round() {
        let dir = std::env::temp_dir().join(format!("vpi-pup-playlist-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("clips")).expect("a folder");
        for n in 0..6 {
            std::fs::write(dir.join("clips").join(format!("{n}.mp4")), b"x").expect("a file");
        }
        let mut p = Playlist::from_csv(&dir, "2,clips,,1,,,0").expect("a playlist");
        assert_eq!((p.rest_seconds, p.volume), (30, 100.0));
        let order: Vec<String> = p.files.clone();
        let round: Vec<String> = (0..6).filter_map(|_| p.next_file()).collect();
        assert_eq!(round, order);
        let mut sorted = round.clone();
        sorted.sort();
        assert_eq!(sorted, ["0.mp4", "1.mp4", "2.mp4", "3.mp4", "4.mp4", "5.mp4"]);
        assert_eq!(p.next_file().as_deref(), Some(order[0].as_str()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
