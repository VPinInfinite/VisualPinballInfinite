//! The Sound page's side: the sound outputs as our player sees them (`vpi-player
//! --sound-outputs --json`), and the settings that say what plays where, under VPX 5436's names —
//! `[Player]` `SoundDevice` (the playfield's sounds) and `SoundDeviceBG` (the backglass's: music,
//! the machine's own sound, PinUP packs), `Sound3D` (the playfield's speaker layout), `SoundVolume`
//! and `MusicVolume` — with a PinUP pack's main volume (`[Plugin.PUP]` `MainVol`) and AltSound packs
//! on or off (`[Plugin.AltSound]` `Enable`); and, in VPX from build 6640, a copy of the playfield's
//! sounds for a bass shaker on an output of its own (`SoundDeviceHaptic`, off while unset; its
//! layout `Sound3DHaptic`, its volume `HapticVolume`). Saved as the Screens page saves: only what changed,
//! each file before kept beside it as `<name>.previous.ini` (VPX's as `VPinballX.previous.ini`).

use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use vpi_settings::Change;

/// One of the PC's sound outputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub name: String,
    /// Its channels, as Windows says: 2 for stereo, 6 for 5.1, 8 for 7.1.
    pub channels: Option<i32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Outputs {
    outputs: Vec<Output>,
    default_channels: Option<i32>,
    default_name: Option<String>,
}

/// What the page shows and saves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundSettings {
    /// The playfield's sounds' output, by name; None: Windows' default.
    pub playfield_output: Option<String>,
    /// The backglass's sounds' output.
    pub backglass_output: Option<String>,
    /// The playfield's speaker layout, VPX's 0 to 5.
    pub layout: i32,
    /// 0 to 100.
    pub playfield_volume: i32,
    pub backglass_volume: i32,
    /// A PinUP pack's main volume, 0 to 100.
    pub pup_volume: i32,
    /// AltSound packs play when a game has one.
    pub altsound: bool,
    /// A sound plays as each achievement is announced (`[VPI]` `AchievementSound`).
    pub achievement_sound: bool,
    /// Its level, 0 to 200 (`[VPI]` `AchievementGain`, 0 to 2: 100 as it comes).
    pub achievement_volume: i32,
    /// The bass shaker's output, a copy of the playfield's sounds; None: none, the copy off.
    pub haptic_output: Option<String>,
    /// Its speaker layout, as the playfield's.
    pub haptic_layout: i32,
    /// 0 to 100.
    pub haptic_volume: i32,
}

/// The first VPX build that plays a copy of the playfield's sounds to an output of its own, for a
/// bass shaker (its efb29490b).
pub const HAPTIC_FROM: u16 = 6640;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundView {
    pub outputs: Vec<Output>,
    pub default_channels: Option<i32>,
    /// The output Windows plays through by default now: where VPX plays a sound naming none.
    pub default_name: Option<String>,
    pub settings: SoundSettings,
    /// The settings file, and whether it's there yet.
    pub file: String,
    pub exists: bool,
    /// The VPX tables play with has the bass shaker's output (`HAPTIC_FROM`).
    pub haptic: bool,
}

/// The sound outputs, as the player sees them.
fn outputs() -> Result<Outputs, String> {
    let mut command = Command::new(crate::screens::player()?);
    command.args(["--sound-outputs", "--json"]).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the player is a console program; no console shows
        command.creation_flags(0x0800_0000);
    }
    let out = command.output().map_err(|e| format!("The player couldn't list the sound outputs: {e}"))?;
    if !out.status.success() {
        return Err("The player couldn't list the sound outputs.".into());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("The player's list of sound outputs couldn't be read: {e}"))
}

