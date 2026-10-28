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
/// loses.
pub fn log_to_file() {
    let Some(dir) = crate::library::data_folder().map(|d| d.join("Logs")) else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let log = dir.join("app.log");
    if log.exists() {
        let _ = std::fs::rename(&log, dir.join("app.previous.log"));
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
    // what it is, who it's from, and what the user said
    let mut about = format!(
        "Visual Pinball Infinite {} — a problem report, made {}\nNothing in it was sent anywhere: attach it to your post on Discord (#bug-reports) or a GitHub issue.\n\nWhat happened, in your words:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        crate::library::stamp(),
        if asked.note.trim().is_empty() { "(nothing written)" } else { asked.note.trim() },
    );
    if asked.pc {
        about.push_str("\nThis PC:\n");
        about.push_str(&this_pc());
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
    }
    if asked.settings {
        if let Some(main) = vpi_settings::settings_path()
            && let Ok(bytes) = std::fs::read(&main)
        {
            put("settings/Settings.ini", &without_keys(&String::from_utf8_lossy(&bytes)))?;
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

/// Whether this build sends reports, or only makes them for the user to attach to a post.
pub fn can_send() -> bool {
    SEND_TO.is_some()
}

/// A report's files as Send passes them on: each one in the zip under the plain name the reports
/// service takes it by, as text.
const PARTS: [(&str, &str); 9] = [
    ("about.txt", "about.txt"),
    ("table.json", "table.json"),
    ("settings/the table's own.ini", "table.ini"),
    ("settings/Settings.ini", "Settings.ini"),
    ("settings/Machine settings.ini", "Machine-settings.ini"),
    ("logs/app.log", "app.log"),
    ("logs/app.previous.log", "app.previous.log"),
    ("logs/player.log", "player.log"),
    ("logs/player.previous.log", "player.previous.log"),
];

/// The most of one file Send passes on; a longer one keeps its end, the newest lines of a log.
const PART_MOST: usize = 2 * 1024 * 1024;

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
/// on one by one as plain text (never the zip), through Windows' own curl with no window; the
/// service's answer said back.
pub fn send(path: &Path, note: &str, table: Option<&str>) -> Result<(), String> {
    let Some(address) = SEND_TO else {
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
    let sent = (|| -> Result<(), String> {
        let version = env!("CARGO_PKG_VERSION");
        let mut curl = ::library::vps::curl();
        curl.args(["-sS", "--max-time", "120", "-A", &format!("VisualPinballInfinite/{version}")])
            .arg("--form-string")
            .arg(format!("note={}", note.chars().take(1500).collect::<String>()))
            .arg("--form-string")
            .arg(format!("version={version}"))
            .arg("--form-string")
            .arg(format!("table={}", table.unwrap_or("")));
        for (name, text) in &parts {
            let file = dir.join(name);
            std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
            curl.arg("-F").arg(format!("files=@\"{}\";type=text/plain;filename={name}", file.display()));
        }
        curl.arg("-w").arg("\n%{http_code}").arg(address);
        let out = curl.output().map_err(|e| format!("The report couldn't be sent: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let (body, code) = text.rsplit_once('\n').unwrap_or((text.as_str(), ""));
        if code.trim() == "200" {
            return Ok(());
        }
        // the service's own words when it said why, else curl's
        let said = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| v["error"].as_str().map(str::to_string));
        Err(said.unwrap_or_else(|| {
            let err = String::from_utf8_lossy(&out.stderr);
            let why = if err.trim().is_empty() { format!("answer {}", code.trim()) } else { err.trim().to_string() };
            format!("The report couldn't be sent ({why}). Show it, and attach it to a post on Discord or GitHub instead.")
        }))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

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
                ("something else.exe", "MZ".to_string()),
            ] {
                zip.start_file(name, options).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let parts = parts(&path).unwrap();
        let names: Vec<&str> = parts.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["about.txt", "table.ini", "player.log"]);
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
            for (name, text) in [("about.txt", "Visual Pinball Infinite: a check of Send, made by the app's own tests.\n"), ("logs/app.log", "a line of a log\n")] {
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
