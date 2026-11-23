//! VPX itself, as the app starts it to play a table: its program found beside the tables — one of
//! the programs VPX comes as, its renderers', the newest by the version each carries — started as a
//! front end starts it (`-Play <table>`, which quits when the game ends), and where VPX 10.8.1 and
//! its own add-ons keep what a game leaves beside a table (its `FileLayout.md`): the values a
//! script saves in the table folder's `user\VPReg.stg`, else VPX's own `user`; a machine's memory
//! in the table folder's `pinmame\nvram`, where the table brings its own `pinmame\roms`, else
//! PinMAME's folder's `nvram`. Ours.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::plan::Layout;

/// The programs VPX comes as, a renderer each (10.8.1's BGFX, OpenGL and Direct3D 9; 10.8.0's
/// OpenGL and Direct3D 9): among programs of the same version, the first named here is started.
pub const PROGRAMS: [&str; 3] = ["VPinballX_BGFX64.exe", "VPinballX_GL64.exe", "VPinballX64.exe"];

/// The first version the app plays tables in: VPX 10.8.0, VPX's release before 10.8.1, started
/// minimized so it quits when the game ends (`play_command`).
pub const PLAYS_FROM: [u16; 4] = [10, 8, 0, 0];

/// The first version with VPX's own add-ons and the plugin interface the app's add-on is made for,
/// its quiet capture mode (`-CaptureAttract`) and its preferences folder given (`-PrefPath`):
/// VPX 10.8.1.
pub const ADDONS_FROM: [u16; 4] = [10, 8, 1, 0];

/// One VPX program and the version it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vpx {
    pub program: PathBuf,
    /// Its file version, as Windows reads it from the program (10.8.1.5436); zeros when it carries
    /// none.
    pub version: [u16; 4],
}

impl Vpx {
    /// Whether the app plays tables in it: 10.8.0 or newer.
    pub fn plays(&self) -> bool {
        self.version >= PLAYS_FROM
    }

    /// Whether it carries VPX's own add-ons, plays ours and runs quietly: 10.8.1 or newer.
    pub fn has_addons(&self) -> bool {
        self.version >= ADDONS_FROM
    }

    /// Its line, as `[VPI]` `PlayWith` names one: `10.8.0`.
    pub fn line(&self) -> String {
        format!("{}.{}.{}", self.version[0], self.version[1], self.version[2])
    }
}

/// The version a program carries in its version resource (VS_FIXEDFILEINFO's file version), as
/// Explorer shows it; None when it carries none.
#[cfg(windows)]
pub fn version(program: &Path) -> Option<[u16; 4]> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "version")]
    unsafe extern "system" {
        fn GetFileVersionInfoSizeW(file: *const u16, handle: *mut u32) -> u32;
        fn GetFileVersionInfoW(file: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
        fn VerQueryValueW(block: *const c_void, sub_block: *const u16, buffer: *mut *mut c_void, len: *mut u32) -> i32;
    }
    let file: Vec<u16> = program.as_os_str().encode_wide().chain([0]).collect();
    let mut handle = 0;
    // SAFETY: a NUL-ended path
    let size = unsafe { GetFileVersionInfoSizeW(file.as_ptr(), &mut handle) };
    if size == 0 {
        return None;
    }
    let mut block = vec![0u8; size as usize];
    // SAFETY: `block` holds the `size` bytes asked for
    if unsafe { GetFileVersionInfoW(file.as_ptr(), 0, size, block.as_mut_ptr().cast()) } == 0 {
        return None;
    }
    let root: Vec<u16> = "\\".encode_utf16().chain([0]).collect();
    let (mut fixed, mut len): (*mut c_void, u32) = (std::ptr::null_mut(), 0);
    // SAFETY: the block read above; the root names its fixed part
    if unsafe { VerQueryValueW(block.as_ptr().cast(), root.as_ptr(), &mut fixed, &mut len) } == 0 || fixed.is_null() || len < 16 {
        return None;
    }
    // its signature, its own layout's version, then the file's version, its high half first
    // SAFETY: `fixed` points at `len` bytes inside `block`, which outlives the read
    let [signature, _, high, low] = unsafe { std::ptr::read_unaligned(fixed.cast::<[u32; 4]>()) };
    (signature == 0xFEEF_04BD).then_some([(high >> 16) as u16, high as u16, (low >> 16) as u16, low as u16])
}

#[cfg(not(windows))]
pub fn version(_program: &Path) -> Option<[u16; 4]> {
    None
}

/// Every one of VPX's programs in `folders`, the newest first, the first named in `PROGRAMS` first
/// among equals.
pub fn all_in(folders: &[PathBuf]) -> Vec<Vpx> {
    let mut all: Vec<Vpx> = Vec::new();
    for folder in folders {
        for name in PROGRAMS {
            let program = folder.join(name);
            if program.is_file() && !all.iter().any(|v| v.program == program) {
                all.push(Vpx { version: version(&program).unwrap_or_default(), program });
            }
        }
    }
    // a stable sort: among equals, the order found
    all.sort_by_key(|v| std::cmp::Reverse(v.version));
    all
}

/// The newest of VPX's programs in `folders`, the first named in `PROGRAMS` among equals.
pub fn newest_in(folders: &[PathBuf]) -> Option<Vpx> {
    all_in(folders).into_iter().next()
}

/// The folders a library's VPX is looked for in: VPX's folder as the settings name it, the folder
/// above the tables (VPX keeps its `Tables` in its own folder), and the tables folder itself.
fn folders_of(layout: &Layout) -> Vec<PathBuf> {
    layout.vpx_folder.iter().cloned().chain(layout.tables.parent().map(Path::to_path_buf)).chain([layout.tables.clone()]).collect()
}

/// Every VPX program a library has: the one `VPI_VPX` names, or those in the folder it names; else
/// those in its folders (`folders_of`); the newest first.
pub fn all_for(layout: &Layout) -> Vec<Vpx> {
    if let Some(named) = std::env::var_os("VPI_VPX").map(PathBuf::from) {
        if named.is_file() {
            return vec![Vpx { version: version(&named).unwrap_or_default(), program: named }];
        }
        return all_in(&[named]);
    }
    all_in(&folders_of(layout))
}

