//! What a player has earned, kept in the data folder: for each pack by its ID, each achievement's
//! records — when, hardcore or practice, in which version of the pack, on which table — the
//! earliest of each mode kept; how far each count over every game has come (`progress`); and each
//! pack's masteries, a version mastered when every achievement in it has a hardcore record from a
//! version between its `since` and that version (VPIDB's rule), older masteries kept beside newer
//! ones. Sealed by Windows' data protection for the user's account (`earned.sealed` beside the
//! `earned.json` it's asked for, which is read once, the first time, and put aside), so it can't
//! be changed by hand: a file that doesn't open is set aside and the copy before it read. Ours.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::pack::{Pack, version_key};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Earned {
    /// Each pack's, by its ID.
    pub packs: BTreeMap<String, PackEarned>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PackEarned {
    /// Each achievement's records, by its ID.
    pub achievements: BTreeMap<String, Vec<Record>>,
    /// Each version mastered, and when.
    pub mastered: Vec<Mastered>,
    /// How far each count over every game has come, by achievement ID, then by the mode and the
    /// count's place in its condition (`hardcore 0.1`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub progress: BTreeMap<String, BTreeMap<String, u32>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Record {
    /// When, in UTC (`2026-11-02T21:14:05Z`).
    pub at: String,
    /// `hardcore` or `practice`.
    pub mode: String,
    pub pack_version: String,
    /// The table file it was earned on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Mastered {
    pub pack_version: String,
    pub at: String,
}

impl Earned {
    /// What's kept for `file` (`earned.json`): its sealed copy beside it, else the copy before
    /// that, else — the first time — the plain file itself.
    pub fn load(file: &Path) -> Earned {
        let sealed = file.with_extension("sealed");
        let opened = |f: &Path| std::fs::read(f).ok().and_then(|b| seal::open(&b)).and_then(|b| serde_json::from_slice(&b).ok());
        if sealed.exists() {
            if let Some(e) = opened(&sealed) {
                return e;
            }
            // changed by hand, or sealed for another account: set aside, the copy before it read
            let _ = std::fs::rename(&sealed, sealed.with_extension("sealed.unreadable"));
            return opened(&sealed.with_extension("sealed.previous")).unwrap_or_default();
        }
        std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    /// Sealed and written beside its place first, then put in it, the one before kept as the
    /// previous copy, so a failed write leaves the one before; a plain file read the first time is
    /// put aside, never read again.
    pub fn save(&self, file: &Path) -> Result<(), String> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let sealed = file.with_extension("sealed");
        let part = file.with_extension("sealed.part");
        let bytes = seal::close(&serde_json::to_vec(self).map_err(|e| e.to_string())?)?;
        std::fs::write(&part, bytes).map_err(|e| format!("{}: {e}", part.display()))?;
        if sealed.exists() {
            let _ = std::fs::copy(&sealed, sealed.with_extension("sealed.previous"));
        }
        std::fs::rename(&part, &sealed).map_err(|e| format!("{}: {e}", sealed.display()))?;
        if file.exists() {
            let _ = std::fs::rename(file, file.with_extension("json.before-sealing"));
        }
        Ok(())
    }

    /// How far a count over every game has come: `achievement`'s of `pack`, in `mode`, by place.
    pub fn progress_of(&self, pack: &str, achievement: &str, mode: &str) -> Vec<(String, u32)> {
        let prefix = format!("{mode} ");
        self.packs
            .get(pack)
            .and_then(|p| p.progress.get(achievement))
            .map(|m| m.iter().filter_map(|(k, v)| k.strip_prefix(&prefix).map(|at| (at.to_string(), *v))).collect())
            .unwrap_or_default()
    }

    /// Where the counts over every game of `achievement` stand now, in `mode`; whether that changed.
    pub fn set_progress(&mut self, pack: &str, achievement: &str, mode: &str, counts: &[(String, u32)]) -> bool {
        let mine = self.packs.entry(pack.to_string()).or_default();
        let prefix = format!("{mode} ");
        let before = mine.progress.get(achievement).cloned().unwrap_or_default();
        let mut after: BTreeMap<String, u32> = before.iter().filter(|(k, _)| !k.starts_with(&prefix)).map(|(k, v)| (k.clone(), *v)).collect();
        after.extend(counts.iter().filter(|(_, n)| *n > 0).map(|(at, n)| (format!("{prefix}{at}"), *n)));
        if after == before {
            return false;
        }
        if after.is_empty() {
            mine.progress.remove(achievement);
        } else {
            mine.progress.insert(achievement.to_string(), after);
        }
        true
    }