/// The settings as the file has them, each unset one as VPX's own default.
fn settings_of(entries: &[(String, String, String)]) -> SoundSettings {
    let get = |section: &str, name: &str| vpi_settings::get(entries, section, name).map(|v| vpi_settings::vpx_trim(v).to_string());
    let name = |key: &str| get("Player", key).filter(|v| !v.is_empty());
    let int = |section: &str, key: &str, min: i32, max: i32, default: i32| get(section, key).and_then(|v| vpi_settings::vpx_int(&v)).filter(|v| (min..=max).contains(v)).unwrap_or(default);
    SoundSettings {
        playfield_output: name("SoundDevice"),
        backglass_output: name("SoundDeviceBG"),
        layout: int("Player", "Sound3D", 0, 5, 0),
        playfield_volume: int("Player", "SoundVolume", 0, 100, 100),
        backglass_volume: int("Player", "MusicVolume", 0, 100, 100),
        pup_volume: get("Plugin.PUP", "MainVol").and_then(|v| vpi_settings::vpx_float(&v)).filter(|v| (0.0..=1.0).contains(v)).map_or(100, |v| (v * 100.0).round() as i32),
        altsound: get("Plugin.AltSound", "Enable").as_deref().and_then(vpi_settings::vpx_bool).unwrap_or(true),
        achievement_sound: get("VPI", "AchievementSound").as_deref().and_then(vpi_settings::vpx_bool).unwrap_or(true),
        achievement_volume: get("VPI", "AchievementGain").and_then(|v| vpi_settings::vpx_float(&v)).filter(|v| (0.0..=2.0).contains(v)).map_or(100, |v| (v * 100.0).round() as i32),
        haptic_output: name("SoundDeviceHaptic"),
        haptic_layout: int("Player", "Sound3DHaptic", 0, 6, 0),
        haptic_volume: int("Player", "HapticVolume", 0, 100, 100),
    }
}

/// Whether the VPX tables play with has the bass shaker's output.
fn haptic_played() -> bool {
    let Some(folders) = vpi_settings::library_folders_now() else { return false };
    let layout = ::library::plan::Layout::from_folders(&folders);
    ::library::vpx::to_play(&layout).is_some_and(|v| v.version >= [10, 8, 1, HAPTIC_FROM])
}

pub fn load() -> Result<SoundView, String> {
    let found = outputs()?;
    let path = vpi_settings::vpx_settings_path().ok_or("no APPDATA folder")?;
    Ok(SoundView {
        outputs: found.outputs,
        default_channels: found.default_channels,
        default_name: found.default_name,
        settings: settings_of(&vpi_settings::read_all()),
        file: path.display().to_string(),
        exists: path.exists(),
        haptic: haptic_played(),
    })
}

/// What saving `after` changes, the file's settings before being `before`: only what differs, an
/// output back to Windows' default removed, every other setting written as VPX writes it.
fn changes(before: &SoundSettings, after: &SoundSettings) -> Vec<Change> {
    let mut out = Vec::new();
    let device = |key: &str, was: &Option<String>, now: &Option<String>, out: &mut Vec<Change>| {
        if was != now {
            out.push(match now {
                Some(name) => Change::set("Player", key, name),
                None => Change::remove("Player", key),
            });
        }
    };
    device("SoundDevice", &before.playfield_output, &after.playfield_output, &mut out);
    device("SoundDeviceBG", &before.backglass_output, &after.backglass_output, &mut out);
    device("SoundDeviceHaptic", &before.haptic_output, &after.haptic_output, &mut out);
    if before.haptic_layout != after.haptic_layout {
        out.push(Change::set("Player", "Sound3DHaptic", after.haptic_layout.clamp(0, 6)));
    }
    if before.haptic_volume != after.haptic_volume {
        out.push(Change::set("Player", "HapticVolume", after.haptic_volume.clamp(0, 100)));
    }
    if before.layout != after.layout {
        out.push(Change::set("Player", "Sound3D", after.layout.clamp(0, 5)));
    }
    if before.playfield_volume != after.playfield_volume {
        out.push(Change::set("Player", "SoundVolume", after.playfield_volume.clamp(0, 100)));
    }
    if before.backglass_volume != after.backglass_volume {
        out.push(Change::set("Player", "MusicVolume", after.backglass_volume.clamp(0, 100)));
    }
    if before.pup_volume != after.pup_volume {
        out.push(Change::set("Plugin.PUP", "MainVol", format!("{:.2}", f64::from(after.pup_volume.clamp(0, 100)) / 100.0)));
    }
    if before.altsound != after.altsound {
        out.push(Change::set("Plugin.AltSound", "Enable", i32::from(after.altsound)));
    }
    if before.achievement_sound != after.achievement_sound {
        out.push(Change::set("VPI", "AchievementSound", i32::from(after.achievement_sound)));
    }
    if before.achievement_volume != after.achievement_volume {
        // as the pause menu writes the level
        out.push(Change::set("VPI", "AchievementGain", format!("{:.6}", f64::from(after.achievement_volume.clamp(0, 200)) / 100.0)));
    }
    out
}

/// The settings saved into the player's settings file in place, only what changed.
pub fn save(settings: &SoundSettings) -> Result<(), String> {
    let was = settings_of(&vpi_settings::read_all());
    vpi_settings::save(&changes(&was, settings))
}

