//! A table tested in VPX itself, unseen and unheard, for the app's Test it and the test of new
//! tables: VPX's capture mode (`-CaptureAttract`), which runs a table's first 30 seconds as fast as
//! it can, one more as it comes, then takes a picture a second, stopping at once on a script error
//! — on a desktop of its own that's never shown, with a copy of VPX's settings in a passing folder
//! (`-PrefPath`): a small window, no backglass, score or topper window, its sound at nothing, no
//! toys or real DMD panel. Its log says what happened: each script error, the machine its PinMAME
//! started; and a message box VPX or the table shows there (a table asking for something, PinMAME
//! missing a ROM) is read and closed, as nobody's there to — the run going on, and the message told
//! (`MESSAGE`, a line of the log handed back). The pictures it takes, in the table folder's
//! `Capture`, are taken out again. Ours.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::test::{Report, Script};
use crate::vpx::Vpx;
use vpi_settings::Change;

/// The pictures taken, a second of the table's time apart, after its first 30 seconds: for a
/// test, and for a machine's first start. VPX's scripts keep PinMAME in step with the table's time
/// (`core.vbs`, `Controller.TimeFence`), so a machine boots in those 30 seconds however fast VPX
/// runs them.
const PICTURES: &str = "10";
const FIRST_START_PICTURES: &str = "15";
/// How long VPX is given before it's stopped.
const GIVEN_S: u64 = 120;
/// How a message box VPX showed is told in the log handed back.
const MESSAGE: &str = "ERROR [VPI] A message VPX showed: ";

/// VPX's settings for a test: the user's own, unseen and unheard.
pub(crate) fn quiet(settings: &str) -> String {
    let set = |s: &str, k: &str, v: i32| Change::set(s, k, v);
    vpi_settings::edit(
        settings,
        &[
            set("Player", "PlayfieldFullScreen", 0),
            set("Player", "PlayfieldWndX", 0),
            set("Player", "PlayfieldWndY", 0),
            set("Player", "PlayfieldWidth", 640),
            set("Player", "PlayfieldHeight", 480),
            set("Player", "SoundVolume", 0),
            set("Player", "MusicVolume", 0),
            set("Player", "MaxFramerate", 30),
            set("Player", "BGSet", 0),
            set("Backglass", "BackglassOutput", 0),
            set("ScoreView", "ScoreViewOutput", 0),
            set("Topper", "TopperOutput", 0),
            set("Plugin.DOF", "Enable", 0),
            set("Plugin.DMDUtil", "Enable", 0),
        ],
    )
}

/// What VPX's log says of a test: each script error — the first as VPX words it — the machine
/// PinMAME started, and whether the script started at all.
fn read_log(log: &str) -> (Script, Option<String>) {
    let mut s = Script::default();
    let (mut started, mut first) = (false, None);
    let said = |line: &str| line.split("] ").last().unwrap_or(line).trim().to_string();
    for line in log.lines() {
        if line.contains("Starting script") {
            started = true;
        }
        if let Some(at) = line.find("Game found: name=") {
            s.machine = line[at + 17..].split([',', ' ']).next().map(str::to_string).filter(|m| !m.is_empty());
        }
        if let Some(at) = line.find(MESSAGE) {
            // a message box shown, told as it said
            s.messages.push(line[at + MESSAGE.len()..].trim().to_string());
        } else if line.contains("ERROR") && (line.contains("Compile error on line") || line.contains("Runtime error on line")) {
            s.errors += 1;
            first.get_or_insert_with(|| said(line));
        } else if line.contains("ERROR") && line.contains("PinMAME") && s.machine.is_none() {
            s.machine_problem = Some(said(line));
        }
    }
    if !started && s.errors == 0 {
        s.failed = Some("VPX didn't start its script".into());
    }
    (s, first)
}

/// What a test in VPX comes to: `good` or `bad`, and in words.
fn verdict(s: &Script, first: Option<&str>, code: Option<i32>) -> (&'static str, String) {
    if let Some(f) = &s.failed {
        return ("bad", format!("{f}{}", code.filter(|c| *c != 0).map_or(String::new(), |c| format!(" (it stopped with {c})"))));
    }
    if s.errors > 0 {
        let first = first.map_or(String::new(), |e| format!(": {e}"));
        return ("bad", if s.errors == 1 { format!("A script error in VPX{first}") } else { format!("{} script errors in VPX{first}", s.errors) });
    }
    if let Some(m) = &s.machine_problem {
        return ("bad", format!("Its machine didn't start in VPX: {m}"));
    }
    let machine = s.machine.as_ref().map_or(String::new(), |m| format!(", its machine running ({m})"));
    if let Some(message) = s.messages.first() {
        return ("warn", format!("It plays in VPX, but showed a message as it started: “{message}”{machine}"));
    }
    ("good", format!("It plays in VPX — its script ran without an error{machine}"))
}

