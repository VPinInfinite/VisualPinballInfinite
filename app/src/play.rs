//! PLAY: the table started in our player (`vpi-player`, beside the app) with nothing but the table,
//! so the settings file places its windows, as a front end starts VPX; the app minimised once the
//! player's window shows (so the screen never stands empty between them) — but for cabinet mode,
//! which stays behind it — and back, its page focused, when the player ends. The player's own output goes to `Logs\player.log` in the
//! player's data folder, the run before kept as `player.previous.log`. A table that has never
//! played here starts from what VPX keeps for it — its machine's memory or its script's saved
//! values, high scores and all — where VPX keeps any (`scores::start_from_vpx`). A PinUP pack's
//! layout tried before it's chosen starts the table the same way, the layout's files read in the
//! pack's own place (`VPI_PUP_LAYOUT`), the run kept out of its plays.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{self, Shared};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Ended {
    id: String,
    name: String,
    /// The player's exit code; None when it was stopped.
    code: Option<i32>,
    log: Option<PathBuf>,
}

fn player() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let player = exe.with_file_name("vpi-player.exe");
    if player.exists() { Ok(player) } else { Err(format!("The player isn't built yet: {}", player.display())) }
}

fn log_file() -> Option<PathBuf> {
    let dir = library::data_folder()?.join("Logs");
    std::fs::create_dir_all(&dir).ok()?;
    let log = dir.join("player.log");
    if log.exists() {
        let _ = std::fs::rename(&log, dir.join("player.previous.log"));
    }
    Some(log)
}

/// The scores the player said the script last showed on its backglass as play ended (its `scores at
/// the end: 1=…, 2=…` line), each player's in order.
fn said_scores(log: &std::path::Path) -> Option<Vec<u64>> {
    let text = std::fs::read(log).ok()?;
    let text = String::from_utf8_lossy(&text);
    let line = text.lines().rev().find_map(|l| l.strip_prefix("scores at the end: "))?;
    let mut scores: Vec<(i32, u64)> = line
        .split(", ")
        .filter_map(|p| {
            let (player, score) = p.split_once('=')?;
            Some((player.trim().parse().ok()?, score.trim().parse::<i64>().ok()?.max(0) as u64))
        })
        .collect();
    scores.sort_unstable();
    let mut scores: Vec<u64> = scores.into_iter().map(|(_, s)| s).collect();
    // the places of players who didn't play
    scores.truncate(scores.iter().rposition(|&s| s > 0)? + 1);
    Some(scores)
}

/// PLAY: `asked`, as it stands, or Play Hardcore — the table as its author made it, what the
/// player's settings change of how it plays left out for this game (`VPI_HARDCORE`) — or Play
/// Softcore, its achievements kept as softcore whatever it plays as.
pub fn play(app: &AppHandle, id: &str, preset: Option<&str>, stay: bool, asked: crate::achievements::Asked) -> Result<(), String> {
    start(app, id, preset, None, None, None, stay, asked)
}

/// The table started to try a layout of its PinUP pack, written down in `trying` for the player:
/// as PLAY starts it, not counted as a play.
pub fn try_layout(app: &AppHandle, id: &str, trying: PathBuf) -> Result<(), String> {
    start(app, id, None, Some(trying), None, None, false, crate::achievements::Asked::AsItStands)
}

/// The table played with its score display's frames recorded for colouring it, into `dump`
/// (`VPI_FRAME_DUMP`): as PLAY starts it, counted as a play.
pub fn play_recording(app: &AppHandle, id: &str, dump: PathBuf) -> Result<(), String> {
    start(app, id, None, None, Some(dump), None, false, crate::achievements::Asked::AsItStands)
}

/// The table started to try an achievement pack being made, the file handed to the player in
/// `asked` (`VPI_ACHIEVEMENTS`) in place of its own packs': as PLAY starts it, not counted as a
/// play.
pub fn try_achievements(app: &AppHandle, id: &str, asked: PathBuf) -> Result<(), String> {
    start(app, id, None, None, None, Some(asked), false, crate::achievements::Asked::AsItStands)
}

/// Whether the process `pid` shows a window yet: a visible top-level window of its own.
#[cfg(windows)]
fn shows_window(pid: u32) -> bool {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(each: unsafe extern "system" fn(*mut c_void, isize) -> i32, data: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: *mut c_void) -> i32;
    }
    unsafe extern "system" fn each(hwnd: *mut c_void, data: isize) -> i32 {
        // SAFETY: `data` is the `(pid, found)` pair below, alive through EnumWindows
        let seek = unsafe { &mut *(data as *mut (u32, bool)) };
        let mut pid = 0;
        // SAFETY: a window Windows handed over
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid == seek.0 && unsafe { IsWindowVisible(hwnd) } != 0 {
            seek.1 = true;
            return 0;
        }
        1
    }
    let mut seek = (pid, false);
    // SAFETY: the callback reads only the pair, which outlives the call
    unsafe { EnumWindows(each, &mut seek as *mut (u32, bool) as isize) };
    seek.1
}

#[cfg(not(windows))]
fn shows_window(_pid: u32) -> bool {
    true
}