    /// Whether `achievement` of `pack` was earned in `mode` (`hardcore` counts for practice too).
    pub fn has(&self, pack: &str, achievement: &str, mode: &str) -> bool {
        self.packs.get(pack).and_then(|p| p.achievements.get(achievement)).is_some_and(|rs| rs.iter().any(|r| r.mode == mode || r.mode == "hardcore"))
    }

    /// A record kept, unless one of its mode is there already; whether it's new.
    pub fn add(&mut self, pack: &str, achievement: &str, record: Record) -> bool {
        let records = self.packs.entry(pack.to_string()).or_default().achievements.entry(achievement.to_string()).or_default();
        if records.iter().any(|r| r.mode == record.mode) {
            return false;
        }
        records.push(record);
        true
    }

    /// The version of `pack` mastered now, and when — the latest of the records that master it —
    /// if every achievement in it has a hardcore record from a version between its `since` and the
    /// pack's.
    pub fn masters(&self, pack: &Pack) -> Option<Mastered> {
        let version = version_key(&pack.pack.version)?;
        let mine = self.packs.get(&pack.pack.id)?;
        let mut latest = String::new();
        for a in &pack.achievements {
            let since = version_key(&a.since)?;
            let first = mine
                .achievements
                .get(&a.id)?
                .iter()
                .filter(|r| r.mode == "hardcore" && version_key(&r.pack_version).is_some_and(|v| v >= since && v <= version))
                .map(|r| r.at.clone())
                .min()?;
            latest = latest.max(first);
        }
        (!pack.achievements.is_empty()).then(|| Mastered { pack_version: pack.pack.version.clone(), at: latest })
    }

    /// A mastery noted where it's new; whether it was.
    pub fn note_mastery(&mut self, pack: &Pack) -> bool {
        let Some(m) = self.masters(pack) else { return false };
        let mine = self.packs.entry(pack.pack.id.clone()).or_default();
        if mine.mastered.iter().any(|x| x.pack_version == m.pack_version) {
            return false;
        }
        mine.mastered.push(m);
        true
    }
}

/// Now, in UTC, as the records keep times: `2026-11-02T21:14:05Z`.
pub fn now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    utc(secs)
}

/// Seconds since 1970 as a UTC time (Howard Hinnant's days-to-civil).
pub fn utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// Sealing with Windows' data protection (`CryptProtectData`), for the user's account alone, with
/// words of our own beside it: a file changed by hand, or copied from another account, doesn't
/// open. Elsewhere (the tests' other systems) the bytes pass as they are.
mod seal {
    const WORDS: &[u8] = b"Visual Pinball Infinite: what's earned";

