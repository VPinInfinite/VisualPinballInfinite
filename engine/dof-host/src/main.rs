// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! `vpi-dof`: drives a cabinet's toys beside the player through
//! libdof, the VPX team's C++ port of the DirectOutput Framework — our build
//! of it at VPX 5436's commit (`tools\dof\setup.ps1`), else VPX 5436's own,
//! `dof64.dll` (libdof 0.4.7), with the USB libraries it uses beside it. The player creates the link, starts this program with
//! the table, the name its toys are configured under (the ROM, or the
//! script's `cGameName` for a table without one) and the folder holding the
//! configuration, and hands over the game's events as VPX 5436's event
//! stream makes them; this program feeds them to libdof on its own thread,
//! as VPX's DOF plugin does, and libdof drives the output boards its
//! configuration names. With no boards, libdof finds none and the toys stay
//! still. It works in `%APPDATA%\Visual Pinball Infinite\DOF\`, where libdof
//! writes its log (`DirectOutput.log`) unless the configuration puts it
//! elsewhere; the configuration's own folder is only read.
//!
//! ```text
//! vpi-dof --link <name> --table <file> --name <game> --config <folder> [--library <dof64.dll>] [--parent <pid>]
//! vpi-dof --test <event> --name <game> --config <folder> [--ms 500] [--table <file>] [--library <dof64.dll>]
//! ```
//!
//! `--config` is the folder holding `directoutputconfig\` (libdof's base path).
//!
//! `--test` fires one event as the game would — a solenoid `S5`, a lamp `L12`, a switch `W24`, a
//! GI string `G2` or a table's own event `E101` — on for `--ms` milliseconds, then off, with no
//! player: whatever toys the configuration hangs on that event for the game pulse once (for
//! checking a cabinet's toys; the Test buttons come with the settings screen).

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;

use dof_link::{Link, STATE_FAILED, STATE_READY};

const USAGE: &str = "vpi-dof --link <name> --table <file> --name <game> --config <folder> [--library <dof64.dll>] [--parent <pid>]\n       vpi-dof --test <event> --name <game> --config <folder> [--ms 500] [--table <file>] [--library <dof64.dll>]";

unsafe extern "C" {
    fn vpi_dof_load(path: *const u16) -> c_int;
    fn vpi_dof_start(base_path: *const c_char, sink: extern "C" fn(c_int, *const c_char), table: *const c_char, rom: *const c_char) -> c_int;
    fn vpi_dof_data(kind: c_char, number: c_int, value: c_int) -> c_int;
    fn vpi_dof_finish();
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn WaitForMultipleObjects(count: u32, handles: *const *mut c_void, wait_all: i32, milliseconds: u32) -> u32;
}

const SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_OBJECT_0: u32 = 0;

struct Options {
    /// The link, or the event a test fires (its kind and number) and for how long.
    run: Run,
    table: String,
    name: String,
    config: PathBuf,
    library: Option<PathBuf>,
    parent: Option<u32>,
}

enum Run {
    Link(String),
    Test { kind: u8, number: i32, ms: u64 },
}

