// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the controller of its
// PinMAME plugin (plugins/pinmame/Controller.cpp), VPX's rebuild of VPinMAME's controller for table scripts; and how
// its later plugin finds VPinMAME's folders on Windows (PinMAMEPlugin.cpp, d3e5425b1).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! PinMAME for ROM tables: the machine a table's
//! script drives through VPinMAME's controller. The emulator itself runs in
//! `vpi-pinmame`, beside the player, linked through shared memory; this is
//! the player's end of it: the game the script asked for, every switch it
//! set, and what the machine changed since the script last asked.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pinmame_link::{Command, HostState, Link, MAX_GIS, MAX_LAMPS, MAX_LEDS, MAX_SOLENOIDS, MechConfig, Output, OutputKind, OutputReader, Read, SCRIPT_MECHS};

/// The version VPinMAME's controller reports, as VPX's answers for the
/// PinMAME it carries: 3.7.0, the build `vpi-pinmame` loads.
pub const VERSION: &str = "03070000";

/// The reference install's VPinMAME folder, when Windows names none.
const REFERENCE_VPINMAME: &str = r"C:\vPinball\VisualPinball\VPinMAME";

/// VPinMAME's folder on this machine — its ROMs, samples, ROM aliases, colour and sound packs —
/// found as VPX's PinMAME plugin finds it when nothing else names one (`PinMAMEPlugin.cpp`, since
/// d3e5425b1): the folder of the registered VPinMAME controller (any bitness) while its library is
/// there; else the folder holding VPinMAME's first ROM folder setting; else the reference install's,
/// until the player has settings of its own. Only read, never written.
pub(crate) fn vpinmame() -> &'static Path {
    static FOLDER: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    FOLDER.get_or_init(|| {
        FOLDER_SETTING
            .get()
            .cloned()
            .or_else(registered_vpinmame)
            .or_else(|| {
                let first = vpinmame_setting("rompath")?.split(';').next()?.trim().to_string();
                let roms = PathBuf::from(first);
                // the folder holding the ROM folder
                roms.parent().map(Path::to_path_buf)
            })
            .unwrap_or_else(|| PathBuf::from(REFERENCE_VPINMAME))
    })
}

/// The settings' PinMAME folder (`[Plugin.PinMAME]` `PinMAMEPath`), which outranks the
/// registered VPinMAME's as VPinMAME's folder.
static FOLDER_SETTING: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Sets the settings' PinMAME folder, once, before the first game: a folder that isn't there is
/// left aside.
pub fn set_folder_setting(path: &Path) {
    if path.is_dir() {
        let _ = FOLDER_SETTING.set(path.to_path_buf());
    } else {
        eprintln!("PinMAME: {} isn't a folder (`[Plugin.PinMAME]` `PinMAMEPath`); VPinMAME's own instead", path.display());
    }
}

/// The folder of the registered VPinMAME controller's library, in any of the registry's views.
fn registered_vpinmame() -> Option<PathBuf> {
    const KEY: &str = r"CLSID\{F389C8B7-144F-4C63-A2E3-246D168F9D39}\InprocServer32";
    [0, registry::WOW64_64, registry::WOW64_32].into_iter().find_map(|view| {
        let dll = PathBuf::from(registry::string(registry::HKEY_CLASSES_ROOT, KEY, None, view)?);
        dll.is_file().then(|| dll.parent().map(Path::to_path_buf)).flatten()
    })
}

/// One of VPinMAME's folder settings (`rompath`, `nvram_directory`, …), which may list several
/// folders separated by `;`.
fn vpinmame_setting(name: &str) -> Option<String> {
    registry::string(registry::HKEY_CURRENT_USER, r"Software\Freeware\Visual PinMame\globals", Some(name), 0)
}

/// Reading the registry's text values (Windows' `RegGetValueW`).
mod registry {
    use std::ffi::c_void;

    pub const HKEY_CLASSES_ROOT: isize = 0x8000_0000_u32 as i32 as isize;
    pub const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
    /// `RRF_SUBKEY_WOW6464KEY`, `RRF_SUBKEY_WOW6432KEY`: the 64-bit or 32-bit view.
    pub const WOW64_64: u32 = 0x0001_0000;
    pub const WOW64_32: u32 = 0x0002_0000;
    /// `RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ`, expanded.
    const TEXT: u32 = 0x0000_0002 | 0x0000_0004;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(key: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, size: *mut u32) -> i32;
    }

    /// A text value (`name`, or the key's default), expanded; None when missing or empty.
    pub fn string(root: isize, key: &str, name: Option<&str>, view: u32) -> Option<String> {
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let (key, name) = (wide(key), name.map(wide));
        let name = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
        let mut size = 0u32;
        // SAFETY: NUL-terminated wide strings; a null buffer asks for the size in bytes
        if unsafe { RegGetValueW(root, key.as_ptr(), name, TEXT | view, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) } != 0 {
            return None;
        }
        // an expanded value may need more room than the first answer: one more try, larger
        for extra in [0usize, 512] {
            let mut buffer = vec![0u16; size as usize / 2 + 1 + extra];
            let mut bytes = (buffer.len() * 2) as u32;
            // SAFETY: the buffer holds `bytes` bytes
            if unsafe { RegGetValueW(root, key.as_ptr(), name, TEXT | view, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut bytes) } == 0 {
                let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                return Some(String::from_utf16_lossy(&buffer[..len])).filter(|s| !s.is_empty());
            }
        }
        None
    }
}

/// How long a game may take to start: PinMAME loads and checks its ROMs.
const START_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the host may take to stop: PinMAME saves the NVRAM.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a machine's first start here runs, unheard and unseen, before it's started for play:
/// with no memory, a machine restores its factory settings, and many then show those notices until
/// they're switched off and on again — Attack from Mars, its memory kept after 5 s of that first
/// start, still showed them; after 10 s it played.
const FIRST_START: Duration = Duration::from_secs(15);

/// Until when a machine's first start runs, while one does (the playfield window's notice).
static FIRST_START_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

/// The notice to show now, if any (the window thread): a machine's first start running.
pub fn notice() -> Option<String> {
    let until = (*FIRST_START_UNTIL.lock().unwrap_or_else(|e| e.into_inner()))?;
    let left = until.checked_duration_since(Instant::now())?;
    Some(format!("First-time ROM machine boot-up: {} s", left.as_secs() + 1))
}

