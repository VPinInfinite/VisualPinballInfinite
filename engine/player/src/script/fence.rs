//! The fence's door for outside objects: `CreateObject`
//! gets only what is on the list. Everything else is refused, and named
//! once so a table's needs show, until the player provides it.
//!
//! libwinevbs makes three objects itself without asking its host —
//! `Scripting.Dictionary`, `Scripting.FileSystemObject` and `WScript.Shell`
//! — so the player gives the table's script a named item called
//! `CreateObject`, which VBScript finds before its own function of that
//! name: every call reaches [`create`]. What is allowed of the first two is
//! made by a second script engine that runs no table code; `WScript.Shell`
//! is ours, answering only add-ons' settings in the registry.
//!
//! A table the user lets reach outside its own files (a setting, for every
//! table or one) gets what VPX
//! gives it: Windows' own objects for every class that isn't one of ours —
//! the shell, the internet, any program's — and every file ([`set_open`]).

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use super::com::*;
use super::host::ScriptHost;
use super::idl_types::{Access, Interface, Member, Param, Ty};
use super::value::{Args, Ret};

/// The object behind the table script's `CreateObject`: one method, its
/// default (`CreateObject(class, [server])`).
pub static I_CREATE_OBJECT: Interface = Interface {
    name: "ICreateObject",
    members: &[Member {
        name: "CreateObject",
        dispid: DISPID_VALUE,
        access: Access::Method,
        params: &[
            Param { name: "Class", ty: Ty::Str, optional: false, default: None, by_ref: false },
            Param { name: "ServerName", ty: Ty::Str, optional: true, default: None, by_ref: false },
        ],
        ret: Some(Ty::Object),
    }],
};

/// VBScript's error 429, "ActiveX component can't create object": what a
/// script sees when a class isn't there.
const CTL_E_CANTCREATEOBJECT: HRESULT = 0x800A_01AD_u32 as i32;

/// The table may reach outside its own files: the user's setting, read from
/// the main settings file alone, never from a file that comes with a table.
static OPEN: AtomicBool = AtomicBool::new(false);

/// Lets the table's script reach outside its own files, or not (the default).
pub fn set_open(open: bool) {
    OPEN.store(open, Ordering::Release);
}

/// Whether the table's script may reach outside its own files.
pub fn open() -> bool {
    OPEN.load(Ordering::Acquire)
}

/// Whether the user lets a table reach outside its own files: `[VPI]`
/// `ScriptsReachOutside` for every table, and a table's own line in the
/// `[ScriptsReachOutside]` section, by its file's name, over it — both read
/// from the main settings file alone (`main`), so a file that comes with a
/// table can't grant it.
pub fn open_for(main: &crate::settings::Settings, table: &std::path::Path) -> bool {
    let name = table.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let own = main.section("ScriptsReachOutside").into_iter().rev().find(|(n, _)| n.eq_ignore_ascii_case(&name)).and_then(|(_, v)| crate::settings::vpx_bool(&v));
    own.or_else(|| main.get("VPI", "ScriptsReachOutside").and_then(crate::settings::vpx_bool)).unwrap_or(false)
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CLSIDFromProgID(progid: *const u16, clsid: *mut GUID) -> HRESULT;
    fn CoCreateInstance(clsid: *const GUID, outer: *mut c_void, context: u32, iid: *const GUID, out: *mut *mut c_void) -> HRESULT;
    fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> HRESULT;
}

const CLSCTX_INPROC_SERVER: u32 = 0x1;
const CLSCTX_LOCAL_SERVER: u32 = 0x4;
const COINIT_APARTMENTTHREADED: u32 = 0x2;

thread_local! {
    /// COM started on the script's thread, the first time a Windows object is asked for.
    static COM_STARTED: Cell<bool> = const { Cell::new(false) };
}

/// Windows' own object of a class, as VPX's script engine would make it
/// (`CreateObject` in Microsoft's VBScript), for a table let outside.
fn windows_object(progid: &str) -> Option<*mut IDispatch> {
    if !COM_STARTED.get() {
        // SAFETY: the script's thread; a second start is answered and harmless
        unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        COM_STARTED.set(true);
    }
    let wide: Vec<u16> = progid.encode_utf16().chain(Some(0)).collect();
    let mut clsid = GUID { data1: 0, data2: 0, data3: 0, data4: [0; 8] };
    let mut object: *mut c_void = std::ptr::null_mut();
    // SAFETY: a NUL-terminated name, out-pointers to our own locals
    unsafe {
        if failed(CLSIDFromProgID(wide.as_ptr(), &mut clsid)) {
            return None;
        }
        if failed(CoCreateInstance(&clsid, std::ptr::null_mut(), CLSCTX_INPROC_SERVER | CLSCTX_LOCAL_SERVER, &IID_IDISPATCH, &mut object)) || object.is_null() {
            return None;
        }
    }
    Some(object.cast())
}

/// libwinevbs's own objects that are on the list. `FileSystemObject` is held
/// to the table's folders by our build of libwinevbs ([`super::files`]); with
/// one that can't, it is refused.
const MADE_BY_LIBWINEVBS: &[&str] = &["Scripting.Dictionary", "Scripting.FileSystemObject"];

thread_local! {
    /// The script engine that makes libwinevbs's own objects, started the
    /// first time one is asked for.
    static MAKER: RefCell<Option<ScriptHost>> = const { RefCell::new(None) };
}