#[allow(clippy::too_many_arguments)]
fn start(app: &AppHandle, id: &str, preset: Option<&str>, trying: Option<PathBuf>, dump: Option<PathBuf>, tried_pack: Option<PathBuf>, stay: bool, asked: crate::achievements::Asked) -> Result<(), String> {
    let hardcore = asked == crate::achievements::Asked::Hardcore;
    let shared = app.state::<Shared>();
    let counted = trying.is_none() && tried_pack.is_none();
    let (path, name, at, rom, keeps, achievements) = {
        let mut st = shared.0.lock().unwrap();
        if st.playing.is_some() {
            return Err("A table is playing already.".into());
        }
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        let (path, name, rom) = (t.path.clone(), t.stem.clone(), t.holds.rom_set.clone());
        // its achievement packs handed to the player, but while a PinUP layout's only tried; a
        // pack being made in their place while it's tried
        let achievements = match &tried_pack {
            Some(asked) => Some(asked.clone()),
            None if trying.is_none() => crate::achievements::for_play(&st, t, asked),
            None => None,
        };
        let saved_under = t.read.saved_under.clone();
        let layout = st.layout.clone();
        st.playing = Some(id.to_string());
        let at = if counted { library::record_play(&mut st, &path) } else { 0 };
        (path, name, at, rom, layout.map(|l| (l, saved_under)), achievements)
    };
    // VPX's high scores to carry on from, the first time it plays here
    if let Some((layout, saved_under)) = &keeps
        && let Err(e) = ::library::scores::start_from_vpx(layout, rom.as_deref(), saved_under)
    {
        eprintln!("VPX's copy of what {name} keeps couldn't be taken: {e}");
    }
    let layout = keeps.map(|(l, _)| l);
    let started = (|| -> Result<std::process::Child, String> {
        let player = player()?;
        let log = log_file();
        let mut command = Command::new(&player);
        command.arg(&path).stdin(Stdio::null());
        // the screens a preset gives, over the settings' own (cabinet mode's)
        if let Some(p) = preset.filter(|p| !p.trim().is_empty()) {
            command.env("VPI_SCREEN_PRESET", p);
        }
        // a PinUP layout tried: its files in the pack's own place
        if let Some(t) = &trying {
            command.env("VPI_PUP_LAYOUT", t);
        }
        // the score display's frames recorded, for colouring it
        if let Some(d) = &dump {
            command.env("VPI_FRAME_DUMP", d);
        }
        // its achievements followed as it plays
        if let Some(a) = &achievements {
            command.env("VPI_ACHIEVEMENTS", a);
        }
        // played hardcore: as its author made it, whatever its own settings say
        if hardcore {
            command.env("VPI_HARDCORE", "1");
        }
        if let Some(dir) = player.parent() {
            command.current_dir(dir);
        }
        match log.as_ref().and_then(|l| std::fs::File::create(l).ok()) {
            Some(out) => {
                let err = out.try_clone().map_err(|e| e.to_string())?;
                command.stdout(out).stderr(err);
            }
            None => {
                command.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: the player is a console program; no console shows
            command.creation_flags(0x0800_0000);
        }
        command.spawn().map_err(|e| format!("The player didn't start: {e}"))
    })();
    let mut child = match started {
        Ok(c) => c,
        Err(e) => {
            shared.0.lock().unwrap().playing = None;
            return Err(e);
        }
    };
    let _ = app.emit("library-changed", ());
    // the app out of the way once the player's window shows (cabinet mode staying behind it), told
    // to the screens; given up after a minute, or once the player's ended
    let ended = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let (app, ended, pid) = (app.clone(), ended.clone(), child.id());
        let _ = std::thread::Builder::new().name("player-shows".into()).spawn(move || {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
            while !shows_window(pid) && std::time::Instant::now() < until {
                if ended.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            if ended.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let _ = app.emit("player-showing", ());
            if !stay && let Some(w) = app.get_webview_window("main") {
                let _ = w.minimize();
            }
        });
    }
    let app = app.clone();
    let id = id.to_string();
    std::thread::Builder::new()
        .name("player".into())
        .spawn(move || {
            let code = child.wait().ok().and_then(|s| s.code());
            ended.store(true, std::sync::atomic::Ordering::Relaxed);
            let log = library::data_folder().map(|d| d.join("Logs").join("player.log"));
            // its last game's scores: the machine's memory says them where its map knows where,
            // else the scores the script last showed on its backglass, as the player said them
            let scores = rom
                .as_deref()
                .zip(layout.as_ref())
                .and_then(|(rom, layout)| ::library::scores::last_game(layout, rom, at))
                .or_else(|| log.as_deref().and_then(said_scores))
                .unwrap_or_default();
            {
                let shared = app.state::<Shared>();
                let mut st = shared.0.lock().unwrap();
                st.playing = None;
                if counted {
                    library::record_end(&mut st, &path, at, scores);
                }
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                crate::cabinet::focus(&w);
            }
            let _ = app.emit("player-ended", Ended { id, name, code, log });
            let _ = app.emit("library-changed", ());
            // what's earned shared with VPIDB, where the player connected and switched sharing on
            if achievements.is_some() {
                crate::vpidb::share_after_play(&app);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