/// Windows' sound outputs as DirectSound lists them in `program`'s own process, by name, as VPX
/// 10.8.0 numbers its outputs (`library::settings_1080`): DirectSound lists Windows' default
/// ("Primary Sound Driver") first, then the process's own default output, then the rest in
/// Windows' order — and a program Windows keeps an output of its own for (an app's output in its
/// sound settings, as `route_older` gives VPX's) has that one as its default, so its numbers
/// aren't the app's. The app's own list where Windows' order can't be read.
pub fn direct_sound_outputs_for(program: &std::path::Path) -> Vec<String> {
    let here = direct_sound_devices();
    let order = endpoint_order();
    if here.len() < 2 || order.is_empty() {
        return here.into_iter().map(|(name, _)| name).collect();
    }
    let default = own_output_of(program).or_else(|| here[1].1.clone());
    ordered(&here, &order, default.as_deref())
}

/// `devices` (DirectSound's list here: Windows' default first, then each output by its name and
/// GUID) in the order a process whose own default is `default` lists them: the first, then
/// `default`, then the rest in Windows' `order`, any it misses last.
fn ordered(devices: &[(String, Option<String>)], order: &[String], default: Option<&str>) -> Vec<String> {
    let mut out = vec![devices[0].0.clone()];
    let rest = &devices[1..];
    let mut listed: Vec<&str> = Vec::new();
    let wanted = default.into_iter().chain(order.iter().map(String::as_str));
    for guid in wanted.chain(rest.iter().filter_map(|(_, g)| g.as_deref())) {
        if listed.contains(&guid) {
            continue;
        }
        if let Some((name, _)) = rest.iter().find(|(_, g)| g.as_deref() == Some(guid)) {
            out.push(name.clone());
            listed.push(guid);
        }
    }
    out
}

/// The endpoint's GUID in an output's ID as Windows writes it (`{0.0.0.00000000}.{89665076-…}`,
/// in full `\\?\SWD#MMDEVAPI#{0.0.0.00000000}.{…}#{…}`), in lower case, without its braces.
fn guid_in(id: &str) -> Option<String> {
    let start = id.find("}.{")? + 3;
    let len = id[start..].find('}')?;
    Some(id[start..start + len].to_ascii_lowercase())
}

/// Windows' outputs that are on, by their endpoints' GUIDs, in Windows' own order: the order
/// DirectSound lists them in after a process's own default.
#[cfg(windows)]
fn endpoint_order() -> Vec<String> {
    use windows::Win32::Media::Audio::{DEVICE_STATE_ACTIVE, IMMDeviceEnumerator, MMDeviceEnumerator, eRender};
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize};
    // SAFETY: COM started for this thread for these calls and ended after, where it was started
    // here; each ID Windows hands is freed once read
    unsafe {
        let started = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let read = || -> windows::core::Result<Vec<String>> {
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let list = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let mut out = Vec::new();
            for i in 0..list.GetCount()? {
                let id = list.Item(i)?.GetId()?;
                let text = id.to_string().unwrap_or_default();
                CoTaskMemFree(Some(id.0 as *const std::ffi::c_void));
                out.extend(guid_in(&text));
            }
            Ok(out)
        };
        let out = read().unwrap_or_default();
        if started {
            CoUninitialize();
        }
        out
    }
}

#[cfg(not(windows))]
fn endpoint_order() -> Vec<String> {
    Vec::new()
}