/// Where the ROMs are: a `pinmame\roms` folder beside the table, as VPX 5436's PinMAME plugin looks
/// first; the settings' PinMAME folder's `roms` (`[Plugin.PinMAME]` `PinMAMEPath`, VPX 5436's next);
/// then ours: the player's own `roms` folder, VPinMAME's ROM folder setting (its first folder
/// there), VPinMAME's own `roms` (VPX's PinMAME plugin applies VPinMAME's folder settings over its
/// install folder's).
fn rom_folder(table_dir: Option<&Path>, pinmame_path: Option<&Path>) -> Option<PathBuf> {
    let table = table_dir.and_then(|d| crate::pup::find_case_insensitive(&d.join("pinmame").join("roms"), true));
    let own = pinmame_path.map(|p| p.join("roms"));
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("roms")));
    let setting = vpinmame_setting("rompath").and_then(|s| s.split(';').map(str::trim).filter(|f| !f.is_empty()).map(PathBuf::from).find(|d| d.is_dir()));
    [table, own, beside, setting, Some(vpinmame().join("roms"))].into_iter().flatten().find(|d| d.is_dir())
}

/// Where game `rom`'s AltSound pack is, as VPX 5436's AltSound plugin looks for it:
/// `altsound\<rom>` beside the table, then in its `pinmame` folder, then under the folder the
/// settings' `[Plugin.AltSound]` `Folder` names (VPX's: the `altsound` folder itself); then, ours,
/// beside the player and in VPinMAME's folder, where VPin Studio puts packs. The folder holding
/// `altsound`, as libaltsound takes it; None without a pack, or when `Enable` is 0 (VPX's switch
/// for the plugin, on by default here). A pack without its settings file gets one written on its
/// first play, by libaltsound, as VPinMAME writes it — in VPinMAME's folder too.
pub fn altsound_base(table_dir: Option<&Path>, rom: &str, settings: &[(String, String)]) -> Option<PathBuf> {
    let setting = |key: &str| settings.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| crate::settings::vpx_trim(v).to_string());
    if setting("Enable").as_deref().and_then(crate::settings::vpx_bool) == Some(false) || rom.is_empty() {
        return None;
    }
    let has_pack = |base: &Path| crate::pup::find_case_insensitive(&base.join("altsound").join(rom), true).is_some();
    if let Some(dir) = table_dir {
        if has_pack(dir) {
            return Some(dir.to_path_buf());
        }
        let pinmame = dir.join("pinmame");
        if has_pack(&pinmame) {
            return Some(pinmame);
        }
    }
    if let Some(folder) = setting("Folder").filter(|f| !f.is_empty())
        && let Some(base) = Path::new(&folder).parent()
        && has_pack(base)
    {
        return Some(base.to_path_buf());
    }
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    beside.into_iter().chain(std::iter::once(vpinmame().to_path_buf())).find(|b| has_pack(b))
}

/// Where PinMAME keeps each game's NVRAM, settings and high scores: the
/// player's own data folder, never VPinMAME's.
fn data_folder() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("PinMAME");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// The ROM a table's game name stands for: VPinMAME's `VPMAlias.txt` maps
/// a name a table asks for to the set that's there (lines of
/// `requested,actual`); a name it doesn't list stands for itself.
pub fn resolve_alias(game: &str) -> String {
    let file = vpinmame().join("VPMAlias.txt");
    let Ok(text) = std::fs::read_to_string(file) else { return game.to_string() };
    alias_in(&text, game).unwrap_or_else(|| game.to_string())
}

/// `game`'s line in an alias list, read as libpinmame reads its own: the
/// first two words, split at commas and spaces, and nothing after a `#`,
/// `;` or `'`.
fn alias_in(text: &str, game: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.split(['#', ';', '\'']).next().unwrap_or("");
        let mut words = line.split([',', ' ', '\t']).filter(|w| !w.is_empty());
        let (from, to) = (words.next()?, words.next()?);
        from.eq_ignore_ascii_case(game).then(|| to.to_string())
    })
}

/// One kind of output (lamps, solenoids or lighting strings): the values
/// PinMAME last reported, and what the script was last handed.
#[derive(Default)]
pub struct Changes {
    now: BTreeMap<i32, i32>,
    told: HashMap<i32, i32>,
    /// Numbers PinMAME changed since the script last asked.
    dirty: BTreeSet<i32>,
}

impl Changes {
    fn set(&mut self, number: i32, value: i32) {
        self.now.insert(number, value);
        self.dirty.insert(number);
    }

    /// An output's value now.
    pub fn current(&self, number: i32) -> i32 {
        self.now.get(&number).copied().unwrap_or(0)
    }

    /// VPX's `GetChangedLamps` and kin: each output whose value differs from
    /// what the script was last told, with its value now, by number. A
    /// change undone before the script asks isn't reported, as in VPX.
    /// `wanted` filters what's handed over (the solenoid mask); what it
    /// leaves out still counts as told.
    pub fn take(&mut self, wanted: impl Fn(i32) -> bool) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for n in std::mem::take(&mut self.dirty) {
            let v = self.current(n);
            if self.told.get(&n).copied().unwrap_or(0) != v {
                self.told.insert(n, v);
                if wanted(n) {
                    out.push((n, v));
                }
            }
        }
        out
    }
}

/// The digits of the machine's segment displays: the segments PinMAME
/// last drew lit in each, a bit a segment, and what the script was last told
/// (VPinMAME's `ChangedLEDs`).
pub struct Leds {
    now: [i32; MAX_LEDS],
    told: [i32; MAX_LEDS],
}

impl Default for Leds {
    fn default() -> Self {
        Leds { now: [0; MAX_LEDS], told: [0; MAX_LEDS] }
    }
}

impl Leds {
    fn set(&mut self, digit: i32, segments: i32) {
        if let Some(d) = usize::try_from(digit).ok().and_then(|d| self.now.get_mut(d)) {
            *d = segments;
        }
    }

    /// A digit's lit segments now.
    pub fn current(&self, digit: i32) -> i32 {
        usize::try_from(digit).ok().and_then(|d| self.now.get(d)).copied().unwrap_or(0)
    }

