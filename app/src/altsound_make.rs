//! AltSound packs made from a table's own ROM: rom2altsound (BSD-3-Clause, by Sylvain Gargasson)
//! plays every sound command of the machine's sound board through the PinMAME built into it and
//! writes each sound as a file, into a pack AltSound plays. The program sits beside the app, else
//! it's the repository's build while developing (`tools\rom2altsound\setup.ps1`). Whether a pack
//! made so would play is told by the machine's sound board, read from PinMAME's table as
//! rom2altsound dumps it, against rom2altsound's measure of the boards whose sound commands reach
//! AltSound as the pack keys them (its `docs/vpx_playback.json`, 0.4.0).
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};

/// The program, where it is.
fn exe() -> Option<PathBuf> {
    let here = std::env::current_exe().ok();
    let mut places = Vec::new();
    if let Some(dir) = here.as_deref().and_then(Path::parent) {
        places.push(dir.join("rom2altsound.exe"));
    }
    if let Some(p) = std::env::var_os("VPI_ROM2ALTSOUND") {
        places.push(PathBuf::from(p));
    }
    // the repository's build, the app being in target\<profile> (a release build, compiled with
    // VPI_RELEASE_BUILD set, looks beside itself alone)
    if option_env!("VPI_RELEASE_BUILD").is_none()
        && let Some(repo) = here.as_deref().and_then(|e| e.ancestors().nth(3))
    {
        places.push(repo.join(r"vpx-build\rom2altsound\target\release\rom2altsound.exe"));
    }
    places.into_iter().find(|p| p.is_file())
}

/// The program started with no console window, below normal priority (it plays the machine's
/// sound board as fast as the processor goes).
fn command(exe: &Path) -> Command {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    let mut command = Command::new(exe);
    command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    command
}

/// Each PinMAME set's sound board by set name, `SNDBRD_` left off, with the program it was read
/// for (its size and date): read once while the app runs.
static BOARDS: Mutex<Option<(String, HashMap<String, String>)>> = Mutex::new(None);

#[derive(serde::Deserialize)]
struct Set {
    name: String,
    board: Option<String>,
}

fn board_of(exe: &Path, set: &str) -> Result<Option<String>, String> {
    let stamp = std::fs::metadata(exe).map(|m| format!("{} {:?}", m.len(), m.modified().ok())).map_err(|e| e.to_string())?;
    let mut known = BOARDS.lock().unwrap_or_else(|e| e.into_inner());
    if known.as_ref().is_none_or(|(s, _)| *s != stamp) {
        *known = Some((stamp.clone(), read_boards(exe, &stamp)?));
    }
    Ok(known.as_ref().and_then(|(_, boards)| boards.get(&set.to_lowercase()).cloned()))
}

/// PinMAME's table as rom2altsound dumps it (`roms --dump-table`), kept in the data folder with the
/// program's stamp and dumped again when the program changes.
fn read_boards(exe: &Path, stamp: &str) -> Result<HashMap<String, String>, String> {
    let dir = crate::library::data_folder().ok_or("The app's data folder isn't there.")?.join("AltSound");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let table = dir.join("pinmame-table.json");
    let stamp_file = dir.join("pinmame-table.stamp");
    if !table.is_file() || std::fs::read_to_string(&stamp_file).ok().as_deref() != Some(stamp) {
        // a folder with no ROMs to check: only the table wanted
        let empty = dir.join("no-roms");
        std::fs::create_dir_all(&empty).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&table);
        let _ = command(exe).arg("roms").arg(&empty).arg("--dump-table").arg(&table).arg("-q").stdout(Stdio::null()).stderr(Stdio::null()).status();
        if !table.is_file() {
            return Err("rom2altsound couldn't list PinMAME's machines.".into());
        }
        let _ = std::fs::write(&stamp_file, stamp);
    }
    let text = std::fs::read(&table).map_err(|e| e.to_string())?;
    let sets: Vec<Set> = serde_json::from_slice(&text).map_err(|e| format!("PinMAME's machines couldn't be read: {e}"))?;
    Ok(sets.into_iter().filter_map(|s| Some((s.name.to_lowercase(), s.board?.trim_start_matches("SNDBRD_").to_string()))).collect())
}