/// A test's event: a kind letter libdof knows (VPX's DOF plugin's) and its number.
fn parse_event(text: &str) -> Option<(u8, i32)> {
    let kind = *text.as_bytes().first()?;
    let kind = kind.to_ascii_uppercase();
    if !matches!(kind, b'S' | b'L' | b'W' | b'G' | b'E') {
        return None;
    }
    Some((kind, text[1..].parse().ok()?))
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let (mut link, mut table, mut name, mut config, mut library, mut parent) = (None, None, None, None, None, None);
    let (mut test, mut ms) = (None, 500u64);
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{a} needs a value\n{USAGE}"));
        match a.as_str() {
            "--link" => link = Some(value()?),
            "--table" => table = Some(value()?),
            "--name" => name = Some(value()?),
            "--config" => config = Some(PathBuf::from(value()?)),
            "--library" => library = Some(PathBuf::from(value()?)),
            "--parent" => parent = Some(value()?.parse().map_err(|_| "--parent takes a process id".to_string())?),
            "--test" => {
                let event = value()?;
                test = Some(parse_event(&event).ok_or_else(|| format!("--test takes an event such as S5, L12, W24, G2 or E101, not {event}"))?);
            }
            "--ms" => ms = value()?.parse().map_err(|_| "--ms takes milliseconds".to_string())?,
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    let name: String = name.ok_or(USAGE)?;
    let run = match (test, link) {
        (Some((kind, number)), _) => Run::Test { kind, number, ms },
        (None, Some(link)) => Run::Link(link),
        (None, None) => return Err(USAGE.into()),
    };
    // a test without a table names the game alone, as libdof needs only that
    let table = table.or_else(|| matches!(run, Run::Test { .. }).then(|| name.clone())).ok_or(USAGE)?;
    Ok(Options { run, table, name, config: config.ok_or(USAGE)?, library, parent })
}

/// Whether this is a development build, which also looks in the repository's
/// `vpx-build` folders for the libraries it loads. A release build, compiled
/// with `VPI_RELEASE_BUILD` set (`tools\release`), looks beside the program
/// alone.
const DEVELOPMENT: bool = option_env!("VPI_RELEASE_BUILD").is_none();

/// libdof: beside this program (how it ships), else, in the repository, our
/// build of it (`tools\dof\setup.ps1`), else VPX 5436's from its library
/// bundle (development builds).
fn find_library() -> Option<PathBuf> {
    const NAME: &str = "dof64.dll";
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let beside = dir.join(NAME);
    if beside.exists() {
        return Some(beside);
    }
    if !DEVELOPMENT {
        return None;
    }
    let folders = [r"vpx-build\libdof\build\Release", r"vpx-build\downloads\5436-third-party-windows-x64-Release\runtime-libs\windows-x64"];
    folders.iter().find_map(|f| dir.ancestors().take(4).map(|up| up.join(f).join(NAME)).find(|p| p.exists()))
}

/// libdof's log, a line each, as VPX's DOF plugin logs it.
extern "C" fn log(level: c_int, text: *const c_char) {
    if text.is_null() {
        return;
    }
    // SAFETY: the bridge hands over a null-terminated line it formatted
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    let level = match level {
        1 => "warning: ",
        2 => "error: ",
        3 => "debug: ",
        _ => "",
    };
    eprintln!("vpi-dof: {level}{}", text.trim_end());
}

fn c_string(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| format!("{s:?} holds a null character"))
}

fn main() -> ExitCode {
    let options = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("vpi-dof: {e}");
            return ExitCode::FAILURE;
        }
    };
    let link_name = match &options.run {
        Run::Link(name) => name.clone(),
        Run::Test { kind, number, ms } => return test(&options, *kind, *number, *ms),
    };
    let link = match Link::open(&link_name) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vpi-dof: could not open the link {link_name}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let shared = link.shared();
    let started = start(&options);
    if let Err(e) = started {
        eprintln!("vpi-dof: {e}; the toys stay still");
        shared.set_state(STATE_FAILED);
        return ExitCode::FAILURE;
    }
    shared.set_state(STATE_READY);

    // SAFETY: the player's process, waited on so this program ends with it
    let parent = options.parent.map(|pid| unsafe { OpenProcess(SYNCHRONIZE, 0, pid) }).filter(|h| !h.is_null());
    let mut handles = vec![link.to_host_event()];
    handles.extend(parent);
    let (mut failed_said, mut handed) = (false, 0u64);
    loop {
        // SAFETY: waits for new events or the player's end
        let woke = unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, 1000) };
        if parent.is_some() && woke == WAIT_OBJECT_0 + 1 {
            break;
        }
        shared.drain(|e| {
            handed += 1;
            // SAFETY: libdof started; the bridge catches what it throws
            let ok = unsafe { vpi_dof_data(e.kind as u8 as c_char, e.number, e.value) } != 0;
            if !ok && !failed_said {
                failed_said = true;
                eprintln!("vpi-dof: libdof refused the event {}{} = {}", e.kind, e.number, e.value);
            }
        });
        if shared.stop_requested() {
            break;
        }
    }
    // SAFETY: libdof ends the table's toys
    unsafe { vpi_dof_finish() };
    eprintln!("vpi-dof: {handed} events from the game handed to libdof");
    if shared.dropped() > 0 {
        eprintln!("vpi-dof: {} events didn't fit the link and were dropped", shared.dropped());
    }
    ExitCode::SUCCESS
}