    /// `ChangedLEDs`, as PinMAME answers it (`vp_getChangedLEDs`): each digit
    /// the masks name — digit n by bit n of `mask`, digit 64 + n by bit n of
    /// `mask2` — whose segments differ from what the script was last told,
    /// with the segments that changed and those lit now; then every digit
    /// counts as told, named or not.
    pub fn take(&mut self, mask: u64, mask2: u64) -> Vec<(i32, i32, i32)> {
        let mut out = Vec::new();
        for (digit, (&now, &told)) in self.now.iter().zip(&self.told).enumerate() {
            let bits = if digit < 64 { mask >> digit } else { mask2 >> (digit - 64) };
            let named = bits & 1 != 0;
            if named && now != told {
                out.push((digit as i32, now ^ told, now));
            }
        }
        self.told = self.now;
        out
    }
}

/// A mask of `ChangedLEDs` from its two whole-number halves, as VPX's
/// controller builds it, and VPinMAME's before it: a negative low half
/// (`&H80000000` and up) fills the high half too.
pub fn led_mask(high: i32, low: i32) -> u64 {
    (u64::from(high as u32) << 32) | i64::from(low) as u64
}

/// Set to print every switch a script sets and every solenoid change it
/// receives, with the game time (`vpi-headless --trace`).
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// With `VPI_LATENCY_LOG` naming a file, every switch the script sets
/// (`S`) and every solenoid change it is handed (`C`) go there as they
/// happen, in microseconds on the machine's clock: `kind time number
/// value`. A switch-to-solenoid latency measurement reads it, and a copy of
/// VPX built to be measured writes the same from its controller; the host
/// logs its side on the same clock (`<file>.host`).
pub fn latency_log(kind: char, number: i32, value: i32) {
    use std::io::Write as _;
    static LOG: std::sync::OnceLock<Option<std::sync::Mutex<std::fs::File>>> = std::sync::OnceLock::new();
    let us = pinmame_link::clock_us();
    let log = LOG.get_or_init(|| {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(std::env::var_os("VPI_LATENCY_LOG")?).ok()?;
        Some(std::sync::Mutex::new(file))
    });
    if let Some(log) = log {
        let _ = writeln!(log.lock().unwrap_or_else(|e| e.into_inner()), "{kind} {us} {number} {value}");
    }
}

/// Numbers each link the player makes, so no two meet.
static NEXT_LINK: AtomicU32 = AtomicU32::new(0);

/// The most sound commands kept for a script that hasn't asked for them.
const MAX_SOUND_COMMANDS: usize = 1024;

/// A game running in `vpi-pinmame`.
pub struct Machine {
    /// Shared with the mixer, which plays the sound PinMAME puts in it.
    link: Arc<Link>,
    child: Child,
    reader: OutputReader,
    batch: Vec<Output>,
    pub lamps: Changes,
    pub solenoids: Changes,
    pub gis: Changes,
    pub leds: Leds,
    /// The commands the game sent its sound board since the script last
    /// asked (`NewSoundCommands`).
    pub sound_commands: Vec<i32>,
    frame: Vec<u8>,
    /// The game's segment displays, once PinMAME has described every display
    /// (None inside for a game without any).
    segment_layout: Option<Option<Arc<crate::segments::SegLayout>>>,
    /// The game thread's waker, which the host's event wakes while the game runs.
    waker: Option<Arc<crate::waker::Waker>>,
    /// The host's end, said once when it ends on its own (a crash).
    ended: bool,
}

/// What a game needs to start.
pub struct Start<'a> {
    /// The ROM set to run (an alias already resolved).
    pub rom: &'a str,
    pub handle_mechanics: i32,
    pub sound: bool,
    /// PinMAME's cheat mode (VPinMAME's `cheat`), and the machine's sound louder or softer, in
    /// decibels (VPinMAME's `volume`).
    pub cheat: bool,
    pub volume_db: i32,
    /// PinMAME's samples, older machines' recorded sounds (VPinMAME's `samples`).
    pub use_samples: bool,
    /// The table's folder, whose `pinmame\roms` comes first, and the settings' PinMAME folder
    /// (`[Plugin.PinMAME]` `PinMAMEPath`), whose `roms` and `samples` come next.
    pub table_dir: Option<&'a Path>,
    pub pinmame_path: Option<&'a Path>,
    /// The folder holding `altsound\<rom>`, when the game's AltSound pack plays.
    pub altsound: Option<&'a Path>,
    /// The game thread's waker: the host wakes it when solenoids change, so
    /// the script hears of them at once rather than at its next tick.
    pub waker: Option<Arc<crate::waker::Waker>>,
}

