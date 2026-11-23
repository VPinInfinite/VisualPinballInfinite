//! Colouring a table's score display, the app's side: the table recorded by itself — its machine's
//! attract mode, then a game it plays on its own, in the headless runner, no window — or played,
//! its display's frames recorded as PPUC's colouring editor's frame dump, kept in the player's
//! data folder (`Coloring\<rom>\<rom> <when>.txt.gz`, an older `Colouring` folder's moved in);
//! the recordings listed; and a finished
//! colouring chosen in Windows' file picker put where the player and VPX read it — the table's own
//! colour folder, else VPinMAME's `altcolor\<rom>` — named for the ROM, the one there before and
//! another kind of Serum file that would play over it to the Recycle Bin. ColorizingDMD, a
//! colouring program of its own, reads a recording only as plain text named for the ROM: every
//! recording of a machine written out so (`For ColorizingDMD\<rom>.txt`); the program itself is
//! the user's to download, opened from where they chose it once (`colorizingdmd_open`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::library::{self, Shared};

/// Where every machine's recordings are kept: `Coloring` in the data folder, an older `Colouring`
/// folder's recordings moved into it first.
fn recordings_root() -> Option<PathBuf> {
    let data = library::data_folder()?;
    let root = data.join("Coloring");
    move_older(&data.join("Colouring"), &root);
    Some(root)
}

/// An older folder's files moved to where they're kept now, each where nothing of its name is
/// already; the older folder taken away once it's empty.
fn move_older(old: &Path, new: &Path) {
    if !old.is_dir() {
        return;
    }
    if !new.exists() && std::fs::rename(old, new).is_ok() {
        return;
    }
    let _ = std::fs::create_dir_all(new);
    for e in std::fs::read_dir(old).into_iter().flatten().flatten() {
        let to = new.join(e.file_name());
        if !to.exists() {
            let _ = std::fs::rename(e.path(), &to);
        } else if e.path().is_dir() {
            move_older(&e.path(), &to);
        }
    }
    let _ = std::fs::remove_dir(old);
}

/// Where a ROM's recordings are kept.
fn recordings_dir(rom: &str) -> Option<PathBuf> {
    Some(recordings_root()?.join(rom))
}

fn rom_of(shared: &Shared, id: &str) -> Result<String, String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    t.holds.rom_set.clone().ok_or_else(|| "Its script drives no machine, so it has no score display of its own to color.".to_string())
}

/// The table played with its display recorded; the recording's file.
#[tauri::command]
pub fn colour_record(app: AppHandle, shared: State<'_, Shared>, id: String) -> Result<String, String> {
    let rom = rom_of(&shared, &id)?;
    let dir = recordings_dir(&rom).ok_or("no APPDATA folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let dump = dir.join(format!("{rom} {}.txt.gz", library::stamp()));
    crate::play::play_recording(&app, &id, dump.clone())?;
    Ok(dump.display().to_string())
}

/// How long a recording by itself runs, and how much of it the machine's left in its attract mode
/// before a game's begun.
const BY_ITSELF_MS: u64 = 240_000;
const ATTRACT_MS: u64 = 75_000;

/// The table being recorded by itself, by its ID and name: one at a time.
static BY_ITSELF: Mutex<Option<(String, String)>> = Mutex::new(None);

/// The keys of a game played by itself, as the headless runner takes them (`ms:key:d|u`): the
/// attract mode left to run, then two coins and start, the plunger pulled every 20 seconds (for a
/// ball waiting in the shooter lane) and both flippers flicked each second.
fn keys_by_itself() -> String {
    let mut keys: Vec<(u64, &str, bool)> = Vec::new();
    let mut press = |at: u64, key: &'static str, hold: u64| {
        keys.push((at, key, true));
        keys.push((at + hold, key, false));
    };
    press(ATTRACT_MS, "5", 150);
    press(ATTRACT_MS + 1_000, "5", 150);
    press(ATTRACT_MS + 3_000, "1", 150);
    let game = ATTRACT_MS + 6_000;
    for at in (game..BY_ITSELF_MS).step_by(20_000) {
        press(at, "enter", 1_000);
    }
    for at in (game + 2_000..BY_ITSELF_MS).step_by(1_000) {
        press(at + 400, "lshift", 200);
        press(at + 400, "rshift", 200);
    }
    keys.sort_by_key(|k| k.0);
    keys.iter().map(|(at, key, down)| format!("{at}:{key}:{}", if *down { "d" } else { "u" })).collect::<Vec<_>>().join(",")
}