/// The test command: libdof started on the game, the event on for `ms`, then off, then libdof
/// given a moment to finish the toys' effects before it ends.
fn test(options: &Options, kind: u8, number: i32, ms: u64) -> ExitCode {
    if let Err(e) = start(options) {
        eprintln!("vpi-dof: {e}");
        return ExitCode::FAILURE;
    }
    let event = format!("{}{number}", kind as char);
    let send = |value: c_int| {
        // SAFETY: libdof started; the bridge catches what it throws
        unsafe { vpi_dof_data(kind as c_char, number, value) != 0 }
    };
    let ok = send(1);
    std::thread::sleep(std::time::Duration::from_millis(ms));
    let ok = send(0) && ok;
    std::thread::sleep(std::time::Duration::from_millis(250));
    // SAFETY: libdof ends the toys
    unsafe { vpi_dof_finish() };
    if ok {
        println!("vpi-dof: {event} sent on for {ms} ms, then off: the toys {}'s configuration hangs on it pulsed (libdof's log: DirectOutput.log)", options.name);
        ExitCode::SUCCESS
    } else {
        eprintln!("vpi-dof: libdof refused {event}");
        ExitCode::FAILURE
    }
}

/// libdof loaded and started on the table, as VPX 5436's DOF plugin starts it.
fn start(o: &Options) -> Result<(), String> {
    let library = o.library.clone().or_else(find_library).ok_or("dof64.dll wasn't found")?;
    let wide: Vec<u16> = library.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: a null-terminated path
    if unsafe { vpi_dof_load(wide.as_ptr()) } == 0 {
        return Err(format!("could not load libdof from {}: {}", library.display(), std::io::Error::last_os_error()));
    }
    // libdof also looks for its files in the current folder, and by default writes its log
    // there (`DirectOutput.log`, unless the user's configuration says otherwise): a folder of our
    // own, so the configuration's folder — perhaps VPX's — is only ever read
    let folder = o.config.join("directoutputconfig");
    if let Some(own) = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Visual Pinball Infinite").join("DOF"))
        && std::fs::create_dir_all(&own).is_ok()
    {
        let _ = std::env::set_current_dir(&own);
    }
    let base = format!("{}{}", o.config.display(), if o.config.to_string_lossy().ends_with('\\') { "" } else { "\\" });
    let (base, table, name) = (c_string(&base)?, c_string(&o.table)?, c_string(&o.name)?);
    eprintln!("vpi-dof: {}'s toys from {}, with libdof from {}", o.name, folder.display(), library.display());
    // SAFETY: null-terminated strings, kept alive across the call
    if unsafe { vpi_dof_start(base.as_ptr(), log, table.as_ptr(), name.as_ptr()) } == 0 {
        return Err("libdof failed to start".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_tests_event_reads_as_the_game_sends_it() {
        assert_eq!(super::parse_event("S5"), Some((b'S', 5)));
        assert_eq!(super::parse_event("e101"), Some((b'E', 101)));
        assert_eq!((super::parse_event("X3"), super::parse_event("S"), super::parse_event("")), (None, None, None));
    }

    #[test]
    fn libdof_is_found_in_the_bundle_while_developing() {
        // in the repository, VPX 5436's bundle is where the library comes from
        if super::DEVELOPMENT && std::path::Path::new(r"..\..\vpx-build\downloads\5436-third-party-windows-x64-Release\runtime-libs\windows-x64\dof64.dll").exists() {
            assert!(super::find_library().is_some());
        }
    }
}
