//! Our script objects as COM objects the script engine can call:
//! the table, the global functions, each part, each collection, each ball.
//! One object type serves them all: VPX's interface tables give the names
//! and ids, and `api` does the work.
//!
//! Objects live until play ends (the script may keep references anywhere);
//! only the enumerators `For Each` makes are freed when released.

use std::cell::Cell;
use std::ffi::c_void;

use physics_compat::env::BallId;

use super::com::*;
use super::idl::{CLASSES, CONSTANTS, I_TABLE_GLOBAL};
use super::idl_types::{Access, Interface, Member};
use super::value::{Args, Ret, write};
use super::winevbs;

/// What an object stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The global functions and constants (VPX's `ScriptGlobalTable`).
    Global,
    /// The table itself.
    Table,
    /// `Debug.Print`.
    Debug,
    /// A part, by its index in `vpx.gameitems`.
    Part(u32),
    /// A collection, by its index in `vpx.collections`.
    Collection(u32),
    /// A ball.
    Ball(BallId),
    /// The fence's `CreateObject`, which every call to it in a table's
    /// script reaches.
    Fence,
    /// VPinMAME's controller, its settings, and the games and
    /// game settings scripts ask it for, by their order of asking.
    Controller,
    ControllerSettings,
    Game(u32),
    GameSettings(u32),
    /// B2S.Server: the backglass server, which is the controller too.
    B2sServer,
    /// PinUP Player's `PinDisplay`.
    PupDisplay,
}

/// The first id given to the enum constants on the global object (VPX's
/// own build outside Windows gives them ids from 2000).
const CONSTANT_BASE: i32 = 0x7000_0000;

#[repr(C)]
pub struct Obj {
    vtbl: &'static IDispatchVtbl,
    refs: Cell<u32>,
    pub target: Target,
    pub iface: &'static Interface,
}

impl Obj {
    /// A new object, owned by the caller until freed with `free`.
    pub fn new(target: Target, iface: &'static Interface) -> *mut Obj {
        Box::into_raw(Box::new(Obj { vtbl: &OBJ_VTBL, refs: Cell::new(1), target, iface }))
    }

    /// The object as the script engine sees it.
    pub fn dispatch(p: *mut Obj) -> *mut IDispatch {
        p.cast()
    }

    /// Frees an object made by `new`, once the script engine is closed.
    ///
    /// # Safety
    /// `p` must come from `new` and not be used again.
    pub unsafe fn free(p: *mut Obj) {
        unsafe { drop(Box::from_raw(p)) };
    }

    /// The object behind a script value, if it is one of ours.
    ///
    /// # Safety
    /// `p` must be a live COM object.
    pub unsafe fn ours(p: *mut IDispatch) -> Option<&'static Obj> {
        unsafe {
            let o = p as *const Obj;
            (std::ptr::eq((*o).vtbl, &OBJ_VTBL)).then(|| &*o)
        }
    }
}

static OBJ_VTBL: IDispatchVtbl = IDispatchVtbl {
    base: IUnknownVtbl { QueryInterface: query_interface, AddRef: add_ref, Release: release },
    GetTypeInfoCount: get_type_info_count,
    GetTypeInfo: get_type_info,
    GetIDsOfNames: get_ids_of_names,
    Invoke: invoke,
};

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
    let o = unsafe { &*(this as *const Obj) };
    o.refs.set(o.refs.get() + 1);
    o.refs.get()
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let o = unsafe { &*(this as *const Obj) };
    o.refs.set(o.refs.get().saturating_sub(1).max(1)); // freed only when play ends
    o.refs.get()
}

unsafe extern "system" fn get_type_info_count(_this: *mut c_void, count: *mut u32) -> HRESULT {
    if !count.is_null() {
        unsafe { *count = 1 };
    }
    S_OK
}

/// What `TypeName` answers for an object, as on Windows, where VBScript
/// names an object by the class it reports: VPX's parts, table and
/// collections report their class ("Kicker", "Wall", "Table"), but the
/// ball reports its interface ("IBall") — `IProvideClassInfo2Impl` is given
/// `CLSID_Kicker` for a kicker and `IID_IBall` for the ball. Scripts rely
/// on both: `core.vbs` takes only a "Kicker" for a trough's exit, and
/// flipper corrections forget a ball when they see "IBall". libwinevbs's
/// `TypeName` asks only the type information, so that answers it.
fn type_name(o: &Obj) -> &'static str {
    match o.target {
        Target::Ball(_) => o.iface.name,
        // VPinMAME's controller is the class `Controller`
        Target::Controller => "Controller",
        // B2S.Server is the class `Server`
        Target::B2sServer => "Server",
        // PinUpPlayer.PinDisplay is the class `PinDisplay`
        Target::PupDisplay => "PinDisplay",
        _ => CLASSES.iter().find(|c| c.interface.is_some_and(|i| std::ptr::eq(i, o.iface))).map_or(o.iface.name, |c| c.name),
    }
}