/// The table recorded by itself in the background, no window: its machine's attract mode, then a
/// game it plays on its own, every new frame kept beside the recordings played. Said when it's
/// done (`colour-recorded`).
#[tauri::command]
pub fn colour_by_itself(app: AppHandle, shared: State<'_, Shared>, id: String) -> Result<(), String> {
    let rom = rom_of(&shared, &id)?;
    let (table, name) = {
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        (t.path.clone(), t.stem.clone())
    };
    let runner = ::library::test::runner().ok_or("The headless runner isn't beside the app, so it can't record by itself.")?;
    let dir = recordings_dir(&rom).ok_or("no APPDATA folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    {
        let mut now = BY_ITSELF.lock().unwrap();
        if let Some((_, other)) = now.as_ref() {
            return Err(format!("{other} is being recorded by itself now: one at a time, about four minutes each."));
        }
        *now = Some((id.clone(), name.clone()));
    }
    let dump = dir.join(format!("{rom} {} by itself.txt.gz", library::stamp()));
    std::thread::spawn(move || {
        let task = crate::tasks::begin(&app, format!("Recording {name}’s display by itself"), (BY_ITSELF_MS / 1000) as u32);
        let done = record_by_itself(&runner, &table, &rom, &dump, &|seconds| task.step(seconds, None));
        drop(task);
        *BY_ITSELF.lock().unwrap() = None;
        let _ = app.emit("colour-recorded", serde_json::json!({ "id": id, "problem": done.err() }));
    });
    Ok(())
}

/// The table being recorded by itself now, by its ID, if one is.
#[tauri::command]
pub fn colour_by_itself_now() -> Option<String> {
    BY_ITSELF.lock().unwrap().as_ref().map(|(id, _)| id.clone())
}

/// The runner's game, in a data folder of its own holding a copy of the machine's memory as the
/// player keeps it, so nothing of the player's changes (its audits, its credits, its scores); the
/// seconds it's run told each second.
fn record_by_itself(runner: &Path, table: &Path, rom: &str, dump: &Path, progress: &dyn Fn(u32)) -> Result<(), String> {
    let scratch = std::env::temp_dir().join(format!("vpi-colour-{}-{}", rom.to_ascii_lowercase(), std::process::id()));
    let nvram = scratch.join("Visual Pinball Infinite").join("PinMAME").join("nvram");
    std::fs::create_dir_all(&nvram).map_err(|e| format!("{}: {e}", nvram.display()))?;
    let file = format!("{}.nv", rom.to_ascii_lowercase());
    if let Some(ours) = library::data_folder().map(|d| d.join("PinMAME").join("nvram").join(&file)).filter(|f| f.is_file()) {
        let _ = std::fs::copy(&ours, nvram.join(&file));
    }
    let mut cmd = Command::new(runner);
    cmd.arg(table)
        .args(["--script", "--realtime", "--ms", &BY_ITSELF_MS.to_string(), "--keys", &keys_by_itself()])
        .env("APPDATA", &scratch)
        .env("VPI_FRAME_DUMP", dump)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window, and below the player's own work in Windows' eyes
        cmd.creation_flags(0x0800_0000 | 0x0000_4000);
    }
    let run = cmd.spawn().map_err(|e| format!("the runner didn't start: {e}")).and_then(|mut child| {
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {}
                Err(e) => break Err(e.to_string()),
            }
            progress(started.elapsed().as_secs() as u32);
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    let _ = std::fs::remove_dir_all(&scratch);
    let status = run?;
    if !dump.is_file() {
        return Err(if status.success() { "Its machine showed nothing to record.".into() } else { "The recording stopped before it began.".into() });
    }
    Ok(())
}

/// A recording kept, with how many frames it holds where the player said (`<dump>.frames`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub name: String,
    pub bytes: u64,
    pub frames: Option<u64>,
}

