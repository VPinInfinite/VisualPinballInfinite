//! Logs and Report a problem. The app keeps its own
//! log beside the player's, each run's and the one before (`Logs\app.log`, `Logs\player.log`); a
//! report gathers them with the settings, this PC's facts and, asked from a table's page, the
//! table's, into one zip the user can look through before attaching it anywhere — nothing is sent
//! by itself. The user's own folder is taken out of every path in it, and a PIN2DMD key never
//! goes in.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::library::Shared;

/// The app's own messages into `Logs\app.log` from here on, the last run's kept as
/// `app.previous.log`: what it writes to its error output, which a windowed program otherwise
/// loses. A run that wrote nothing but its start isn't kept over the one before, so a restart or
/// two leave the last run that said something.
pub fn log_to_file() {
    let Some(dir) = crate::library::data_folder().map(|d| d.join("Logs")) else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let log = dir.join("app.log");
    if log.exists() {
        let said = std::fs::read_to_string(&log).map_or(true, |t| t.lines().filter(|l| !l.trim().is_empty()).count() > 1);
        if said {
            let _ = std::fs::rename(&log, dir.join("app.previous.log"));
        } else {
            let _ = std::fs::remove_file(&log);
        }
    }
    let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(&log) else { return };
    #[cfg(windows)]
    {
        use std::os::windows::io::IntoRawHandle;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn SetStdHandle(which: u32, handle: *mut std::ffi::c_void) -> i32;
        }
        const STD_ERROR_HANDLE: u32 = -12i32 as u32;
        // SAFETY: a handle we own, handed to the process for good
        unsafe { SetStdHandle(STD_ERROR_HANDLE, file.into_raw_handle()) };
    }
    eprintln!("Visual Pinball Infinite {} started {}", env!("CARGO_PKG_VERSION"), crate::library::stamp());
}

/// What the user asked to go in.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
    /// What happened, in their words.
    #[serde(default)]
    pub note: String,
    /// The table it's about, by its ID.
    pub table: Option<String>,
    pub logs: bool,
    pub settings: bool,
    pub pc: bool,
}

/// The user's own folder taken out of `text`, in any case and either slash, as JSON escapes it too.
fn private(text: &str) -> String {
    let Some(profile) = std::env::var_os("USERPROFILE").map(|p| p.to_string_lossy().into_owned()).filter(|p| p.len() > 3) else {
        return text.to_string();
    };
    let mut out = text.to_string();
    for form in [profile.clone(), profile.replace('\\', "/"), profile.replace('\\', "\\\\")] {
        if let Ok(re) = regex::RegexBuilder::new(&regex::escape(&form)).case_insensitive(true).build() {
            out = re.replace_all(&out, "%USERPROFILE%").into_owned();
        }
    }
    out
}

/// A settings file's text with any PIN2DMD key taken out.
fn without_keys(text: &str) -> String {
    text.lines()
        .map(|l| if l.trim_start().to_ascii_lowercase().starts_with("packey") { "PacKey = (taken out)" } else { l })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A text value of the local machine's registry, or None.
#[cfg(windows)]
fn machine_text(key: &str, name: &str) -> Option<String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(key: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut std::ffi::c_void, size: *mut u32) -> i32;
    }
    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
    // RRF_RT_REG_SZ | RRF_RT_REG_DWORD
    const FLAGS: u32 = 0x2 | 0x10;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (k, n) = (wide(key), wide(name));
    let mut buffer = vec![0u16; 256];
    let mut bytes = (buffer.len() * 2) as u32;
    let mut kind = 0u32;
    // SAFETY: NUL-terminated names, a buffer of `bytes` bytes
    if unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, k.as_ptr(), n.as_ptr(), FLAGS, &mut kind, buffer.as_mut_ptr().cast(), &mut bytes) } != 0 {
        return None;
    }
    // a number (REG_DWORD, 4) or text
    if kind == 4 {
        return Some((u32::from(buffer[0]) | (u32::from(buffer[1]) << 16)).to_string());
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]))
}