/// `table` tested in `vpx`, its settings `settings` (VPX's own, as VPX reads them) made unseen and
/// unheard.
pub fn test(vpx: &Vpx, table: &Path, settings: &str) -> Report {
    let started = Instant::now();
    let ((script, first), code) = match run(vpx, table, settings, PICTURES) {
        Ok((log, code)) => (read_log(&log), code),
        Err(e) => ((Script { failed: Some(e), ..Default::default() }, None), None),
    };
    let (look, text) = verdict(&script, first.as_deref(), code);
    Report { physics: None, script, look: look.to_string(), text, seconds: started.elapsed().as_secs_f64() }
}

/// A ROM machine's first start, where VPX's PinMAME has no memory of it yet: the table run unseen
/// and unheard as for a test, so its machine restores its factory settings and is switched off
/// after, its memory kept where VPX's PinMAME keeps it — its next start, the game, a normal one.
pub fn first_start(vpx: &Vpx, table: &Path, settings: &str) -> Result<(), String> {
    let (script, _) = read_log(&run(vpx, table, settings, FIRST_START_PICTURES)?.0);
    match script.machine_problem.or(script.failed) {
        Some(why) => Err(why),
        None => Ok(()),
    }
}

/// A script error as VPX's log tells it: its line, its column, and what it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError {
    pub line: u32,
    pub column: i32,
    pub text: String,
}

/// Each script error VPX's log tells (`Compile error on line 120, col 4: …`, `Runtime error …`).
fn script_errors(log: &str) -> Vec<ScriptError> {
    log.lines()
        .filter(|l| l.contains("ERROR"))
        .filter_map(|l| {
            let at = l.find(" error on line ")?;
            let rest = &l[at + 15..];
            let (line, rest) = rest.split_once(", col ")?;
            let (column, text) = rest.split_once(": ")?;
            Some(ScriptError { line: line.trim().parse().ok()?, column: column.trim().parse().unwrap_or(0), text: format!("{} error: {}", l[..at].rsplit(' ').next().unwrap_or("A"), text.trim()) })
        })
        .collect()
}

/// `table` run in `vpx` as for a test — its script the one beside it, where there is one — its
/// script's errors as VPX's log tells them, and why it didn't run, where it didn't.
pub fn check(vpx: &Vpx, table: &Path, settings: &str) -> Result<(Vec<ScriptError>, Option<String>), String> {
    let (log, code) = run(vpx, table, settings, PICTURES)?;
    let (script, _) = read_log(&log);
    let errors = script_errors(&log);
    let failed = script.failed.filter(|_| errors.is_empty()).map(|f| format!("{f}{}", code.filter(|c| *c != 0).map_or(String::new(), |c| format!(" (it stopped with {c})"))));
    Ok((errors, failed))
}

/// The table run unseen and unheard: what VPX's log says, and VPX's exit code (None when it was
/// stopped).
pub(crate) fn run(vpx: &Vpx, table: &Path, settings: &str, pictures: &str) -> Result<(String, Option<i32>), String> {
    // no script runs in VPX without Windows' VBScript, and VPX says nothing of it
    if !crate::vpx::vbscript_ready() {
        return Err(crate::vpx::NO_VBSCRIPT.into());
    }
    // a folder for each run: a test and a first start can run at once
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let prefs = std::env::temp_dir().join(format!("vpi-vpx-test-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prefs);
    std::fs::create_dir_all(&prefs).map_err(|e| format!("{}: {e}", prefs.display()))?;
    std::fs::write(prefs.join("VPinballX.ini"), quiet(settings)).map_err(|e| format!("{}: {e}", prefs.display()))?;
    // the pictures it takes go in the table folder's Capture: taken out again
    let capture = table.parent().map(|d| d.join("Capture")).unwrap_or_default();
    let before: Vec<PathBuf> = std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).collect();
    let had = capture.is_dir();
    let args = [std::ffi::OsString::from("-PrefPath"), prefs.clone().into(), "-CaptureAttract".into(), pictures.into(), "1".into(), table.into()];
    let (code, messages) = unseen_reading(&vpx.program, &args, GIVEN_S)?;
    for e in std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !before.contains(p)) {
        let _ = if e.is_dir() { std::fs::remove_dir_all(&e) } else { std::fs::remove_file(&e) };
    }
    if !had {
        let _ = std::fs::remove_dir(&capture);
    }
    let mut log = std::fs::read(prefs.join("vpinball.log")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&prefs);
    // stopped by Windows as it loaded its files: no log to read, and why said
    if let Some(why) = code.and_then(crate::vpx::stopped_loading) {
        return Err(why);
    }
    for m in messages {
        log.push_str(&format!("\n{MESSAGE}{m}"));
    }
    Ok((log, code))
}