/// The object's type information: only its name, which is what `TypeName`
/// answers ([`type_name`]).
unsafe extern "system" fn get_type_info(this: *mut c_void, index: u32, _lcid: u32, info: *mut *mut c_void) -> HRESULT {
    if info.is_null() {
        return E_POINTER;
    }
    unsafe {
        if index != 0 {
            *info = std::ptr::null_mut();
            return DISP_E_BADINDEX;
        }
        let o = &*(this as *const Obj);
        *info = Box::into_raw(Box::new(TypeInfo { vtbl: &TYPE_INFO_VTBL, refs: Cell::new(1), name: type_name(o) })).cast();
    }
    S_OK
}

/// Just enough type information for `TypeName`: the interface's name.
#[repr(C)]
struct TypeInfo {
    vtbl: &'static ITypeInfoVtbl,
    refs: Cell<u32>,
    name: &'static str,
}

static TYPE_INFO_VTBL: ITypeInfoVtbl = ITypeInfoVtbl {
    base: IUnknownVtbl { QueryInterface: type_info_query_interface, AddRef: type_info_add_ref, Release: type_info_release },
    GetTypeAttr: not_implemented,
    GetTypeComp: not_implemented,
    GetFuncDesc: not_implemented,
    GetVarDesc: not_implemented,
    GetNames: not_implemented,
    GetRefTypeOfImplType: not_implemented,
    GetImplTypeFlags: not_implemented,
    GetIDsOfNames: not_implemented,
    Invoke: not_implemented,
    GetDocumentation: type_info_documentation,
    GetDllEntry: not_implemented,
    GetRefTypeInfo: not_implemented,
    AddressOfMember: not_implemented,
    CreateInstance: not_implemented,
    GetMops: not_implemented,
    GetContainingTypeLib: not_implemented,
    ReleaseTypeAttr: not_implemented,
    ReleaseFuncDesc: not_implemented,
    ReleaseVarDesc: not_implemented,
};

unsafe extern "system" fn not_implemented(_this: *mut c_void) -> HRESULT {
    E_NOTIMPL
}

unsafe extern "system" fn type_info_query_interface(this: *mut c_void, riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    unsafe {
        if out.is_null() {
            return E_POINTER;
        }
        if *riid == IID_IUNKNOWN || *riid == IID_ITYPEINFO {
            type_info_add_ref(this);
            *out = this;
            S_OK
        } else {
            *out = std::ptr::null_mut();
            E_NOINTERFACE
        }
    }
}

unsafe extern "system" fn type_info_add_ref(this: *mut c_void) -> u32 {
    let t = unsafe { &*(this as *const TypeInfo) };
    t.refs.set(t.refs.get() + 1);
    t.refs.get()
}

unsafe extern "system" fn type_info_release(this: *mut c_void) -> u32 {
    let t = unsafe { &*(this as *const TypeInfo) };
    let n = t.refs.get().saturating_sub(1);
    t.refs.set(n);
    if n == 0 {
        // SAFETY: the last reference is gone; it was made by `get_type_info`
        unsafe { drop(Box::from_raw(this as *mut TypeInfo)) };
    }
    n
}

/// `GetDocumentation` for the type itself: its name; nothing else.
unsafe extern "system" fn type_info_documentation(
    this: *mut c_void,
    memid: i32,
    name: *mut *mut u16,
    doc: *mut *mut u16,
    help_context: *mut u32,
    help_file: *mut *mut u16,
) -> HRESULT {
    unsafe {
        for p in [name, doc, help_file] {
            if !p.is_null() {
                *p = std::ptr::null_mut();
            }
        }
        if !help_context.is_null() {
            *help_context = 0;
        }
        if memid != MEMBERID_NIL {
            return TYPE_E_ELEMENTNOTFOUND;
        }
        let Ok(lib) = winevbs::get() else { return E_FAIL };
        if !name.is_null() {
            *name = lib.bstr((*(this as *const TypeInfo)).name);
        }
    }
    S_OK
}

unsafe extern "system" fn get_ids_of_names(this: *mut c_void, _riid: *const GUID, names: *const *const u16, count: u32, _lcid: u32, ids: *mut i32) -> HRESULT {
    unsafe {
        let o = &*(this as *const Obj);
        for i in 0..count as usize {
            *ids.add(i) = DISPID_UNKNOWN;
        }
        if count == 0 {
            return S_OK;
        }
        let name = from_wide(*names);
        if let Some(m) = o.iface.find(&name).next() {
            *ids = m.dispid;
            return if count > 1 { DISP_E_UNKNOWNNAME } else { S_OK };
        }
        if o.target == Target::Global
            && let Some(k) = CONSTANTS.iter().position(|(n, _)| n.eq_ignore_ascii_case(&name))
        {
            *ids = CONSTANT_BASE + k as i32;
            return S_OK;
        }
        DISP_E_UNKNOWNNAME
    }
}