/// The newest VPX a library has: the one its quiet runs use — tests, a machine's first start, the
/// script editor's check — which need 10.8.1's capture mode (`Vpx::has_addons`), whichever plays.
pub fn find(layout: &Layout) -> Option<Vpx> {
    all_for(layout).into_iter().next()
}

/// The VPX a library plays its tables in: the newest of the line the settings choose (`[VPI]`
/// `PlayWith`, `Layout::play_with`), where there's one; else the newest.
pub fn to_play(layout: &Layout) -> Option<Vpx> {
    choose(all_for(layout), layout.play_with.as_deref())
}

/// Of `all` (the newest first), the newest of `line` where there's one, else the newest.
fn choose(all: Vec<Vpx>, line: Option<&str>) -> Option<Vpx> {
    let chosen = line.and_then(|line| all.iter().find(|v| v.line() == line.trim()).cloned());
    chosen.or_else(|| all.into_iter().next())
}

/// Whether a library plays its tables in VPX 10.8.0, kept beside a newer one: the line its settings
/// choose, found.
pub fn plays_older(layout: &Layout) -> bool {
    to_play(layout).is_some_and(|v| !v.has_addons())
}

/// The preferences folder VPX is to use in place of its own, where `VPI_VPX_PREFS` names one (a
/// test library's, so VPX's own settings are never read or written).
pub fn prefs_override() -> Option<PathBuf> {
    std::env::var_os("VPI_VPX_PREFS").map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

/// `table` played in `vpx` as a front end starts it, which quits when the game ends: `-Play
/// <table>` — 10.8.0's started minimized as front ends start it, its pause menu kept
/// (`-ExtMinimized`), as only then does it quit after the game (else its editor shows) — VPX's
/// preferences in `prefs` where they're not its own (`-PrefPath`, 10.8.1's), started in VPX's
/// folder.
pub fn play_command(vpx: &Vpx, table: &Path, prefs: Option<&Path>) -> Command {
    let mut command = Command::new(&vpx.program);
    if let Some(prefs) = prefs.filter(|_| vpx.has_addons()) {
        command.arg("-PrefPath").arg(prefs);
    }
    if !vpx.has_addons() {
        command.arg("-ExtMinimized");
    }
    command.arg("-Play").arg(table);
    if let Some(dir) = vpx.program.parent() {
        command.current_dir(dir);
    }
    if let Some(path) = runtime_path() {
        command.env("PATH", path);
    }
    command
}

/// Microsoft's C++ runtime as VPX's own libraries load it — its picture library, SDL and most of
/// its add-ons — which VPX's builds don't carry, so that a PC without it can't start VPX at all.
const RUNTIME: [&str; 3] = ["msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"];

/// PATH for a VPX this program starts, with this program's folder last where it holds a copy of
/// Microsoft's C++ runtime, as the installer puts one beside the app: Windows looks there only after
/// its own folders, so a PC's own runtime, where it has one, is the one VPX loads, and VPX's folder
/// is left as it is. None where this program's folder holds none, as a development build's.
pub fn runtime_path() -> Option<std::ffi::OsString> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    runtime_path_with(&dir, std::env::var_os("PATH"))
}

/// `path` with `dir` last, where `dir` holds the runtime and isn't there already.
fn runtime_path_with(dir: &Path, path: Option<std::ffi::OsString>) -> Option<std::ffi::OsString> {
    if !RUNTIME.iter().all(|f| dir.join(f).is_file()) || dir.as_os_str().to_string_lossy().contains(';') {
        return None;
    }
    let mut path = path.unwrap_or_default();
    if std::env::split_paths(&path).any(|p| p.as_os_str().eq_ignore_ascii_case(dir.as_os_str())) {
        return Some(path);
    }
    if !path.is_empty() && !path.to_string_lossy().ends_with(';') {
        path.push(";");
    }
    path.push(dir.as_os_str());
    Some(path)
}

/// What a PC without Windows' VBScript is told: VPX runs every table's script with it, and where
/// Windows has none, VPX shows the table and runs nothing, saying nothing of it.
pub const NO_VBSCRIPT: &str = "Windows' VBScript is off on this PC, and VPX runs every table's script with it. To turn it on, open Windows' Settings › System › Optional features, choose View features, and add VBSCRIPT; then play again.";

/// Whether Windows gives VPX its VBScript engine, asked as VPX asks for it (`CoCreateInstance` of
/// its class): Windows 11 keeps VBScript as an optional part that can be turned off. Asked on a
/// thread of its own, so COM is set up there as VPX sets it up; true where it couldn't be asked.
#[cfg(windows)]
pub fn vbscript_ready() -> bool {
    use std::ffi::c_void;
    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }
    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoInitializeEx(reserved: *const c_void, init: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(class: *const Guid, outer: *mut c_void, context: u32, iid: *const Guid, out: *mut *mut c_void) -> i32;
    }
    // VBScript's engine, {B54F3741-5B07-11CF-A4B0-00AA004A55E8}, as VPX names it, and IUnknown
    const VBSCRIPT: Guid = Guid { data1: 0xB54F_3741, data2: 0x5B07, data3: 0x11CF, data4: [0xA4, 0xB0, 0x00, 0xAA, 0x00, 0x4A, 0x55, 0xE8] };
    const IUNKNOWN: Guid = Guid { data1: 0, data2: 0, data3: 0, data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46] };
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const CLSCTX_INPROC_SERVER: u32 = 0x1;
    std::thread::spawn(|| {
        // SAFETY: COM set up on this thread alone, and taken down below where it was
        let init = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED) };
        let mut engine: *mut c_void = std::ptr::null_mut();
        // SAFETY: the GUIDs outlive the call; `engine` receives the object or stays null
        let made = unsafe { CoCreateInstance(&VBSCRIPT, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, &IUNKNOWN, &mut engine) };
        let ready = made >= 0 && !engine.is_null();
        if ready {
            // SAFETY: a COM object's first field points at its methods, IUnknown's Release third
            unsafe {
                let methods = *engine.cast::<*const usize>();
                let release: unsafe extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(*methods.add(2));
                release(engine);
            }
        }
        if init >= 0 {
            // SAFETY: as set up above
            unsafe { CoUninitialize() };
        }
        ready
    })
    .join()
    .unwrap_or(true)
}