/// A table script's `CreateObject`.
pub fn create(a: &Args) -> Result<Ret, HRESULT> {
    let class = a.string(0).unwrap_or_default();
    // PinMAME, through our controller
    if class.trim().eq_ignore_ascii_case("VPinMAME.Controller") {
        return super::controller::create();
    }
    // the backglass server, which is the controller too
    if class.trim().eq_ignore_ascii_case("B2S.Server") {
        return super::controller::create_b2s();
    }
    // PinUP Player, through our pack player
    if class.trim().eq_ignore_ascii_case("PinUpPlayer.PinDisplay") {
        return super::pup::create();
    }
    // FlexDMD, and its stand-in for UltraDMD
    if class.trim().eq_ignore_ascii_case("FlexDMD.FlexDMD") {
        return super::flexdmd::create_flex();
    }
    if class.trim().eq_ignore_ascii_case("UltraDMD.DMDObject") {
        return super::flexdmd::create_ultra();
    }
    // VPX 5436's stand-in for Windows Media Player's object, for tables' music (its WMP plugin)
    if class.trim().eq_ignore_ascii_case("WMPlayer.OCX") {
        return super::wmp::create();
    }
    // a table the user let outside: Windows' own objects for every class that isn't ours or
    // libwinevbs's, the shell's among them
    if open() && !MADE_BY_LIBWINEVBS.iter().any(|c| c.eq_ignore_ascii_case(class.trim())) {
        if let Some(object) = windows_object(class.trim()) {
            super::log::once(&format!("CreateObject(\"{class}\"): Windows' own, the table being let outside its files"));
            return Ok(Ret::NewObject(object));
        }
        super::log::once(&format!("CreateObject(\"{class}\"): no such class on this PC"));
        return Err(CTL_E_CANTCREATEOBJECT);
    }
    // the shell, for add-ons' settings in the registry only: it starts no program and reaches
    // no other key
    if class.trim().eq_ignore_ascii_case("WScript.Shell") {
        return super::flexdmd::create_shell();
    }
    // Windows' shell, for a file's details only — a video's length (Munsters times its pack's
    // videos by them): folders the script may read already, no program started
    if class.trim().eq_ignore_ascii_case("Shell.Application") {
        return super::flexdmd::create_shell_app();
    }
    let Some(&known) = MADE_BY_LIBWINEVBS.iter().find(|c| c.eq_ignore_ascii_case(class.trim())) else {
        super::log::once(&format!("CreateObject(\"{class}\") refused: not provided yet"));
        return Err(CTL_E_CANTCREATEOBJECT);
    };
    if known == "Scripting.FileSystemObject" && !super::files::fenced() {
        super::log::once("CreateObject(\"Scripting.FileSystemObject\") refused: this libwinevbs can't hold files to the table's folders");
        return Err(CTL_E_CANTCREATEOBJECT);
    }
    let made = MAKER.with(|m| {
        let mut maker = m.borrow_mut();
        if maker.is_none() {
            let mut host = ScriptHost::new(&[]).ok()?;
            host.load("").ok()?;
            *maker = Some(host);
        }
        maker.as_ref()?.evaluate(&format!("CreateObject(\"{known}\")"))
    });
    match made {
        Some(v) if v.vt == VT_DISPATCH && !v.ptr::<IDispatch>().is_null() => Ok(Ret::NewObject(v.ptr())),
        Some(mut v) => {
            if let Ok(lib) = super::winevbs::get() {
                // SAFETY: a VARIANT the engine handed us
                unsafe { (lib.variant_clear)(&mut v) };
            }
            Err(CTL_E_CANTCREATEOBJECT)
        }
        None => Err(CTL_E_CANTCREATEOBJECT),
    }
}

/// libwinevbs's `create_object` callback, for the classes it doesn't make
/// itself. The table's script no longer reaches it ([`create`] answers
/// first); it refuses everything, and never uses the factory it is given
/// (this libwinevbs passes one it never set).
///
/// # Safety
/// `object` must be null or writable.
pub unsafe fn create_object(progid: &str, _factory: *mut c_void, object: *mut *mut c_void) -> HRESULT {
    unsafe {
        if !object.is_null() {
            *object = std::ptr::null_mut();
        }
    }
    super::log::once(&format!("CreateObject(\"{progid}\") refused: not provided yet"));
    REGDB_E_CLASSNOTREG
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn reaching_outside_is_the_users_word_for_every_table_or_one() {
        let main = crate::settings::Settings::from_text("[VPI]\nScriptsReachOutside = 0\n\n[ScriptsReachOutside]\nDarkest Dungeon.vpx = 1\n");
        assert!(open_for(&main, Path::new(r"C:\Tables\Darkest Dungeon.vpx")));
        assert!(!open_for(&main, Path::new(r"C:\Tables\Hang Glider.vpx")));
        // every table let out, one kept in
        let main = crate::settings::Settings::from_text("[VPI]\nScriptsReachOutside = 1\n\n[ScriptsReachOutside]\nhang glider.vpx = 0\n");
        assert!(!open_for(&main, Path::new(r"C:\Tables\Hang Glider.vpx")));
        assert!(open_for(&main, Path::new(r"C:\Tables\Other.vpx")));
        // nothing said: kept in
        assert!(!open_for(&crate::settings::Settings::from_text(""), Path::new("x.vpx")));
    }
}
