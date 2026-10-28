// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! `vpi-colour`: colours a ROM game's score display beside the player
//!, with the game's Serum file (libserum) or PIN2DMD's older palette
//! and animation files (libvni). The player creates the link, starts this
//! program with its name and hands over each new frame of the display as
//! PinMAME's raw shades (its identify frame); this program colours
//! it, steps the file's colour rotations in time, and publishes the
//! coloured frame and the triggers the file gives frames, for PinUP packs.
//!
//! ```text
//! vpi-colour --link <name> --serum <folder> --rom <rom> [--no-cache] [--library <serum64.dll>] [--parent <pid>]
//! vpi-colour --link <name> --vni-pal <file> [--vni <file>] [--library <vni64.dll>] [--parent <pid>]
//! vpi-colour --link <name> --vni-pac <file> [--library <vni64.dll>] [--parent <pid>]
//! ```
//!
//! `--serum` names the folder holding `<rom>\<rom>.cROMc` or `.cRZ`;
//! `--no-cache` keeps libserum from writing a `.cROMc` beside a `.cRZ`.
//! `--vni-pac` names PIN2DMD's locked file, opened with the user's own key,
//! which the player hands over in `VPI_PAC_KEY` (not on the command line,
//! which other programs can read): its palette and animations come out of
//! it into a folder of this program's own, for libvni to read, and are
//! deleted once read (`pac.rs`).
//!
//! GPL-2.0-or-later, apart from the rest of the engine (GPL-3.0-or-later):
//! libserum and libvni are under version 2 of the GPL alone, which code
//! under version 3 can't be combined with, so they run here, in a program
//! of their own. It's written from their headers, not from VPX's plugins
//! (GPL-3.0-or-later), and uses only BSD code besides.

mod pac;
mod serum;
mod vni;
mod win;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use colour_link::{Link, STATE_FAILED, STATE_READY};

const USAGE: &str =
    "vpi-colour --link <name> (--serum <folder> --rom <rom> [--no-cache] | --vni-pal <file> [--vni <file>] | --vni-pac <file>) [--library <dll>] [--parent <pid>]";

enum Source {
    Serum { folder: PathBuf, rom: String, cache: bool },
    Vni { pal: PathBuf, vni: Option<PathBuf> },
    Pac { pac: PathBuf },
}