/// The table's machine's recordings, newest first.
#[tauri::command]
pub fn colour_recordings(shared: State<'_, Shared>, id: String) -> Result<Vec<Recording>, String> {
    let rom = rom_of(&shared, &id)?;
    let dir = recordings_dir(&rom).ok_or("no APPDATA folder")?;
    let mut out: Vec<(std::time::SystemTime, Recording)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".txt.gz"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            let frames = std::fs::read_to_string(format!("{}.frames", e.path().display())).ok().and_then(|t| t.trim().parse().ok());
            Some((m.modified().ok()?, Recording { name: e.file_name().to_string_lossy().into_owned(), bytes: m.len(), frames }))
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out.into_iter().map(|(_, r)| r).collect())
}

/// What's recorded of a machine: how many recordings, their frames where the player counted them,
/// their size.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Recorded {
    pub count: usize,
    pub frames: u64,
    pub bytes: u64,
}

/// Every machine's recordings at once, by its ROM (lower case), for Tools › Colour DMD.
#[tauri::command]
pub fn colour_recordings_all() -> std::collections::HashMap<String, Recorded> {
    let mut out = std::collections::HashMap::new();
    let Some(root) = recordings_root() else { return out };
    for dir in std::fs::read_dir(&root).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
        let mut r = Recorded::default();
        for e in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            if !e.file_name().to_string_lossy().to_lowercase().ends_with(".txt.gz") {
                continue;
            }
            r.count += 1;
            r.bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
            r.frames += std::fs::read_to_string(format!("{}.frames", e.path().display())).ok().and_then(|t| t.trim().parse::<u64>().ok()).unwrap_or(0);
        }
        if r.count > 0 {
            out.insert(dir.file_name().to_string_lossy().to_lowercase(), r);
        }
    }
    out
}

/// Every recording of the table's machine written out as one plain text file named for its ROM,
/// as ColorizingDMD opens a recording (it takes the ROM's name from the file's name, up to its
/// first dot): `For ColorizingDMD\<rom>.txt` beside the recordings, made again each time, shown in
/// Explorer. Its path.
#[tauri::command]
pub async fn colour_for_colorizingdmd(shared: State<'_, Shared>, id: String) -> Result<String, String> {
    let rom = rom_of(&shared, &id)?;
    let dir = recordings_dir(&rom).ok_or("no APPDATA folder")?;
    let file = tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf, String> {
        let mut recordings: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().ends_with(".txt.gz")))
            .collect();
        if recordings.is_empty() {
            return Err("Nothing's recorded of it yet: record its display first.".into());
        }
        // oldest first, as they were recorded
        recordings.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        let out_dir = dir.join("For ColorizingDMD");
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
        let file = out_dir.join(format!("{rom}.txt"));
        let passing = out_dir.join(format!("{rom}.txt.vpi-part"));
        {
            use std::io::Write;
            let mut out = std::io::BufWriter::new(std::fs::File::create(&passing).map_err(|e| format!("{}: {e}", passing.display()))?);
            for r in &recordings {
                let f = std::fs::File::open(r).map_err(|e| format!("{}: {e}", r.display()))?;
                std::io::copy(&mut flate2::read::GzDecoder::new(std::io::BufReader::new(f)), &mut out).map_err(|e| format!("{}: {e}", r.display()))?;
            }
            out.flush().map_err(|e| e.to_string())?;
        }
        std::fs::rename(&passing, &file).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(file)
    })
    .await
    .map_err(|e| e.to_string())??;
    crate::shell::show(&file)?;
    Ok(file.display().to_string())
}

/// The coloring programs the screens offer — other people's, never ours: by a key, their name, their
/// GitHub repository, and a word their program's file is named with.
const PROGRAMS: [(&str, &str, &str, &str); 2] = [("colorizingdmd", "ColorizingDMD", "SerumColor/ColorizingDMD", "colorizingdmd"), ("ppuc", "PPUC's Colorizing DMD", "PPUC/PPUC-Colorizing-DMD", "coloriz")];