/// This PC, in a few lines: Windows, its processor, its memory, its graphics cards and its screens.
#[cfg(windows)]
fn this_pc() -> String {
    #[repr(C)]
    struct DisplayDevice {
        cb: u32,
        name: [u16; 32],
        string: [u16; 128],
        flags: u32,
        id: [u16; 128],
        key: [u16; 128],
    }
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total: u64,
        avail: u64,
        rest: [u64; 5],
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDisplayDevicesW(device: *const u16, index: u32, out: *mut DisplayDevice, flags: u32) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(out: *mut MemoryStatus) -> i32;
    }
    let mut lines = Vec::new();
    let nt = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build: u32 = machine_text(nt, "CurrentBuild").and_then(|b| b.parse().ok()).unwrap_or(0);
    // Windows 11 still names itself Windows 10 in the registry
    let product = machine_text(nt, "ProductName").unwrap_or_else(|| "Windows".into());
    let product = if build >= 22000 { product.replace("Windows 10", "Windows 11") } else { product };
    lines.push(format!("Windows: {product} {} (build {build}.{})", machine_text(nt, "DisplayVersion").unwrap_or_default(), machine_text(nt, "UBR").unwrap_or_default()));
    if let Some(cpu) = machine_text(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", "ProcessorNameString") {
        lines.push(format!("Processor: {}", cpu.trim()));
    }
    let mut memory = MemoryStatus { length: std::mem::size_of::<MemoryStatus>() as u32, load: 0, total: 0, avail: 0, rest: [0; 5] };
    // SAFETY: a struct of the size it says
    if unsafe { GlobalMemoryStatusEx(&mut memory) } != 0 {
        lines.push(format!("Memory: {:.0} GB", memory.total as f64 / (1u64 << 30) as f64));
    }
    let mut cards: Vec<String> = Vec::new();
    for i in 0..16 {
        let mut d = DisplayDevice { cb: std::mem::size_of::<DisplayDevice>() as u32, name: [0; 32], string: [0; 128], flags: 0, id: [0; 128], key: [0; 128] };
        // SAFETY: a struct of the size it says
        if unsafe { EnumDisplayDevicesW(std::ptr::null(), i, &mut d, 0) } == 0 {
            break;
        }
        let len = d.string.iter().position(|&c| c == 0).unwrap_or(d.string.len());
        let card = String::from_utf16_lossy(&d.string[..len]);
        // those showing the desktop
        if d.flags & 0x1 != 0 && !card.is_empty() && !cards.contains(&card) {
            cards.push(card);
        }
    }
    lines.push(format!("Graphics: {}", if cards.is_empty() { "not known".into() } else { cards.join(", ") }));
    match crate::screens::screens() {
        Ok(screens) => {
            for s in screens {
                lines.push(format!("Screen: {}", serde_json::to_string(&s).unwrap_or_default()));
            }
        }
        Err(e) => lines.push(format!("Screens: {e}")),
    }
    lines.join("\n")
}

#[cfg(not(windows))]
fn this_pc() -> String {
    String::new()
}

/// VPinMAME's folders and files as it keeps them for every game (`HKEY_CURRENT_USER\Software\
/// Freeware\Visual PinMame\globals`), those a game's machine is found and kept by first: where it
/// looks for ROMs, samples, settings and memory, and the rest of its folders.
const VPINMAME_FOLDERS: [&str; 12] = [
    "rompath",
    "nvram_directory",
    "cfg_directory",
    "samplepath",
    "hiscore_directory",
    "memcard_directory",
    "input_directory",
    "state_directory",
    "artwork_directory",
    "snapshot_directory",
    "diff_directory",
    "wave_directory",
];

/// Where VPX and VPinMAME are on this PC, in a few lines: the VPX the tables play in and every VPX
/// found beside them, the library's folders, and the classic VPinMAME VPX 10.8.0 plays a machine
/// with — the program Windows has registered for it (`VPinMAME.Controller`) and its folders as it
/// keeps them, where they're set (else VPinMAME takes its own folder's).
fn pinball_programs(app: &AppHandle) -> String {
    let mut lines = Vec::new();
    let layout = app.state::<Shared>().0.lock().unwrap().layout.clone();
    let shown = |v: &::library::vpx::Vpx| format!("{} ({}.{}.{}.{})", v.program.display(), v.version[0], v.version[1], v.version[2], v.version[3]);
    match layout.as_ref() {
        Some(l) => {
            match ::library::vpx::to_play(l) {
                Some(v) => lines.push(format!("VPX the tables play in: {}", shown(&v))),
                None => lines.push("VPX the tables play in: none found".into()),
            }
            for v in ::library::vpx::all_for(l) {
                lines.push(format!("VPX found: {}", shown(&v)));
            }
            lines.push(format!("Tables: {}", l.tables.display()));
            lines.push(format!("Scripts: {}", l.scripts.display()));
            lines.push(format!("VPinMAME's folder: {}", l.vpinmame.display()));
        }
        None => lines.push("No table library is set up.".into()),
    }
    lines.extend(vpinmame_lines());
    lines.join("\n")
}

