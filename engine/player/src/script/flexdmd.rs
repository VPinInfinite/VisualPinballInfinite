// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): the members its script interfaces offer
// (FlexDMD/IFlexDMD.cs: IFlexDMD, IGroupActor, IFrameActor, IImageActor, IVideoActor, ILabelActor, IActionFactory,
// ICompositeAction, ITweenAction, IUltraDMD) and what each does (FlexDMD.cs, Actors/*, UltraDMD.cs). Copyright 2019
// Vincent Bousquet, licensed under the Apache License, Version 2.0 (engine/player/src/flexdmd/LICENSE-FlexDMD.txt).
// Changed in the porting, from C# to Rust, for Visual Pinball Infinite; with the classes VPX 5436 stands in for
// (plugins/flexdmd/FlexDMDPlugin.cpp, license:GPLv3+: `FlexDMD.FlexDMD`, `UltraDMD.DMDObject`). The port is
// GPL-3.0-or-later.

//! `CreateObject("FlexDMD.FlexDMD")` and `CreateObject("UltraDMD.DMDObject")`:
//! our FlexDMD (`crate::flexdmd`) as the script sees it. Every object the
//! script gets — the FlexDMD, its actors, fonts, pictures, actions and
//! action factories, UltraDMD — is a COM object of one kind here, holding
//! what it stands for, freed when the script lets go (the actors it put on
//! stage stay there). Members are found by name as FlexDMD's .NET objects
//! offer them; values convert as .NET's COM interop converts them (colours
//! as Windows keeps them, red in the lowest byte).
//!
//! Also here: the part of `WScript.Shell` add-ons' settings need,
//! its registry calls for UltraDMD's own settings, answered from our app's
//! settings (`crate::addon_settings`).

use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;

use super::com::*;
use super::value::{Args, Ret};
use super::winevbs;
use crate::flexdmd::action::{self, Action, ActionRef, MoveTo};
use crate::flexdmd::actor::{self, Actor, ActorRef, Kind};
use crate::flexdmd::animated;
use crate::flexdmd::assets::AssetManager;
use crate::flexdmd::font::Font;
use crate::flexdmd::layout::{Alignment, Scaling};
use crate::flexdmd::surface::Bitmap;
use crate::flexdmd::ultradmd::{self, Ultra, UltraRef};
use crate::flexdmd::{self, FlexDmd, FlexRef};

type Res = Result<Ret, HRESULT>;

const E_INVALIDCAST: HRESULT = E_NOINTERFACE;
const DISP_E_OVERFLOW: HRESULT = 0x8002_000A_u32 as i32;
/// What .NET raises for an exception inside a call, as COM hands it on.
const COR_E_EXCEPTION: HRESULT = 0x8013_1500_u32 as i32;
/// `WScript.Shell`'s answer when a registry value isn't there.
const E_FILE_NOT_FOUND: HRESULT = 0x8007_0002_u32 as i32;
const E_ACCESSDENIED: HRESULT = 0x8007_0005_u32 as i32;

/// What one of our objects stands for.
#[derive(Clone)]
pub enum Handle {
    Flex(FlexRef),
    Ultra(UltraRef),
    Actor(ActorRef),
    Factory(ActorRef),
    Action(ActionRef),
    Font(Rc<Font>),
    Bitmap(Rc<Bitmap>),
    /// `WScript.Shell`, for add-ons' registry settings only.
    Shell,
    /// `Shell.Application`, for a file's details only (Munsters reads its pack's videos'
    /// lengths): a folder the script may read, and a file in it.
    ShellApp,
    ShellFolder(std::path::PathBuf),
    ShellItem(std::path::PathBuf),
}

#[repr(C)]
pub struct FlexObj {
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

impl FlexObj {
    /// A new object with one reference, which the caller hands on.
    pub fn create(handle: Handle) -> *mut IDispatch {
        Box::into_raw(Box::new(FlexObj { vtbl: &VTBL, refs: Cell::new(1), handle })).cast()
    }