/// `program` run unseen (`unseen_reading`): its exit code, for checks.
#[cfg(all(windows, test))]
pub(crate) fn unseen(program: &Path, args: &[std::ffi::OsString], seconds: u64) -> Result<Option<i32>, String> {
    unseen_reading(program, args, seconds).map(|(code, _)| code)
}

/// `program` run with `args` on a desktop of its own that's never shown, its error dialogs off,
/// stopped after `seconds`: its exit code, None when it was stopped; and each message box it showed
/// there, read and closed as it came (its title and words), so a run waits on none.
#[cfg(windows)]
pub(crate) fn unseen_reading(program: &Path, args: &[std::ffi::OsString], seconds: u64) -> Result<(Option<i32>, Vec<String>), String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[repr(C)]
    struct StartupInfo {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        x_size: u32,
        y_size: u32,
        x_count_chars: u32,
        y_count_chars: u32,
        fill_attribute: u32,
        flags: u32,
        show_window: u16,
        reserved2_len: u16,
        reserved2: *mut u8,
        std_input: *mut c_void,
        std_output: *mut c_void,
        std_error: *mut c_void,
    }
    #[repr(C)]
    struct ProcessInformation {
        process: *mut c_void,
        thread: *mut c_void,
        process_id: u32,
        thread_id: u32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn CreateDesktopW(name: *const u16, device: *const u16, mode: *const c_void, flags: u32, access: u32, attributes: *const c_void) -> *mut c_void;
        fn CloseDesktop(desktop: *mut c_void) -> i32;
        fn EnumDesktopWindows(desktop: *mut c_void, each: unsafe extern "system" fn(*mut c_void, isize) -> i32, data: isize) -> i32;
        fn EnumChildWindows(parent: *mut c_void, each: unsafe extern "system" fn(*mut c_void, isize) -> i32, data: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
        fn GetClassNameW(hwnd: *mut c_void, name: *mut u16, max: i32) -> i32;
        fn GetWindowTextW(hwnd: *mut c_void, text: *mut u16, max: i32) -> i32;
        fn PostMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn IsWindow(hwnd: *mut c_void) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateProcessW(app: *const u16, line: *mut u16, pa: *const c_void, ta: *const c_void, inherit: i32, flags: u32, env: *const c_void, dir: *const u16, si: *const StartupInfo, pi: *mut ProcessInformation) -> i32;
        fn WaitForSingleObject(handle: *mut c_void, ms: u32) -> u32;
        fn TerminateProcess(handle: *mut c_void, code: u32) -> i32;
        fn GetExitCodeProcess(handle: *mut c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn SetErrorMode(mode: u32) -> u32;
    }
    const GENERIC_ALL: u32 = 0x1000_0000;
    // CREATE_DEFAULT_ERROR_MODE left out: the error mode set here goes with it
    const SEM: u32 = 0x8003;
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain([0]).collect::<Vec<u16>>();
    let quote = |s: &std::ffi::OsStr| {
        let t = s.to_string_lossy();
        if t.contains([' ', '\t']) { format!("\"{t}\"") } else { t.into_owned() }
    };
    let mut line: Vec<u16> = std::iter::once(quote(program.as_os_str())).chain(args.iter().map(|a| quote(a))).collect::<Vec<_>>().join(" ").encode_utf16().chain([0]).collect();
    let name = format!("vpi-unseen-{}", std::process::id());
    let mut desktop_name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    let program_w = wide(program.as_os_str());
    let dir_w = program.parent().map(|d| wide(d.as_os_str()));
    // SAFETY: NUL-ended names; the desktop is closed below, once the program has ended
    let desktop = unsafe { CreateDesktopW(desktop_name.as_ptr(), std::ptr::null(), std::ptr::null(), 0, GENERIC_ALL, std::ptr::null()) };
    if desktop.is_null() {
        return Err(format!("A desktop of its own couldn't be made for VPX ({})", std::io::Error::last_os_error()));
    }
    let si = StartupInfo {
        cb: std::mem::size_of::<StartupInfo>() as u32,
        reserved: std::ptr::null_mut(),
        desktop: desktop_name.as_mut_ptr(),
        title: std::ptr::null_mut(),
        x: 0,
        y: 0,
        x_size: 0,
        y_size: 0,
        x_count_chars: 0,
        y_count_chars: 0,
        fill_attribute: 0,
        flags: 0,
        show_window: 0,
        reserved2_len: 0,
        reserved2: std::ptr::null_mut(),
        std_input: std::ptr::null_mut(),
        std_output: std::ptr::null_mut(),
        std_error: std::ptr::null_mut(),
    };
    let mut pi = ProcessInformation { process: std::ptr::null_mut(), thread: std::ptr::null_mut(), process_id: 0, thread_id: 0 };
    // this program's environment, PATH as a VPX it starts is given it (`vpx::runtime_path`): each
    // `name=value` ended by a NUL, sorted by name as Windows keeps them, then a NUL
    const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;
    let env: Option<Vec<u16>> = crate::vpx::runtime_path().map(|path| {
        let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> = std::env::vars_os().filter(|(k, _)| !k.eq_ignore_ascii_case("PATH")).collect();
        vars.push(("PATH".into(), path));
        vars.sort_by_key(|(k, _)| k.to_string_lossy().to_uppercase());
        let mut block = Vec::new();
        for (k, v) in &vars {
            block.extend(k.encode_wide());
            block.push(u16::from(b'='));
            block.extend(v.encode_wide());
            block.push(0);
        }
        block.push(0);
        block
    });
    let (flags, env_ptr) = env.as_ref().map_or((0, std::ptr::null()), |b| (CREATE_UNICODE_ENVIRONMENT, b.as_ptr().cast::<c_void>()));
    // SAFETY: the error mode this process starts it with: no dialog waits on the unseen desktop
    let mode_before = unsafe { SetErrorMode(SEM) };
    // SAFETY: a writable, NUL-ended command line; an environment block ended as Windows reads one,
    // or none; `si`, `pi` and the block outlive the call
    let made = unsafe { CreateProcessW(program_w.as_ptr(), line.as_mut_ptr(), std::ptr::null(), std::ptr::null(), 0, flags, env_ptr, dir_w.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()), &si, &mut pi) };
    // SAFETY: as it was
    unsafe { SetErrorMode(mode_before) };
    if made == 0 {
        let e = std::io::Error::last_os_error();
        // SAFETY: the desktop made above
        unsafe { CloseDesktop(desktop) };
        return Err(format!("VPX didn't start: {e}"));
    }
    // a window's class name and text, as Windows gives them
    let read = |hwnd: *mut c_void, class: bool| -> String {
        let mut buf = [0u16; 1024];
        // SAFETY: a window Windows handed over; a buffer of the size given
        let n = unsafe { if class { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) } else { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) } };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    };
    // the dialog boxes the program shows on its desktop
    unsafe extern "system" fn each(hwnd: *mut c_void, data: isize) -> i32 {
        // SAFETY: `data` is the list below, alive through the enumeration
        let found = unsafe { &mut *(data as *mut Vec<usize>) };
        found.push(hwnd as usize);
        1
    }
    let dialogs = |pid: u32| -> Vec<usize> {
        let mut all: Vec<usize> = Vec::new();
        // SAFETY: the desktop made above; the callback fills the list, which outlives the call
        unsafe { EnumDesktopWindows(desktop, each, &mut all as *mut Vec<usize> as isize) };
        all.into_iter()
            .filter(|&h| {
                let mut owner = 0;
                // SAFETY: a window Windows handed over
                unsafe { GetWindowThreadProcessId(h as *mut c_void, &mut owner) };
                owner == pid && read(h as *mut c_void, true) == "#32770"
            })
            .collect()
    };
    // a dialog's words: its title, then the text of each of its labels
    let words = |h: usize| -> String {
        let mut children: Vec<usize> = Vec::new();
        // SAFETY: a window Windows handed over; the callback fills the list, which outlives the call
        unsafe { EnumChildWindows(h as *mut c_void, each, &mut children as *mut Vec<usize> as isize) };
        let said: Vec<String> = children.into_iter().filter(|&c| read(c as *mut c_void, true).eq_ignore_ascii_case("Static")).map(|c| read(c as *mut c_void, false)).map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|t| !t.is_empty()).collect();
        let title = read(h as *mut c_void, false);
        if said.is_empty() { title } else { format!("{title}: {}", said.join(" ")) }
    };
    const WM_CLOSE: u32 = 0x0010;
    const WM_COMMAND: u32 = 0x0111;
    // IDOK, IDYES, IDCANCEL, IDNO: what a box takes to close, tried in turn after WM_CLOSE
    const ANSWERS: [usize; 4] = [1, 6, 2, 7];
    let mut messages: Vec<String> = Vec::new();
    let mut tries: Vec<(usize, usize)> = Vec::new();
    let until = Instant::now() + std::time::Duration::from_secs(seconds);
    // SAFETY: the handles CreateProcessW handed over, each closed once; the dialogs are the
    // program's own, on its own desktop
    let code = unsafe {
        let ended = loop {
            if WaitForSingleObject(pi.process, 500) == 0 {
                break true;
            }
            for h in dialogs(pi.process_id) {
                match tries.iter_mut().find(|(w, _)| *w == h) {
                    None => {
                        messages.push(words(h));
                        tries.push((h, 0));
                        PostMessageW(h as *mut c_void, WM_CLOSE, 0, 0);
                    }
                    Some((_, n)) if IsWindow(h as *mut c_void) != 0 && *n < ANSWERS.len() => {
                        PostMessageW(h as *mut c_void, WM_COMMAND, ANSWERS[*n], 0);
                        *n += 1;
                    }
                    _ => {}
                }
            }
            if Instant::now() >= until {
                break false;
            }
        };
        if !ended {
            TerminateProcess(pi.process, 1);
            WaitForSingleObject(pi.process, 10_000);
        }
        let mut code = 0u32;
        GetExitCodeProcess(pi.process, &mut code);
        CloseHandle(pi.thread);
        CloseHandle(pi.process);
        CloseDesktop(desktop);
        ended.then_some(code as i32)
    };
    Ok((code, messages))
}

