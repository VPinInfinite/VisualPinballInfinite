// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its WMP plugin
// (plugins/wmp: WMPPlugin.cpp's script classes WMP_Core, WMP_Controls and WMP_Settings, WMPCore.cpp,
// WMPControls.cpp, WMPSettings.cpp, and common.cpp's find_case_insensitive_file_path).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! `CreateObject("WMPlayer.OCX")`: VPX 5436's stand-in for Windows Media
//! Player's object, which some tables play their music through (the plugin
//! names Blood Machines and American Graffiti). As VPX's: setting `URL`
//! loads the file (found from the table's folder, any letter case) and
//! nothing plays until `Controls.Play`; `Settings.Volume` (0 to 100, 50 to
//! start) and `Mute` set its loudness; `Controls` can pause, stop and move
//! it. The members VPX leaves unanswered answer as VPX's do, quietly, with
//! a default. Its sound is mixed by `audio::MediaStream`.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use super::com::*;
use super::value::{Args, Ret};
use super::winevbs;
use crate::audio::MediaStream;

type Res = Result<Ret, HRESULT>;

/// Windows Media Player's play states (`WMPPlayState`).
const PLAY_STATE_UNDEFINED: i32 = 0;
const PLAY_STATE_STOPPED: i32 = 1;
const PLAY_STATE_PAUSED: i32 = 2;
const PLAY_STATE_PLAYING: i32 = 3;

/// The player behind the objects (`WMPCore`): its file, and its settings.
struct Core {
    url: String,
    stream: Option<Arc<MediaStream>>,
    auto_start: bool,
    mute: bool,
    volume: i32,
}

impl Core {
    /// VPX's `UpdateVolume`: 0 to 100, nothing when muted.
    fn loudness(&self) -> f32 {
        if self.mute { 0.0 } else { self.volume.clamp(0, 100) as f32 / 100.0 }
    }

    /// VPX's `Close` and `UnloadFile`.
    fn unload(&mut self) {
        if let Some(s) = self.stream.take() {
            s.close();
        }
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.unload();
    }
}

type CoreRef = Rc<RefCell<Core>>;

/// What one of the objects stands for.
#[derive(Clone)]
enum Handle {
    Core(CoreRef),
    Controls(CoreRef),
    Settings(CoreRef),
}

const CORE: &[&str] = &["Close", "URL", "OpenState", "PlayState", "Controls", "Settings", "VersionInfo", "IsOnline", "Status"];
const CONTROLS: &[&str] = &["Play", "Stop", "Pause", "FastForward", "FastReverse", "CurrentPosition", "CurrentPositionString", "Next", "Previous", "GetIsAvailable"];
const SETTINGS: &[&str] = &[
    "AutoStart", "Mute", "Volume", "Rate", "Balance", "PlayCount", "BaseURL", "DefaultFrame", "InvokeURLs", "EnableErrorDialogs", "GetIsAvailable", "GetMode", "SetMode",
];

fn members(h: &Handle) -> &'static [&'static str] {
    match h {
        Handle::Core(_) => CORE,
        Handle::Controls(_) => CONTROLS,
        Handle::Settings(_) => SETTINGS,
    }
}

#[repr(C)]
struct WmpObj {
    vtbl: &'static IDispatchVtbl,
    refs: Cell<u32>,
    handle: Handle,
}

static VTBL: IDispatchVtbl = IDispatchVtbl {
    base: IUnknownVtbl { QueryInterface: query_interface, AddRef: add_ref, Release: release },
    GetTypeInfoCount: get_type_info_count,
    GetTypeInfo: get_type_info,
    GetIDsOfNames: get_ids_of_names,
    Invoke: invoke,
};

fn new_obj(handle: Handle) -> Ret {
    Ret::NewObject(Box::into_raw(Box::new(WmpObj { vtbl: &VTBL, refs: Cell::new(1), handle })).cast())
}