struct Options {
    link: String,
    source: Source,
    library: Option<PathBuf>,
    parent: Option<u32>,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let (mut link, mut serum, mut rom, mut cache, mut pal, mut vni, mut library, mut parent) = (None, None, None, true, None, None, None, None);
    let mut pac = None;
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{a} needs a value\n{USAGE}"));
        match a.as_str() {
            "--link" => link = Some(value()?),
            "--serum" => serum = Some(PathBuf::from(value()?)),
            "--rom" => rom = Some(value()?),
            "--no-cache" => cache = false,
            "--vni-pal" => pal = Some(PathBuf::from(value()?)),
            "--vni" => vni = Some(PathBuf::from(value()?)),
            "--vni-pac" => pac = Some(PathBuf::from(value()?)),
            "--library" => library = Some(PathBuf::from(value()?)),
            "--parent" => parent = Some(value()?.parse().map_err(|_| "--parent takes a process id".to_string())?),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    let source = match (serum, rom, pal, pac) {
        (Some(folder), Some(rom), None, None) => Source::Serum { folder, rom, cache },
        (None, _, Some(pal), None) => Source::Vni { pal, vni },
        (None, _, None, Some(pac)) => Source::Pac { pac },
        _ => return Err(USAGE.to_string()),
    };
    Ok(Options { link: link.ok_or(USAGE)?, source, library, parent })
}

/// Whether this is a development build, which also looks in the repository's `vpx-build` folders
/// for the libraries it loads. A release build, compiled with `VPI_RELEASE_BUILD` set
/// (`tools\release`), looks beside the program alone.
const DEVELOPMENT: bool = option_env!("VPI_RELEASE_BUILD").is_none();

/// The library: beside this program (how it ships), else, in the repository, our own build of it
/// (tools\thirdparty\setup.ps1), else VPX 5436's from its library bundle (development builds).
fn find_library(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let beside = dir.join(name);
    if beside.exists() {
        return Some(beside);
    }
    if !DEVELOPMENT {
        return None;
    }
    [r"vpx-build\thirdparty\out\windows-x64\bin", r"vpx-build\downloads\5436-third-party-windows-x64-Release\runtime-libs\windows-x64"]
        .into_iter()
        .find_map(|under| dir.ancestors().take(4).map(|up| up.join(under).join(name)).find(|p| p.exists()))
}

/// PIN2DMD's locked file, opened with the key the player handed over: its two files written into a
/// folder of this program's own, read by libvni, then deleted.
fn load_pac(library: &std::path::Path, pac: &std::path::Path) -> Result<vni::Vni, String> {
    let key = pac::parse_key(&std::env::var("VPI_PAC_KEY").map_err(|_| "no PAC key was given".to_string())?)?;
    let file = std::fs::read(pac).map_err(|e| format!("{}: {e}", pac.display()))?;
    let unpacked = pac::unpack(&file, &key)?;
    let dir = std::env::temp_dir().join(format!("vpi-colour-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let pal = dir.join("pin2dmd.pal");
    let vni_file = dir.join("pin2dmd.vni");
    let written = std::fs::write(&pal, &unpacked.pal).and_then(|()| match &unpacked.vni {
        Some(v) => std::fs::write(&vni_file, v),
        None => Ok(()),
    });
    let loaded = written.map_err(|e| e.to_string()).and_then(|()| vni::Vni::load(library, &pal, unpacked.vni.as_ref().map(|_| vni_file.as_path())));
    let _ = std::fs::remove_file(&pal);
    let _ = std::fs::remove_file(&vni_file);
    let _ = std::fs::remove_dir(&dir);
    loaded
}

/// What colours the frames.
enum Colourer {
    Serum(serum::Serum),
    Vni(vni::Vni),
}

fn main() -> ExitCode {
    let options = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("vpi-colour: {e}");
            return ExitCode::FAILURE;
        }
    };
    let link = match Link::open(&options.link) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vpi-colour: could not open the link {}: {e}", options.link);
            return ExitCode::FAILURE;
        }
    };
    let shared = link.shared();
    let default_library = match options.source {
        Source::Serum { .. } => "serum64.dll",
        Source::Vni { .. } | Source::Pac { .. } => "vni64.dll",
    };
    let loaded = options.library.clone().or_else(|| find_library(default_library)).ok_or_else(|| format!("{default_library} wasn't found")).and_then(|library| {
        match &options.source {
            Source::Serum { folder, rom, cache } => serum::Serum::load(&library, folder, rom, *cache).map(|s| {
                eprintln!("vpi-colour: {rom}'s colours from {} (Serum, format {}, libserum {})", folder.display(), s.format(), s.library_version());
                Colourer::Serum(s)
            }),
            Source::Vni { pal, vni } => vni::Vni::load(&library, pal, vni.as_deref()).map(|v| {
                eprintln!("vpi-colour: colours from {}{}", pal.display(), vni.as_ref().map_or(String::new(), |v| format!(" and {}", v.display())));
                Colourer::Vni(v)
            }),
            Source::Pac { pac } => load_pac(&library, pac).map(|v| {
                eprintln!("vpi-colour: colours from {} (PIN2DMD's locked file, opened with the user's key)", pac.display());
                Colourer::Vni(v)
            }),
        }
    });
    let mut colourer = match loaded {
        Ok(c) => c,
        Err(e) => {
            eprintln!("vpi-colour: {e}; the game shows its own display");
            shared.set_state(STATE_FAILED, colour_link::FORMAT_NONE);
            return ExitCode::FAILURE;
        }
    };
    let format = match &colourer {
        Colourer::Serum(s) => s.format(),
        Colourer::Vni(v) => v.format(),
    };
    shared.set_state(STATE_READY, format);

    // SAFETY: the player's process, waited on so this program ends with it
    let parent = options.parent.map(|pid| unsafe { win::OpenProcess(win::SYNCHRONIZE, 0, pid) }).filter(|h| !h.is_null());
    let mut handles = vec![link.to_host_event()];
    handles.extend(parent);
    let (mut last, mut shades) = (0, Vec::with_capacity(colour_link::MAX_DOTS));
    // when the colours rotate next: the frame's own times, one after another, caught up if late
    let mut next_rotation: Option<Instant> = None;
    loop {
        let wait = next_rotation.map_or(1000, |t| t.saturating_duration_since(Instant::now()).as_millis().min(1000) as u32);
        // SAFETY: waits for a new frame, the player's end, or the next rotation
        let woke = unsafe { win::WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, wait) };
        if parent.is_some() && woke == win::WAIT_OBJECT_0 + 1 {
            break;
        }
        if shared.stop_requested() {
            break;
        }
        if shared.identify_seq() != last
            && let Some(frame) = shared.read_identify(&mut shades)
        {
            last = frame.seq;
            match &mut colourer {
                Colourer::Serum(s) => match s.colour(&shades, &frame, shared) {
                    // a frame the file doesn't know: the last one stays, rotating as it did
                    serum::Coloured::Unchanged => {}
                    serum::Coloured::New(rotation) => next_rotation = rotation.map(|ms| Instant::now() + Duration::from_millis(ms.into())),
                },
                Colourer::Vni(v) => v.colour(&shades, &frame, shared),
            }
        }
        if let (Colourer::Serum(s), Some(due)) = (&mut colourer, next_rotation) {
            next_rotation = rotate_until(s, shared, due, Instant::now());
        }
    }
    ExitCode::SUCCESS
}

/// Steps the colour rotations due by `now`, each on the time the last one
/// gave, so a late wake-up catches up; when the next is due, or None once
/// the frame's rotations end.
fn rotate_until(s: &mut serum::Serum, shared: &colour_link::Shared, mut due: Instant, now: Instant) -> Option<Instant> {
    while due <= now {
        due += Duration::from_millis(s.rotate(shared)?.into());
    }
    Some(due)
}