#[cfg(not(windows))]
pub fn vbscript_ready() -> bool {
    true
}

/// Why VPX ended before it started, where Windows stopped it as it loaded its files: its exit
/// code, Windows' own, told in words. None for any other ending.
pub fn stopped_loading(code: i32) -> Option<String> {
    let why = match code as u32 {
        0xC000_0135 => "a file it needs isn't on this PC",
        0xC000_0139 => "a file it needs is older than it expects",
        0xC000_007B => "a file it needs is made for 32-bit Windows",
        0xC000_0142 => "a file it needs couldn't start",
        _ => return None,
    };
    Some(format!("VPX couldn't start: {why} (Windows' code 0x{:08X})", code as u32))
}

/// VPX's log: `vpinball.log` in its preferences folder — `prefs` where it's given one, else its
/// own, `%APPDATA%\VPinballX\<major>.<minor>`, kept apart for each minor version; 10.8.0's in the
/// folder it starts in, VPX's own, where its settings switch it on (`[Editor]` `EnableLog`).
pub fn log_file(vpx: &Vpx, prefs: Option<&Path>) -> Option<PathBuf> {
    if !vpx.has_addons() {
        return Some(vpx.program.parent()?.join("vpinball.log"));
    }
    Some(prefs_folder(vpx, prefs)?.join("vpinball.log"))
}

/// VPX 10.8.1's preferences folder, beside its settings: `prefs` where the app names another
/// (`-PrefPath`), else `%APPDATA%\VPinballX\<major>.<minor>`.
pub fn prefs_folder(vpx: &Vpx, prefs: Option<&Path>) -> Option<PathBuf> {
    match prefs {
        Some(p) => Some(p.to_path_buf()),
        None => Some(PathBuf::from(std::env::var_os("APPDATA")?).join("VPinballX").join(format!("{}.{}", vpx.version[0], vpx.version[1]))),
    }
}

/// VPX's preferences folder made ready for the app to write in, as VPX's first start makes it where
/// it isn't there yet, an older version's settings brought in (`vpi_settings::vpx_prefs_ready_for`):
/// `prefs` where the app names another (`-PrefPath`).
pub fn prefs_ready(vpx: &Vpx, prefs: Option<&Path>) -> Result<PathBuf, String> {
    match prefs {
        Some(p) => {
            std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
            Ok(p.to_path_buf())
        }
        None => vpi_settings::vpx_prefs_ready_for(vpx.version, vpx.program.parent()),
    }
}

/// The settings file the VPX that plays reads: 10.8.1's and newer's in their preferences folder,
/// their version's (`vpi_settings::vpx_settings_path_for`), which the app's settings pages keep
/// for the one they follow (`follow`); 10.8.0's its own (`vpi_settings::vpx_1080_settings_path`).
pub fn settings_file(vpx: &Vpx) -> Option<PathBuf> {
    if vpx.has_addons() { vpi_settings::vpx_settings_path_for(vpx.version, vpx.program.parent()) } else { vpi_settings::vpx_1080_settings_path(vpx.program.parent()) }
}