/// The output Windows keeps for `program` as its own — an app's output in its sound settings, as
/// `route_older` gives VPX's — by its endpoint's GUID: Windows' record of it in the user's registry
/// (`Multimedia\Audio\DefaultEndpoint`, a key for each program, its default value the program's
/// path in Windows' device form, `\Device\HarddiskVolume3\…`, then its output for each role). None
/// where it keeps none.
#[cfg(windows)]
fn own_output_of(program: &std::path::Path) -> Option<String> {
    use std::ffi::c_void;
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(hkey: isize, sub_key: *const u16, options: u32, access: u32, out: *mut isize) -> i32;
        fn RegEnumKeyExW(hkey: isize, index: u32, name: *mut u16, len: *mut u32, reserved: *mut u32, class: *mut u16, class_len: *mut u32, time: *mut c_void) -> i32;
        fn RegGetValueW(hkey: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
        fn RegCloseKey(hkey: isize) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn QueryDosDeviceW(name: *const u16, out: *mut u16, len: u32) -> u32;
    }
    const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
    const KEY_READ: u32 = 0x20019;
    const RRF_RT_REG_SZ: u32 = 0x2;
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    // the program's path in Windows' device form: its drive's device, then the rest
    let (drive, rest) = program.to_str()?.split_at_checked(2)?;
    if !drive.ends_with(':') {
        return None;
    }
    let mut device = vec![0u16; 512];
    // SAFETY: a NUL-ended drive name, a buffer of its length
    if unsafe { QueryDosDeviceW(wide(drive).as_ptr(), device.as_mut_ptr(), device.len() as u32) } == 0 {
        return None;
    }
    let end = device.iter().position(|&c| c == 0).unwrap_or(0);
    let path = format!("{}{rest}", String::from_utf16_lossy(&device[..end]));
    let read = |key: isize, name: Option<&str>| -> Option<String> {
        let name = name.map(wide);
        let mut buf = vec![0u16; 1024];
        let mut bytes = (buf.len() * 2) as u32;
        let value = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
        // SAFETY: an open key, a NUL-ended value's name or none (its default value), a buffer of
        // `bytes` bytes
        (unsafe { RegGetValueW(key, std::ptr::null(), value, RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut bytes) } == 0).then(|| {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            String::from_utf16_lossy(&buf[..len])
        })
    };
    let store = wide(r"Software\Microsoft\Multimedia\Audio\DefaultEndpoint");
    let mut list = 0isize;
    // SAFETY: a key of Windows' own, read only, a handle to fill
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, store.as_ptr(), 0, KEY_READ, &mut list) } != 0 {
        return None;
    }
    let mut found = None;
    for i in 0.. {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        // SAFETY: the open key, a buffer for a subkey's name
        if unsafe { RegEnumKeyExW(list, i, name.as_mut_ptr(), &mut len, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) } != 0 {
            break;
        }
        let sub: Vec<u16> = name[..len as usize].iter().copied().chain(std::iter::once(0)).collect();
        let mut key = 0isize;
        // SAFETY: a subkey of the open key, read only
        if unsafe { RegOpenKeyExW(list, sub.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            continue;
        }
        if read(key, None).is_some_and(|p| p.eq_ignore_ascii_case(&path)) {
            // its output for multimedia, else for the console: the roles Windows' sound settings set alike
            found = read(key, Some("001_000")).or_else(|| read(key, Some("000_000"))).and_then(|id| guid_in(&id));
        }
        // SAFETY: the key opened above
        unsafe { RegCloseKey(key) };
        if found.is_some() {
            break;
        }
    }
    // SAFETY: the key opened above
    unsafe { RegCloseKey(list) };
    found
}

#[cfg(not(windows))]
fn own_output_of(_program: &std::path::Path) -> Option<String> {
    None
}

/// A GUID as Windows writes it, in lower case, without its braces.
#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl Guid {
    fn text(&self) -> String {
        let d = self.data4;
        format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", self.data1, self.data2, self.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7])
    }
}

/// Windows' sound outputs as DirectSound lists them, in its order, each by name and by its GUID —
/// the endpoint's own, which Windows' sound settings know it by — none for the first, Windows'
/// default.
#[cfg(windows)]
fn direct_sound_devices() -> Vec<(String, Option<String>)> {
    use std::ffi::c_void;
    type Each = unsafe extern "system" fn(*mut c_void, *const u16, *const u16, *mut c_void) -> i32;
    #[link(name = "dsound")]
    unsafe extern "system" {
        fn DirectSoundEnumerateW(each: Each, context: *mut c_void) -> i32;
    }
    unsafe extern "system" fn each(guid: *mut c_void, description: *const u16, _module: *const u16, context: *mut c_void) -> i32 {
        // SAFETY: `context` is the list handed to DirectSoundEnumerateW below, alive for its call
        let out = unsafe { &mut *context.cast::<Vec<(String, Option<String>)>>() };
        let mut name = Vec::new();
        if !description.is_null() {
            // SAFETY: DirectSound hands a NUL-ended name
            unsafe {
                let mut p = description;
                while *p != 0 {
                    name.push(*p);
                    p = p.add(1);
                }
            }
        }
        // SAFETY: DirectSound hands its device's GUID, or none for the default output
        let id = (!guid.is_null()).then(|| unsafe { *guid.cast::<Guid>() }.text());
        out.push((String::from_utf16_lossy(&name), id));
        1
    }
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    // SAFETY: a callback above and the list it fills
    unsafe { DirectSoundEnumerateW(each, (&raw mut out).cast()) };
    out
}

#[cfg(not(windows))]
fn direct_sound_devices() -> Vec<(String, Option<String>)> {
    Vec::new()
}