/// Where a coloring program the app installed is kept: the data folder's `Programs\<key>`.
fn program_dir(key: &str) -> Option<PathBuf> {
    Some(library::data_folder()?.join("Programs").join(key))
}

/// A program's file in the folder it was unpacked into, a few folders down at most.
fn program_exe(dir: &Path, word: &str) -> Option<PathBuf> {
    let mut todo = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && depth < 3 {
                todo.push((p, depth + 1));
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")) && p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(word)) {
                return Some(p);
            }
        }
    }
    None
}

/// A coloring program as the screens show it: installed where, whether its makers have released
/// it, its page.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    pub key: String,
    pub name: String,
    pub installed: Option<String>,
    pub released: bool,
    pub page: String,
}

/// Whether a repository has a release, asked of GitHub once a run.
static RELEASED: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());

fn released(repo: &str) -> bool {
    if let Some((_, r)) = RELEASED.lock().unwrap().iter().find(|(k, _)| k == repo) {
        return *r;
    }
    let answer = ::library::vps::curl()
        .args(["-sS", "-L", "--max-time", "15", "-A", "VisualPinballInfinite", &format!("https://api.github.com/repos/{repo}/releases?per_page=1")])
        .stdin(Stdio::null())
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok());
    // unknown (offline): taken as released, the install saying what's wrong
    let r = answer.as_ref().and_then(|v| v.as_array()).map_or(true, |a| !a.is_empty());
    RELEASED.lock().unwrap().push((repo.to_string(), r));
    r
}