/// That file's text as VPX reads it, where it's there: for 10.8.1 and newer, before VPX has made
/// its preferences folder, the settings its first start brings in
/// (`vpi_settings::vpx_settings_text_for`).
pub fn settings_text(vpx: &Vpx) -> String {
    if vpx.has_addons() {
        return vpi_settings::vpx_settings_text_for(vpx.version, vpx.program.parent());
    }
    settings_file(vpx).and_then(|p| std::fs::read(p).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// The VPX whose settings the app's settings pages keep: the one that plays (`to_play`), where it's
/// 10.8.1 or newer; else, where 10.8.0 plays, the newest beside it that is.
pub fn settings_vpx(layout: &Layout) -> Option<Vpx> {
    settings_of(all_for(layout), layout.play_with.as_deref())
}

/// Of `all` (the newest first), the one whose settings are kept, as `settings_vpx` chooses.
fn settings_of(all: Vec<Vpx>, line: Option<&str>) -> Option<Vpx> {
    let playing = choose(all.clone(), line);
    playing.filter(Vpx::has_addons).or_else(|| all.into_iter().find(Vpx::has_addons))
}

/// The app's settings followed to that VPX's own (`vpi_settings::follow_vpx`), as each minor
/// version keeps its own preferences folder: the VPX.
pub fn follow(layout: &Layout) -> Option<Vpx> {
    let vpx = settings_vpx(layout);
    vpi_settings::follow_vpx(vpx.as_ref().map(|v| (v.version, v.program.parent().map(Path::to_path_buf))));
    vpx
}

/// Where VPX keeps the values a table's script saves (`SaveValue`), in the order it reads them: the
/// table folder's `user\VPReg.stg` (10.8.1's own folder for each table), the tables folder's (the
/// same, for a table kept in the tables folder itself), else VPX's own `User` (where VPX kept them
/// before 10.8.1, and still reads them).
pub fn saved_values(layout: &Layout, table: Option<&Path>) -> Vec<PathBuf> {
    let own = layout.vpx_folder.clone().or_else(|| layout.tables.parent().map(Path::to_path_buf)).map(|v| v.join("User").join("VPReg.stg"));
    // VPX 10.8.0 keeps them in its own `User` alone
    if layout.play_with.as_deref().is_some_and(|l| l.trim() == "10.8.0") && plays_older(layout) {
        return own.into_iter().collect();
    }
    let mut files: Vec<PathBuf> = table.and_then(Path::parent).map(|d| d.join("user").join("VPReg.stg")).into_iter().collect();
    files.push(layout.tables.join("user").join("VPReg.stg"));
    files.extend(own);
    files.dedup();
    files
}

/// The folder VPX's PinMAME keeps a table's machine's memory in: the table folder's
/// `pinmame\nvram` where the table brings its own `pinmame\roms` (VPX 10.8.1's PinMAME add-on
/// looks there first), else the `nvram` of PinMAME's own folder (the add-on's `PinMAMEPath`, or
/// VPinMAME's).
pub fn machine_memory_folder(layout: &Layout, table: Option<&Path>) -> PathBuf {
    match table.and_then(Path::parent) {
        Some(dir) if dir.join("pinmame").join("roms").is_dir() => dir.join("pinmame").join("nvram"),
        _ => layout.vpinmame.join("nvram"),
    }
}

/// The runs of VPX's main automatic build of its newest code that succeeded on `master`, newest
/// first, as GitHub's API lists them to anyone.
const AUTOMATIC_RUNS: &str = "https://api.github.com/repos/vpinball/vpinball/actions/workflows/vpinball.yml/runs?branch=master&event=push&status=success&per_page=5";

/// One of VPX's builds for Windows: the zip of its BGFX program, as GitHub hands it out.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// Its tag (`v10.8.1-6758-0af977650`) and a title saying what it is.
    pub tag: String,
    pub title: String,
    pub prerelease: bool,
    pub zip: String,
    pub url: String,
    pub size: u64,
    /// The checksum GitHub keeps of the zip: SHA-256, in hex.
    pub sha256: String,
}

/// The VPX 10.8.1 build the app is made and checked with. VPX 10.8.1 has no release: its builds are
/// VPX's automatic builds of its newest code, the 10.8.1 builds on its releases page older ones it
/// names "DO NOT USE". Its zip comes as VPX's own build made it, through nightly.link, which hands
/// out a public project's automatic builds without a GitHub sign-in, and is checked against the
/// checksum GitHub keeps of it. GitHub keeps an automatic build for 90 days.
pub const MADE_FOR: u16 = 6758;

/// That build, as Get VPX 10.8.1 downloads it.
pub fn made_for() -> Release {
    Release {
        tag: "v10.8.1-6758-0af977650".into(),
        title: "VPX 10.8.1 build 6758, an automatic build of VPX's code".into(),
        prerelease: true,
        zip: "VPinballX_BGFX-10.8.1-6758-0af977650-windows-x64-Release.zip".into(),
        url: "https://nightly.link/vpinball/vpinball/actions/artifacts/13887182659.zip".into(),
        size: 128_435_358,
        sha256: "6c3e1b71c5022fb70e77c9e7ee9aa03cadeca5984667103189de0c48564f14c2".into(),
    }
}

/// Whether a VPX 10.8.1 is older than the build the app is made for.
pub fn older_than_made_for(version: [u16; 4]) -> bool {
    version[..3] == [10, 8, 1] && version[3] < MADE_FOR
}

/// Whether GitHub still keeps the build the app is made for, an automatic build being kept 90 days:
/// `None` where GitHub can't tell.
pub fn made_for_kept() -> Option<bool> {
    let id = artifact_of(&made_for().url)?;
    let (code, body) = github_get(&format!("https://api.github.com/repos/vpinball/vpinball/actions/artifacts/{id}")).ok()?;
    match code {
        404 | 410 => Some(false),
        200 => serde_json::from_str::<serde_json::Value>(&body).ok()?.get("expired")?.as_bool().map(|expired| !expired),
        _ => None,
    }
}

/// The automatic build a nightly.link address hands out: its number on GitHub.
fn artifact_of(url: &str) -> Option<u64> {
    url.rsplit('/').next()?.strip_suffix(".zip")?.parse().ok()
}

/// VPX's newest automatic build of `line` (`10.8.1`) for Windows, for where GitHub no longer keeps
/// the build the app is made for: the BGFX program's zip from the newest successful run of VPX's
/// main build on `master`, through nightly.link, with the checksum GitHub keeps of it. `None` where
/// the newest runs made none of that line — VPX's newest code numbered a newer VPX since.
pub fn newest_automatic(line: &str) -> Result<Option<Release>, String> {
    let (code, runs) = github_get(AUTOMATIC_RUNS)?;
    if code != 200 {
        return Err(format!("VPX's automatic builds couldn't be read from GitHub (HTTP {code})."));
    }
    for run in run_ids(&runs) {
        let (code, made) = github_get(&format!("https://api.github.com/repos/vpinball/vpinball/actions/runs/{run}/artifacts?per_page=100"))?;
        if code != 200 {
            return Err(format!("VPX's automatic build couldn't be read from GitHub (HTTP {code})."));
        }
        if let Some(release) = bgfx_in(&made, line) {
            return Ok(Some(release));
        }
    }
    Ok(None)
}

/// A GitHub API address read through Windows' own `curl.exe`, no sign-in: its HTTP status and body.
fn github_get(url: &str) -> Result<(u16, String), String> {
    let out = crate::vps::curl()
        .args(["--silent", "--show-error", "--location", "--max-time", "60", "--header", "Accept: application/vnd.github+json", "--write-out", "\\n%{http_code}", url])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("GitHub couldn't be reached: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    Ok((code.trim().parse().unwrap_or(0), body.to_string()))
}

/// The runs in GitHub's list of a workflow's runs (its JSON), in its order.
fn run_ids(json: &str) -> Vec<u64> {
    let list: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    list.get("workflow_runs").and_then(|r| r.as_array()).into_iter().flatten().filter_map(|r| r.get("id")?.as_u64()).collect()
}

/// The zip of `line`'s BGFX program for Windows among what a run made (GitHub's list of its
/// artifacts, as JSON), while GitHub keeps it, as nightly.link hands it out.
fn bgfx_in(json: &str, line: &str) -> Option<Release> {
    use serde_json::Value;
    let list: Value = serde_json::from_str(json).ok()?;
    let start = format!("VPinballX_BGFX-{line}-");
    list.get("artifacts")?.as_array()?.iter().find_map(|a| {
        let name = a.get("name")?.as_str()?;
        let made = name.strip_prefix(&start)?.strip_suffix("-windows-x64-Release.zip")?;
        if a.get("expired").and_then(Value::as_bool) != Some(false) {
            return None;
        }
        let sha256 = a.get("digest")?.as_str()?.strip_prefix("sha256:")?.to_ascii_lowercase();
        let build = made.split('-').next()?;
        Some(Release {
            tag: format!("v{line}-{made}"),
            title: format!("VPX {line} build {build}, an automatic build of VPX's code"),
            prerelease: true,
            zip: name.to_string(),
            url: format!("https://nightly.link/vpinball/vpinball/actions/artifacts/{}.zip", a.get("id")?.as_u64()?),
            size: a.get("size_in_bytes").and_then(Value::as_u64).unwrap_or(0),
            sha256,
        })
    })
}

/// Why a download of VPX wasn't kept.
#[derive(Debug, PartialEq, Eq)]
pub enum NotKept {
    /// It couldn't be fetched, or written: why.
    Unreachable(String),
    /// It came, but didn't match the checksum GitHub keeps of it.
    Mismatch,
}

impl std::fmt::Display for NotKept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotKept::Unreachable(why) => f.write_str(why),
            NotKept::Mismatch => f.write_str("VPX's download doesn't match the checksum GitHub keeps of it, so it wasn't kept."),
        }
    }
}