/// Which member a call means: an assignment is a `Put` (or `PutRef` for
/// `Set`); a read or a call finds the property reader first, then the method.
fn member_for(iface: &'static Interface, dispid: i32, flags: u16) -> Option<&'static Member> {
    if flags & DISPATCH_PROPERTYPUTREF != 0 {
        return iface.by_id(dispid, Access::PutRef).or_else(|| iface.by_id(dispid, Access::Put));
    }
    if flags & DISPATCH_PROPERTYPUT != 0 {
        return iface.by_id(dispid, Access::Put).or_else(|| iface.by_id(dispid, Access::PutRef));
    }
    iface.by_id(dispid, Access::Get).or_else(|| iface.by_id(dispid, Access::Method))
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
        let o = &*(this as *const Obj);
        if o.target == Target::Global && dispid >= CONSTANT_BASE {
            let Some((_, v)) = CONSTANTS.get((dispid - CONSTANT_BASE) as usize) else { return DISP_E_MEMBERNOTFOUND };
            write(lib, Ret::Int(*v), result);
            return S_OK;
        }
        let Some(member) = member_for(o.iface, dispid, flags) else { return DISP_E_MEMBERNOTFOUND };
        let empty = DISPPARAMS { rgvarg: std::ptr::null_mut(), rgdispidNamedArgs: std::ptr::null_mut(), cArgs: 0, cNamedArgs: 0 };
        let params = if params.is_null() { &empty } else { &*params };
        let args = Args::new(lib, params);
        match super::api::invoke(o.target, o.iface, member, &args) {
            Ok(ret) => {
                write(lib, ret, result);
                S_OK
            }
            Err(hr) => hr,
        }
    }
}

/// The object `For Each` walks through: the items of a collection.
#[repr(C)]
pub struct Enumerator {
    vtbl: &'static IEnumVARIANTVtbl,
    refs: Cell<u32>,
    items: Vec<*mut IDispatch>,
    at: Cell<usize>,
}

impl Enumerator {
    /// A new enumerator with one reference, which the caller hands on.
    pub fn create(items: Vec<*mut IDispatch>) -> *mut c_void {
        Box::into_raw(Box::new(Enumerator { vtbl: &ENUM_VTBL, refs: Cell::new(1), items, at: Cell::new(0) })).cast()
    }
}

static ENUM_VTBL: IEnumVARIANTVtbl = IEnumVARIANTVtbl {
    base: IUnknownVtbl { QueryInterface: enum_query_interface, AddRef: enum_add_ref, Release: enum_release },
    Next: enum_next,
    Skip: enum_skip,
    Reset: enum_reset,
    Clone: enum_clone,
};

unsafe extern "system" fn enum_query_interface(this: *mut c_void, riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    unsafe {
        if out.is_null() {
            return E_POINTER;
        }
        if *riid == IID_IUNKNOWN || *riid == IID_IENUMVARIANT {
            enum_add_ref(this);
            *out = this;
            S_OK
        } else {
            *out = std::ptr::null_mut();
            E_NOINTERFACE
        }
    }
}

unsafe extern "system" fn enum_add_ref(this: *mut c_void) -> u32 {
    let e = unsafe { &*(this as *const Enumerator) };
    e.refs.set(e.refs.get() + 1);
    e.refs.get()
}

unsafe extern "system" fn enum_release(this: *mut c_void) -> u32 {
    let e = unsafe { &*(this as *const Enumerator) };
    let n = e.refs.get().saturating_sub(1);
    e.refs.set(n);
    if n == 0 {
        // SAFETY: the last reference is gone; it was made by `Enumerator::new`
        unsafe { drop(Box::from_raw(this as *mut Enumerator)) };
    }
    n
}

unsafe extern "system" fn enum_next(this: *mut c_void, celt: u32, rgvar: *mut VARIANT, fetched: *mut u32) -> HRESULT {
    let Ok(lib) = winevbs::get() else { return E_FAIL };
    unsafe {
        let e = &*(this as *const Enumerator);
        let mut n = 0;
        while n < celt as usize && e.at.get() < e.items.len() {
            write(lib, Ret::Object(e.items[e.at.get()]), rgvar.add(n));
            e.at.set(e.at.get() + 1);
            n += 1;
        }
        if !fetched.is_null() {
            *fetched = n as u32;
        }
        if n == celt as usize { S_OK } else { S_FALSE }
    }
}

unsafe extern "system" fn enum_skip(this: *mut c_void, celt: u32) -> HRESULT {
    let e = unsafe { &*(this as *const Enumerator) };
    let to = (e.at.get() + celt as usize).min(e.items.len());
    let skipped = to - e.at.get();
    e.at.set(to);
    if skipped == celt as usize { S_OK } else { S_FALSE }
}

unsafe extern "system" fn enum_reset(this: *mut c_void) -> HRESULT {
    let e = unsafe { &*(this as *const Enumerator) };
    e.at.set(0);
    S_OK
}

unsafe extern "system" fn enum_clone(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    unsafe {
        let e = &*(this as *const Enumerator);
        let copy = Enumerator::create(e.items.clone());
        (*(copy as *const Enumerator)).at.set(e.at.get());
        *out = copy;
    }
    S_OK
}

/// The global object's interface: VPX's `ITableGlobal`.
pub fn global_interface() -> &'static Interface {
    &I_TABLE_GLOBAL
}