#[cfg(not(windows))]
pub(crate) fn unseen_reading(_program: &Path, _args: &[std::ffi::OsString], _seconds: u64) -> Result<(Option<i32>, Vec<String>), String> {
    Err("VPX's tests run on Windows".into())
}

#[cfg(all(not(windows), test))]
pub(crate) fn unseen(_program: &Path, _args: &[std::ffi::OsString], _seconds: u64) -> Result<Option<i32>, String> {
    Err("VPX's tests run on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine's first start in VPX, for real: `VPI_TEST_TABLE=<a table in a test library, VPX
    /// in the folder above its tables> VPI_TEST_ROM=<its set> cargo test -p library --lib
    /// a_machines_first_start_in_vpx -- --ignored`, the table bringing its own ROM and no memory.
    #[test]
    #[ignore]
    fn a_machines_first_start_in_vpx() {
        let table = PathBuf::from(std::env::var_os("VPI_TEST_TABLE").expect("VPI_TEST_TABLE"));
        let rom = std::env::var("VPI_TEST_ROM").expect("VPI_TEST_ROM");
        let vpx = crate::vpx::newest_in(&table.ancestors().skip(2).take(2).map(Path::to_path_buf).collect::<Vec<_>>()).expect("VPX above the tables");
        let memory = table.parent().unwrap().join("pinmame").join("nvram").join(format!("{rom}.nv"));
        assert!(!memory.exists(), "the table's machine has a memory already");
        let settings = vpi_settings::vpx_settings_text();
        let started = Instant::now();
        first_start(&vpx, &table, &settings).unwrap();
        println!("first start: {:.1} s, memory {} bytes", started.elapsed().as_secs_f64(), std::fs::metadata(&memory).map_or(0, |m| m.len()));
        assert!(memory.is_file());
    }

    /// Tables run unseen in a test library's VPX (`VPI_TEST_LIBRARY`) with none of ours — its own
    /// settings, our add-on switched off (VPX loads none switched off) — to tell a VPX build's own
    /// problems from ours: what its log says is wrong, and the messages it showed.
    /// `VPI_TEST_TABLES=<a table file>;... cargo test --release -p library --lib
    /// a_table_in_vpx_with_none_of_ours -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_table_in_vpx_with_none_of_ours() {
        let lib = std::fs::canonicalize(std::env::var_os("VPI_TEST_LIBRARY").expect("VPI_TEST_LIBRARY")).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        let own = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let settings = vpi_settings::edit(&own, &[Change::set(&format!("Plugin.{}", crate::vpx_addon::ID), "Enable", 0)]);
        println!("VPX {:?}", vpx.version);
        for table in std::env::var("VPI_TEST_TABLES").expect("VPI_TEST_TABLES").split(';').map(str::trim).filter(|t| !t.is_empty()) {
            let (log, code) = run(&vpx, Path::new(table), &settings, PICTURES).unwrap();
            println!("{}: ended {code:?}; ours loaded: {}", Path::new(table).file_name().unwrap().to_string_lossy(), log.contains(&format!("Plugin {} loaded", crate::vpx_addon::ID)));
            for line in log.lines().filter(|l| l.contains(" ERROR ") || l.contains("error on line") || l.contains(MESSAGE)) {
                println!("  {line}");
            }
        }
    }

    /// Tables of the library tested in the library's own VPX with the user's own settings, as the
    /// app's Test it tests them (unseen and unheard): `VPI_TEST_TABLES=<a table file>;<another>
    /// cargo test --release -p library --lib tables_of_the_library_tested_in_its_vpx -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore]
    fn tables_of_the_library_tested_in_its_vpx() {
        let tables = std::env::var("VPI_TEST_TABLES").expect("VPI_TEST_TABLES");
        let layout = crate::plan::Layout::from_folders(&vpi_settings::library_folders_now().expect("a library"));
        let vpx = crate::vpx::find(&layout).expect("the library's VPX");
        println!("VPX {:?} in {}", vpx.version, vpx.program.display());
        // our add-on as built here, as PLAY puts it in before a game
        crate::vpx_addon::put_in(&vpx).unwrap();
        let settings = vpi_settings::vpx_settings_text();
        let mut bad = Vec::new();
        for table in tables.split(';').map(str::trim).filter(|t| !t.is_empty()) {
            let r = test(&vpx, Path::new(table), &settings);
            println!("{}: {} ({:.1} s): {}", Path::new(table).file_name().unwrap().to_string_lossy(), r.look, r.seconds, r.text);
            if r.look == "bad" {
                // VPX's own log's end, for what it was doing
                if bad.is_empty()
                    && let Ok((log, _)) = run(&vpx, Path::new(table), &settings, PICTURES)
                {
                    let lines: Vec<&str> = log.lines().collect();
                    println!("--- the end of VPX's log\n{}", lines[lines.len().saturating_sub(25)..].join("\n"));
                }
                bad.push(table.to_string());
            }
        }
        assert!(bad.is_empty(), "{bad:?}");
    }

    /// A table's picture swapped without a copy of the table: lines added at the end of the script
    /// beside it (`<table>.vbs`), which VPX reads in the table's own's place — VPX 10.8.1's
    /// `LoadTexture` takes a file under a name of its own, and parts are pointed at it
    /// (`VPI_TEST_PICTURE_ON`, `ActiveTable.Image` unless set; a file of one's own,
    /// `VPI_TEST_PICTURE_FILE`). Captured unseen twice, as the table is and swapped, the frames kept
    /// beside the copy (`Capture-own`, `Capture-swapped`, QOI) for looking at:
    /// `VPI_TEST_LIBRARY=<a test library, VPX in it> VPI_TEST_PICTURE_TABLE=<a table folder in its
    /// Tables> VPI_TEST_PICTURE=<the picture name its playfield shows> cargo test --release -p library
    /// --lib a_tables_picture_swapped_by_its_script_in_vpx -- --ignored --nocapture`. Build 5436, on
    /// Leprechaun King: the picture loads without an error and the parts lose their own, but VPX
    /// draws them plain grey, the playfield and the flippers alike — no way to swap a picture yet.
    #[test]
    #[ignore]
    fn a_tables_picture_swapped_by_its_script_in_vpx() {
        let lib = PathBuf::from(std::env::var_os("VPI_TEST_LIBRARY").expect("VPI_TEST_LIBRARY"));
        let folder = std::env::var("VPI_TEST_PICTURE_TABLE").expect("VPI_TEST_PICTURE_TABLE");
        let picture = std::env::var("VPI_TEST_PICTURE").expect("VPI_TEST_PICTURE");
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).expect("VPX in the test library");
        assert!(vpx.has_addons(), "{:?}", vpx.version);
        let from = lib.join("Tables").join(&folder);
        let original = std::fs::read_dir(&from).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx"))).expect("a table file");
        let dir = lib.join("Tables").join(format!("{folder} (pictures test)"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("Pictures test.vpx");
        std::fs::copy(&original, &table).unwrap();
        // a picture of red and white stripes, as a PNG
        let png = dir.join("stripes.png");
        let png = std::env::var_os("VPI_TEST_PICTURE_FILE").map(PathBuf::from).unwrap_or(png);
        image::RgbaImage::from_fn(512, 1024, |_, y| if (y / 64) % 2 == 0 { image::Rgba([220, 30, 30, 255]) } else { image::Rgba([245, 245, 245, 255]) }).save(&png).unwrap();
        let capture_into = |name: &str| -> (Option<i32>, Vec<String>, usize) {
            let prefs = dir.join(format!("prefs-{name}"));
            std::fs::create_dir_all(&prefs).unwrap();
            std::fs::write(prefs.join("VPinballX.ini"), quiet(&vpi_settings::vpx_settings_text())).unwrap();
            let args = [std::ffi::OsString::from("-PrefPath"), prefs.clone().into(), "-CaptureAttract".into(), "3".into(), "1".into(), table.clone().into(), "noloop".into()];
            let (code, messages) = unseen_reading(&vpx.program, &args, GIVEN_S).unwrap();
            let capture = dir.join("Capture");
            let kept = dir.join(format!("Capture-{name}"));
            let _ = std::fs::remove_dir_all(&kept);
            let frames = std::fs::read_dir(&capture).into_iter().flatten().flatten().count();
            if capture.is_dir() {
                std::fs::rename(&capture, &kept).unwrap();
            }
            let log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
            let errors: Vec<String> = log.lines().filter(|l| l.contains("error") || l.contains("ERROR")).map(str::to_string).collect();
            println!("{name}: exit {code:?}, {frames} files kept in {}, messages {messages:?}", kept.display());
            (code, errors, frames)
        };
        let (_, own_errors, own_frames) = capture_into("own");
        // the table's own script, then the lines that swap its playfield's picture
        let mut script = crate::table::own_script(&table).unwrap();
        let target = std::env::var("VPI_TEST_PICTURE_ON").unwrap_or_else(|_| "ActiveTable.Image".into());
        let on: String = target.split(',').map(|t| format!("{} = \"vpi_{picture}\"\r\n", t.trim())).collect();
        script.extend_from_slice(format!("\r\n' a picture of the player's own in the table's place\r\nLoadTexture \"vpi_{picture}\", \"{}\"\r\n{on}", png.display()).as_bytes());
        std::fs::write(table.with_extension("vbs"), script).unwrap();
        let (_, swapped_errors, swapped_frames) = capture_into("swapped");
        println!("errors as it is: {own_errors:?}\nerrors swapped: {swapped_errors:?}");
        assert!(own_frames > 0 && swapped_frames > 0);
    }

    /// A copy of a table with its playfield's picture swapped for one of the player's own, written
    /// by `alt_pictures::make_copy`, played unseen in VPX as the table is: the copy's frames kept
    /// beside it (`Capture-copy`, QOI) for looking at, and VPX's log read for errors.
    /// `VPI_TEST_LIBRARY=<a test library, VPX in it> VPI_TEST_PICTURE_TABLE=<a table folder in its
    /// Tables> cargo test --release -p library --lib a_copy_with_its_pictures_swapped_plays_in_vpx --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_copy_with_its_pictures_swapped_plays_in_vpx() {
        let lib = PathBuf::from(std::env::var_os("VPI_TEST_LIBRARY").expect("VPI_TEST_LIBRARY"));
        let folder = std::env::var("VPI_TEST_PICTURE_TABLE").expect("VPI_TEST_PICTURE_TABLE");
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).expect("VPX in the test library");
        let from = lib.join("Tables").join(&folder);
        let original = std::fs::read_dir(&from).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx")) && !p.to_string_lossy().contains("(alt pictures)")).expect("a table file");
        let dir = lib.join("Tables").join(format!("{folder} (copy test)"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("Copy test.vpx");
        std::fs::copy(&original, &table).unwrap();
        // the playfield's own picture, by its name in the table, swapped for red and white stripes
        let playfield = vpin::vpx::read(&table).unwrap().gamedata.image;
        assert!(!playfield.is_empty(), "the table names no playfield picture");
        let pictures = dir.join("Copy test pictures");
        std::fs::create_dir_all(&pictures).unwrap();
        let png = pictures.join(format!("{playfield}.png"));
        image::RgbaImage::from_fn(512, 1024, |_, y| if (y / 64) % 2 == 0 { image::Rgba([220, 30, 30, 255]) } else { image::Rgba([245, 245, 245, 255]) }).save(&png).unwrap();
        let made = crate::alt_pictures::make_copy(&table, &[png], &crate::alt_pictures::copy_stem(&table)).unwrap();
        println!("copy: {} — swapped {:?}, companions {:?}", made.table.display(), made.swapped, made.companions);
        let prefs = dir.join("prefs-copy");
        std::fs::create_dir_all(&prefs).unwrap();
        std::fs::write(prefs.join("VPinballX.ini"), quiet(&vpi_settings::vpx_settings_text())).unwrap();
        let args = [std::ffi::OsString::from("-PrefPath"), prefs.clone().into(), "-CaptureAttract".into(), "3".into(), "1".into(), made.table.clone().into(), "noloop".into()];
        let (code, messages) = unseen_reading(&vpx.program, &args, GIVEN_S).unwrap();
        let capture = dir.join("Capture");
        let kept = dir.join("Capture-copy");
        let frames = std::fs::read_dir(&capture).into_iter().flatten().flatten().count();
        if capture.is_dir() {
            std::fs::rename(&capture, &kept).unwrap();
        }
        let log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
        let errors: Vec<&str> = log.lines().filter(|l| l.contains("ERROR") && !l.contains("[PUP")).collect();
        println!("exit {code:?}, {frames} frames in {}, messages {messages:?}\nerrors: {errors:?}", kept.display());
        assert!(frames > 0, "VPX drew nothing of the copy");
    }

    #[test]
    fn what_vpxs_log_says_of_a_test() {
        let good = "2000-01-01 00:00:00.000 INFO  [Main] [Player::Player@727] Starting script\n\
                    INFO  [Main] [PinMAME:PinMAME::Controller::SetGameName::<lambda_1>::operator ()@130] Game found: name=afm_113b, description=Attack From Mars\n";
        let (s, first) = read_log(good);
        assert_eq!((s.errors, s.machine.as_deref(), s.failed.is_none()), (0, Some("afm_113b"), true));
        assert_eq!(verdict(&s, first.as_deref(), Some(0)).0, "good");
        let bad = "INFO  [Main] [Player::Player@727] Starting script\n\
                   ERROR [Main] [Player::OnScriptError@1238] Runtime error on line 120, col 4: Object required: 'Controller'\n";
        let (s, first) = read_log(bad);
        let (look, text) = verdict(&s, first.as_deref(), Some(0));
        assert_eq!((s.errors, look), (1, "bad"));
        assert_eq!(text, "A script error in VPX: Runtime error on line 120, col 4: Object required: 'Controller'");
        assert!(read_log("").0.failed.is_some());
        assert_eq!(
            script_errors(bad),
            [ScriptError { line: 120, column: 4, text: "Runtime error: Object required: 'Controller'".into() }]
        );
        // unseen and unheard, the user's own settings otherwise as they were
        let q = vpi_settings::parse_ini(&quiet("[Player]\nSoundVolume = 80\nPlayfieldDisplay = Main Screen\n[Input]\nMapping.Start = K2\n"));
        assert_eq!(vpi_settings::get(&q, "Player", "SoundVolume"), Some("0"));
        assert_eq!(vpi_settings::get(&q, "Backglass", "BackglassOutput"), Some("0"));
        assert_eq!(vpi_settings::get(&q, "Plugin.DOF", "Enable"), Some("0"));
        assert_eq!(vpi_settings::get(&q, "Input", "Mapping.Start"), Some("K2"));
        // a message box read and closed: said, the test a warning
        let shown = format!("INFO  [Main] [Player::Player@727] Starting script\n{MESSAGE}VBScript: The table needs its DMD folder\n");
        let (s, first) = read_log(&shown);
        let (look, text) = verdict(&s, first.as_deref(), Some(0));
        assert_eq!(look, "warn");
        assert_eq!(text, "It plays in VPX, but showed a message as it started: “VBScript: The table needs its DMD folder”");
    }
}