/// `release`'s zip downloaded into `folder` through Windows' own `curl.exe` (as `<zip>.part` while
/// it comes, how much has come told as it grows), kept only where it matches the checksum GitHub
/// keeps of it. The zip.
pub fn download(release: &Release, folder: &Path, told: &dyn Fn(u64)) -> Result<PathBuf, NotKept> {
    let unreachable = NotKept::Unreachable;
    std::fs::create_dir_all(folder).map_err(|e| unreachable(format!("{}: {e}", folder.display())))?;
    let zip = folder.join(&release.zip);
    let part = folder.join(format!("{}.part", release.zip));
    let mut child = crate::vps::curl()
        .args(["--fail", "--silent", "--show-error", "--location", "--max-time", "3600", "--output"])
        .arg(&part)
        .arg(&release.url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| unreachable(format!("curl: {e}")))?;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| unreachable(e.to_string()))? {
            break status;
        }
        told(std::fs::metadata(&part).map_or(0, |m| m.len()));
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(unreachable(format!("VPX couldn't be downloaded ({status}).")));
    }
    if sha256_of(&part).map_err(unreachable)? != release.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(NotKept::Mismatch);
    }
    std::fs::rename(&part, &zip).map_err(|e| unreachable(format!("{}: {e}", zip.display())))?;
    Ok(zip)
}

fn sha256_of(file: &Path) -> Result<String, String> {
    use sha2::Digest as _;
    use std::io::Read as _;
    let fail = |e: std::io::Error| format!("{}: {e}", file.display());
    let mut f = std::fs::File::open(file).map_err(fail)?;
    let (mut hash, mut buf) = (sha2::Sha256::new(), vec![0u8; 1 << 20]);
    loop {
        let n = f.read(&mut buf).map_err(fail)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The add-on a file of VPX's belongs to, by its folder in `plugins`, in lower case.
fn addon_of(rel: &Path) -> Option<String> {
    let mut parts = rel.components();
    let first = parts.next()?.as_os_str().to_string_lossy().to_lowercase();
    let second = parts.next()?.as_os_str().to_string_lossy().to_lowercase();
    // a file directly in `plugins` belongs to none
    parts.next()?;
    (first == "plugins").then_some(second)
}

/// The files a VPX put in by `install` brought, as its `came in.txt` in `aside` lists them.
pub fn came_in(aside: &Path) -> Vec<String> {
    std::fs::read_to_string(aside.join("came in.txt")).map(|t| t.lines().map(str::trim).filter(|l| !l.is_empty() && !l.contains("..")).map(str::to_string).collect()).unwrap_or_default()
}

/// VPX's files put into `folder` from its zip, so the VPX before can be put back (`put_back`):
/// every file it brings listed in `aside` first (`came in.txt`), then each of the folder's own it
/// replaces moved aside into `aside`, as it lay in the folder. The add-ons the VPX before brought
/// (`brought_before`, its own `came in.txt`) that this one doesn't — renamed or gone — are moved
/// aside too: built for that build's add-on interface, a later VPX loading them could crash. Ours,
/// which reads either interface, stays. How many files came in.
pub fn install(zip: &Path, folder: &Path, aside: &Path, brought_before: &[String]) -> Result<usize, String> {
    let fail = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let file = std::fs::File::open(zip).map_err(|e| fail(zip, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", zip.display()))?;
    let files: Vec<(usize, PathBuf)> = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            (!entry.is_dir()).then(|| entry.enclosed_name()).flatten().map(|rel| (i, rel))
        })
        .collect();
    std::fs::create_dir_all(aside).map_err(|e| fail(aside, e))?;
    let list: String = files.iter().map(|(_, rel)| format!("{}\n", rel.display())).collect();
    std::fs::write(aside.join("came in.txt"), list).map_err(|e| fail(aside, e))?;
    for (i, rel) in &files {
        let to = folder.join(rel);
        if to.is_file() {
            let kept = aside.join(rel);
            if let Some(dir) = kept.parent() {
                std::fs::create_dir_all(dir).map_err(|e| fail(dir, e))?;
            }
            if std::fs::rename(&to, &kept).is_err() {
                std::fs::copy(&to, &kept).map_err(|e| fail(&to, e))?;
                std::fs::remove_file(&to).map_err(|e| fail(&to, e))?;
            }
        }
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| fail(dir, e))?;
        }
        let mut entry = archive.by_index(*i).map_err(|e| e.to_string())?;
        let mut out = std::fs::File::create(&to).map_err(|e| fail(&to, e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| fail(&to, e))?;
    }
    // the earlier VPX's add-ons this one doesn't bring
    let brings: std::collections::HashSet<String> = files.iter().filter_map(|(_, rel)| addon_of(rel)).collect();
    let mut emptied = std::collections::BTreeSet::new();
    for rel in brought_before.iter().map(PathBuf::from) {
        let Some(addon) = addon_of(&rel) else { continue };
        if brings.contains(&addon) || addon == crate::vpx_addon::FOLDER {
            continue;
        }
        let from = folder.join(&rel);
        if !from.is_file() {
            continue;
        }
        let kept = aside.join(&rel);
        if let Some(dir) = kept.parent() {
            std::fs::create_dir_all(dir).map_err(|e| fail(dir, e))?;
        }
        if std::fs::rename(&from, &kept).is_err() {
            std::fs::copy(&from, &kept).map_err(|e| fail(&from, e))?;
            std::fs::remove_file(&from).map_err(|e| fail(&from, e))?;
        }
        emptied.insert(folder.join("plugins").join(&addon));
    }
    // their folders taken away where nothing's left in them (deepest first)
    for dir in emptied {
        let mut inner: Vec<PathBuf> = walk_dirs(&dir);
        inner.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for d in inner.iter().chain(std::iter::once(&dir)) {
            let _ = std::fs::remove_dir(d);
        }
    }
    Ok(files.len())
}

/// Every folder under `dir`.
fn walk_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                out.push(p.clone());
                todo.push(p);
            }
        }
    }
    out
}

