//! An AltSound pack checked as libaltsound reads it: its format told by the file it holds, as its
//! `get_altsound_format` tells it — `g-sound.csv`, else `altsound.csv`, else PinSound's folders
//! (`jingle`, `music`, `sfx`, `voice`, `single`) — and, for the two lists, each sound they name
//! looked for in the pack (`FNAME`, relative to the pack's folder, as its CSV parsers join it).
//! The verdict in words, and how it looks: good, warn or bad. Ours.

use std::path::Path;

/// PinSound's folders, which a pack with no list of its sounds holds them in.
const PINSOUND: [&str; 5] = ["jingle", "music", "sfx", "voice", "single"];

/// The pack at `pack` checked: how it looks, and what it says.
pub fn check(pack: &Path) -> (&'static str, String) {
    // each list, with where its FNAME sits when its header doesn't name it (libaltsound's own order)
    for (file, column) in [("g-sound.csv", 4), ("altsound.csv", 7)] {
        let Ok(bytes) = std::fs::read(pack.join(file)) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let mut lines = text.lines();
        let header = lines.next().unwrap_or_default();
        let column = header.split(',').position(|h| h.trim().trim_matches('"').eq_ignore_ascii_case("fname")).unwrap_or(column);
        let (mut named, mut missing) = (0usize, 0usize);
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let Some(field) = line.split(',').nth(column) else { continue };
            let name = field.trim().trim_matches('"').replace('\\', "/");
            if name.is_empty() {
                continue;
            }
            named += 1;
            if !pack.join(&name).is_file() {
                missing += 1;
            }
        }
        // a few sounds a list names and the pack lacks are common, the pack's own slip (libaltsound
        // looks for them as named, so they don't play anywhere): said, not counted against it
        return match (named, missing) {
            (0, _) => ("warn", format!("Its AltSound pack's list ({file}) names no sounds, so it won't play anything.")),
            (n, 0) => ("good", format!("All {n} of its sounds are there")),
            (n, m) if m * 2 < n => ("good", format!("{} of its {n} sounds are there; the pack itself lacks {m}", n - m)),
            (n, m) => ("bad", format!("Its AltSound pack is missing {m} of the {n} sounds its list names, so most won't play: it may be incomplete, or for another version.")),
        };
    }
    let sounds: usize = PINSOUND.iter().filter_map(|f| std::fs::read_dir(pack.join(f)).ok()).map(|entries| entries.flatten().filter(|e| e.path().is_file()).count()).sum();
    if PINSOUND.iter().any(|f| pack.join(f).is_dir()) {
        return if sounds == 0 { ("warn", "Its AltSound pack's folders hold no sounds, so it won't play anything.".into()) } else { ("good", format!("{sounds} sounds in PinSound's folders")) };
    }
    ("warn", "Its AltSound pack has no list of its sounds (altsound.csv or g-sound.csv) and none of PinSound's folders, so it won't play.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packs_sounds_looked_for_as_its_list_names_them() {
        let dir = std::env::temp_dir().join(format!("vpi-altsound-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("jingle")).unwrap();
        std::fs::write(dir.join("jingle").join("a.ogg"), b"x").unwrap();
        std::fs::write(dir.join("altsound.csv"), "ID,CHANNEL,DUCK,GAIN,LOOP,STOP,NAME,FNAME\n0x0001,0,100,100,0,0,a,\"jingle\\a.ogg\"\n0x0002,0,100,100,0,0,b,jingle/b.ogg\n").unwrap();
        assert_eq!(check(&dir).0, "bad");
        assert!(check(&dir).1.contains("missing 1 of the 2"), "{}", check(&dir).1);
        std::fs::write(dir.join("jingle").join("b.ogg"), b"x").unwrap();
        assert_eq!(check(&dir), ("good", "All 2 of its sounds are there".into()));
        // no list: PinSound's folders
        std::fs::remove_file(dir.join("altsound.csv")).unwrap();
        assert_eq!(check(&dir), ("good", "2 sounds in PinSound's folders".into()));
        std::fs::remove_dir_all(dir.join("jingle")).unwrap();
        assert_eq!(check(&dir).0, "warn");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