impl Machine {
    /// Starts `vpi-pinmame` on a new link and waits until PinMAME runs the
    /// game, or says it can't.
    pub fn start(start: &Start) -> Result<Machine, String> {
        let host = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("vpi-pinmame.exe");
        if !host.exists() {
            return Err(format!("{} is missing", host.display()));
        }
        let roms = rom_folder(start.table_dir, start.pinmame_path).ok_or("no ROM folder")?;
        let data = data_folder().ok_or("no data folder for PinMAME")?;
        let name = format!(r"Local\VPI-PinMAME-{}-{}", std::process::id(), NEXT_LINK.fetch_add(1, Ordering::Relaxed));
        let link = Arc::new(Link::create(&name).map_err(|e| format!("the link to PinMAME: {e}"))?);
        let mut process = Process::new(&host);
        process
            .args(["--link", &name, "--game", start.rom, "--parent", &std::process::id().to_string()])
            .arg("--roms")
            .arg(&roms)
            .arg("--data")
            .arg(&data)
            .args(["--handle-mechanics", &start.handle_mechanics.to_string()]);
        // the settings' PinMAME folder's `samples`, else VPinMAME's sample folder setting (its first
        // folder there), else its own `samples`
        let own = start.pinmame_path.map(|p| p.join("samples")).filter(|d| d.is_dir());
        let setting = own.or_else(|| vpinmame_setting("samplepath").and_then(|s| s.split(';').map(str::trim).filter(|f| !f.is_empty()).map(PathBuf::from).find(|d| d.is_dir())));
        if let Some(samples) = setting.or_else(|| Some(vpinmame().join("samples")).filter(|d| d.is_dir())) {
            process.arg("--samples").arg(samples);
        }
        if !start.sound {
            process.arg("--no-sound");
        }
        if start.cheat {
            process.arg("--cheat");
        }
        if start.use_samples {
            process.arg("--use-samples");
        }
        if start.volume_db != 0 {
            process.args(["--volume", &start.volume_db.to_string()]);
        }
        if let Some(base) = start.altsound {
            process.arg("--altsound").arg(base);
        }
        let mut child = process.spawn().map_err(|e| format!("start {}: {e}", host.display()))?;
        let launched = Instant::now();
        let shared = link.shared();
        loop {
            match shared.host_state() {
                HostState::Running => break,
                HostState::Failed => {
                    let _ = child.wait();
                    return Err(format!("PinMAME couldn't run {} (status {})", start.rom, shared.run_status()));
                }
                _ => {}
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!("vpi-pinmame ended before {} ran ({status})", start.rom));
            }
            if launched.elapsed() > START_TIMEOUT {
                let _ = child.kill();
                return Err(format!("{} didn't start within {} s", start.rom, START_TIMEOUT.as_secs()));
            }
            link.wait_for_host(10);
        }
        let reader = OutputReader::from_now(shared);
        if let Some(waker) = &start.waker {
            // SAFETY: the link's own event, live while the link is
            unsafe { waker.watch(link.to_engine_event()) };
        }
        let mut machine = Machine {
            link,
            child,
            reader,
            batch: Vec::new(),
            lamps: Changes::default(),
            solenoids: Changes::default(),
            gis: Changes::default(),
            leds: Leds::default(),
            sound_commands: Vec::new(),
            frame: Vec::new(),
            segment_layout: None,
            waker: start.waker.clone(),
            ended: false,
        };
        // what changed before the reader started
        machine.resync();
        Ok(machine)
    }

    /// Sends a command, waiting briefly if the host is behind.
    pub fn send(&mut self, command: Command) {
        let waited = Instant::now();
        while !self.link.shared().send(command) {
            if waited.elapsed() > Duration::from_millis(100) || !self.running() {
                crate::script::log::once("PinMAME didn't take commands for 100 ms; some were dropped");
                return;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        self.link.wake_host();
    }

    /// Takes in the outputs PinMAME changed since the last look.
    pub fn poll(&mut self) {
        // the host gone while the table plays: its display and sound stop, said once
        if !self.ended
            && let Ok(Some(status)) = self.child.try_wait()
        {
            self.ended = true;
            eprintln!("vpi-pinmame stopped while the game ran ({status}): the machine's displays and sound stop, the table plays on");
        }
        self.batch.clear();
        let shared = self.link.shared();
        if self.reader.read(shared, &mut self.batch) == Read::Lost {
            self.resync();
            return;
        }
        for o in &self.batch {
            match o.kind {
                OutputKind::Lamp => self.lamps.set(o.number, o.value),
                OutputKind::Solenoid => self.solenoids.set(o.number, o.value),
                OutputKind::Gi => self.gis.set(o.number, o.value),
                OutputKind::Led => self.leds.set(o.number, o.value),
                OutputKind::SoundCommand => self.sound_commands.push(o.number),
            }
        }
        // a script that never asks keeps only the latest
        if self.sound_commands.len() > MAX_SOUND_COMMANDS {
            self.sound_commands.drain(..self.sound_commands.len() - MAX_SOUND_COMMANDS);
        }
    }

    /// Takes every output's current value from the link, after falling a
    /// whole ring behind.
    fn resync(&mut self) {
        let shared = self.link.shared();
        for (changes, kind, max) in
            [(&mut self.lamps, OutputKind::Lamp, MAX_LAMPS), (&mut self.solenoids, OutputKind::Solenoid, MAX_SOLENOIDS), (&mut self.gis, OutputKind::Gi, MAX_GIS)]
        {
            for n in 0..max as i32 {
                let v = shared.current(kind, n);
                if v != changes.current(n) {
                    changes.set(n, v);
                }
            }
        }
        for n in 0..MAX_LEDS as i32 {
            self.leds.set(n, shared.current(OutputKind::Led, n));
        }
    }

    /// Whether the game still runs.
    pub fn running(&mut self) -> bool {
        self.link.shared().host_state() == HostState::Running && matches!(self.child.try_wait(), Ok(None))
    }

    /// The game's main dot-matrix display, as VPX's controller picks it (the
    /// widest, at least 128 dots): its size and latest frame, a byte a dot
    /// from 0 (off) to 255.
    pub fn dmd(&mut self) -> Option<(i32, i32, &[u8])> {
        let (index, layout) = self.main_dmd()?;
        self.link.shared().read_frame(index, &mut self.frame)?;
        Some((layout.width, layout.height, &self.frame))
    }

    /// The main DMD's latest identify frame (PinMAME's raw shades, a byte a dot, as PinUP's
    /// frame triggers and colour files read them), into `out`: its width, height, bits a dot and
    /// frame number. None before the first, or with a PinMAME build that doesn't give them.
    pub fn dmd_identify(&self, out: &mut Vec<u8>) -> Option<(i32, i32, u32, u32)> {
        let (index, layout) = self.main_dmd()?;
        let (number, bits) = self.link.shared().read_identify(index, out)?;
        Some((layout.width, layout.height, bits, number))
    }

    /// A bank of the game's DIP switches as PinMAME has them, once it runs.
    pub fn dip_bank(&self, bank: i32) -> i32 {
        self.link.shared().dip_bank(bank)
    }

    /// The game's NVRAM as PinMAME has it now (VPX's PinMAME plugin's `GetNVRAM`), read by the
    /// host when asked: None when it has none to give, or doesn't answer within a fifth of a second.
    pub fn nvram(&self) -> Option<Vec<u8>> {
        let shared = self.link.shared();
        let request = shared.request_nvram();
        let start = std::time::Instant::now();
        loop {
            // this request's answer, or a newer one (the high scores' watcher may ask too)
            if let Some(answer) = shared.nvram_answer_since(request) {
                return answer;
            }
            if start.elapsed() > std::time::Duration::from_millis(200) {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// PinMAME's hardware generation for the game (`core_gameData->gen`), 0 until it's known.
    pub fn hardware_gen(&self) -> u64 {
        self.link.shared().hardware_gen()
    }

    /// How many identify frames the main DMD has had: a cheap check for a new one.
    pub fn dmd_identify_count(&self) -> Option<u32> {
        let (index, _) = self.main_dmd()?;
        Some(self.link.shared().identify_published(index))
    }

    /// A lamp's, solenoid's or lighting string's value now, straight from
    /// the link (for the backglass, apart from what the script was told).
    pub fn output(&self, kind: OutputKind, number: i32) -> i32 {
        self.link.shared().current(kind, number)
    }

    /// How many lamps, solenoids and lighting strings the game has.
    pub fn output_counts(&self) -> (u32, u32, u32) {
        self.link.shared().output_counts()
    }

    /// Mechanism `number`'s position now.
    pub fn mech_position(&self, number: i32) -> i32 {
        self.link.shared().mech(number)
    }

    /// How many frames the main DMD has had: a cheap check for a new one.
    pub fn dmd_number(&self) -> Option<u32> {
        let (index, _) = self.main_dmd()?;
        Some(self.link.shared().frames_published(index))
    }

    /// The main DMD's display number and layout.
    fn main_dmd(&self) -> Option<(usize, pinmame_link::DisplayLayout)> {
        let shared = self.link.shared();
        (0..shared.display_count())
            .filter_map(|i| Some((i, shared.display(i)?)))
            .filter(|&(i, l)| l.is_dmd() && l.width >= 128 && !self.drawn_from_segments(i))
            .max_by_key(|(i, l)| (l.width, usize::MAX - i))
    }

    /// Whether display `index` is the DMD libpinmame draws from a game's
    /// segment displays, after them, when the game has no DMD: VPX never
    /// shows it (libpinmame hands it to the program it runs in, for display
    /// devices, but not to VPX's renderer).
    fn drawn_from_segments(&self, index: usize) -> bool {
        let shared = self.link.shared();
        let count = shared.display_count();
        index > 0 && index + 1 == count && (0..index).all(|i| shared.display(i).is_some_and(|l| l.is_segments()))
    }

    /// The game's segment displays, once PinMAME has described all its
    /// displays, and a number that changes whenever one of them has a new
    /// frame. None for a game without any.
    pub fn segments(&mut self) -> Option<(Arc<crate::segments::SegLayout>, u64)> {
        let shared = self.link.shared();
        if self.segment_layout.is_none() {
            let count = shared.display_count();
            let described: Option<Vec<_>> = (0..count).map(|i| Some((i, shared.display(i)?))).collect();
            let described = described.filter(|d| !d.is_empty())?;
            self.segment_layout = Some(crate::segments::layout(&described, shared.hardware_gen()).map(Arc::new));
        }
        let layout = self.segment_layout.clone().flatten()?;
        let mut frames = 0u64;
        let mut last = None;
        for (_, digit) in layout.digits() {
            if last != Some(digit.display) {
                frames += u64::from(shared.frames_published(digit.display));
                last = Some(digit.display);
            }
        }
        Some((layout, frames))
    }

    /// Display `index`'s latest frame, into `out` (left empty before its first).
    pub fn display_frame(&self, index: usize, out: &mut Vec<u8>) {
        if self.link.shared().read_frame(index, out).is_none() {
            out.clear();
        }
    }

    /// The game's sound, for the mixer.
    pub fn sound(&self) -> MachineSound {
        MachineSound { link: self.link.clone(), next: None, dry: false }
    }

    /// The link to PinMAME, for a watcher that stops once the machine is gone
    /// (`machine_scores`).
    pub fn link_while_running(&self) -> std::sync::Weak<Link> {
        Arc::downgrade(&self.link)
    }

    /// Hands PinMAME mechanism `number` as described.
    pub fn set_mech(&mut self, number: i32, config: &MechConfig) {
        if self.link.shared().set_mech_config(number, config) {
            self.send(Command::SetMech { number });
        } else {
            crate::script::log::once(&format!("Controller.Mech: PinMAME takes mechanisms 1 to {SCRIPT_MECHS}, not {number}"));
        }
    }

    /// Stops the game, so PinMAME saves its NVRAM, and waits for the host
    /// to end (ending it after a while).
    pub fn stop(&mut self) {
        if let Some(waker) = self.waker.take() {
            waker.unwatch();
        }
        if matches!(self.child.try_wait(), Ok(None)) {
            let shared = self.link.shared();
            let waited = Instant::now();
            while !shared.send(Command::Stop) && waited.elapsed() < Duration::from_millis(100) {
                std::thread::sleep(Duration::from_micros(200));
            }
            self.link.wake_host();
            let stopping = Instant::now();
            while matches!(self.child.try_wait(), Ok(None)) {
                if stopping.elapsed() > STOP_TIMEOUT {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The sound kept waiting ahead of the speakers: PinMAME makes its sound a
/// video frame at a time (800 frames at 48 kHz, every 16.7 ms), so 25 ms
/// covers the gap between two without running dry.
const SOUND_CUSHION: u64 = 1_200;
/// If the speakers fall this far behind PinMAME (after a pause, or a stall),
/// they skip ahead to the cushion, so the sound never lags the table.
const SOUND_MOST_BEHIND: u64 = 4_800;

/// PinMAME's sound for the mixer: read from the link's ring on the audio
/// thread, a little behind the newest, and resampled to the mixer's rate
/// if PinMAME's differs.
pub struct MachineSound {
    link: Arc<Link>,
    /// The next frame to play, and how far past it; `None` until the cushion
    /// first fills.
    next: Option<(u64, f64)>,
    /// Ran dry — PinMAME paused, as the pause menu pauses it, or behind:
    /// silent till the cushion has filled again past `next`, so no sound
    /// plays twice.
    dry: bool,
}

impl crate::audio::StreamSource for MachineSound {
    fn mix_into(&mut self, out: &mut [f32], gain: f32) {
        let shared = self.link.shared();
        let rate = shared.audio_rate();
        if rate == 0 {
            return;
        }
        let written = shared.audio_written();
        let (start, mut frac) = match self.next {
            Some((at, _)) if written.saturating_sub(at) > SOUND_MOST_BEHIND => (written - SOUND_CUSHION, 0.0),
            Some((at, _)) if self.dry && written < at + SOUND_CUSHION => return,
            Some((at, frac)) => (at, frac),
            None if written >= SOUND_CUSHION => (written - SOUND_CUSHION, 0.0),
            None => return,
        };
        self.dry = false;
        let step = f64::from(rate) / f64::from(crate::audio::OUTPUT_RATE);
        let mut at = start;
        let scale = gain / 32_768.0;
        for frame in out.as_chunks_mut::<2>().0 {
            // a frame and the next, to interpolate between
            if at + 1 >= written {
                // ran dry: on from here once the cushion has filled again
                self.next = Some((at, frac));
                self.dry = true;
                return;
            }
            let (a, b) = (shared.audio_frame(at), shared.audio_frame(at + 1));
            let t = frac as f32;
            frame[0] += (f32::from(a[0]) + (f32::from(b[0]) - f32::from(a[0])) * t) * scale;
            frame[1] += (f32::from(a[1]) + (f32::from(b[1]) - f32::from(a[1])) * t) * scale;
            frac += step;
            at += frac as u64;
            frac = frac.fract();
        }
        self.next = Some((at, frac));
    }
}

/// VPX's device modes (`SolMask(2)`): how PinMAME reports its outputs.
pub const DEVICE_BINARY: i32 = 0;
pub const DEVICE_PHYSICAL: i32 = 2;

/// VPinMAME's controller, as a table's script sees it: the game it asked
/// for, its settings, every switch and DIP switch it set, and the running
/// machine.
pub struct Controller {
    pub game_name: String,
    pub rom_name: String,
    /// Every switch the script set: PinMAME gets them when the game starts,
    /// and again after a reset, which clears them in PinMAME.
    pub switches: BTreeMap<i32, bool>,
    pub dips: BTreeMap<i32, i32>,
    /// Applied to `ChangedSolenoids` only, as in VPX: solenoids 1–64.
    pub sol_mask: u64,
    pub device_mode: i32,
    pub handle_mechanics: i32,
    pub paused: bool,
    pub time_fence: f64,
    /// The mechanism `Mech(n)` is describing, and those handed over, by
    /// number: PinMAME gets them again when a game starts.
    mech_pending: MechConfig,
    pub mechs: BTreeMap<i32, MechConfig>,
    /// Games' settings by game and name, lower case (`Games(name).Settings`).
    pub game_settings: HashMap<String, HashMap<String, i32>>,
    /// The controller's own settings (`Settings.Value`).
    pub settings: HashMap<String, i32>,
    /// VPinMAME's settings for each game from our file (`machine_settings`), under the script's.
    pub machine_file: Arc<crate::machine_settings::MachineSettings>,
    /// VPX 5436's `[Plugin.PinMAME]` settings: sound, cheat mode, PinMAME folder.
    pub plugin: crate::machine_settings::PinmameSettings,
    /// The table's folder, for a `pinmame` folder beside it (VPX 5436 looks there first).
    pub table_dir: Option<PathBuf>,
    /// The folder holding the game's AltSound pack, for the next start.
    pub altsound: Option<PathBuf>,
    /// The game's colour file, for the next start.
    pub colour_files: Option<crate::colour::Files>,
    pub machine: Option<Machine>,
    /// `vpi-colour` colouring the running game's display, beside the machine.
    pub colour: Option<crate::colour::Colour>,
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            game_name: String::new(),
            rom_name: String::new(),
            switches: BTreeMap::new(),
            dips: BTreeMap::new(),
            sol_mask: u64::MAX,
            device_mode: DEVICE_BINARY,
            // VPX's controller: PinMAME simulates every mechanism it knows
            handle_mechanics: 0xFF,
            paused: false,
            time_fence: 0.0,
            mech_pending: MechConfig::default(),
            mechs: BTreeMap::new(),
            game_settings: HashMap::new(),
            settings: HashMap::new(),
            machine_file: Arc::new(crate::machine_settings::MachineSettings::default()),
            plugin: crate::machine_settings::PinmameSettings::default(),
            table_dir: None,
            altsound: None,
            colour_files: None,
            machine: None,
            colour: None,
        }
    }
}

impl Controller {
    /// `GameName = ...`: the game to run, and the ROM set that stands for it.
    pub fn set_game_name(&mut self, name: &str) {
        self.game_name = name.to_string();
        self.rom_name = resolve_alias(name);
    }

    /// A game setting: the script's (`Games(name).Settings.Value(key)`), else our file's for the
    /// game (`machine_settings`, VPinMAME's registry in VPinMAME's place); None when neither says.
    pub fn setting(&self, key: &str) -> Option<i32> {
        let script = self.game_settings.get(&self.game_name.to_lowercase()).and_then(|s| s.get(&key.to_lowercase()).copied());
        script.or_else(|| self.machine_file.get(&self.game_name, key))
    }

    /// VPinMAME's display options as it hands them to a display device (`tPMoptions`, in its
    /// order): the game's settings the script set, else VPinMAME's defaults.
    pub fn display_options(&self) -> [i32; 19] {
        const KEYS: [&str; 19] = [
            "dmd_red", "dmd_green", "dmd_blue", "dmd_perc66", "dmd_perc33", "dmd_perc0", "dmd_only", "dmd_compact", "dmd_antialias", "dmd_colorize",
            "dmd_red66", "dmd_green66", "dmd_blue66", "dmd_red33", "dmd_green33", "dmd_blue33", "dmd_red0", "dmd_green0", "dmd_blue0",
        ];
        KEYS.map(|k| self.setting(k).unwrap_or_else(|| crate::script::controller::setting_default(k)))
    }

    /// `Run`: starts the game, then hands PinMAME every switch, DIP switch
    /// and setting the script made before. `waker` is the game thread's,
    /// which the game's solenoid changes then wake.
    pub fn run(&mut self, sound: bool, waker: Option<Arc<crate::waker::Waker>>) -> Result<(), String> {
        if self.rom_name.is_empty() {
            return Err("no game name was given".into());
        }
        self.colour = None;
        if let Some(mut old) = self.machine.take() {
            old.stop();
        }
        let start = Start {
            rom: &self.rom_name,
            handle_mechanics: self.handle_mechanics,
            // the machine's sound unless the plugin's setting or the game's says none; its cheat mode
            // and volume as the game's settings, else the plugin's, say
            sound: sound && self.plugin.sound && self.setting("sound") != Some(0),
            cheat: !crate::achievements::hardcore_play() && self.setting("cheat").map_or(self.plugin.cheat, |v| v != 0),
            // the game's samples only when its settings ask (VPinMAME plays them unless told not
            // to; the PinMAME library, ours and VPX 5436's, never does: off by default)
            use_samples: self.setting("samples").is_some_and(|v| v != 0),
            volume_db: self.setting("volume").unwrap_or(0).clamp(-32, 32),
            table_dir: self.table_dir.as_deref(),
            pinmame_path: self.plugin.path.as_deref(),
            altsound: self.altsound.as_deref(),
            waker,
        };
        // a machine with no memory of ours: VPinMAME's taken as ours to start from, as the app takes
        // it before play (a front end may start the table itself); a machine with none at all makes
        // its factory settings on its first start, which many then show until they're switched off
        // and on: that first start made first, quietly
        if let Some(ours) = data_folder().map(|d| d.join("nvram").join(format!("{}.nv", self.rom_name)))
            && !ours.exists()
        {
            let vpx = self.plugin.path.as_deref().unwrap_or_else(|| vpinmame()).join("nvram").join(format!("{}.nv", self.rom_name));
            if vpx.is_file() && ours.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::copy(&vpx, &ours).is_ok() {
                eprintln!("{}: VPinMAME's memory of the machine taken as ours to start from", self.rom_name);
            } else {
                self.first_start(&start);
            }
        }
        let mut machine = Machine::start(&start)?;
        if self.device_mode != DEVICE_BINARY {
            machine.send(Command::SolenoidMask { which: 2, mask: self.device_mode as u32 });
        }
        for (&bank, &value) in &self.dips {
            machine.send(Command::Dip { bank, value });
        }
        for (&number, &closed) in &self.switches {
            if closed {
                machine.send(Command::Switch { number, closed });
            }
        }
        if self.paused {
            machine.send(Command::Pause(true));
        }
        for (&number, config) in &self.mechs {
            machine.set_mech(number, config);
        }
        if self.time_fence > 0.0 {
            machine.link.shared().set_time_fence(self.time_fence);
            machine.link.wake_host();
        }
        self.machine = Some(machine);
        // the game's display in colour, from its colour file
        if let Some(files) = &self.colour_files {
            match crate::colour::Colour::start(&self.rom_name, files) {
                Ok(c) => self.colour = Some(c),
                Err(e) => crate::script::log::once(&format!("{}'s colour file: {e}", self.rom_name)),
            }
        }
        Ok(())
    }

    /// The machine's first start here, unheard and unseen for `FIRST_START`, with the switches and
    /// DIP switches the script set, as play starts it: its factory settings made, and kept as it
    /// stops.
    fn first_start(&self, start: &Start) {
        let first = Start { sound: false, altsound: None, waker: None, ..*start };
        let began = Instant::now();
        *FIRST_START_UNTIL.lock().unwrap_or_else(|e| e.into_inner()) = Some(began + FIRST_START);
        match Machine::start(&first) {
            Ok(mut machine) => {
                for (&bank, &value) in &self.dips {
                    machine.send(Command::Dip { bank, value });
                }
                for (&number, &closed) in &self.switches {
                    if closed {
                        machine.send(Command::Switch { number, closed });
                    }
                }
                while began.elapsed() < FIRST_START && machine.running() {
                    std::thread::sleep(Duration::from_millis(50));
                }
                machine.stop();
                eprintln!("{}: its first start here, {:.0} s before play, its factory settings kept", first.rom, began.elapsed().as_secs_f64());
            }
            Err(e) => eprintln!("{}'s first start: {e}", first.rom),
        }
        *FIRST_START_UNTIL.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// `Stop`: the game ends and PinMAME saves its NVRAM.
    pub fn stop(&mut self) {
        self.colour = None;
        if let Some(mut m) = self.machine.take() {
            m.stop();
        }
    }

    /// The running machine, its outputs brought up to date.
    pub fn machine(&mut self) -> Option<&mut Machine> {
        let m = self.machine.as_mut()?;
        m.poll();
        Some(m)
    }

    pub fn set_switch(&mut self, number: i32, closed: bool) {
        self.switches.insert(number, closed);
        if let Some(m) = &mut self.machine {
            m.send(Command::Switch { number, closed });
        }
    }

    pub fn switch(&self, number: i32) -> bool {
        self.switches.get(&number).copied().unwrap_or(false)
    }

    pub fn set_dip(&mut self, bank: i32, value: i32) {
        self.dips.insert(bank, value);
        if let Some(m) = &mut self.machine {
            m.send(Command::Dip { bank, value });
        }
    }

    pub fn set_pause(&mut self, pause: bool) {
        self.paused = pause;
        if let Some(m) = &mut self.machine {
            m.send(Command::Pause(pause));
        }
    }

    /// `SolMask(n) = ...`: 0 and 1 mask the reported solenoids 1–32 and
    /// 33–64; 2 sets how outputs report, which PinMAME applies.
    pub fn set_sol_mask(&mut self, which: i32, value: i32) {
        match which {
            0 => self.sol_mask = (self.sol_mask & 0xFFFF_FFFF_0000_0000) | u64::from(value as u32),
            1 => self.sol_mask = (self.sol_mask & 0x0000_0000_FFFF_FFFF) | (u64::from(value as u32) << 32),
            2 if (DEVICE_BINARY..=DEVICE_PHYSICAL).contains(&value) => {
                self.device_mode = value;
                if let Some(m) = &mut self.machine {
                    m.send(Command::SolenoidMask { which: 2, mask: value as u32 });
                }
            }
            _ => {}
        }
    }

    pub fn sol_mask(&self, which: i32) -> i32 {
        match which {
            0 => self.sol_mask as u32 as i32,
            1 => (self.sol_mask >> 32) as u32 as i32,
            2 => self.device_mode,
            _ => -1,
        }
    }

    /// Whether `ChangedSolenoids` hands a solenoid over: the mask covers
    /// solenoids 1–64; others always pass.
    pub fn solenoid_wanted(mask: u64) -> impl Fn(i32) -> bool {
        move |n| !(1..=64).contains(&n) || mask & (1 << (n - 1)) != 0
    }

    /// `Mech(n) = value`: builds a mechanism's description field by field;
    /// `Mech(0) = number` hands it to PinMAME as that mechanism, as VPX's
    /// controller does.
    pub fn set_mech(&mut self, which: i32, value: i32) {
        let m = &mut self.mech_pending;
        match which {
            0 => {
                let config = std::mem::take(m);
                self.mechs.insert(value, config);
                if let Some(machine) = &mut self.machine {
                    machine.set_mech(value, &config);
                }
            }
            1 => m.sol1 = value,
            2 => m.sol2 = value,
            3 => m.length = value,
            4 => m.steps = value,
            5 => m.kind = value,
            6 => m.acc = value,
            7 => m.ret = value,
            8 => m.initial_pos = value + 1,
            _ => {
                let Some(s) = usize::try_from(which / 10 - 1).ok().and_then(|i| m.switches.get_mut(i)) else { return };
                match which % 10 {
                    0 => s.number = value,
                    1 => s.start = value,
                    2 => s.end = value,
                    3 => s.pulse = value,
                    _ => {}
                }
            }
        }
    }

    /// `GetMech(n)`: mechanism n's position, or under -n its speed.
    pub fn mech(&mut self, number: i32) -> i32 {
        self.machine.as_ref().map_or(0, |m| m.link.shared().mech(number))
    }

    /// `TimeFence = seconds`: PinMAME's emulation runs up to that game time
    /// and no further (0 for no fence).
    pub fn set_time_fence(&mut self, seconds: f64) {
        self.time_fence = seconds;
        if let Some(m) = &mut self.machine {
            m.link.shared().set_time_fence(seconds);
            m.link.wake_host();
        }
    }

    /// `ModOutputType(output, number) = kind`: VPX passes on only solenoids'.
    pub fn set_mod_output_type(&mut self, output: i32, number: i32, kind: i32) {
        if output == 0
            && let Some(m) = &mut self.machine
        {
            m.send(Command::ModOutputType { output, number, kind });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_are_read_as_libpinmame_reads_them() {
        let list = "# a comment\nmtl_180h,mtl_170h\r\nfoo bar ; trailing\n'quoted,skip\n";
        assert_eq!(alias_in(list, "MTL_180H").as_deref(), Some("mtl_170h"));
        assert_eq!(alias_in(list, "foo").as_deref(), Some("bar"));
        assert_eq!(alias_in(list, "quoted"), None);
        assert_eq!(alias_in(list, "cv_20hc"), None);
    }

    #[test]
    fn changes_report_what_differs_from_what_was_told() {
        let mut c = Changes::default();
        c.set(11, 1);
        c.set(12, 1);
        c.set(12, 0); // on and off again before the script asked
        assert_eq!(c.take(|_| true), vec![(11, 1)]);
        assert!(c.take(|_| true).is_empty());
        c.set(11, 0);
        c.set(13, 255);
        // a masked-out change still counts as told
        assert_eq!(c.take(|n| n != 13), vec![(11, 0)]);
        c.set(13, 255);
        assert!(c.take(|_| true).is_empty());
    }

    #[test]
    fn the_machine_sound_waits_for_its_cushion_then_plays_in_order() {
        use crate::audio::StreamSource;
        let name = format!("Local\\VPI-PinMAME-sound-test-{}", std::process::id());
        let link = Arc::new(Link::create(&name).unwrap());
        let shared = link.shared();
        let mut sound = MachineSound { link: link.clone(), next: None, dry: false };
        let mut out = vec![0.0f32; 2 * 256];
        let level = |x: f32| (x * 32_768.0).round() as i32;
        // no sound yet: silence
        sound.mix_into(&mut out, 1.0);
        assert!(out.iter().all(|&x| x == 0.0));
        // a ramp, frame i reading i on the left and -i on the right
        shared.set_audio_rate(48_000);
        shared.publish_audio((0..2000).map(|i| [i as i16, -(i as i16)]));
        sound.mix_into(&mut out, 1.0);
        // it starts a cushion behind the newest frame
        assert_eq!((level(out[0]), level(out[1])), (800, -800));
        assert_eq!(level(out[2 * 255]), 1055);
        // and carries on where it stopped, at half the gain
        out.fill(0.0);
        sound.mix_into(&mut out, 0.5);
        assert_eq!(level(out[0]), 1056 / 2);
        // past the newest frame it runs dry; with nothing new (PinMAME paused) it stays silent,
        // never playing its last frames again
        for _ in 0..3 {
            out.fill(0.0);
            sound.mix_into(&mut out, 1.0);
        }
        assert!(sound.dry);
        out.fill(0.0);
        sound.mix_into(&mut out, 1.0);
        assert!(out.iter().all(|&x| x == 0.0));
        // PinMAME at half the mixer's rate: each frame plays twice, blended
        shared.set_audio_rate(24_000);
        shared.publish_audio((2000..4000).map(|i| [i as i16, 0]));
        out.fill(0.0);
        sound.mix_into(&mut out, 1.0);
        // on from where it ran dry: frame 1999, which it hadn't played
        let start = 1999;
        // halfway between two frames, their blend
        assert_eq!((out[0] * 32_768.0, out[2] * 32_768.0, out[4] * 32_768.0), (start as f32, start as f32 + 0.5, start as f32 + 1.0));
    }

    #[test]
    fn changed_leds_report_named_digits_and_count_every_digit_told() {
        let mut leds = Leds::default();
        leds.set(0, 0x3F); // a 0
        leds.set(5, 0x06); // a 1
        leds.set(70, 0x7F); // an 8, beyond the first 64
        leds.set(MAX_LEDS as i32, 1); // beyond what PinMAME numbers: ignored
        // the first mask names digits 0 to 63, the second 64 to 127
        assert_eq!(leds.take(u64::MAX, 1 << 6), vec![(0, 0x3F, 0x3F), (5, 0x06, 0x06), (70, 0x7F, 0x7F)]);
        assert!(leds.take(u64::MAX, u64::MAX).is_empty());
        // the changed segments are those that differ from what was told
        leds.set(0, 0x06);
        leds.set(5, 0x5B);
        // digit 5 isn't named, but counts as told all the same
        assert_eq!(leds.take(1, 0), vec![(0, 0x39, 0x06)]);
        assert!(leds.take(u64::MAX, u64::MAX).is_empty());
        assert_eq!(leds.current(5), 0x5B);
        // the masks scripts pass: all digits, or the low ones only
        assert_eq!(led_mask(-1, -1), u64::MAX);
        assert_eq!(led_mask(0xFF, 0xFFFF), 0xFF_0000_FFFF);
        assert_eq!(led_mask(0, -1), u64::MAX);
    }

    #[test]
    fn the_solenoid_mask_covers_1_to_64() {
        let wanted = Controller::solenoid_wanted(!(1u64 << 8)); // solenoid 9 masked
        assert!(wanted(1) && wanted(64) && wanted(65) && wanted(0));
        assert!(!wanted(9));
    }
}