/// The VPX before put back into `folder` from what `install` kept in `aside`: each file the newer
/// one brought taken out (`recycle`: to the Recycle Bin), each it replaced moved back, then `aside`
/// emptied.
pub fn put_back(folder: &Path, aside: &Path, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<(), String> {
    let list = std::fs::read_to_string(aside.join("came in.txt")).map_err(|e| format!("{}: {e}", aside.display()))?;
    for rel in list.lines().map(str::trim).filter(|l| !l.is_empty() && !l.contains("..")) {
        let file = folder.join(rel);
        if file.is_file() {
            recycle(&file)?;
        }
    }
    let mut kept = Vec::new();
    let mut folders = vec![aside.to_path_buf()];
    while let Some(dir) = folders.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
            let path = e.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.file_name().is_some_and(|n| n != "came in.txt") {
                kept.push(path);
            }
        }
    }
    for from in kept {
        let Ok(rel) = from.strip_prefix(aside) else { continue };
        let to = folder.join(rel);
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        if std::fs::rename(&from, &to).is_err() {
            std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    std::fs::remove_dir_all(aside).map_err(|e| format!("{}: {e}", aside.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_runtime_beside_the_app_looked_for_after_windows_own_folders() {
        let dir = std::env::temp_dir().join(format!("vpi-runtime-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let windows = std::ffi::OsString::from(r"C:\Windows\system32;C:\Windows");
        // none beside it: PATH left as it is
        assert_eq!(runtime_path_with(&dir, Some(windows.clone())), None);
        for f in RUNTIME {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        // last, after Windows' own folders, once
        let path = runtime_path_with(&dir, Some(windows.clone())).unwrap();
        let folders: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(folders, [PathBuf::from(r"C:\Windows\system32"), PathBuf::from(r"C:\Windows"), dir.clone()]);
        assert_eq!(runtime_path_with(&dir, Some(path.clone())), Some(path));
        // a PATH ending in its separator, or none
        assert_eq!(std::env::split_paths(&runtime_path_with(&dir, Some(r"C:\Windows;".into())).unwrap()).filter(|p| !p.as_os_str().is_empty()).count(), 2);
        assert_eq!(runtime_path_with(&dir, None), Some(dir.clone().into_os_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(windows)]
    fn windows_vbscript_found_as_vpx_asks_for_it() {
        // a development PC plays tables in VPX: VBScript is there
        assert!(vbscript_ready());
        assert!(NO_VBSCRIPT.contains("VBSCRIPT"));
    }

    #[test]
    fn windows_stopping_vpx_as_it_loads_told_in_words() {
        assert_eq!(stopped_loading(0xC000_0135_u32 as i32).as_deref(), Some("VPX couldn't start: a file it needs isn't on this PC (Windows' code 0xC0000135)"));
        assert!(stopped_loading(0xC000_0139_u32 as i32).is_some());
        assert_eq!(stopped_loading(0), None);
        assert_eq!(stopped_loading(1), None);
        // a crash during play is VPX's log's to tell
        assert_eq!(stopped_loading(0xC000_0005_u32 as i32), None);
    }

    #[test]
    fn the_newest_automatic_build_found_in_githubs_lists() {
        let runs = r#"{"total_count": 2, "workflow_runs": [{"id": 44255688327, "run_number": 6445}, {"id": 44251321441}]}"#;
        assert_eq!(run_ids(runs), [44255688327, 44251321441]);
        assert!(run_ids("not json").is_empty());
        let made = r#"{"total_count": 5, "artifacts": [
            {"id": 1, "name": "VPinballX-10.8.1-6921-bdbccaf30-windows-x64-Release.zip", "size_in_bytes": 5, "expired": false, "digest": "sha256:aa"},
            {"id": 2, "name": "VPinballX_GL-10.8.1-6921-bdbccaf30-windows-x64-Release.zip", "size_in_bytes": 5, "expired": false, "digest": "sha256:bb"},
            {"id": 3, "name": "VPinballX_BGFX-10.8.1-6921-bdbccaf30-windows-x86-Release.zip", "size_in_bytes": 5, "expired": false, "digest": "sha256:cc"},
            {"id": 4, "name": "VPinballX_BGFX-10.8.1-6921-bdbccaf30-windows-x64-Debug.zip", "size_in_bytes": 5, "expired": false, "digest": "sha256:dd"},
            {"id": 14406868243, "name": "VPinballX_BGFX-10.8.1-6921-bdbccaf30-windows-x64-Release.zip", "size_in_bytes": 130262029, "expired": false, "digest": "sha256:3FE5"}]}"#;
        let r = bgfx_in(made, "10.8.1").unwrap();
        assert_eq!(
            (r.tag.as_str(), r.url.as_str(), r.size, r.sha256.as_str(), r.prerelease),
            ("v10.8.1-6921-bdbccaf30", "https://nightly.link/vpinball/vpinball/actions/artifacts/14406868243.zip", 130_262_029, "3fe5", true)
        );
        assert_eq!(r.title, "VPX 10.8.1 build 6921, an automatic build of VPX's code");
        assert!(bgfx_in(made, "10.9.0").is_none());
        assert!(bgfx_in(&made.replace(r#""expired": false, "digest": "sha256:3FE5""#, r#""expired": true, "digest": "sha256:3FE5""#), "10.8.1").is_none());
        assert!(bgfx_in(&made.replace(r#""digest": "sha256:3FE5""#, r#""digest": null"#), "10.8.1").is_none());
    }

    /// VPX's newest automatic build of each line as GitHub lists them today (`VPI_VPX_LINES`, else
    /// 10.8.1 and 10.9.0), and whether it still keeps the one the app is made for:
    /// `cargo test -p library --lib vpx::tests::on_github_today -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn on_github_today() {
        let lines = std::env::var("VPI_VPX_LINES").unwrap_or_else(|_| "10.8.1,10.9.0".into());
        for line in lines.split(',') {
            match newest_automatic(line.trim()).unwrap() {
                Some(r) => {
                    println!("newest {line}: {} · {} · {} bytes · {}", r.tag, r.url, r.size, r.sha256);
                    assert_eq!(r.sha256.len(), 64);
                }
                None => println!("newest {line}: none among the newest runs"),
            }
        }
        println!("build {MADE_FOR} still kept: {:?}", made_for_kept());
    }

    #[test]
    fn the_build_the_app_is_made_for() {
        let r = made_for();
        assert!(r.zip.contains(&format!("-{MADE_FOR}-")) && r.tag.contains(&format!("-{MADE_FOR}-")));
        assert!(r.url.starts_with("https://nightly.link/vpinball/vpinball/actions/artifacts/"));
        assert!(artifact_of(&r.url).is_some());
        assert_eq!(r.sha256.len(), 64);
        assert!(older_than_made_for([10, 8, 1, 5436]));
        assert!(!older_than_made_for([10, 8, 1, MADE_FOR]));
        assert!(!older_than_made_for([10, 8, 0, 2051]));
    }

    #[test]
    fn a_newer_vpx_put_in_and_the_one_before_put_back() {
        use std::io::Write as _;
        let root = std::env::temp_dir().join(format!("vpi-vpx-install-{}", std::process::id()));
        let (folder, aside) = (root.join("VPX"), root.join("aside"));
        std::fs::create_dir_all(folder.join("scripts")).unwrap();
        std::fs::write(folder.join("scripts").join("core.vbs"), b"10.8.0's").unwrap();
        std::fs::write(folder.join("VPinballX64.exe"), b"10.8.0").unwrap();
        let zip_file = root.join("vpx.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&zip_file).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, text) in [("VPinballX_BGFX64.exe", "10.8.1"), ("scripts/core.vbs", "10.8.1's")] {
            zip.start_file(name, options).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        let came = install(&zip_file, &folder, &aside, &[]).unwrap();
        let after = (std::fs::read_to_string(folder.join("scripts").join("core.vbs")).unwrap(), std::fs::read_to_string(aside.join("scripts").join("core.vbs")).unwrap());
        put_back(&folder, &aside, &|f| std::fs::remove_file(f).map_err(|e| e.to_string())).unwrap();
        let back = std::fs::read_to_string(folder.join("scripts").join("core.vbs")).unwrap();
        let (bgfx_gone, old_kept, aside_gone) = (!folder.join("VPinballX_BGFX64.exe").exists(), folder.join("VPinballX64.exe").is_file(), !aside.exists());
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(came, 2);
        assert_eq!(after, ("10.8.1's".to_string(), "10.8.0's".to_string()));
        assert_eq!(back, "10.8.0's");
        assert!(bgfx_gone && old_kept && aside_gone);
    }

    #[test]
    fn the_add_ons_a_later_vpx_drops_kept_aside_and_put_back() {
        use std::io::Write as _;
        let root = std::env::temp_dir().join(format!("vpi-vpx-addons-{}", std::process::id()));
        let (folder, aside) = (root.join("VPX"), root.join("aside"));
        let file = |rel: &str, text: &str| {
            let p = folder.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        file("plugins/scoreview/plugin-scoreview64.dll", "5436");
        file("plugins/scoreview/layouts/lay.scv", "5436");
        file("plugins/b2s/plugin-b2s64.dll", "5436");
        file("plugins/vpi-achievements/plugin-vpi-achievements64.dll", "ours");
        file("plugins/someone-elses/plugin-x64.dll", "theirs");
        let before: Vec<String> = ["plugins\\scoreview\\plugin-scoreview64.dll", "plugins\\scoreview\\layouts\\lay.scv", "plugins\\b2s\\plugin-b2s64.dll"].map(String::from).to_vec();
        let zip_file = root.join("vpx.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&zip_file).unwrap());
        for (name, text) in [("plugins/b2s/plugin-b2s64.dll", "6625"), ("plugins/nixie/plugin-nixie64.dll", "6625")] {
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        install(&zip_file, &folder, &aside, &before).unwrap();
        let gone = !folder.join("plugins/scoreview").exists();
        let kept = std::fs::read_to_string(aside.join("plugins/scoreview/layouts/lay.scv")).unwrap();
        let stays = (folder.join("plugins/vpi-achievements/plugin-vpi-achievements64.dll").is_file(), folder.join("plugins/someone-elses/plugin-x64.dll").is_file());
        put_back(&folder, &aside, &|f| std::fs::remove_file(f).map_err(|e| e.to_string())).unwrap();
        let back = (std::fs::read_to_string(folder.join("plugins/scoreview/plugin-scoreview64.dll")).unwrap(), std::fs::read_to_string(folder.join("plugins/b2s/plugin-b2s64.dll")).unwrap());
        let _ = std::fs::remove_dir_all(&root);
        assert!(gone && kept == "5436" && stays == (true, true));
        assert_eq!(back, ("5436".to_string(), "5436".to_string()));
    }

    #[cfg(windows)]
    #[test]
    fn a_programs_version_read_as_windows_shows_it() {
        let windows = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap();
        // Windows' own files carry its build (26100 on Windows 11 24H2), after a version of 6.2
        let kernel = version(&windows.join("System32").join("kernel32.dll")).unwrap();
        assert!(kernel[2] >= 10240, "{kernel:?}");
        assert_eq!(version(&std::env::current_exe().unwrap()), None);
        // VPX 10.8.1's own, where the repository's build folder holds a copy of it
        let vpx = Path::new(env!("CARGO_MANIFEST_DIR")).join(r"..\..\vpx-build\vpx-10.8.1-5436\VPinballX\VPinballX_BGFX64.exe");
        if vpx.is_file() {
            assert_eq!(version(&vpx), Some([10, 8, 1, 5436]));
        }
    }

    #[test]
    fn the_settings_kept_are_the_playing_vpxs_or_the_newest_with_addons() {
        let v = |version: [u16; 4]| Vpx { program: PathBuf::from(format!("{}.{}.{}", version[0], version[1], version[2])), version };
        let (v109, v1081, v1080) = (v([10, 9, 0, 6948]), v([10, 8, 1, 6758]), v([10, 8, 0, 2058]));
        let all = vec![v109.clone(), v1081.clone(), v1080.clone()];
        // the newest plays; one chosen to play, its own; 10.8.0 chosen, the newest with add-ons
        assert_eq!(settings_of(all.clone(), None), Some(v109.clone()));
        assert_eq!(settings_of(all.clone(), Some("10.8.1")), Some(v1081.clone()));
        assert_eq!(settings_of(all.clone(), Some("10.8.0")), Some(v109.clone()));
        assert_eq!(settings_of(vec![v1081.clone(), v1080.clone()], Some("10.8.0")), Some(v1081));
        // 10.8.0 alone keeps its own settings: none followed
        assert_eq!(settings_of(vec![v1080], None), None);
        assert_eq!(settings_of(Vec::new(), None), None);
    }

    #[test]
    fn the_newest_program_plays_the_renderer_named_first_among_equals() {
        let root = std::env::temp_dir().join(format!("vpi-vpx-find-{}", std::process::id()));
        let (vpx, tables) = (root.join("VPX"), root.join("VPX").join("Tables"));
        std::fs::create_dir_all(&tables).unwrap();
        for name in ["VPinballX64.exe", "VPinballX_GL64.exe", "VPinballX_BGFX64.exe"] {
            std::fs::write(vpx.join(name), b"").unwrap();
        }
        let found = find(&Layout { tables: tables.clone(), ..Layout::under(&root) });
        let none = newest_in(&[root.join("nothing")]);
        let _ = std::fs::remove_dir_all(&root);
        // carrying no version: the order of PROGRAMS alone, and never played in
        let found = found.unwrap();
        assert_eq!(found.program.file_name().unwrap(), "VPinballX_BGFX64.exe");
        assert!(!found.plays());
        assert!(none.is_none());
        let (new, old) = (Vpx { program: PathBuf::from("BGFX"), version: [10, 8, 1, 5436] }, Vpx { program: PathBuf::from("DX9"), version: [10, 8, 0, 2058] });
        assert!(new.plays() && new.has_addons());
        assert!(old.plays() && !old.has_addons());
        assert!(!Vpx { program: PathBuf::new(), version: [10, 7, 3, 0] }.plays());
        // the line the settings choose, where it's there; else the newest
        let all = vec![new.clone(), old.clone()];
        assert_eq!(choose(all.clone(), Some("10.8.0")), Some(old.clone()));
        assert_eq!(choose(all.clone(), None), Some(new.clone()));
        assert_eq!(choose(all, Some("10.7.4")), Some(new));
        assert_eq!(old.line(), "10.8.0");
    }

    #[test]
    fn a_table_played_as_a_front_end_starts_it() {
        let vpx = Vpx { program: PathBuf::from(r"C:\VPX\VPinballX_BGFX64.exe"), version: [10, 8, 1, 5436] };
        let table = Path::new(r"C:\VPX\Tables\Attack from Mars (Bally 1995)\Attack from Mars (Bally 1995).vpx");
        let args = |c: &Command| c.get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>();
        let own = play_command(&vpx, table, None);
        let test = play_command(&vpx, table, Some(Path::new(r"C:\Test\prefs")));
        assert_eq!(args(&own), ["-Play", &table.to_string_lossy()]);
        assert_eq!(args(&test), ["-PrefPath", r"C:\Test\prefs", "-Play", &table.to_string_lossy()]);
        assert_eq!(own.get_current_dir(), Some(Path::new(r"C:\VPX")));
        assert_eq!(log_file(&vpx, Some(Path::new(r"C:\Test\prefs"))), Some(PathBuf::from(r"C:\Test\prefs\vpinball.log")));
        // 10.8.0 started minimized, as only then it quits after the game; it has no -PrefPath
        let older = Vpx { program: PathBuf::from(r"C:\VPX\VPinballX64.exe"), version: [10, 8, 0, 2058] };
        assert_eq!(args(&play_command(&older, table, Some(Path::new(r"C:\Test\prefs")))), ["-ExtMinimized", "-Play", &table.to_string_lossy()]);
        assert_eq!(log_file(&older, None), Some(PathBuf::from(r"C:\VPX\vpinball.log")));
    }

    #[test]
    fn what_a_game_leaves_found_where_vpx_keeps_it() {
        let root = std::env::temp_dir().join(format!("vpi-vpx-places-{}", std::process::id()));
        let layout = Layout::under(&root);
        let own_folder = layout.tables.join("Attack from Mars (Bally 1995)");
        let in_own = own_folder.join("Attack from Mars (Bally 1995).vpx");
        let flat = layout.tables.join("Black Knight (Williams 1980).vpx");
        std::fs::create_dir_all(own_folder.join("pinmame").join("roms")).unwrap();
        let memory_own = machine_memory_folder(&layout, Some(&in_own));
        let memory_flat = machine_memory_folder(&layout, Some(&flat));
        let _ = std::fs::remove_dir_all(&root);
        // a table bringing its own ROMs keeps its machine's memory beside them
        assert_eq!(memory_own, own_folder.join("pinmame").join("nvram"));
        assert_eq!(memory_flat, layout.vpinmame.join("nvram"));
        assert_eq!(machine_memory_folder(&layout, None), layout.vpinmame.join("nvram"));
        // saved values: the table's own folder first, the tables folder's once, VPX's own last
        assert_eq!(
            saved_values(&layout, Some(&in_own)),
            [own_folder.join("user").join("VPReg.stg"), layout.tables.join("user").join("VPReg.stg"), root.join("User").join("VPReg.stg")]
        );
        assert_eq!(saved_values(&layout, Some(&flat)), [layout.tables.join("user").join("VPReg.stg"), root.join("User").join("VPReg.stg")]);
    }
}