/// `CreateObject("WMPlayer.OCX")`: a player of its own.
pub fn create() -> Res {
    let core = Core { url: String::new(), stream: None, auto_start: false, mute: false, volume: 50 };
    Ok(new_obj(Handle::Core(Rc::new(RefCell::new(core)))))
}

/// A member VPX's stand-in doesn't answer: named once, its default given.
fn not_answered(name: &str) {
    super::log::once(&format!("WMPlayer.OCX: {name} isn't answered (as in VPX 5436)"));
}

// ---------------------------------------------------------------------------------------------
// COM

unsafe extern "system" fn query_interface(this: *mut c_void, riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    unsafe {
        if out.is_null() {
            return E_POINTER;
        }
        if *riid == IID_IUNKNOWN || *riid == IID_IDISPATCH {
            add_ref(this);
            *out = this;
            S_OK
        } else {
            *out = std::ptr::null_mut();
            E_NOINTERFACE
        }
    }
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    let o = unsafe { &*(this as *const WmpObj) };
    o.refs.set(o.refs.get() + 1);
    o.refs.get()
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let o = unsafe { &*(this as *const WmpObj) };
    let n = o.refs.get().saturating_sub(1);
    o.refs.set(n);
    if n == 0 {
        // SAFETY: the last reference is gone; it was made by `new_obj`
        unsafe { drop(Box::from_raw(this as *mut WmpObj)) };
    }
    n
}

unsafe extern "system" fn get_type_info_count(_this: *mut c_void, count: *mut u32) -> HRESULT {
    if !count.is_null() {
        unsafe { *count = 0 };
    }
    S_OK
}

unsafe extern "system" fn get_type_info(_this: *mut c_void, _index: u32, _lcid: u32, info: *mut *mut c_void) -> HRESULT {
    if !info.is_null() {
        unsafe { *info = std::ptr::null_mut() };
    }
    E_NOTIMPL
}

unsafe extern "system" fn get_ids_of_names(this: *mut c_void, _riid: *const GUID, names: *const *const u16, count: u32, _lcid: u32, ids: *mut i32) -> HRESULT {
    unsafe {
        let o = &*(this as *const WmpObj);
        for i in 0..count as usize {
            *ids.add(i) = DISPID_UNKNOWN;
        }
        if count == 0 {
            return S_OK;
        }
        let name = from_wide(*names);
        match members(&o.handle).iter().position(|m| m.eq_ignore_ascii_case(&name)) {
            Some(i) => {
                *ids = i as i32 + 1;
                if count > 1 { DISP_E_UNKNOWNNAME } else { S_OK }
            }
            None => DISP_E_UNKNOWNNAME,
        }
    }
}