/// The coloring programs: each installed (by the app, else where the user once chose it, from
/// `chosen`), and whether there's a release to install.
#[tauri::command]
pub async fn colour_programs(chosen: Option<std::collections::HashMap<String, String>>) -> Vec<Program> {
    let chosen = chosen.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        PROGRAMS
            .iter()
            .map(|(key, name, repo, word)| {
                let installed = program_dir(key).and_then(|d| program_exe(&d, word)).or_else(|| chosen.get(*key).map(PathBuf::from).filter(|p| p.is_file()));
                Program { key: key.to_string(), name: name.to_string(), installed: installed.map(|p| p.display().to_string()), released: released(repo), page: format!("https://github.com/{repo}") }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// A coloring program installed from its makers' newest release on GitHub: its Windows zip
/// downloaded and unpacked into the data folder's `Programs\<key>` (the one there before replaced),
/// its program's file found in it. Where that is.
#[tauri::command]
pub async fn colour_program_install(key: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let (_, name, repo, word) = PROGRAMS.iter().find(|(k, ..)| *k == key).ok_or("No such program.")?;
        let api = ::library::vps::curl()
            .args(["-sS", "-L", "--fail", "--max-time", "20", "-A", "VisualPinballInfinite", &format!("https://api.github.com/repos/{repo}/releases/latest")])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("GitHub couldn't be reached: {e}"))?;
        if !api.status.success() {
            return Err(format!("{name} has no release to install yet."));
        }
        let release: serde_json::Value = serde_json::from_slice(&api.stdout).map_err(|e| format!("GitHub's answer couldn't be read: {e}"))?;
        let asset = release["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|a| a["name"].as_str().is_some_and(|n| n.to_lowercase().ends_with(".zip")))
            .find(|a| a["name"].as_str().is_some_and(|n| !n.to_lowercase().contains("linux") && !n.to_lowercase().contains("mac")))
            .ok_or_else(|| format!("{name}'s newest release holds no Windows zip."))?;
        let url = asset["browser_download_url"].as_str().ok_or("GitHub gave no address for it.")?;
        let zip_file = std::env::temp_dir().join(format!("vpi-{key}-{}.zip", std::process::id()));
        let got = ::library::vps::curl()
            .args(["-sS", "-L", "--fail", "--max-time", "600", "-A", "VisualPinballInfinite", "-o"])
            .arg(&zip_file)
            .arg(url)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("It couldn't be downloaded: {e}"))?;
        if !got.status.success() {
            let _ = std::fs::remove_file(&zip_file);
            return Err(format!("It couldn't be downloaded: {}", String::from_utf8_lossy(&got.stderr).trim()));
        }
        let dir = program_dir(&key).ok_or("no APPDATA folder")?;
        let passing = dir.with_extension("installing");
        let _ = std::fs::remove_dir_all(&passing);
        std::fs::create_dir_all(&passing).map_err(|e| format!("{}: {e}", passing.display()))?;
        let unpacked = (|| -> Result<(), String> {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            for i in 0..zip.len() {
                let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
                // nothing written outside its folder
                let Some(rel) = f.enclosed_name() else { continue };
                let to = passing.join(rel);
                if f.is_dir() {
                    std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
                    continue;
                }
                std::fs::create_dir_all(to.parent().unwrap_or(&passing)).map_err(|e| e.to_string())?;
                let mut out = std::fs::File::create(&to).map_err(|e| format!("{}: {e}", to.display()))?;
                std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
        let _ = std::fs::remove_file(&zip_file);
        if let Err(e) = unpacked {
            let _ = std::fs::remove_dir_all(&passing);
            return Err(format!("It couldn't be unpacked: {e}"));
        }
        if program_exe(&passing, word).is_none() {
            let _ = std::fs::remove_dir_all(&passing);
            return Err(format!("{name}'s release holds no program file this app knows."));
        }
        if dir.exists() {
            ::library::install::to_recycle_bin(&dir)?;
        }
        std::fs::rename(&passing, &dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        program_exe(&dir, word).map(|p| p.display().to_string()).ok_or_else(|| "It went missing after unpacking.".into())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A coloring program started, from where it's installed. It's a program of its own, not ours.
#[tauri::command]
pub fn colour_program_open(path: String) -> Result<(), String> {
    let exe = PathBuf::from(&path);
    if !exe.is_file() {
        return Err("It isn't there any more: install it again.".into());
    }
    let mut command = Command::new(&exe);
    if let Some(dir) = exe.parent() {
        command.current_dir(dir);
    }
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("{} didn't start: {e}", exe.display()))?;
    Ok(())
}

/// The machine's recordings' folder in Explorer, made where it isn't yet.
#[tauri::command]
pub fn colour_show(shared: State<'_, Shared>, id: String) -> Result<(), String> {
    let rom = rom_of(&shared, &id)?;
    let dir = recordings_dir(&rom).ok_or("no APPDATA folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    crate::shell::show(&dir)
}

/// A finished colouring chosen in Windows' file picker put in place for the table; false when the
/// picker's cancelled. The library read again after.
#[tauri::command]
pub async fn colour_use(app: AppHandle, window: tauri::WebviewWindow, shared: State<'_, Shared>, id: String) -> Result<bool, String> {
    let rom = rom_of(&shared, &id)?;
    let folder = {
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        let layout = st.layout.as_ref().ok_or("The library isn't read yet.")?;
        // a table keeping a PIN2DMD colouring in its own `vni` folder keeps a Serum one in `serum`
        let serum_beside = |f: &std::path::Path| f.parent().filter(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("vni"))).and_then(|p| p.parent()).map(|d| d.join("serum").join(&rom));
        t.holds.colour.as_ref().map(|c| serum_beside(&c.folder).unwrap_or_else(|| c.folder.clone())).unwrap_or_else(|| layout.vpinmame.join("altcolor").join(&rom))
    };
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    let done = tauri::async_runtime::spawn_blocking(move || -> Result<bool, String> {
        let kinds = [("A Serum coloring", "*.cRZ;*.cROMc")];
        let Some(picked) = crate::pick::file(owner, "The coloring to use", &kinds)? else { return Ok(false) };
        let ext = picked.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        let ext = match ext.as_str() {
            "crz" => "cRZ",
            "cromc" => "cROMc",
            _ => return Err("That isn't a Serum coloring (.cRZ or .cROMc).".into()),
        };
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let to = folder.join(format!("{rom}.{ext}"));
        if to.to_string_lossy().eq_ignore_ascii_case(&picked.to_string_lossy()) {
            return Ok(true);
        }
        // copied beside its place first, so a failure leaves the one there as it was
        let passing = folder.join(format!("{rom}.{ext}.vpi-part"));
        std::fs::copy(&picked, &passing).map_err(|e| format!("{}: {e}", picked.display()))?;
        // the one there before, and the other kind of Serum file, which would play over it
        for old in ["cRZ", "cROMc"].map(|x| folder.join(format!("{rom}.{x}"))) {
            if old.is_file() {
                ::library::install::to_recycle_bin(&old)?;
            }
        }
        std::fs::rename(&passing, &to).map_err(|e| format!("{}: {e}", to.display()))?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())??;
    if done {
        library::start(app);
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_recordings_folder_moved_into_the_new_one() {
        let base = std::env::temp_dir().join(format!("vpi-coloring-move-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (old, new) = (base.join("Colouring"), base.join("Coloring"));
        // alone: the whole folder renamed
        std::fs::create_dir_all(old.join("afm_113b")).unwrap();
        std::fs::write(old.join("afm_113b").join("a.txt.gz"), b"a").unwrap();
        move_older(&old, &new);
        assert!(!old.exists());
        assert!(new.join("afm_113b").join("a.txt.gz").is_file());
        // beside one made since: each recording moved in, the one there kept
        std::fs::create_dir_all(old.join("afm_113b")).unwrap();
        std::fs::create_dir_all(old.join("tz_94h")).unwrap();
        std::fs::write(old.join("afm_113b").join("a.txt.gz"), b"older").unwrap();
        std::fs::write(old.join("afm_113b").join("b.txt.gz"), b"b").unwrap();
        std::fs::write(old.join("tz_94h").join("c.txt.gz"), b"c").unwrap();
        move_older(&old, &new);
        assert_eq!(std::fs::read(new.join("afm_113b").join("a.txt.gz")).unwrap(), b"a");
        assert!(new.join("afm_113b").join("b.txt.gz").is_file());
        assert!(new.join("tz_94h").join("c.txt.gz").is_file());
        // what couldn't move stays where it was
        assert!(old.join("afm_113b").join("a.txt.gz").is_file());
        assert!(!old.join("tz_94h").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_game_played_by_itself_after_the_attract_mode() {
        let keys = keys_by_itself();
        let parsed: Vec<(u64, &str, &str)> = keys
            .split(',')
            .map(|k| {
                let mut f = k.split(':');
                (f.next().unwrap().parse().unwrap(), f.next().unwrap(), f.next().unwrap())
            })
            .collect();
        // in time's order, nothing before the attract mode's run, every press let go
        assert!(parsed.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(parsed[0], (ATTRACT_MS, "5", "d"));
        for key in ["5", "1", "enter", "lshift", "rshift"] {
            let downs = parsed.iter().filter(|k| k.1 == key && k.2 == "d").count();
            assert!(downs > 0 && downs == parsed.iter().filter(|k| k.1 == key && k.2 == "u").count(), "{key}");
        }
        assert!(parsed.last().unwrap().0 <= BY_ITSELF_MS + 1_000);
    }

    /// The real thing on a table (`VPI_COLOUR_TABLE=<table.vpx>`, `VPI_COLOUR_ROM=<its set>`): four
    /// minutes of its game recorded by itself into a scratch file, nothing of the player's changed.
    #[test]
    #[ignore]
    fn a_table_recorded_by_itself() {
        let table = PathBuf::from(std::env::var("VPI_COLOUR_TABLE").expect("VPI_COLOUR_TABLE"));
        let rom = std::env::var("VPI_COLOUR_ROM").expect("VPI_COLOUR_ROM");
        let runner = ::library::test::runner().expect("the headless runner");
        let dump = std::env::temp_dir().join(format!("vpi-colour-check-{}.txt.gz", std::process::id()));
        record_by_itself(&runner, &table, &rom, &dump, &|s| {
            if s % 30 == 0 {
                println!("{s} s");
            }
        })
        .unwrap();
        let frames: u64 = std::fs::read_to_string(format!("{}.frames", dump.display())).unwrap().trim().parse().unwrap();
        println!("{frames} frames, {} bytes", std::fs::metadata(&dump).unwrap().len());
        assert!(frames > 100);
        let _ = std::fs::remove_file(&dump);
        let _ = std::fs::remove_file(format!("{}.frames", dump.display()));
    }
}