/// The classic VPinMAME, for a report: the program Windows has registered for it, and its folders
/// as it keeps them, where they're set.
#[cfg(windows)]
fn vpinmame_lines() -> Vec<String> {
    use crate::b2s_screen::{registry_text, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
    let program = registry_text(HKEY_CLASSES_ROOT, r"VPinMAME.Controller\CLSID", "").and_then(|clsid| registry_text(HKEY_CLASSES_ROOT, &format!(r"CLSID\{}\InprocServer32", clsid.trim()), ""));
    let mut lines = vec![format!("VPinMAME registered: {}", program.as_deref().unwrap_or("none"))];
    if program.is_some() {
        for name in VPINMAME_FOLDERS {
            let value = registry_text(HKEY_CURRENT_USER, r"Software\Freeware\Visual PinMame\globals", name);
            lines.push(format!("VPinMAME {name}: {}", value.as_deref().unwrap_or("not set (its own folder's)")));
        }
    }
    lines
}

#[cfg(not(windows))]
fn vpinmame_lines() -> Vec<String> {
    Vec::new()
}

/// The VPX that plays the library's tables, as Settings › Library chooses.
fn playing_vpx(app: &AppHandle) -> Option<::library::vpx::Vpx> {
    let layout = app.state::<Shared>().0.lock().unwrap().layout.clone();
    layout.as_ref().and_then(::library::vpx::to_play)
}

/// The report: `Logs\Reports\VPI report <when>.zip`, made of what the user asked for.
pub fn make(app: &AppHandle, asked: &Asked) -> Result<PathBuf, String> {
    let data = crate::library::data_folder().ok_or("no APPDATA folder")?;
    let dir = data.join("Logs").join("Reports");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let when = crate::library::stamp().replace(':', "-");
    let path = dir.join(format!("VPI report {when}.zip"));
    let file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut put = |name: &str, text: &str| -> Result<(), String> {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(private(text).as_bytes()).map_err(|e| e.to_string())
    };
    // what it is, who it's from, and what the user said: true whether it's sent or attached
    let how = if can_send() {
        "Made on this PC, and sent to the team only with Send it to us: or attach it to your post on Discord (#bug-reports) or a GitHub issue."
    } else {
        "Made on this PC, and not sent anywhere: attach it to your post on Discord (#bug-reports) or a GitHub issue."
    };
    let mut about = format!(
        "Visual Pinball Infinite {} — a problem report, made {}\n{how}\n\nWhat happened, in your words:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        crate::library::stamp(),
        if asked.note.trim().is_empty() { "(nothing written)" } else { asked.note.trim() },
    );
    if asked.pc {
        about.push_str("\nThis PC:\n");
        about.push_str(&this_pc());
        about.push('\n');
    }
    if asked.settings {
        about.push_str("\nVPX and VPinMAME:\n");
        about.push_str(&pinball_programs(app));
        about.push('\n');
    }
    put("about.txt", &about)?;
    // the table it's about, as its page shows it, and its own settings
    if let Some(id) = &asked.table {
        let (view, own) = {
            let shared = app.state::<Shared>();
            let st = shared.0.lock().unwrap();
            let t = st.tables.iter().find(|t| &t.id == id).ok_or("That table isn't in the library any more.")?;
            (serde_json::to_string_pretty(&crate::library::view(&st, t)).unwrap_or_default(), t.path.with_extension("ini"))
        };
        put("table.json", &view)?;
        if let Ok(text) = std::fs::read(&own) {
            put("settings/the table's own.ini", &without_keys(&String::from_utf8_lossy(&text)))?;
        }
    }
    if asked.logs {
        for name in ["app.log", "app.previous.log", "player.log", "player.previous.log"] {
            if let Ok(bytes) = std::fs::read(data.join("Logs").join(name)) {
                put(&format!("logs/{name}"), &String::from_utf8_lossy(&bytes))?;
            }
        }
        // the log of the VPX that plays the tables
        if let Some(Ok(bytes)) = playing_vpx(app).as_ref().and_then(|v| ::library::vpx::log_file(v, ::library::vpx::prefs_override().as_deref())).or_else(|| vpi_settings::vpx_prefs_folder().map(|f| f.join("vpinball.log"))).map(std::fs::read) {
            put("logs/vpinball.log", &String::from_utf8_lossy(&bytes))?;
        }
    }
    if asked.settings {
        if let Some(main) = vpi_settings::settings_path()
            && let Ok(bytes) = std::fs::read(&main)
        {
            put("settings/Settings.ini", &without_keys(&String::from_utf8_lossy(&bytes)))?;
        }
        // the settings of the VPX that plays the tables
        if let Some(vpx) = playing_vpx(app).as_ref().and_then(::library::vpx::settings_file).or_else(vpi_settings::vpx_settings_path)
            && let Ok(bytes) = std::fs::read(&vpx)
        {
            put("settings/VPinballX.ini", &without_keys(&String::from_utf8_lossy(&bytes)))?;
        }
        if let Ok(bytes) = std::fs::read(data.join("Machine settings.ini")) {
            put("settings/Machine settings.ini", &String::from_utf8_lossy(&bytes))?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(path)
}

/// Where a report went, shown in Explorer.
pub fn show(path: &Path) -> Result<(), String> {
    crate::shell::show(path)
}


/// Where Send takes a report: given to the build from outside the repository (`build.rs`), so a
/// build made from the source alone has none and sends nothing.
const SEND_TO: Option<&str> = option_env!("VPI_REPORTS_ADDRESS");

/// The reports' address, where the build was given one on https; anything else counts as none (the
/// build's own environment reaches `option_env!` as it is, past `build.rs`'s check).
fn send_to() -> Option<&'static str> {
    SEND_TO.filter(|a| a.starts_with("https://"))
}

/// Whether this build sends reports, or only makes them for the user to attach to a post.
pub fn can_send() -> bool {
    send_to().is_some()
}

/// Where Send takes a report, in a build that sends them: the reports service, which also answers
/// for replies and notices (`replies.rs`).
pub fn address() -> Option<&'static str> {
    send_to()
}

/// A report's files as Send passes them on: each one in the zip under the plain name the reports
/// service takes it by, as text. The service puts them in one zip for the team.
const PARTS: [(&str, &str); 11] = [
    ("about.txt", "about.txt"),
    ("table.json", "table.json"),
    ("settings/the table's own.ini", "table.ini"),
    ("settings/Settings.ini", "Settings.ini"),
    ("settings/Machine settings.ini", "Machine-settings.ini"),
    ("settings/VPinballX.ini", "VPinballX.ini"),
    ("logs/app.log", "app.log"),
    ("logs/app.previous.log", "app.previous.log"),
    ("logs/player.log", "player.log"),
    ("logs/player.previous.log", "player.previous.log"),
    ("logs/vpinball.log", "vpinball.log"),
];

/// The most of one file Send passes on; a longer one keeps its end, the newest lines of a log.
const PART_MOST: usize = 1024 * 1024;

/// The most Send passes on in all: within what the service takes (8 MB, with its form) and what a
/// file on Discord holds.
const ALL_MOST: usize = 7 * 1024 * 1024;

/// The parts within `most` together: the biggest cut to half, its end kept, until they are.
fn keep_within(parts: &mut [(&str, String)], most: usize) {
    while parts.iter().map(|(_, t)| t.len()).sum::<usize>() > most {
        let Some((i, len)) = parts.iter().enumerate().map(|(i, (_, t))| (i, t.len())).max_by_key(|&(_, l)| l) else { return };
        let cut = cut_to_end(&parts[i].1, len / 2);
        if cut.len() >= len {
            return;
        }
        parts[i].1 = cut;
    }
}

/// A report's files, read from its zip, as Send passes them on: anything else in it is left out.
fn parts(path: &Path) -> Result<Vec<(&'static str, String)>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("The report couldn't be read: {e}"))?;
    let mut out = Vec::new();
    for (inside, name) in PARTS {
        let Ok(mut entry) = zip.by_name(inside) else { continue };
        let mut text = String::new();
        std::io::Read::read_to_string(&mut entry, &mut text).map_err(|e| format!("The report couldn't be read: {e}"))?;
        out.push((name, cut_to_end(&text, PART_MOST)));
    }
    if !out.iter().any(|(name, _)| *name == "about.txt") {
        return Err("Only a report made here can be sent.".into());
    }
    keep_within(&mut out, ALL_MOST);
    Ok(out)
}

/// `text` within `most` bytes: its end, from the start of a line, after a line saying so.
fn cut_to_end(text: &str, most: usize) -> String {
    const SAID: &str = "(the start of this file is cut here: the whole of it is in the report kept on the PC)\n";
    if text.len() <= most {
        return text.to_string();
    }
    let mut from = text.len() - most.saturating_sub(SAID.len());
    while !text.is_char_boundary(from) {
        from += 1;
    }
    if let Some(end) = text[from..].find('\n') {
        from += end + 1;
    }
    format!("{SAID}{}", &text[from..])
}

/// A report made here sent to the team, with the user's note and the table's name, on their word
/// (Report a problem's "Send it to us"): only a zip in the app's own reports folder, its files passed
/// on one by one as plain text (never the zip), through Windows' own curl with no window, with a key
/// of its own for the team's replies; the report as sent, to keep, or the service's own words why
/// not.
pub fn send(path: &Path, note: &str, table: Option<&str>) -> Result<crate::replies::Sent, String> {
    let Some(address) = send_to() else {
        return Err("This copy of Visual Pinball Infinite doesn't send reports. Show it, and attach it to a post on Discord (#bug-reports) or a GitHub issue instead.".into());
    };
    let reports = crate::library::data_folder().ok_or("no APPDATA folder")?.join("Logs").join("Reports");
    let ours = path.parent().is_some_and(|d| d == reports) && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) && path.is_file();
    if !ours {
        return Err("Only a report made here can be sent.".into());
    }
    let parts = parts(path)?;
    // each file where curl reads it, in a folder of its own, gone again after
    let dir = reports.join(format!("sending {}", crate::library::stamp().replace(':', "-")));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let note: String = note.chars().take(1500).collect();
    let key = crate::replies::new_key()?;
    let version = env!("CARGO_PKG_VERSION");
    let sent = (|| -> Result<crate::replies::Sent, String> {
        for (name, text) in &parts {
            let file = dir.join(name);
            std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
        }
        // the report's files and words, with its key or without
        let post = |key: Option<&str>| -> Result<(String, String, String), String> {
            let mut curl = ::library::vps::curl();
            curl.args(["-sS", "--max-time", "120", "-A", &format!("VisualPinballInfinite/{version}")])
                .arg("--form-string")
                .arg(format!("note={note}"))
                .arg("--form-string")
                .arg(format!("version={version}"))
                .arg("--form-string")
                .arg(format!("table={}", table.unwrap_or("")));
            if let Some(key) = key {
                curl.arg("--form-string").arg(format!("key={key}"));
            }
            for (name, _) in &parts {
                curl.arg("-F").arg(format!("files=@\"{}\";type=text/plain;filename={name}", dir.join(name).display()));
            }
            curl.arg("-w").arg("\n%{http_code}").arg(address);
            let out = curl.output().map_err(|e| format!("The report couldn't be sent: {e}"))?;
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            let (body, code) = text.rsplit_once('\n').unwrap_or((text.as_str(), ""));
            Ok((body.to_string(), code.trim().to_string(), String::from_utf8_lossy(&out.stderr).trim().to_string()))
        };
        let mut with_key = true;
        let (mut body, mut code, mut err) = post(Some(key.as_str()))?;
        // a service from before replies refuses a report with a key: it goes again without one
        if code == "400" {
            with_key = false;
            (body, code, err) = post(None)?;
        }
        let said = serde_json::from_str::<serde_json::Value>(&body).unwrap_or(serde_json::Value::Null);
        if code == "200" {
            return Ok(crate::replies::Sent {
                key: key.clone(),
                at: said["at"].as_i64().unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)),
                note: note.clone(),
                table: table.map(str::to_string).filter(|t| !t.trim().is_empty()),
                version: version.to_string(),
                file: Some(path.display().to_string()),
                answerable: with_key && said["answerable"].as_bool().unwrap_or(false),
                messages: Vec::new(),
                read: 0,
                closed: false,
                unread: 0,
            });
        }
        // the service's own words when it said why, else curl's
        Err(said["error"].as_str().map(str::to_string).unwrap_or_else(|| {
            let why = if err.is_empty() { format!("answer {code}") } else { err };
            format!("The report couldn't be sent ({why}). Show it, and attach it to a post on Discord or GitHub instead.")
        }))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The classic VPinMAME as a report on this PC says it, the user's folder taken out:
    /// `cargo test -p app vpinmame_on_this_pc -- --ignored --nocapture`. Nothing written.
    #[test]
    #[ignore]
    fn vpinmame_on_this_pc() {
        let lines = vpinmame_lines();
        println!("{}", private(&lines.join("\n")));
        assert!(lines.first().is_some_and(|l| l.starts_with("VPinMAME registered: ")));
    }

    #[test]
    fn a_report_holds_no_key_nor_the_users_folder() {
        assert_eq!(without_keys("[Plugin.VNI]\nPacKey = 12345\nEnable = 1"), "[Plugin.VNI]\nPacKey = (taken out)\nEnable = 1");
        if let Some(profile) = std::env::var_os("USERPROFILE").map(|p| p.to_string_lossy().into_owned()) {
            let text = format!("{profile}\\Tables\\x.vpx and {} and {}", profile.to_uppercase(), profile.replace('\\', "\\\\"));
            let out = private(&text);
            assert!(!out.to_lowercase().contains(&profile.to_lowercase()), "{out}");
            assert!(out.contains("%USERPROFILE%\\Tables\\x.vpx"), "{out}");
        }
    }

    /// Send passes on a report's own files alone, by their plain names, a long log cut to its end
    /// within the most, and nothing else a zip might hold.
    #[test]
    fn a_report_sends_its_own_files_alone_as_text() {
        let dir = std::env::temp_dir().join(format!("vpi-report-parts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.zip");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, text) in [
                ("about.txt", "a report".to_string()),
                ("logs/player.log", "line\n".repeat(PART_MOST / 5 + 1000)),
                ("settings/the table's own.ini", "[TableOverride]".to_string()),
                ("logs/vpinball.log", "VPX's own".to_string()),
                ("settings/VPinballX.ini", "[Player]".to_string()),
                ("something else.exe", "MZ".to_string()),
            ] {
                zip.start_file(name, options).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let parts = parts(&path).unwrap();
        let names: Vec<&str> = parts.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["about.txt", "table.ini", "VPinballX.ini", "player.log", "vpinball.log"]);
        // together within the most: the biggest cut, again and again, the small ones left whole
        let mut many: Vec<(&str, String)> = vec![("about.txt", "a report".into()), ("app.log", "a\n".repeat(400)), ("vpinball.log", "b\n".repeat(900))];
        keep_within(&mut many, 1000);
        assert!(many.iter().map(|(_, t)| t.len()).sum::<usize>() <= 1000, "{many:?}");
        assert_eq!(many[0].1, "a report");
        assert!(many[2].1.ends_with("b\n") && many[2].1.starts_with("(the start of this file is cut here"));
        let log = &parts.iter().find(|(n, _)| *n == "player.log").unwrap().1;
        assert!(log.len() <= PART_MOST && log.starts_with("(the start of this file is cut here") && log.ends_with("line\n"), "{}", log.len());
        assert_eq!(cut_to_end("short", 10), "short");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// One real report through this build's address, end to end, into the team's channel: `cargo test
    /// -p app a_report_reaches_the_team -- --ignored` (a build given the address alone).
    #[test]
    #[ignore]
    fn a_report_reaches_the_team() {
        assert!(can_send(), "this build has no reports address");
        let reports = crate::library::data_folder().unwrap().join("Logs").join("Reports");
        std::fs::create_dir_all(&reports).unwrap();
        let path = reports.join("VPI report (a check of Send).zip");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            // VPX's own log and settings among them, as a report from a game in VPX carries them
            for (name, text) in [
                ("about.txt", "Visual Pinball Infinite: a check of Send, made by the app's own tests.\n"),
                ("logs/app.log", "a line of a log\n"),
                ("logs/vpinball.log", "a line of VPX's log, made up for the check\n"),
                ("settings/VPinballX.ini", "[Player]\nMusicVolume = 80\n"),
            ] {
                zip.start_file(name, options).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let sent = send(&path, "A check of the reports service, sent from the app's own tests: nothing to do.", None);
        std::fs::remove_file(&path).ok();
        sent.unwrap();
    }

    /// Only a build given the reports' address sends them; without it, Send says to attach the report.
    #[test]
    fn a_build_without_the_address_sends_nothing() {
        if !can_send() {
            let err = send(Path::new("nowhere.zip"), "", None).unwrap_err();
            assert!(err.contains("doesn't send reports"), "{err}");
        }
    }
}