#[allow(clippy::too_many_arguments)] // IDispatch::Invoke's signature
unsafe extern "system" fn invoke(
    this: *mut c_void,
    dispid: i32,
    _riid: *const GUID,
    _lcid: u32,
    flags: u16,
    params: *mut DISPPARAMS,
    result: *mut VARIANT,
    _excep: *mut EXCEPINFO,
    _arg_err: *mut u32,
) -> HRESULT {
    let Ok(lib) = winevbs::get() else { return E_FAIL };
    unsafe {
        let o = &*(this as *const WmpObj);
        let Some(&name) = usize::try_from(dispid - 1).ok().and_then(|i| members(&o.handle).get(i)) else { return DISP_E_MEMBERNOTFOUND };
        let empty = DISPPARAMS { rgvarg: std::ptr::null_mut(), rgdispidNamedArgs: std::ptr::null_mut(), cArgs: 0, cNamedArgs: 0 };
        let params = if params.is_null() { &empty } else { &*params };
        let args = Args::new(lib, params);
        let put = flags & (DISPATCH_PROPERTYPUT | DISPATCH_PROPERTYPUTREF) != 0;
        let handle = o.handle.clone();
        let call = std::panic::AssertUnwindSafe(|| match &handle {
            Handle::Core(c) => core_member(c, name, put, &args),
            Handle::Controls(c) => controls_member(c, name, put, &args),
            Handle::Settings(c) => settings_member(c, name, put, &args),
        });
        match std::panic::catch_unwind(call).unwrap_or(Err(E_FAIL)) {
            Ok(ret) => {
                super::value::write(lib, ret, result);
                S_OK
            }
            Err(hr) => hr,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The members

fn core_member(c: &CoreRef, name: &str, put: bool, a: &Args) -> Res {
    Ok(match (name, put) {
        ("Close", false) => {
            let mut core = c.borrow_mut();
            core.unload();
            core.url.clear();
            Ret::Empty
        }
        ("URL", false) => Ret::Str(c.borrow().url.clone()),
        ("URL", true) => {
            set_url(c, &a.value_string().unwrap_or_default());
            Ret::Empty
        }
        ("OpenState", false) => {
            not_answered("OpenState");
            Ret::Int(0)
        }
        ("PlayState", false) => Ret::Int(match &c.borrow().stream {
            None => PLAY_STATE_UNDEFINED,
            Some(s) if s.is_playing() => {
                if s.is_paused() {
                    PLAY_STATE_PAUSED
                } else {
                    PLAY_STATE_PLAYING
                }
            }
            Some(_) => PLAY_STATE_STOPPED,
        }),
        // a new object each time, as VPX's `GetControls` and `GetSettings` make
        ("Controls", false) => new_obj(Handle::Controls(c.clone())),
        ("Settings", false) => new_obj(Handle::Settings(c.clone())),
        ("VersionInfo", false) | ("Status", false) => {
            not_answered(name);
            Ret::Str(String::new())
        }
        ("IsOnline", false) => {
            not_answered(name);
            Ret::Bool(false)
        }
        _ => return Err(DISP_E_MEMBERNOTFOUND),
    })
}

/// VPX's `SetURL`: the file found from the table's folder in any letter case, loaded; nothing
/// plays until asked. Its `URL` reads back as the file found, empty when there's none.
fn set_url(c: &CoreRef, url: &str) {
    let mut core = c.borrow_mut();
    core.unload();
    let found = super::with(|_, w| {
        let path = find_file(w.table_dir.as_deref(), url)?;
        // the fence: the table's own folders, or the music folders `PlayMusic` reads
        let allowed = super::files::readable(&path) || super::api::music_folders(w).iter().any(|d| path.starts_with(d));
        if !allowed {
            super::log::once(&format!("the fence kept WMPlayer.OCX from reading {}", path.display()));
            return None;
        }
        Some(path)
    })
    .flatten();
    core.url = found.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let Some(path) = found else {
        super::log::once(&format!("WMPlayer.OCX: {url} not found"));
        return;
    };
    let stream = MediaStream::load(path, core.loudness());
    let _ = super::with(|_, w| w.sounds.play_media(&stream));
    core.stream = Some(stream);
}

/// A file named relative to the table's folder (VPX's current folder while it plays) or absolutely,
/// each part of its name in any letter case (`find_case_insensitive_file_path`).
fn find_file(table_dir: Option<&Path>, name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let given = PathBuf::from(name.replace('/', "\\"));
    let full = if given.is_absolute() { given } else { table_dir?.join(given) };
    let mut out = PathBuf::new();
    for part in full.components() {
        use std::path::Component;
        match part {
            Component::Prefix(_) | Component::RootDir => out.push(part.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(n) => {
                let exact = out.join(n);
                if exact.exists() {
                    out = exact;
                } else {
                    let wanted = n.to_string_lossy().to_lowercase();
                    let entry = std::fs::read_dir(&out).ok()?.filter_map(Result::ok).find(|e| e.file_name().to_string_lossy().to_lowercase() == wanted)?;
                    out = entry.path();
                }
            }
        }
    }
    out.is_file().then_some(out)
}

fn controls_member(c: &CoreRef, name: &str, put: bool, a: &Args) -> Res {
    let stream = c.borrow().stream.clone();
    Ok(match (name, put) {
        ("Play", false) => {
            if let Some(s) = stream {
                s.play();
            }
            Ret::Empty
        }
        ("Stop", false) => {
            if let Some(s) = stream {
                s.stop();
            }
            Ret::Empty
        }
        ("Pause", false) => {
            if let Some(s) = stream {
                s.pause();
            }
            Ret::Empty
        }
        ("CurrentPosition", false) => Ret::Double(stream.map_or(0.0, |s| s.position())),
        ("CurrentPosition", true) => {
            if let Some(s) = stream {
                s.set_position(a.value_f64().unwrap_or(0.0));
            }
            Ret::Empty
        }
        ("FastForward", false) | ("FastReverse", false) | ("Next", false) | ("Previous", false) => {
            not_answered(name);
            Ret::Empty
        }
        ("CurrentPositionString", false) => {
            not_answered(name);
            Ret::Str(String::new())
        }
        ("GetIsAvailable", false) => {
            not_answered(name);
            Ret::Bool(false)
        }
        _ => return Err(DISP_E_MEMBERNOTFOUND),
    })
}

fn settings_member(c: &CoreRef, name: &str, put: bool, a: &Args) -> Res {
    Ok(match (name, put) {
        ("AutoStart", false) => Ret::Bool(c.borrow().auto_start),
        ("AutoStart", true) => {
            // kept, never used, as in VPX: setting `URL` doesn't start the file
            c.borrow_mut().auto_start = a.value_bool().unwrap_or(false);
            Ret::Empty
        }
        ("Mute", false) => Ret::Bool(c.borrow().mute),
        ("Mute", true) => {
            c.borrow_mut().mute = a.value_bool().unwrap_or(false);
            apply_volume(c);
            Ret::Empty
        }
        ("Volume", false) => Ret::Int(c.borrow().volume),
        ("Volume", true) => {
            c.borrow_mut().volume = a.value_i32().unwrap_or(0);
            apply_volume(c);
            Ret::Empty
        }
        ("Rate", false) => {
            not_answered(name);
            Ret::Double(1.0)
        }
        ("Balance", false) => {
            not_answered(name);
            Ret::Int(0)
        }
        ("PlayCount", false) => {
            not_answered(name);
            Ret::Int(1)
        }
        ("BaseURL", false) | ("DefaultFrame", false) => {
            not_answered(name);
            Ret::Str(String::new())
        }
        ("InvokeURLs", false) | ("EnableErrorDialogs", false) | ("GetIsAvailable", false) | ("GetMode", false) => {
            not_answered(name);
            Ret::Bool(false)
        }
        ("Rate", true) | ("Balance", true) | ("PlayCount", true) | ("BaseURL", true) | ("DefaultFrame", true) | ("InvokeURLs", true) | ("EnableErrorDialogs", true) | ("SetMode", false) => {
            not_answered(name);
            Ret::Empty
        }
        _ => return Err(DISP_E_MEMBERNOTFOUND),
    })
}

fn apply_volume(c: &CoreRef) {
    let core = c.borrow();
    if let Some(s) = &core.stream {
        s.set_volume(core.loudness());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_found_in_any_letter_case() {
        let dir = std::env::temp_dir().join(format!("vpi-wmp-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Music")).unwrap();
        std::fs::write(dir.join("Music").join("Theme.MP3"), b"x").unwrap();
        // Windows finds a file in any case itself: the name comes back as given, as VPX's does
        let found = |name: &str| find_file(Some(&dir), name).map(|p| p.to_string_lossy().to_lowercase());
        let wanted = Some(dir.join("Music").join("Theme.MP3").to_string_lossy().to_lowercase());
        assert_eq!(found("music/theme.mp3"), wanted);
        assert_eq!(found(".\\Music\\..\\Music\\THEME.mp3"), wanted);
        assert_eq!(found("music/none.mp3"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