    #[cfg(windows)]
    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }

    #[cfg(windows)]
    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(input: *const Blob, name: *const u16, entropy: *const Blob, reserved: *mut core::ffi::c_void, prompt: *mut core::ffi::c_void, flags: u32, out: *mut Blob) -> i32;
        fn CryptUnprotectData(input: *const Blob, name: *mut *mut u16, entropy: *const Blob, reserved: *mut core::ffi::c_void, prompt: *mut core::ffi::c_void, flags: u32, out: *mut Blob) -> i32;
    }

    #[cfg(windows)]
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    #[cfg(windows)]
    const UI_FORBIDDEN: u32 = 0x1;

    #[cfg(windows)]
    fn run(bytes: &[u8], close: bool) -> Option<Vec<u8>> {
        let input = Blob { len: bytes.len() as u32, data: bytes.as_ptr() as *mut u8 };
        let entropy = Blob { len: WORDS.len() as u32, data: WORDS.as_ptr() as *mut u8 };
        let mut out = Blob { len: 0, data: std::ptr::null_mut() };
        // SAFETY: the blobs point at live bytes of the lengths given; Windows' answer is copied
        // out and its memory freed
        unsafe {
            let ok = if close {
                CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null_mut(), std::ptr::null_mut(), UI_FORBIDDEN, &mut out)
            } else {
                CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null_mut(), std::ptr::null_mut(), UI_FORBIDDEN, &mut out)
            };
            if ok == 0 || out.data.is_null() {
                return None;
            }
            let got = std::slice::from_raw_parts(out.data, out.len as usize).to_vec();
            LocalFree(out.data.cast());
            Some(got)
        }
    }

    /// The bytes sealed.
    pub fn close(bytes: &[u8]) -> Result<Vec<u8>, String> {
        #[cfg(windows)]
        return run(bytes, true).ok_or_else(|| "Windows' data protection didn't seal what's earned".to_string());
        #[cfg(not(windows))]
        Ok(bytes.to_vec())
    }

    /// Sealed bytes opened, None when they don't open (changed, or another account's).
    pub fn open(bytes: &[u8]) -> Option<Vec<u8>> {
        #[cfg(windows)]
        return run(bytes, false);
        #[cfg(not(windows))]
        Some(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_sealed_a_hand_made_change_refused_and_the_plain_file_read_once() {
        let dir = std::env::temp_dir().join(format!("vpi-earned-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("earned.json");
        let mut first = Earned::default();
        first.add("p", "a", rec("2026-11-02T21:14:05Z", "hardcore", "1.0"));
        std::fs::write(&file, serde_json::to_vec(&first).unwrap()).unwrap();
        // the plain file read the first time, then sealed and put aside
        let mut e = Earned::load(&file);
        assert_eq!(e, first);
        e.add("p", "b", rec("2026-11-03T21:14:05Z", "hardcore", "1.0"));
        e.save(&file).unwrap();
        assert!(!file.exists() && dir.join("earned.json.before-sealing").exists());
        assert_eq!(Earned::load(&file), e);
        let sealed = std::fs::read(dir.join("earned.sealed")).unwrap();
        assert!(!sealed.windows(5).any(|w| w == b"\"at\":"), "not readable as it is");
        // saved again: the one before kept; then the newest changed by hand: refused, set aside,
        // the one before read
        let mut later = e.clone();
        later.add("p", "c", rec("2026-11-04T21:14:05Z", "hardcore", "1.0"));
        later.save(&file).unwrap();
        let mut bytes = std::fs::read(dir.join("earned.sealed")).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x5A;
        std::fs::write(dir.join("earned.sealed"), bytes).unwrap();
        assert_eq!(Earned::load(&file), e);
        assert!(dir.join("earned.sealed.unreadable").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progress_over_every_game_kept_by_mode() {
        let mut e = Earned::default();
        assert!(e.set_progress("p", "a", "hardcore", &[("0".into(), 2)]));
        assert!(!e.set_progress("p", "a", "hardcore", &[("0".into(), 2)]));
        assert!(e.set_progress("p", "a", "practice", &[("0".into(), 5)]));
        assert_eq!(e.progress_of("p", "a", "hardcore"), [("0".to_string(), 2)]);
        assert_eq!(e.progress_of("p", "a", "practice"), [("0".to_string(), 5)]);
        assert!(e.set_progress("p", "a", "hardcore", &[("0".into(), 0)]));
        assert!(e.progress_of("p", "a", "hardcore").is_empty());
    }

    fn rec(at: &str, mode: &str, v: &str) -> Record {
        Record { at: at.into(), mode: mode.into(), pack_version: v.into(), table: None }
    }

    #[test]
    fn mastered_by_hardcore_records_from_each_ones_since_on() {
        let mut pack = Pack::parse(crate::pack::tests_example()).unwrap();
        let id = pack.pack.id.clone();
        let mut e = Earned::default();
        assert!(e.add(&id, "billionaire", rec("2026-11-02T21:14:05Z", "practice", "1.0")));
        assert!(!e.add(&id, "billionaire", rec("2026-11-03T00:00:00Z", "practice", "1.0")));
        assert!(e.add(&id, "billionaire", rec("2026-11-04T10:00:00Z", "hardcore", "1.0")));
        // strobe came in 1.1: earned in 1.0 it wouldn't count, but there it couldn't be
        assert_eq!(e.masters(&pack), None);
        assert!(e.add(&id, "strobe", rec("2026-11-05T09:00:00Z", "hardcore", "1.1")));
        assert_eq!(e.masters(&pack), Some(Mastered { pack_version: "1.1".into(), at: "2026-11-05T09:00:00Z".into() }));
        assert!(e.note_mastery(&pack) && !e.note_mastery(&pack));
        // a later version changing billionaire's rules: earned again before it's mastered
        pack.pack.version = "2.0".into();
        pack.achievements[0].since = "2.0".into();
        assert_eq!(e.masters(&pack), None);
        assert_eq!(e.packs[&id].mastered.len(), 1, "the older mastery kept");
        assert!(e.has(&id, "billionaire", "practice") && e.has(&id, "billionaire", "hardcore"));
    }

    #[test]
    fn times_written_in_utc() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_793_049_245), "2026-10-26T21:14:05Z");
    }
}