/// Why a pack made from the ROM of a machine with this sound board wouldn't play, as rom2altsound
/// measured it: none for the boards whose packs play. The boards it lists as playing only in VPX's
/// builds newer than 10.8.1-5436 play here, as our PinMAME and libaltsound carry those fixes.
fn why_not(board: &str) -> Option<&'static str> {
    Some(match board {
        "NONE" => "This machine has no sound board for a pack to follow: its own processor makes its sounds, as on Stern’s SAM machines, or it rings chimes.",
        "DCSP2K" => "Pinball 2000’s sounds don’t go through the sound commands a pack follows, so a pack can’t play yet.",
        "ASTRO" | "ATARI1" | "ATARI2" | "ROMSTAR" | "ST300" | "ST300V" => "Its game makes its sounds itself, with no sound commands for a pack to follow.",
        "CAPCOMS" | "MRGAME" | "PLAYZ" | "ZAC1311" | "SPINB" => "Its game reaches the sound board without the sound commands a pack follows.",
        "BY45BP" | "GPMSU3" | "S3S" | "S67S" | "TABART" | "HANKIN" | "PLAY3" | "S3DFS" | "S3WCS" | "TABART2" | "TAITO_SINTETIZADOR" | "TECHNO" => {
            "What its game sends the sound board isn’t what a pack made from its ROM is keyed by, so the pack wouldn’t play right."
        }
        _ => return None,
    })
}

/// Whether a pack can be made from a table's ROM here, and if not, why.
#[derive(serde::Serialize)]
pub struct Offer {
    can: bool,
    rom: String,
    why: Option<String>,
}

pub fn offer(rom_set: Option<&str>, zip: Option<&Path>) -> Offer {
    let rom = rom_set.unwrap_or_default().to_string();
    let no = |why: &str| Offer { can: false, rom: rom.clone(), why: Some(why.to_string()) };
    if rom.is_empty() {
        return no("Its script names no ROM.");
    }
    if zip.is_none() {
        return no("Its ROM isn’t there yet.");
    }
    let Some(exe) = exe() else { return no("rom2altsound, which makes them, isn’t installed beside the app.") };
    match board_of(&exe, &rom) {
        Ok(Some(board)) => match why_not(&board) {
            Some(why) => no(why),
            None => Offer { can: true, rom, why: None },
        },
        // a set PinMAME's table doesn't name, or the table unread: rom2altsound itself will say
        _ => Offer { can: true, rom, why: None },
    }
}

static TURN: Mutex<()> = Mutex::new(());

/// One pack made at a time.
pub fn turn() -> MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

/// How far making a pack has come, as rom2altsound tells it.
pub enum Progress {
    /// Its machine started, its factory settings written first.
    Starting,
    /// The sound commands recorded of how many, the first time through.
    Recording(u32, u32),
    /// Recordings played again and checked.
    Checking,
    /// The pack written.
    Writing,
}