/// Windows' default output now, by the name DirectSound lists it under (DirectSound's own
/// `GetDeviceID` of its default playback device): none where it can't be told.
#[cfg(windows)]
pub fn default_output() -> Option<String> {
    #[link(name = "dsound")]
    unsafe extern "system" {
        fn GetDeviceID(source: *const Guid, out: *mut Guid) -> i32;
    }
    // DSDEVID_DefaultPlayback
    const DEFAULT_PLAYBACK: Guid = Guid { data1: 0xdef0_0000, data2: 0x9c6d, data3: 0x47ed, data4: [0xaa, 0xf1, 0x4d, 0xda, 0x8f, 0x2b, 0x5c, 0x03] };
    let mut guid = Guid { data1: 0, data2: 0, data3: 0, data4: [0; 8] };
    // SAFETY: DirectSound's own GUID for its default output, and one to fill
    if unsafe { GetDeviceID(&DEFAULT_PLAYBACK, &mut guid) } < 0 {
        return None;
    }
    let id = guid.text();
    direct_sound_devices().into_iter().skip(1).find(|(_, g)| g.as_deref() == Some(id.as_str())).map(|(name, _)| name)
}

#[cfg(not(windows))]
pub fn default_output() -> Option<String> {
    None
}

/// The output Settings › Sound gives the playfield (`SoundDevice`) or the backglass
/// (`SoundDeviceBG`), by its name and its endpoint's GUID, where it names one that's here now.
fn output_of(setting: &str) -> Option<(String, String)> {
    let main = vpi_settings::read_all();
    let wanted = vpi_settings::get(&main, "Player", setting).map(str::trim).filter(|w| !w.is_empty())?.to_string();
    let id = direct_sound_devices().into_iter().skip(1).find(|(name, _)| name.trim() == wanted).and_then(|(_, id)| id)?;
    Some((wanted, id))
}

/// Future Pinball's program, which plays a table's sounds and its music in one mix.
const FUTURE_PINBALL: [&str; 1] = ["future pinball.exe"];

/// Programs given an output of their own: one process by its ID (named for the log), and any
/// process of the programs named, as each starts.
struct Route {
    wanted: String,
    outputs: AppOutputs,
    process: Option<(u32, &'static str)>,
    programs: &'static [&'static str],
}