    /// The object behind a script value, if it's one of these.
    ///
    /// # Safety
    /// `p` must be a live COM object.
    unsafe fn ours<'a>(p: *mut IDispatch) -> Option<&'a FlexObj> {
        unsafe {
            let o = p as *const FlexObj;
            std::ptr::eq((*o).vtbl, &VTBL).then(|| &*o)
        }
    }
}

fn new_obj(h: Handle) -> Ret {
    Ret::NewObject(FlexObj::create(h))
}

fn nothing() -> Ret {
    Ret::Object(std::ptr::null_mut())
}

/// Argument `i` as one of our objects.
fn handle_arg(a: &Args, i: usize) -> Option<Handle> {
    let p = a.object(i)?;
    // SAFETY: a live object the script passed
    unsafe { FlexObj::ours(p) }.map(|o| o.handle.clone())
}

fn actor_arg(a: &Args, i: usize) -> Option<ActorRef> {
    match handle_arg(a, i)? {
        Handle::Actor(x) => Some(x),
        _ => None,
    }
}

fn action_arg(a: &Args, i: usize) -> Option<ActionRef> {
    match handle_arg(a, i)? {
        Handle::Action(x) => Some(x),
        _ => None,
    }
}

fn font_arg(a: &Args, i: usize) -> Option<Rc<Font>> {
    match handle_arg(a, i)? {
        Handle::Font(f) => Some(f),
        _ => None,
    }
}

/// The value assigned, as one of our objects.
fn handle_value(a: &Args) -> Option<Handle> {
    let v = a.value()?;
    if v.vt != VT_DISPATCH {
        return None;
    }
    let p: *mut IDispatch = v.ptr();
    if p.is_null() {
        return None;
    }
    // SAFETY: a live object the script assigned
    unsafe { FlexObj::ours(p) }.map(|o| o.handle.clone())
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// A colour from the script (Windows' `OLE_COLOR`, red in the lowest byte)
/// as GDI+ keeps it: alpha, red, green, blue.
fn color_from(v: i32) -> [u8; 4] {
    let v = v as u32;
    [255, v as u8, (v >> 8) as u8, (v >> 16) as u8]
}

fn color_to(c: [u8; 4]) -> Ret {
    Ret::Int(i32::from(c[1]) | (i32::from(c[2]) << 8) | (i32::from(c[3]) << 16))
}

// ---------------------------------------------------------------------------------------------
// Members, by kind: their position in each list (from 1) is their id.

const FLEX: &[&str] = &[
    "Version", "RuntimeVersion", "Run", "Show", "GameName", "Width", "Height", "Color", "RenderMode", "ProjectFolder", "TableFile", "Clear", "DmdColoredPixels", "DmdPixels", "Segments", "Stage",
    "LockRenderThread", "UnlockRenderThread", "NewGroup", "NewFrame", "NewLabel", "NewVideo", "NewImage", "NewFont", "NewUltraDMD",
];

const ACTOR: &[&str] = &[
    "Name", "X", "Y", "Width", "Height", "Visible", "FillParent", "ClearBackground", "SetBounds", "SetPosition", "SetAlignedPosition", "SetSize", "PrefWidth", "PrefHeight", "Pack", "Remove", "ActionFactory",
    "AddAction", "ClearActions",
    // groups
    "Clip", "ChildCount", "HasChild", "GetGroup", "GetFrame", "GetLabel", "GetVideo", "GetImage", "RemoveAll", "AddActor", "RemoveActor",
    // frames
    "Thickness", "BorderColor", "Fill", "FillColor",
    // images, videos, labels
    "Bitmap", "Scaling", "Alignment", "Length", "Loop", "Paused", "PlaySpeed", "Seek", "AutoPack", "Font", "Text",
];

const FACTORY: &[&str] = &["Wait", "Delayed", "Parallel", "Sequence", "Repeat", "Blink", "Show", "AddTo", "RemoveFromParent", "AddChild", "RemoveChild", "Seek", "MoveTo"];

const ACTION: &[&str] = &["Update", "Add", "Ease"];

const ULTRA: &[&str] = &[
    "LoadSetup", "Init", "Uninit", "GetMajorVersion", "GetMinorVersion", "GetBuildNumber", "SetVisibleVirtualDMD", "SetFlipY", "IsRendering", "CancelRendering", "CancelRenderingWithId", "Clear",
    "SetProjectFolder", "SetVideoStretchMode", "SetScoreboardBackgroundImage", "CreateAnimationFromImages", "RegisterVideo", "DisplayVersionInfo", "DisplayScoreboard", "DisplayScoreboard00",
    "DisplayScene00", "DisplayScene00Ex", "DisplayScene00ExWithId", "ModifyScene00", "ModifyScene00Ex", "DisplayScene01", "DisplayText", "ScrollingCredits",
];

const SHELL: &[&str] = &["RegRead", "RegWrite"];
const SHELL_APP: &[&str] = &["NameSpace"];
const SHELL_FOLDER: &[&str] = &["ParseName", "GetDetailsOf"];
const SHELL_ITEM: &[&str] = &["Name", "Path"];

/// Which members an actor of this kind offers (FlexDMD's interface for it).
fn actor_offers(a: &ActorRef, name: &str) -> bool {
    let common = ACTOR[..19].iter().any(|m| m.eq_ignore_ascii_case(name));
    if common {
        return true;
    }
    let b = a.borrow();
    let list: &[&str] = match &b.kind {
        Kind::Group(_) => &ACTOR[19..30],
        Kind::Frame(_) => &ACTOR[30..34],
        Kind::Image(_) => &["Bitmap", "Scaling", "Alignment"],
        Kind::Anim(_) => &["Scaling", "Alignment", "Length", "Loop", "Paused", "PlaySpeed", "Seek"],
        Kind::Label(_) => &["AutoPack", "Alignment", "Font", "Text"],
        _ => &[],
    };
    list.iter().any(|m| m.eq_ignore_ascii_case(name))
}

fn members(h: &Handle) -> &'static [&'static str] {
    match h {
        Handle::Flex(_) => FLEX,
        Handle::Ultra(_) => ULTRA,
        Handle::Actor(_) => ACTOR,
        Handle::Factory(_) => FACTORY,
        Handle::Action(_) => ACTION,
        Handle::Shell => SHELL,
        Handle::ShellApp => SHELL_APP,
        Handle::ShellFolder(_) => SHELL_FOLDER,
        Handle::ShellItem(_) => SHELL_ITEM,
        Handle::Font(_) | Handle::Bitmap(_) => &[],
    }
}

/// Whether the object offers the member: actors their kind's, actions theirs.
fn offers(h: &Handle, name: &str) -> bool {
    match h {
        Handle::Actor(a) => actor_offers(a, name),
        Handle::Action(x) => match name.to_ascii_lowercase().as_str() {
            "add" => matches!(&*x.borrow(), Action::Sequence { .. } | Action::Parallel { .. }),
            "ease" => matches!(&*x.borrow(), Action::MoveTo(_)),
            _ => true,
        },
        _ => true,
    }
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
    let o = unsafe { &*(this as *const FlexObj) };
    o.refs.set(o.refs.get() + 1);
    o.refs.get()
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let o = unsafe { &*(this as *const FlexObj) };
    let n = o.refs.get().saturating_sub(1);
    o.refs.set(n);
    if n == 0 {
        // SAFETY: the last reference is gone; it was made by `FlexObj::create`
        unsafe { drop(Box::from_raw(this as *mut FlexObj)) };
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
        let o = &*(this as *const FlexObj);
        for i in 0..count as usize {
            *ids.add(i) = DISPID_UNKNOWN;
        }
        if count == 0 {
            return S_OK;
        }
        let name = from_wide(*names);
        let list = members(&o.handle);
        match list.iter().position(|m| m.eq_ignore_ascii_case(&name)) {
            Some(i) if offers(&o.handle, &name) => {
                *ids = i as i32 + 1;
                if count > 1 { DISP_E_UNKNOWNNAME } else { S_OK }
            }
            _ => DISP_E_UNKNOWNNAME,
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
        let o = &*(this as *const FlexObj);
        let empty = DISPPARAMS { rgvarg: std::ptr::null_mut(), rgdispidNamedArgs: std::ptr::null_mut(), cArgs: 0, cNamedArgs: 0 };
        let params = if params.is_null() { &empty } else { &*params };
        // FlexDMD hands its fonts and bitmaps to scripts as plain unknown objects (their classes
        // aren't made visible to COM), which VBScript copies as they are where a value is asked
        // for — SpongeBob's `DMDDisplay(h, 2) = Font`, without Set. libwinevbs's Set takes only
        // dispatch objects, so ours stay dispatch objects and give themselves as their value: the
        // same object ends up there, as in VPX (checked against the installed FlexDMD).
        if dispid == DISPID_VALUE && flags & (DISPATCH_PROPERTYGET | DISPATCH_METHOD) != 0 && params.cArgs == 0 && matches!(o.handle, Handle::Font(_) | Handle::Bitmap(_)) {
            super::value::write(lib, Ret::Object(this as *mut IDispatch), result);
            return S_OK;
        }
        let list = members(&o.handle);
        let Some(&name) = usize::try_from(dispid - 1).ok().and_then(|i| list.get(i)) else { return DISP_E_MEMBERNOTFOUND };
        let args = Args::new(lib, params);
        let put = flags & (DISPATCH_PROPERTYPUT | DISPATCH_PROPERTYPUTREF) != 0;
        let handle = o.handle.clone();
        // a fault of ours becomes the script's error, not the end of the game
        let call = std::panic::AssertUnwindSafe(|| match &handle {
            Handle::Flex(f) => flex_member(f, name, put, &args),
            Handle::Ultra(u) => ultra_member(u, name, &args),
            Handle::Actor(a) => actor_member(a, name, put, &args),
            Handle::Factory(a) => factory_member(a, name, &args),
            Handle::Action(x) => action_member(x, name, put, &args),
            Handle::Shell => shell_member(name, &args),
            Handle::ShellApp => shell_app_member(name, &args),
            Handle::ShellFolder(dir) => shell_folder_member(dir, name, &args),
            Handle::ShellItem(file) => shell_item_member(file, name),
            Handle::Font(_) | Handle::Bitmap(_) => Err(DISP_E_MEMBERNOTFOUND),
        });
        let r = std::panic::catch_unwind(call).unwrap_or_else(|_| Err(failed_with(&format!("{name} failed inside the player"))));
        match r {
            Ok(ret) => {
                super::value::write(lib, ret, result);
                S_OK
            }
            Err(hr) => hr,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Making them

fn assets_for(w: &crate::world::World) -> AssetManager {
    AssetManager::new(w.table_dir.clone(), w.table_path.clone(), Some(w.vpx.clone()))
}

/// A new FlexDMD, which the game steps from now on; outside a game (a
/// harness's script), one of its own, its files found from the current folder.
fn new_flex() -> FlexRef {
    super::with(|_, w| {
        let flex = FlexDmd::new(w.flexdmds.len() as u32, assets_for(w));
        w.flexdmds.push(flex.clone());
        // its videos' sound plays where PinUP's does
        w.sounds.play_pup(crate::pup::playback::sink());
        flex
    })
    .unwrap_or_else(|| FlexDmd::new(0, AssetManager::new(std::env::current_dir().ok(), None, None)))
}

/// `CreateObject("FlexDMD.FlexDMD")`: a FlexDMD of its own.
pub fn create_flex() -> Res {
    Ok(new_obj(Handle::Flex(new_flex())))
}

/// The FlexDMD behind a script value, if it's one of ours (for harnesses).
///
/// # Safety
/// `p` must be a live COM object.
pub unsafe fn flex_of(p: *mut IDispatch) -> Option<FlexRef> {
    match unsafe { FlexObj::ours(p) }.map(|o| o.handle.clone())? {
        Handle::Flex(f) => Some(f),
        Handle::Ultra(u) => Some(u.borrow().flex.clone()),
        _ => None,
    }
}

/// `CreateObject("UltraDMD.DMDObject")`: FlexDMD's stand-in for UltraDMD,
/// on a FlexDMD of its own, UltraDMD's settings read (as FlexDMD's
/// `DMDObject` reads them when made).
pub fn create_ultra() -> Res {
    let flex = new_flex();
    let u = Ultra::new(&flex).map_err(|e| failed_with(&e))?;
    ultradmd::load_setup(&u);
    Ok(new_obj(Handle::Ultra(u)))
}

/// `CreateObject("WScript.Shell")`, for add-ons' registry settings only.
pub fn create_shell() -> Res {
    Ok(new_obj(Handle::Shell))
}

/// `Shell.Application`, for a file's details: no folder the script couldn't read, no program
/// started.
pub fn create_shell_app() -> Res {
    Ok(new_obj(Handle::ShellApp))
}

fn shell_app_member(name: &str, a: &Args) -> Res {
    match name {
        // a folder by its path (Windows' special-folder numbers aren't offered)
        "NameSpace" => {
            let dir = std::path::PathBuf::from(a.string(0).unwrap_or_default());
            Ok(if dir.is_dir() && super::files::readable(&dir) { new_obj(Handle::ShellFolder(dir)) } else { nothing() })
        }
        _ => Err(DISP_E_MEMBERNOTFOUND),
    }
}

fn shell_folder_member(dir: &std::path::Path, name: &str, a: &Args) -> Res {
    match name {
        // a file in the folder by its name alone
        "ParseName" => {
            let file = a.string(0).unwrap_or_default();
            let path = dir.join(&file);
            let plain = !file.is_empty() && !file.contains(['\\', '/', ':']) && file != "..";
            Ok(if plain && path.is_file() && super::files::readable(&path) { new_obj(Handle::ShellItem(path)) } else { nothing() })
        }
        // Explorer's columns: 27 a video's or sound's length (hh:mm:ss, as Explorer shows it,
        // whole seconds), 2 its type; the rest empty
        "GetDetailsOf" => {
            let Some(Handle::ShellItem(file)) = handle_arg(a, 0) else { return Ok(Ret::Str(String::new())) };
            Ok(Ret::Str(match a.i32(1).unwrap_or(-1) {
                27 => crate::pup::media::length(&file).map_or_else(String::new, |ms| {
                    let s = ms / 1000;
                    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
                }),
                2 => file.extension().map_or_else(String::new, |e| format!("{} File", e.to_string_lossy().to_uppercase())),
                _ => String::new(),
            }))
        }
        _ => Err(DISP_E_MEMBERNOTFOUND),
    }
}

fn shell_item_member(file: &std::path::Path, name: &str) -> Res {
    match name {
        "Name" => Ok(Ret::Str(file.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))),
        "Path" => Ok(Ret::Str(file.to_string_lossy().into_owned())),
        _ => Err(DISP_E_MEMBERNOTFOUND),
    }
}

/// An error FlexDMD throws, named once.
fn failed_with(e: &str) -> HRESULT {
    super::log::once(&format!("FlexDMD: {e}"));
    COR_E_EXCEPTION
}

// ---------------------------------------------------------------------------------------------
// FlexDMD

fn flex_member(f: &FlexRef, name: &str, put: bool, a: &Args) -> Res {
    let assets = f.borrow().assets.clone();
    let rt = f.borrow().runtime.clone();
    Ok(match (name, put) {
        ("Version", false) => Ret::Int(FlexDmd::VERSION),
        ("RuntimeVersion", false) => Ret::Int(rt.get()),
        ("RuntimeVersion", true) => {
            rt.set(a.value_i32().ok_or(DISP_E_TYPEMISMATCH)?);
            Ret::Empty
        }
        ("Run", false) => Ret::Bool(f.borrow().run()),
        ("Run", true) => {
            flexdmd::set_run(f, a.value_bool().ok_or(DISP_E_TYPEMISMATCH)?);
            Ret::Empty
        }
        ("Show", false) => Ret::Bool(f.borrow().show),
        ("Show", true) => {
            f.borrow_mut().show = a.value_bool().ok_or(DISP_E_TYPEMISMATCH)?;
            Ret::Empty
        }
        ("GameName", false) => Ret::Str(f.borrow().game_name.clone()),
        ("GameName", true) => {
            f.borrow_mut().game_name = a.value_string().unwrap_or_default();
            Ret::Empty
        }
        ("Width" | "Height", false) => Ret::Int(i32::from(if name == "Width" { f.borrow().width } else { f.borrow().height })),
        ("Width" | "Height", true) => {
            let v = u16::try_from(a.value_i32().ok_or(DISP_E_TYPEMISMATCH)?).map_err(|_| DISP_E_OVERFLOW)?;
            let mut b = f.borrow_mut();
            if name == "Width" {
                b.width = v;
            } else {
                b.height = v;
            }
            Ret::Empty
        }
        ("Color", false) => color_to(f.borrow().color),
        ("Color", true) => {
            f.borrow_mut().color = color_from(a.value_i32().ok_or(DISP_E_TYPEMISMATCH)?);
            Ret::Empty
        }
        ("RenderMode", false) => Ret::Int(f.borrow().render_mode),
        ("RenderMode", true) => {
            f.borrow_mut().render_mode = a.value_i32().ok_or(DISP_E_TYPEMISMATCH)?;
            Ret::Empty
        }
        ("ProjectFolder", false) => Ret::Str(assets.borrow().base_path.clone()),
        ("ProjectFolder", true) => {
            assets.borrow_mut().base_path = a.value_string().unwrap_or_default();
            Ret::Empty
        }
        ("TableFile", false) => Ret::Str(assets.borrow().table_file.clone().unwrap_or_default()),
        ("TableFile", true) => {
            assets.borrow_mut().table_file = a.value_string();
            Ret::Empty
        }
        ("Clear", false) => Ret::Bool(f.borrow().clear),
        ("Clear", true) => {
            f.borrow_mut().clear = a.value_bool().ok_or(DISP_E_TYPEMISMATCH)?;
            Ret::Empty
        }
        ("DmdPixels" | "DmdColoredPixels", _) => {
            let mut b = f.borrow_mut();
            let n = usize::from(b.width) * usize::from(b.height);
            let values: Option<Vec<Ret>> = if name == "DmdPixels" {
                b.ask_pixels().map(|px| px.iter().map(|&v| Ret::Int(i32::from(v))).collect())
            } else {
                b.ask_colored().map(|px| px.iter().map(|&v| Ret::Int(v as i32)).collect())
            };
            // empty until a frame is drawn, as FlexDMD's array of nothing
            Ret::Array(values.unwrap_or_else(|| vec![Ret::Empty; n]))
        }
        ("Segments", true) => {
            let shown = f.borrow().run() && f.borrow().show;
            if shown && let Some(v) = a.value_array_u32() {
                f.borrow_mut().segments = v.into_iter().map(|x| x as u16).collect();
            }
            Ret::Empty
        }
        ("Stage", false) => new_obj(Handle::Actor(f.borrow().stage.clone())),
        ("LockRenderThread", _) => {
            f.borrow_mut().lock_count += 1;
            Ret::Empty
        }
        ("UnlockRenderThread", _) => {
            f.borrow_mut().lock_count -= 1;
            Ret::Empty
        }
        ("NewGroup", _) => new_obj(Handle::Actor(Actor::new_group(&a.string(0).unwrap_or_default(), &rt))),
        ("NewFrame", _) => new_obj(Handle::Actor(Actor::new_frame(&a.string(0).unwrap_or_default()))),
        ("NewLabel", _) => {
            let text = utf16(&a.string(2).unwrap_or_default());
            new_obj(Handle::Actor(Actor::new_label(&a.string(0).unwrap_or_default(), &rt, font_arg(a, 1), &text)))
        }
        ("NewVideo", _) => match ultradmd::new_video(f, &a.string(0).unwrap_or_default(), &a.string(1).unwrap_or_default()) {
            Some(v) => new_obj(Handle::Actor(v)),
            None => nothing(),
        },
        ("NewImage", _) => {
            let img = Actor::new_image(&a.string(0).unwrap_or_default(), &assets, &a.string(1).unwrap_or_default()).map_err(|e| failed_with(&e))?;
            new_obj(Handle::Actor(img))
        }
        ("NewFont", _) => {
            let (path, tint, border, size) = (a.string(0).unwrap_or_default(), a.i32(1).unwrap_or(0), a.i32(2).unwrap_or(0), a.i32(3).unwrap_or(0));
            let font = ultradmd::new_font(f, &path, color_from(tint), color_from(border), size).map_err(|e| failed_with(&e))?;
            new_obj(Handle::Font(font))
        }
        ("NewUltraDMD", _) => new_obj(Handle::Ultra(Ultra::new(f).map_err(|e| failed_with(&e))?)),
        _ => return Err(DISP_E_MEMBERNOTFOUND),
    })
}

// ---------------------------------------------------------------------------------------------
// Actors

/// FlexDMD's cast of a found actor to the interface asked for.
fn cast(found: Option<ActorRef>, ok: impl Fn(&Kind) -> bool) -> Res {
    match found {
        None => Ok(nothing()),
        Some(x) if ok(&x.borrow().kind) => Ok(new_obj(Handle::Actor(x))),
        Some(_) => Err(E_INVALIDCAST),
    }
}

fn anim_get<R>(a: &ActorRef, f: impl FnOnce(&animated::Anim) -> R) -> Option<R> {
    match &a.borrow().kind {
        Kind::Anim(an) => Some(f(an)),
        _ => None,
    }
}

fn anim_set(a: &ActorRef, f: impl FnOnce(&mut animated::Anim)) {
    if let Kind::Anim(an) = &mut a.borrow_mut().kind {
        f(an);
    }
}

fn actor_member(a: &ActorRef, name: &str, put: bool, args: &Args) -> Res {
    let v = || args.value_f32().ok_or(DISP_E_TYPEMISMATCH);
    let vb = || args.value_bool().ok_or(DISP_E_TYPEMISMATCH);
    let vi = || args.value_i32().ok_or(DISP_E_TYPEMISMATCH);
    let f = |i: usize| args.f32(i).unwrap_or(0.0);
    Ok(match (name, put) {
        ("Name", false) => Ret::Str(a.borrow().name.clone()),
        ("Name", true) => {
            a.borrow_mut().name = args.value_string().unwrap_or_default();
            Ret::Empty
        }
        ("X", false) => Ret::Float(a.borrow().x),
        ("X", true) => {
            a.borrow_mut().x = v()?;
            Ret::Empty
        }
        ("Y", false) => Ret::Float(a.borrow().y),
        ("Y", true) => {
            a.borrow_mut().y = v()?;
            Ret::Empty
        }
        ("Width", false) => Ret::Float(a.borrow().width),
        ("Width", true) => {
            a.borrow_mut().width = v()?;
            Ret::Empty
        }
        ("Height", false) => Ret::Float(a.borrow().height),
        ("Height", true) => {
            a.borrow_mut().height = v()?;
            Ret::Empty
        }
        ("Visible", false) => Ret::Bool(a.borrow().visible),
        ("Visible", true) => {
            actor::set_visible(a, vb()?);
            Ret::Empty
        }
        ("FillParent", false) => Ret::Bool(a.borrow().fill_parent),
        ("FillParent", true) => {
            a.borrow_mut().fill_parent = vb()?;
            Ret::Empty
        }
        ("ClearBackground", false) => Ret::Bool(a.borrow().clear_background),
        ("ClearBackground", true) => {
            a.borrow_mut().clear_background = vb()?;
            Ret::Empty
        }
        ("SetBounds", _) => {
            a.borrow_mut().set_bounds(f(0), f(1), f(2), f(3));
            Ret::Empty
        }
        ("SetPosition", _) => {
            let mut b = a.borrow_mut();
            (b.x, b.y) = (f(0), f(1));
            Ret::Empty
        }
        ("SetAlignedPosition", _) => {
            a.borrow_mut().set_aligned_position(f(0), f(1), Alignment::from_i32(args.i32(2).unwrap_or(0)));
            Ret::Empty
        }
        ("SetSize", _) => {
            let mut b = a.borrow_mut();
            (b.width, b.height) = (f(0), f(1));
            Ret::Empty
        }
        ("PrefWidth", false) => Ret::Float(actor::pref_size(&a.borrow()).0),
        ("PrefHeight", false) => Ret::Float(actor::pref_size(&a.borrow()).1),
        ("Pack", _) => {
            actor::pack(a);
            Ret::Empty
        }
        ("Remove", _) => {
            actor::remove(a);
            Ret::Empty
        }
        ("ActionFactory", false) => new_obj(Handle::Factory(a.clone())),
        ("AddAction", _) => {
            if let Some(x) = action_arg(args, 0) {
                a.borrow_mut().actions.push(x);
            }
            Ret::Empty
        }
        ("ClearActions", _) => {
            a.borrow_mut().actions.clear();
            Ret::Empty
        }
        // groups
        ("Clip", false) => Ret::Bool(a.borrow().group().is_some_and(|g| g.clip)),
        ("Clip", true) => {
            let c = vb()?;
            if let Some(g) = a.borrow_mut().group_mut() {
                g.clip = c;
            }
            Ret::Empty
        }
        ("ChildCount", false) => Ret::Int(a.borrow().group().map_or(0, |g| g.children.len() as i32)),
        ("HasChild", _) => Ret::Bool(actor::get(a, &args.string(0).unwrap_or_default()).is_some()),
        ("GetGroup", _) => return cast(actor::get(a, &args.string(0).unwrap_or_default()), |k| matches!(k, Kind::Group(_))),
        ("GetFrame", _) => return cast(actor::get(a, &args.string(0).unwrap_or_default()), |k| matches!(k, Kind::Frame(_))),
        ("GetLabel", _) => return cast(actor::get(a, &args.string(0).unwrap_or_default()), |k| matches!(k, Kind::Label(_))),
        ("GetVideo", _) => return cast(actor::get(a, &args.string(0).unwrap_or_default()), |k| matches!(k, Kind::Anim(_))),
        ("GetImage", _) => return cast(actor::get(a, &args.string(0).unwrap_or_default()), |k| matches!(k, Kind::Image(_))),
        ("RemoveAll", _) => {
            actor::remove_all(a);
            Ret::Empty
        }
        ("AddActor", _) => {
            if let Some(c) = actor_arg(args, 0) {
                actor::add_actor(a, &c, None);
            }
            Ret::Empty
        }
        ("RemoveActor", _) => {
            if let Some(c) = actor_arg(args, 0) {
                actor::remove_actor(a, &c);
            }
            Ret::Empty
        }
        // frames
        ("Thickness" | "BorderColor" | "Fill" | "FillColor", false) => {
            let b = a.borrow();
            let Kind::Frame(fr) = &b.kind else { return Err(DISP_E_MEMBERNOTFOUND) };
            match name {
                "Thickness" => Ret::Int(fr.thickness),
                "BorderColor" => color_to(fr.border_color),
                "Fill" => Ret::Bool(fr.fill),
                _ => color_to(fr.fill_color),
            }
        }
        ("Thickness" | "BorderColor" | "Fill" | "FillColor", true) => {
            let n = vi()?;
            let mut b = a.borrow_mut();
            let Kind::Frame(fr) = &mut b.kind else { return Err(DISP_E_MEMBERNOTFOUND) };
            match name {
                "Thickness" => fr.thickness = n,
                "BorderColor" => fr.border_color = color_from(n),
                "Fill" => fr.fill = n != 0,
                _ => fr.fill_color = color_from(n),
            }
            Ret::Empty
        }
        // images
        ("Bitmap", false) => {
            let todo = match &a.borrow().kind {
                Kind::Image(i) => Some((i.bitmap.clone(), i.assets.clone(), i.src.clone())),
                _ => None,
            };
            let Some((held, assets, src)) = todo else { return Err(DISP_E_MEMBERNOTFOUND) };
            let bmp = match held {
                Some(b) => b,
                None => {
                    let b = assets.borrow_mut().bitmap(&src).map_err(|e| failed_with(&e))?;
                    if let Kind::Image(i) = &mut a.borrow_mut().kind {
                        i.bitmap = Some(b.clone());
                    }
                    b
                }
            };
            new_obj(Handle::Bitmap(bmp))
        }
        ("Bitmap", true) => {
            let bmp = match handle_value(args) {
                Some(Handle::Bitmap(b)) => Some(b),
                _ => None,
            };
            if let Kind::Image(i) = &mut a.borrow_mut().kind {
                i.bitmap = bmp;
            }
            Ret::Empty
        }
        ("Scaling" | "Alignment", false) => {
            let b = a.borrow();
            match (&b.kind, name) {
                (Kind::Image(i), "Scaling") => Ret::Int(i.scaling.to_i32()),
                (Kind::Image(i), _) => Ret::Int(i.alignment.to_i32()),
                (Kind::Anim(an), "Scaling") => Ret::Int(an.scaling.to_i32()),
                (Kind::Anim(an), _) => Ret::Int(an.alignment.to_i32()),
                (Kind::Label(l), "Alignment") => Ret::Int(l.alignment.to_i32()),
                _ => return Err(DISP_E_MEMBERNOTFOUND),
            }
        }
        ("Scaling" | "Alignment", true) => {
            let n = vi()?;
            let mut b = a.borrow_mut();
            match (&mut b.kind, name) {
                (Kind::Image(i), "Scaling") => i.scaling = Scaling::from_i32(n),
                (Kind::Image(i), _) => i.alignment = Alignment::from_i32(n),
                (Kind::Anim(an), "Scaling") => an.scaling = Scaling::from_i32(n),
                (Kind::Anim(an), _) => an.alignment = Alignment::from_i32(n),
                (Kind::Label(l), "Alignment") => l.alignment = Alignment::from_i32(n),
                _ => return Err(DISP_E_MEMBERNOTFOUND),
            }
            Ret::Empty
        }
        // videos
        ("Length", false) => Ret::Float(anim_get(a, animated::length).ok_or(DISP_E_MEMBERNOTFOUND)?),
        ("Loop", false) => Ret::Bool(anim_get(a, |an| an.looping).ok_or(DISP_E_MEMBERNOTFOUND)?),
        ("Loop", true) => {
            let b = vb()?;
            anim_set(a, |an| an.looping = b);
            Ret::Empty
        }
        ("Paused", false) => Ret::Bool(anim_get(a, |an| an.paused).ok_or(DISP_E_MEMBERNOTFOUND)?),
        ("Paused", true) => {
            let b = vb()?;
            anim_set(a, |an| an.paused = b);
            Ret::Empty
        }
        ("PlaySpeed", false) => Ret::Float(anim_get(a, |an| an.play_speed).ok_or(DISP_E_MEMBERNOTFOUND)?),
        ("PlaySpeed", true) => {
            let s = v()?;
            anim_set(a, |an| an.play_speed = s);
            Ret::Empty
        }
        ("Seek", _) => {
            animated::seek(a, f(0));
            Ret::Empty
        }
        // labels
        ("AutoPack", false) => Ret::Bool(a.borrow().label().is_some_and(|l| l.auto_pack)),
        ("AutoPack", true) => {
            let b = vb()?;
            if let Some(l) = a.borrow_mut().label_mut() {
                l.auto_pack = b;
            }
            Ret::Empty
        }
        ("Font", false) => match a.borrow().label().and_then(|l| l.font.clone()) {
            Some(f) => new_obj(Handle::Font(f)),
            None => nothing(),
        },
        ("Font", true) => {
            let font = match handle_value(args) {
                Some(Handle::Font(f)) => Some(f),
                _ => None,
            };
            actor::set_font(a, font);
            Ret::Empty
        }
        ("Text", false) => Ret::Str(String::from_utf16_lossy(&a.borrow().label().map(|l| l.text()).unwrap_or_default())),
        ("Text", true) => {
            actor::set_text(a, &utf16(&args.value_string().unwrap_or_default()));
            Ret::Empty
        }
        _ => return Err(DISP_E_MEMBERNOTFOUND),
    })
}

// ---------------------------------------------------------------------------------------------
// Actions

fn factory_member(target: &ActorRef, name: &str, a: &Args) -> Res {
    let weak = Rc::downgrade(target);
    let f = |i: usize| a.f32(i).unwrap_or(0.0);
    let act = match name {
        "Wait" => Action::Wait { seconds: f(0), time: 0.0 },
        "Delayed" => Action::Delayed { seconds: f(0), action: action_arg(a, 1).ok_or(DISP_E_TYPEMISMATCH)?, time: 0.0 },
        "Parallel" => Action::Parallel { actions: Vec::new(), running: Vec::new() },
        "Sequence" => Action::Sequence { actions: Vec::new(), pos: 0 },
        "Repeat" => Action::Repeat { action: action_arg(a, 0).ok_or(DISP_E_TYPEMISMATCH)?, count: a.i32(1).unwrap_or(0), n: 0 },
        "Blink" => Action::Blink { target: weak, show: f(0), hide: f(1), repeat: a.i32(2).unwrap_or(0), n: 0, time: 0.0 },
        "Show" => Action::Show { target: weak, visible: a.bool(0).unwrap_or(false) },
        "AddTo" => {
            let parent = actor_arg(a, 0).filter(|p| p.borrow().group().is_some()).ok_or(DISP_E_TYPEMISMATCH)?;
            Action::AddTo { target: weak, parent, add: true }
        }
        "RemoveFromParent" => Action::RemoveFromParent { target: weak },
        "AddChild" | "RemoveChild" => {
            if target.borrow().group().is_none() {
                return Err(E_INVALIDCAST);
            }
            Action::AddChild { target: weak, child: actor_arg(a, 0).ok_or(DISP_E_TYPEMISMATCH)?, add: name == "AddChild" }
        }
        "Seek" => {
            if !matches!(target.borrow().kind, Kind::Anim(_)) {
                return Err(E_INVALIDCAST);
            }
            Action::Seek { target: weak, position: f(0) }
        }
        "MoveTo" => Action::MoveTo(MoveTo::new(target, f(0), f(1), f(2))),
        _ => return Err(DISP_E_MEMBERNOTFOUND),
    };
    Ok(new_obj(Handle::Action(action::new(act))))
}

fn action_member(x: &ActionRef, name: &str, put: bool, a: &Args) -> Res {
    Ok(match (name, put) {
        ("Update", _) => Ret::Bool(action::update(x, a.f32(0).unwrap_or(0.0))),
        ("Add", _) => {
            if let Some(child) = action_arg(a, 0) {
                action::add(x, child);
            }
            new_obj(Handle::Action(x.clone()))
        }
        ("Ease", false) => match &*x.borrow() {
            Action::MoveTo(m) => Ret::Int(m.ease),
            _ => return Err(DISP_E_MEMBERNOTFOUND),
        },
        ("Ease", true) => {
            let n = a.value_i32().ok_or(DISP_E_TYPEMISMATCH)?;
            if let Action::MoveTo(m) = &mut *x.borrow_mut() {
                m.ease = n;
            }
            Ret::Empty
        }
        _ => return Err(DISP_E_MEMBERNOTFOUND),
    })
}

// ---------------------------------------------------------------------------------------------
// UltraDMD

fn ultra_member(u: &UltraRef, name: &str, a: &Args) -> Res {
    let s = |i: usize| a.string(i).unwrap_or_default();
    let t = |i: usize| utf16(&a.string(i).unwrap_or_default());
    let n = |i: usize| a.i32(i).unwrap_or(0);
    let flex = u.borrow().flex.clone();
    Ok(match name {
        "LoadSetup" => {
            ultradmd::load_setup(u);
            Ret::Empty
        }
        "Init" => {
            flexdmd::set_run(&flex, true);
            Ret::Empty
        }
        "Uninit" => {
            flexdmd::set_run(&flex, false);
            Ret::Empty
        }
        "GetMajorVersion" => Ret::Int(ultradmd::MAJOR),
        "GetMinorVersion" => Ret::Int(ultradmd::MINOR),
        "GetBuildNumber" => Ret::Int(ultradmd::BUILD),
        "SetVisibleVirtualDMD" => Ret::Bool(ultradmd::set_visible_virtual_dmd(u, a.bool(0).unwrap_or(false))),
        "SetFlipY" => Ret::Bool(false),
        "IsRendering" => Ret::Bool(ultradmd::is_rendering(u)),
        "CancelRendering" => {
            ultradmd::cancel_rendering(u);
            Ret::Empty
        }
        "CancelRenderingWithId" => {
            ultradmd::cancel_rendering_with_id(u, s(0));
            Ret::Empty
        }
        "Clear" => {
            ultradmd::clear(u);
            Ret::Empty
        }
        "SetProjectFolder" => {
            flex.borrow().assets.borrow_mut().base_path = s(0);
            Ret::Empty
        }
        "SetVideoStretchMode" => {
            ultradmd::set_video_stretch_mode(u, n(0));
            Ret::Empty
        }
        "SetScoreboardBackgroundImage" => {
            ultradmd::set_scoreboard_background(u, s(0), n(1), n(2));
            Ret::Empty
        }
        "CreateAnimationFromImages" => Ret::Int(ultradmd::create_animation_from_images(u, n(0), a.bool(1).unwrap_or(false), &s(2))),
        "RegisterVideo" => Ret::Int(ultradmd::register_video(u, n(0), a.bool(1).unwrap_or(false), &s(2))),
        "DisplayVersionInfo" => {
            ultradmd::display_version_info(u);
            Ret::Empty
        }
        "DisplayScoreboard" | "DisplayScoreboard00" => {
            let score = |i: usize| a.f64(i).unwrap_or(0.0) as i64;
            ultradmd::display_scoreboard(u, n(0), n(1), [score(2), score(3), score(4), score(5)], t(6), t(7));
            Ret::Empty
        }
        "DisplayScene00" => {
            ultradmd::display_scene00(u, String::new(), false, s(0), t(1), n(2), -15, t(3), n(4), -15, n(5), n(6), n(7));
            Ret::Empty
        }
        "DisplayScene00Ex" => {
            ultradmd::display_scene00(u, String::new(), false, s(0), t(1), n(2), n(3), t(4), n(5), n(6), n(7), n(8), n(9));
            Ret::Empty
        }
        "DisplayScene00ExWithId" => {
            ultradmd::display_scene00(u, s(0), a.bool(1).unwrap_or(false), s(2), t(3), n(4), n(5), t(6), n(7), n(8), n(9), n(10), n(11));
            Ret::Empty
        }
        "ModifyScene00" => {
            ultradmd::modify_scene00(u, s(0), t(1), t(2), None);
            Ret::Empty
        }
        "ModifyScene00Ex" => {
            ultradmd::modify_scene00(u, s(0), t(1), t(2), Some(n(3)));
            Ret::Empty
        }
        "DisplayScene01" => {
            ultradmd::display_scene01(u, s(0), s(1), t(2), n(3), n(4), n(5), n(6), n(7));
            Ret::Empty
        }
        "DisplayText" => {
            ultradmd::display_text(u);
            Ret::Empty
        }
        "ScrollingCredits" => {
            ultradmd::scrolling_credits(u, s(0), t(1), n(2), n(3), n(4), n(5));
            Ret::Empty
        }
        _ => return Err(DISP_E_MEMBERNOTFOUND),
    })
}

// ---------------------------------------------------------------------------------------------
// WScript.Shell, for add-ons' settings

/// The settings add-ons keep in the registry that we answer: UltraDMD's — its window's place too
/// (`x`, `y`), which tables write for the original UltraDMD (KISS's) and FlexDMD never reads, so
/// it's kept and does nothing, as in VPX.
const REGISTRY: &[(&str, &[&str])] = &[("UltraDMD", &["color", "fullcolor", "hideVDMD", "flipY", "x", "y"])];

/// A registry path (`HKCU\Software\UltraDMD\color`) as an add-on's setting.
fn setting(path: &str) -> Option<(&'static str, &'static str)> {
    let p = path.replace('/', "\\");
    let mut parts = p.split('\\');
    let root = parts.next()?;
    if !root.eq_ignore_ascii_case("HKCU") && !root.eq_ignore_ascii_case("HKEY_CURRENT_USER") {
        return None;
    }
    if !parts.next()?.eq_ignore_ascii_case("Software") {
        return None;
    }
    let key = parts.next()?;
    let value = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let (section, names) = REGISTRY.iter().find(|(s, _)| s.eq_ignore_ascii_case(key))?;
    let name = names.iter().find(|n| n.eq_ignore_ascii_case(value))?;
    Some((section, name))
}

/// A registry path of `controller.vbs`'s own settings (`HKCU\Software\Visual Pinball\Controller\…`),
/// which older copies of it keep there: the setting's name.
fn controller_path(path: &str) -> Option<String> {
    let p = path.replace('/', "\\");
    let parts: Vec<&str> = p.split('\\').collect();
    let [root, software, vp, controller, name] = parts.as_slice() else { return None };
    let root_ok = root.eq_ignore_ascii_case("HKCU") || root.eq_ignore_ascii_case("HKEY_CURRENT_USER");
    (root_ok && software.eq_ignore_ascii_case("Software") && vp.eq_ignore_ascii_case("Visual Pinball") && controller.eq_ignore_ascii_case("Controller")).then(|| name.to_string())
}

fn shell_member(name: &str, a: &Args) -> Res {
    let path = a.string(0).unwrap_or_default();
    // an older `controller.vbs`'s settings: answered as the newer one's are, never from the
    // registry (so its toys fire as any table's do); what it writes stays with the
    // player's settings
    if let Some(key) = controller_path(&path)
        && let Some(value) = super::api::controller_setting(&key)
    {
        return match name {
            "RegRead" => Ok(Ret::Int(value)),
            "RegWrite" => {
                super::log::once("an older controller.vbs wrote its settings to the registry: they're the player's settings instead");
                Ok(Ret::Empty)
            }
            _ => Err(DISP_E_MEMBERNOTFOUND),
        };
    }
    match name {
        "RegRead" => {
            let Some((section, key)) = setting(&path) else {
                super::log::once(&format!("WScript.Shell's RegRead({path}) refused: only add-ons' own settings are answered"));
                return Err(E_ACCESSDENIED);
            };
            crate::addon_settings::get(section, key).map(Ret::Str).ok_or(E_FILE_NOT_FOUND)
        }
        "RegWrite" => {
            let Some((section, key)) = setting(&path) else {
                super::log::once(&format!("WScript.Shell's RegWrite({path}) refused: only add-ons' own settings are kept"));
                return Err(E_ACCESSDENIED);
            };
            crate::addon_settings::set(section, key, &a.string(1).unwrap_or_default());
            Ok(Ret::Empty)
        }
        _ => Err(DISP_E_MEMBERNOTFOUND),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_controller_vbs_finds_its_settings() {
        assert_eq!(controller_path(r"HKEY_CURRENT_USER\SOFTWARE\Visual Pinball\Controller\DOFContactors").as_deref(), Some("DOFContactors"));
        assert_eq!(controller_path(r"HKCU\Software\visual pinball\controller\ForceDisableB2S").as_deref(), Some("ForceDisableB2S"));
        assert_eq!(controller_path(r"HKLM\SOFTWARE\Visual Pinball\Controller\DOFContactors"), None);
        assert_eq!(controller_path(r"HKCU\Software\Visual Pinball\VP10\Player\Width"), None);
    }

    #[test]
    fn registry_paths_name_only_add_ons_settings() {
        assert_eq!(setting(r"HKCU\Software\UltraDMD\color"), Some(("UltraDMD", "color")));
        assert_eq!(setting(r"HKEY_CURRENT_USER\Software\ultradmd\FULLCOLOR"), Some(("UltraDMD", "fullcolor")));
        assert_eq!(setting(r"HKLM\Software\UltraDMD\color"), None);
        assert_eq!(setting(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run"), None);
        assert_eq!(setting(r"HKCU\Software\UltraDMD\other"), None);
    }

    #[test]
    fn colours_convert_as_dotnet_converts_ole_colours() {
        // VBScript's RGB(255, 88, 32)
        assert_eq!(color_from(255 | (88 << 8) | (32 << 16)), [255, 255, 88, 32]);
        assert!(matches!(color_to([255, 255, 88, 32]), Ret::Int(v) if v == 255 | (88 << 8) | (32 << 16)));
    }
}