/// The pack made from the ROM set `rom`, its zip in `roms`, and put in VPinMAME's `altsound` folder
/// as `<rom>`, never over a pack that's there: where it went.
pub fn make(rom: &str, roms: &Path, altsound: &Path, progress: &dyn Fn(Progress)) -> Result<PathBuf, String> {
    let exe = exe().ok_or("rom2altsound, which makes AltSound packs, isn’t installed beside the app.")?;
    let target = altsound.join(rom);
    if target.exists() {
        return Err(format!("{rom} has an AltSound pack already."));
    }
    let work = crate::library::data_folder().ok_or("The app's data folder isn't there.")?.join("AltSound").join("making");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let mut child = command(&exe)
        .arg(rom)
        .arg("--roms")
        .arg(roms)
        .arg("--out")
        .arg(&work)
        .args(["--jobs", "1", "--progress", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("rom2altsound didn't start: {e}"))?;
    // its progress, one JSON object a line, the last for the ROM its outcome
    let mut outcome: Option<Result<(), String>> = None;
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
            let count = |key: &str| v[key].as_u64().unwrap_or(0) as u32;
            match v["stage"].as_str() {
                Some("cold-boot" | "boot") => progress(Progress::Starting),
                Some("record") if v["pass"].as_str() == Some("main") => progress(Progress::Recording(count("done"), count("total"))),
                Some("record") => progress(Progress::Checking),
                Some("pack") => progress(Progress::Writing),
                Some("done") if v.get("rom").is_some() => {
                    outcome = Some(match v["status"].as_str() {
                        Some("ok") => Ok(()),
                        Some("not_applicable") => Err(v["reason"].as_str().unwrap_or("rom2altsound can’t make a pack for this machine.").to_string()),
                        _ => Err(v["error"].as_str().unwrap_or("rom2altsound couldn’t make the pack.").to_string()),
                    })
                }
                _ => {}
            }
        }
    }
    let _ = child.wait();
    outcome.unwrap_or_else(|| Err("rom2altsound stopped before it finished.".into()))?;
    let made = work.join(rom);
    if !made.is_dir() {
        return Err("rom2altsound finished, but its pack isn’t there.".into());
    }
    std::fs::create_dir_all(altsound).map_err(|e| e.to_string())?;
    // moved in whole: a rename on the same drive, else copied and the copy here removed
    if std::fs::rename(&made, &target).is_err() {
        copy_folder(&made, &target).map_err(|e| format!("The pack couldn’t be put in {}: {e}", altsound.display()))?;
    }
    let _ = std::fs::remove_dir_all(&work);
    Ok(target)
}

fn copy_folder(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            copy_folder(&path, &to.join(entry.file_name()))?;
        } else {
            std::fs::copy(&path, to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Whether a pack was made from its ROM by rom2altsound, which names itself atop its settings file.
pub fn made_from_rom(pack: &Path) -> bool {
    std::fs::read(pack.join("altsound.ini")).is_ok_and(|b| String::from_utf8_lossy(&b[..b.len().min(200)]).contains("rom2altsound"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Xenon's pack made from its ROM and put in a scratch `altsound` folder, as the app makes one;
    /// a second time refused, the pack there already. Needs rom2altsound and a folder of ROMs holding
    /// `xenon.zip` (`VPI_ROMS`): `cargo test -p app --release altsound_make -- --ignored`.
    #[test]
    #[ignore]
    fn a_pack_is_made_and_put_in_place() {
        let roms = PathBuf::from(std::env::var_os("VPI_ROMS").expect("VPI_ROMS: a folder of ROMs"));
        let scratch = std::env::temp_dir().join(format!("vpi-altsound-make-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        // the app's data folder in the scratch folder too, and the repository's build of the program
        // (a test runs from target\release\deps, a folder deeper than the app)
        unsafe {
            std::env::set_var("APPDATA", scratch.join("appdata"));
            std::env::set_var("VPI_ROM2ALTSOUND", Path::new(env!("CARGO_MANIFEST_DIR")).join(r"..\vpx-build\rom2altsound\target\release\rom2altsound.exe"));
        }
        let altsound = scratch.join("altsound");
        let seen = Mutex::new(Vec::new());
        let made = make("xenon", &roms, &altsound, &|p| {
            seen.lock().unwrap().push(match p {
                Progress::Starting => "starting",
                Progress::Recording(..) => "recording",
                Progress::Checking => "checking",
                Progress::Writing => "writing",
            })
        })
        .expect("the pack");
        assert_eq!(made, altsound.join("xenon"));
        assert!(made.join("altsound.csv").is_file() && made_from_rom(&made));
        assert!(seen.lock().unwrap().contains(&"recording"));
        assert!(make("xenon", &roms, &altsound, &|_| {}).is_err());
        assert!(offer(Some("xenon"), Some(&roms.join("xenon.zip"))).can);
        assert!(!offer(Some("acd_170h"), Some(&roms.join("acd_170h.zip"))).can);
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