/// A route to the output Settings › Sound gives `setting`'s part, where it names one that's here.
fn route_to(setting: &str, process: Option<(u32, &'static str)>, programs: &'static [&'static str]) -> Option<Route> {
    let (wanted, id) = output_of(setting)?;
    match AppOutputs::to(&id) {
        Ok(outputs) => Some(Route { wanted, outputs, process, programs }),
        Err(e) => {
            eprintln!("The sound output {wanted} wasn't set: {e}");
            None
        }
    }
}

/// Each route's processes given its output, each asked again a tenth of a second apart (a second
/// after the first half minute) until Windows takes it — it takes one only from a process already
/// playing a sound — or the game has ended (`ended`).
fn route(ended: &std::sync::atomic::AtomicBool, routes: &[Route]) {
    let started = std::time::Instant::now();
    let mut done: Vec<u32> = Vec::new();
    while !ended.load(std::sync::atomic::Ordering::Relaxed) {
        let running = ::library::processes::running_with_ids();
        for r in routes {
            let named = running.iter().filter(|(_, n)| r.programs.contains(&n.to_ascii_lowercase().as_str())).map(|(p, n)| (*p, n.clone()));
            for (p, name) in r.process.map(|(p, n)| (p, n.to_string())).into_iter().chain(named) {
                if done.contains(&p) {
                    continue;
                }
                match r.outputs.give(p) {
                    Ok(()) => {
                        done.push(p);
                        eprintln!("{name}'s sound on {}, its own output in Windows' sound settings", r.wanted);
                    }
                    Err(NO_SOUND_YET) => {}
                    Err(hr) => {
                        done.push(p);
                        eprintln!("{name}'s sound output wasn't set: Windows refused it ({hr:#010x})");
                    }
                }
            }
        }
        let pause = if started.elapsed() < std::time::Duration::from_secs(30) { 100 } else { 1000 };
        std::thread::sleep(std::time::Duration::from_millis(pause));
    }
}

/// A Future Pinball game heard on the outputs Settings › Sound sets: Future Pinball's own sound —
/// its table's and its music, one mix — on the playfield's, and PinUP's programs (a PinEvent
/// table's pack) on the backglass's, each given its output as its own in Windows' sound settings,
/// as `route_older` gives VPX's: Future Pinball plays on Windows' default output, which no setting
/// of its own changes. Nothing for a part the settings leave on Windows' default.
pub fn route_future_pinball(ended: &std::sync::atomic::AtomicBool) {
    let routes: Vec<Route> = [route_to("SoundDevice", None, &FUTURE_PINBALL), route_to("SoundDeviceBG", None, &PINUP_PLAYERS)].into_iter().flatten().collect();
    if !routes.is_empty() {
        route(ended, &routes);
    }
}

/// PinUP's programs that play a PinUP pack where VPX's own PinUP add-on doesn't (VPX 10.8.0, which
/// takes no add-on, or a newer VPX with it off), each a program with its own output: the one a
/// table's script asks for (`PinUpPlayer.PinDisplay`, `PinUpPlayer.exe`) and the display programs
/// it shows a pack's videos with.
const PINUP_PLAYERS: [&str; 4] = ["pinupplayer.exe", "pinupdisplay.exe", "pinupdisplayx.exe", "pinupdisplayx2.exe"];

/// Windows' answer to an app's output for a process playing no sound yet (`E_INVALIDARG`): it
/// takes one only from a process with sound.
const NO_SOUND_YET: i32 = 0x8007_0057_u32 as i32;

/// A game played with the classic programs heard on the output Settings › Sound gives the
/// backglass: VPinMAME and Windows Media Player, in VPX's process (`pid`), and PinUP's programs
/// play on Windows' default output, which VPX's settings don't reach (VPX 10.8.0's always), so each
/// program is given that output as its own in Windows' sound settings (an app's output, which
/// Windows keeps for the program, so later games play there from their first sound). Windows takes
/// one only from a process already playing a sound, so each is asked again, a tenth of a second
/// apart (a second after the first half minute), until it takes it or the game has ended
/// (`ended`). Nothing where the settings name Windows' default, or an output not there now.
pub fn route_older(pid: u32, ended: &std::sync::atomic::AtomicBool) {
    if let Some(r) = route_to("SoundDeviceBG", Some((pid, "VPX")), &PINUP_PLAYERS) {
        route(ended, &[r]);
    }
}

/// Windows' own output for a program (`pid`, a process of it running), as its sound settings set
/// one (Settings › System › Sound › Volume mixer): the output by its endpoint's GUID, or Windows'
/// default (`None`), for both of its roles. Through Windows' `AudioPolicyConfig`, which its sound
/// settings use and which has no documented interface: its factory's interface as Windows 10's
/// 21H2 changed it, else as before.
#[cfg(windows)]
mod winrt {
    use std::ffi::c_void;

    #[link(name = "combase", kind = "raw-dylib")]
    unsafe extern "system" {
        fn RoInitialize(kind: u32) -> i32;
        fn RoGetActivationFactory(class: *mut c_void, iid: *const super::Guid, factory: *mut *mut c_void) -> i32;
        fn WindowsCreateString(text: *const u16, length: u32, string: *mut *mut c_void) -> i32;
        pub fn WindowsDeleteString(string: *mut c_void) -> i32;
    }

    /// A Windows Runtime string, to give back with `WindowsDeleteString`.
    pub fn hstring(text: &str) -> Result<*mut c_void, String> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut s = std::ptr::null_mut();
        // SAFETY: a string of `wide.len()` units, a handle to fill
        let hr = unsafe { WindowsCreateString(wide.as_ptr(), wide.len() as u32, &mut s) };
        if hr < 0 { Err(format!("a string wasn't made ({hr:#010x})")) } else { Ok(s) }
    }

    /// The factory of Windows' `AudioPolicyConfig`: its interface as Windows 10's 21H2 changed it,
    /// else as before. To give back with its `Release`.
    pub fn policy_factory() -> Result<*mut c_void, String> {
        // SAFETY: the thread's Windows Runtime, many threads at once (RO_INIT_MULTITHREADED); already so is fine
        unsafe { RoInitialize(1) };
        let class = hstring("Windows.Media.Internal.AudioPolicyConfig")?;
        let iids = [
            super::Guid { data1: 0xab3d4648, data2: 0xe242, data3: 0x459f, data4: [0xb0, 0x2f, 0x54, 0x1c, 0x70, 0x30, 0x63, 0x24] },
            super::Guid { data1: 0x2a59116d, data2: 0x6c4f, data3: 0x45e0, data4: [0xa7, 0x4f, 0x70, 0x7e, 0x3f, 0xef, 0x92, 0x58] },
        ];
        let mut factory: *mut c_void = std::ptr::null_mut();
        for iid in &iids {
            // SAFETY: the class's name, one of its interfaces, a pointer to fill
            if unsafe { RoGetActivationFactory(class, iid, &mut factory) } >= 0 && !factory.is_null() {
                break;
            }
            factory = std::ptr::null_mut();
        }
        // SAFETY: the string made above
        unsafe { WindowsDeleteString(class) };
        if factory.is_null() { Err("Windows' sound settings weren't reached".into()) } else { Ok(factory) }
    }

    /// A COM object given back (IUnknown's `Release`, its table's third).
    pub fn release(object: *mut c_void) {
        type Release = unsafe extern "system" fn(*mut c_void) -> u32;
        // SAFETY: a COM object's table of methods
        unsafe {
            let table = *object.cast::<*const usize>();
            std::mem::transmute::<usize, Release>(*table.add(2))(object);
        }
    }
}

/// One output given to programs as their own in Windows' sound settings, through its
/// `AudioPolicyConfig`; both given back when dropped.
#[cfg(windows)]
struct AppOutputs {
    factory: *mut std::ffi::c_void,
    device: *mut std::ffi::c_void,
}

#[cfg(windows)]
impl AppOutputs {
    /// For the output whose endpoint has this GUID.
    fn to(endpoint: &str) -> Result<Self, String> {
        let factory = winrt::policy_factory()?;
        match winrt::hstring(&format!("\\\\?\\SWD#MMDEVAPI#{{0.0.0.00000000}}.{{{endpoint}}}#{{e6327cad-dcec-4949-ae8a-991e976a79d2}}")) {
            Ok(device) => Ok(Self { factory, device }),
            Err(e) => {
                winrt::release(factory);
                Err(e)
            }
        }
    }

    /// The output given to a process's program (`pid`), for both of its roles: Windows' answer
    /// where it refused it (`NO_SOUND_YET` for a process playing nothing yet).
    fn give(&self, pid: u32) -> Result<(), i32> {
        // its methods after IInspectable's six and nineteen of its own: SetPersistedDefaultAudioEndpoint
        // the 26th
        type Set = unsafe extern "system" fn(*mut std::ffi::c_void, u32, i32, i32, *mut std::ffi::c_void) -> i32;
        // SAFETY: a COM object's table of methods, as the interface asked for lays it out
        let set = unsafe { std::mem::transmute::<usize, Set>(*(*self.factory.cast::<*const usize>()).add(25)) };
        // the playing role (eMultimedia), then the console one (eConsole), output (eRender)
        for role in [1, 0] {
            // SAFETY: the factory, a process, an output
            let hr = unsafe { set(self.factory, pid, 0, role, self.device) };
            if hr < 0 {
                return Err(hr);
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for AppOutputs {
    fn drop(&mut self) {
        // SAFETY: the string made in `to`
        unsafe { winrt::WindowsDeleteString(self.device) };
        winrt::release(self.factory);
    }
}

#[cfg(not(windows))]
struct AppOutputs;

#[cfg(not(windows))]
impl AppOutputs {
    fn to(_endpoint: &str) -> Result<Self, String> {
        Err("Windows' sound settings are Windows' own".into())
    }

    fn give(&self, _pid: u32) -> Result<(), i32> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guid_written_as_windows_writes_it() {
        let g = Guid { data1: 0xab3d4648, data2: 0xe242, data3: 0x459f, data4: [0xb0, 0x2f, 0x54, 0x1c, 0x70, 0x30, 0x63, 0x24] };
        assert_eq!(g.text(), "ab3d4648-e242-459f-b02f-541c70306324");
    }

    /// This PC's outputs with their GUIDs, and whether Windows' sound settings are reached as the
    /// app gives VPX 10.8.0's program its output: `cargo test -p app app_outputs_on_this_pc --
    /// --ignored --nocapture`. Nothing set.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn app_outputs_on_this_pc() {
        for (i, (name, id)) in direct_sound_devices().iter().enumerate() {
            println!("{i}: {name} {}", id.as_deref().unwrap_or("(Windows' default)"));
        }
        let factory = winrt::policy_factory().expect("Windows' AudioPolicyConfig");
        winrt::release(factory);
        println!("Windows' sound settings reached");
    }

    /// This PC's outputs as VPX 10.8.0 numbers them, and where VPX 10.8.1's settings' would go:
    /// `cargo test -p app outputs_for_10_8_0_on_this_pc -- --ignored --nocapture`. Nothing written.
    #[test]
    #[ignore]
    fn outputs_for_10_8_0_on_this_pc() {
        let main = vpi_settings::read_all();
        let folders = vpi_settings::library_folders(&main).expect("the library's folders in the settings");
        let program = folders.vpx.clone().or_else(|| folders.tables.parent().map(std::path::Path::to_path_buf)).expect("VPX's folder").join("VPinballX64.exe");
        println!("its own output in Windows' settings: {:?}", own_output_of(&program));
        let outputs = direct_sound_outputs_for(&program);
        for (i, o) in outputs.iter().enumerate() {
            println!("{i}: {o}");
        }
        let newer = vpi_settings::vpx_settings_path_for([10, 8, 1, 0], None).and_then(|p| std::fs::read(p).ok()).map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b))).unwrap_or_default();
        println!("Windows' default: {:?}", default_output());
        for c in ::library::settings_1080::changes(&newer, &[], &outputs, default_output().as_deref()) {
            println!("{} = {:?}", c.name, c.value);
        }
    }

    #[test]
    fn outputs_numbered_as_a_process_with_its_own_default_lists_them() {
        let id = |g: &str| Some(g.to_string());
        // as DirectSound lists them in a process on Windows' default, System
        let here = [
            ("Primary Sound Driver".to_string(), None),
            ("System (USB Mixer)".to_string(), id("f113")),
            ("Music (USB Mixer)".to_string(), id("236a")),
            ("Chat (USB Mixer)".to_string(), id("36e4")),
            ("Game (USB Mixer)".to_string(), id("8966")),
            ("Main Screen (NVIDIA)".to_string(), id("a776")),
        ];
        let order: Vec<String> = ["236a", "36e4", "8966", "a776", "f113"].iter().map(|s| s.to_string()).collect();
        let names = |list: Vec<String>| list.iter().map(|n| n.split(' ').next().unwrap_or_default().to_string()).collect::<Vec<_>>();
        // a program Windows keeps Game for: Game second, the rest in Windows' order
        assert_eq!(names(ordered(&here, &order, Some("8966"))), ["Primary", "Game", "Music", "Chat", "Main", "System"]);
        // one on Windows' default: as here
        assert_eq!(names(ordered(&here, &order, Some("f113"))), ["Primary", "System", "Music", "Chat", "Game", "Main"]);
        // an output Windows' order misses still listed, last
        assert_eq!(names(ordered(&here, &order[..3], Some("8966"))), ["Primary", "Game", "Music", "Chat", "System", "Main"]);
    }

    #[test]
    fn an_outputs_guid_read_from_its_id() {
        assert_eq!(guid_in(r"\\?\SWD#MMDEVAPI#{0.0.0.00000000}.{89665076-EFDE-4f27-88af-8f8ecea9fe98}#{e6327cad-dcec-4949-ae8a-991e976a79d2}").as_deref(), Some("89665076-efde-4f27-88af-8f8ecea9fe98"));
        assert_eq!(guid_in("{0.0.0.00000000}.{a7764eb2-ef40-43a6-b30b-1764e5e85c73}").as_deref(), Some("a7764eb2-ef40-43a6-b30b-1764e5e85c73"));
        assert_eq!(guid_in("Speakers"), None);
    }

    #[test]
    fn only_what_changed_is_written() {
        let file = "[Player]\nSoundVolume = 80\n; my own note\nSound3D = 5\n\n[Plugin.VNI]\nPacKey = secret\n";
        let before = settings_of(&vpi_settings::parse_ini(file));
        assert_eq!((before.playfield_volume, before.backglass_volume, before.layout, before.pup_volume, before.altsound), (80, 100, 5, 100, true));
        assert!(changes(&before, &before).is_empty());
        let mut after = before.clone();
        after.backglass_volume = 60;
        after.playfield_output = Some("Speakers (Realtek(R) Audio)".into());
        after.pup_volume = 50;
        after.altsound = false;
        let out = vpi_settings::edit(file, &changes(&before, &after));
        let e = vpi_settings::parse_ini(&out);
        assert_eq!(settings_of(&e), after);
        // the rest of the file as it was
        assert!(out.contains("; my own note") && out.contains("PacKey = secret") && out.contains("SoundVolume = 80"));
        // an output back to Windows' default: the setting gone
        let mut back = after.clone();
        back.playfield_output = None;
        let out2 = vpi_settings::edit(&out, &changes(&after, &back));
        assert!(!out2.contains("SoundDevice ="));
    }
}
