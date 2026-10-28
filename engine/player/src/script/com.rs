//! The COM types a VBScript engine and its host exchange: `IUnknown`,
//! `IDispatch`, the Active Scripting interfaces, `VARIANT` and friends,
//! declared by hand with the Windows layout (x64).

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;

pub type HRESULT = i32;

pub const S_OK: HRESULT = 0;
pub const S_FALSE: HRESULT = 1;
pub const E_NOTIMPL: HRESULT = 0x8000_4001_u32 as i32;
pub const E_NOINTERFACE: HRESULT = 0x8000_4002_u32 as i32;
pub const E_POINTER: HRESULT = 0x8000_4003_u32 as i32;
pub const E_FAIL: HRESULT = 0x8000_4005_u32 as i32;
pub const DISP_E_MEMBERNOTFOUND: HRESULT = 0x8002_0003_u32 as i32;
pub const DISP_E_PARAMNOTFOUND: HRESULT = 0x8002_0004_u32 as i32;
pub const DISP_E_TYPEMISMATCH: HRESULT = 0x8002_0005_u32 as i32;
pub const DISP_E_UNKNOWNNAME: HRESULT = 0x8002_0006_u32 as i32;
pub const DISP_E_EXCEPTION: HRESULT = 0x8002_0009_u32 as i32;
pub const DISP_E_BADPARAMCOUNT: HRESULT = 0x8002_000E_u32 as i32;
pub const TYPE_E_ELEMENTNOTFOUND: HRESULT = 0x8002_802B_u32 as i32;
pub const REGDB_E_CLASSNOTREG: HRESULT = 0x8004_0154_u32 as i32;

pub fn failed(hr: HRESULT) -> bool {
    hr < 0
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub const IID_IUNKNOWN: GUID = GUID { data1: 0x0000_0000, data2: 0, data3: 0, data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46] };
pub const IID_IDISPATCH: GUID = GUID { data1: 0x0002_0400, data2: 0, data3: 0, data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46] };
pub const IID_IACTIVESCRIPT: GUID = GUID { data1: 0xBB1A_2AE1, data2: 0xA4F9, data3: 0x11CF, data4: [0x8F, 0x20, 0x00, 0x80, 0x5F, 0x2C, 0xD0, 0x64] };
pub const IID_IACTIVESCRIPTPARSE64: GUID = GUID { data1: 0xC7EF_7658, data2: 0xE1EE, data3: 0x480E, data4: [0x97, 0xEA, 0xD5, 0x2C, 0xB4, 0xD7, 0x6D, 0x17] };
pub const IID_IACTIVESCRIPTSITE: GUID = GUID { data1: 0xDB01_A1E3, data2: 0xA42B, data3: 0x11CF, data4: [0x8F, 0x20, 0x00, 0x80, 0x5F, 0x2C, 0xD0, 0x64] };
pub const CLSID_VBSCRIPT: GUID = GUID { data1: 0xB54F_3741, data2: 0x5B07, data3: 0x11CF, data4: [0xA4, 0xB0, 0x00, 0xAA, 0x00, 0x4A, 0x55, 0xE8] };

pub const CLSCTX_INPROC_SERVER: u32 = 0x1;

// VARIANT types
pub const VT_EMPTY: u16 = 0;
pub const VT_NULL: u16 = 1;
pub const VT_I2: u16 = 2;
pub const VT_I4: u16 = 3;
pub const VT_R4: u16 = 4;
pub const VT_R8: u16 = 5;
pub const VT_CY: u16 = 6;
pub const VT_DATE: u16 = 7;
pub const VT_BSTR: u16 = 8;
pub const VT_DISPATCH: u16 = 9;
pub const VT_ERROR: u16 = 10;
pub const VT_BOOL: u16 = 11;
pub const VT_VARIANT: u16 = 12;
pub const VT_UNKNOWN: u16 = 13;
pub const VT_I1: u16 = 16;
pub const VT_UI1: u16 = 17;
pub const VT_UI2: u16 = 18;
pub const VT_UI4: u16 = 19;
pub const VT_I8: u16 = 20;
pub const VT_UI8: u16 = 21;
pub const VT_INT: u16 = 22;
pub const VT_UINT: u16 = 23;
pub const VT_ARRAY: u16 = 0x2000;
pub const VT_BYREF: u16 = 0x4000;

pub const VARIANT_TRUE: i16 = -1;
pub const VARIANT_FALSE: i16 = 0;

/// One dimension of a `SAFEARRAY`: how many elements, from which index.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SAFEARRAYBOUND {
    pub elements: u32,
    pub lower_bound: i32,
}

/// A VARIANT: a type tag and 16 bytes of value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VARIANT {
    pub vt: u16,
    pub reserved: [u16; 3],
    pub data: [u64; 2],
}

impl Default for VARIANT {
    fn default() -> Self {
        VARIANT { vt: VT_EMPTY, reserved: [0; 3], data: [0; 2] }
    }
}

impl VARIANT {
    pub fn i32(&self) -> i32 {
        self.data[0] as u32 as i32
    }
    pub fn i16(&self) -> i16 {
        self.data[0] as u16 as i16
    }
    pub fn f32(&self) -> f32 {
        f32::from_bits(self.data[0] as u32)
    }
    pub fn f64(&self) -> f64 {
        f64::from_bits(self.data[0])
    }
    pub fn ptr<T>(&self) -> *mut T {
        self.data[0] as usize as *mut T
    }
    pub fn set_ptr<T>(&mut self, vt: u16, p: *mut T) {
        self.vt = vt;
        self.data = [p as usize as u64, 0];
    }
}

#[repr(C)]
pub struct DISPPARAMS {
    pub rgvarg: *mut VARIANT,
    pub rgdispidNamedArgs: *mut i32,
    pub cArgs: u32,
    pub cNamedArgs: u32,
}

#[repr(C)]
pub struct EXCEPINFO {
    pub wCode: u16,
    pub wReserved: u16,
    pub bstrSource: *mut u16,
    pub bstrDescription: *mut u16,
    pub bstrHelpFile: *mut u16,
    pub dwHelpContext: u32,
    pub pvReserved: *mut c_void,
    pub pfnDeferredFillIn: *mut c_void,
    pub scode: i32,
}

impl Default for EXCEPINFO {
    fn default() -> Self {
        EXCEPINFO {
            wCode: 0,
            wReserved: 0,
            bstrSource: std::ptr::null_mut(),
            bstrDescription: std::ptr::null_mut(),
            bstrHelpFile: std::ptr::null_mut(),
            dwHelpContext: 0,
            pvReserved: std::ptr::null_mut(),
            pfnDeferredFillIn: std::ptr::null_mut(),
            scode: 0,
        }
    }
}

pub const DISPATCH_METHOD: u16 = 0x1;
pub const DISPATCH_PROPERTYGET: u16 = 0x2;
pub const DISPATCH_PROPERTYPUT: u16 = 0x4;
pub const DISPATCH_PROPERTYPUTREF: u16 = 0x8;
pub const DISPID_UNKNOWN: i32 = -1;
pub const DISPID_PROPERTYPUT: i32 = -3;
pub const DISPID_NEWENUM: i32 = -4;

#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

#[repr(C)]
pub struct IDispatchVtbl {
    pub base: IUnknownVtbl,
    pub GetTypeInfoCount: unsafe extern "system" fn(this: *mut c_void, count: *mut u32) -> HRESULT,
    pub GetTypeInfo: unsafe extern "system" fn(this: *mut c_void, index: u32, lcid: u32, info: *mut *mut c_void) -> HRESULT,
    pub GetIDsOfNames:
        unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, names: *const *const u16, count: u32, lcid: u32, ids: *mut i32) -> HRESULT,
    pub Invoke: unsafe extern "system" fn(
        this: *mut c_void,
        dispid: i32,
        riid: *const GUID,
        lcid: u32,
        flags: u16,
        params: *mut DISPPARAMS,
        result: *mut VARIANT,
        excep: *mut EXCEPINFO,
        arg_err: *mut u32,
    ) -> HRESULT,
}

/// Any COM object: a pointer to its function table.
#[repr(C)]
pub struct IUnknown {
    pub vtbl: *const IUnknownVtbl,
}

#[repr(C)]
pub struct IDispatch {
    pub vtbl: *const IDispatchVtbl,
}

#[repr(C)]
pub struct IEnumVARIANTVtbl {
    pub base: IUnknownVtbl,
    pub Next: unsafe extern "system" fn(this: *mut c_void, celt: u32, rgvar: *mut VARIANT, fetched: *mut u32) -> HRESULT,
    pub Skip: unsafe extern "system" fn(this: *mut c_void, celt: u32) -> HRESULT,
    pub Reset: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub Clone: unsafe extern "system" fn(this: *mut c_void, out: *mut *mut c_void) -> HRESULT,
}

pub const IID_IENUMVARIANT: GUID = GUID { data1: 0x0002_0404, data2: 0, data3: 0, data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46] };

/// `ITypeInfo`'s methods after `IUnknown`'s, in order. Only
/// `GetDocumentation` is spelled out; the rest are typed by their first
/// argument alone, as our objects only ever answer them "not implemented"
/// (on 64-bit Windows the caller clears the arguments, so the rest are
/// simply not read).
#[repr(C)]
pub struct ITypeInfoVtbl {
    pub base: IUnknownVtbl,
    pub GetTypeAttr: NotImplFn,
    pub GetTypeComp: NotImplFn,
    pub GetFuncDesc: NotImplFn,
    pub GetVarDesc: NotImplFn,
    pub GetNames: NotImplFn,
    pub GetRefTypeOfImplType: NotImplFn,
    pub GetImplTypeFlags: NotImplFn,
    pub GetIDsOfNames: NotImplFn,
    pub Invoke: NotImplFn,
    pub GetDocumentation:
        unsafe extern "system" fn(this: *mut c_void, memid: i32, name: *mut *mut u16, doc: *mut *mut u16, help_context: *mut u32, help_file: *mut *mut u16) -> HRESULT,
    pub GetDllEntry: NotImplFn,
    pub GetRefTypeInfo: NotImplFn,
    pub AddressOfMember: NotImplFn,
    pub CreateInstance: NotImplFn,
    pub GetMops: NotImplFn,
    pub GetContainingTypeLib: NotImplFn,
    pub ReleaseTypeAttr: NotImplFn,
    pub ReleaseFuncDesc: NotImplFn,
    pub ReleaseVarDesc: NotImplFn,
}

pub type NotImplFn = unsafe extern "system" fn(this: *mut c_void) -> HRESULT;

pub const IID_ITYPEINFO: GUID = GUID { data1: 0x0002_0401, data2: 0, data3: 0, data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46] };
pub const MEMBERID_NIL: i32 = -1;
pub const DISP_E_BADINDEX: HRESULT = 0x8002_000B_u32 as i32;

// Active Scripting

pub const SCRIPTITEM_ISVISIBLE: u32 = 0x2;
pub const SCRIPTITEM_ISSOURCE: u32 = 0x4;
pub const SCRIPTITEM_GLOBALMEMBERS: u32 = 0x8;
pub const SCRIPTINFO_IUNKNOWN: u32 = 0x1;
pub const SCRIPTINFO_ITYPEINFO: u32 = 0x2;
pub const SCRIPTTEXT_ISVISIBLE: u32 = 0x2;
pub const SCRIPTTEXT_ISEXPRESSION: u32 = 0x20;
/// The default member of an object: what calling the object itself calls.
pub const DISPID_VALUE: i32 = 0;
pub const SCRIPTSTATE_STARTED: i32 = 1;
pub const SCRIPTSTATE_CONNECTED: i32 = 2;
pub const SCRIPTSTATE_INITIALIZED: i32 = 5;

#[repr(C)]
pub struct IActiveScriptVtbl {
    pub base: IUnknownVtbl,
    pub SetScriptSite: unsafe extern "system" fn(this: *mut c_void, site: *mut c_void) -> HRESULT,
    pub GetScriptSite: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub SetScriptState: unsafe extern "system" fn(this: *mut c_void, state: i32) -> HRESULT,
    pub GetScriptState: unsafe extern "system" fn(this: *mut c_void, state: *mut i32) -> HRESULT,
    pub Close: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub AddNamedItem: unsafe extern "system" fn(this: *mut c_void, name: *const u16, flags: u32) -> HRESULT,
    pub AddTypeLib: unsafe extern "system" fn(this: *mut c_void, guid: *const GUID, major: u32, minor: u32, flags: u32) -> HRESULT,
    pub GetScriptDispatch: unsafe extern "system" fn(this: *mut c_void, item: *const u16, out: *mut *mut IDispatch) -> HRESULT,
    pub GetCurrentScriptThreadID: unsafe extern "system" fn(this: *mut c_void, id: *mut u32) -> HRESULT,
    pub GetScriptThreadID: unsafe extern "system" fn(this: *mut c_void, win32: u32, id: *mut u32) -> HRESULT,
    pub GetScriptThreadState: unsafe extern "system" fn(this: *mut c_void, id: u32, state: *mut i32) -> HRESULT,
    pub InterruptScriptThread: unsafe extern "system" fn(this: *mut c_void, id: u32, excep: *const EXCEPINFO, flags: u32) -> HRESULT,
    pub Clone: unsafe extern "system" fn(this: *mut c_void, out: *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
pub struct IActiveScriptParse64Vtbl {
    pub base: IUnknownVtbl,
    pub InitNew: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub AddScriptlet: *const c_void,
    pub ParseScriptText: unsafe extern "system" fn(
        this: *mut c_void,
        code: *const u16,
        item_name: *const u16,
        context: *mut c_void,
        delimiter: *const u16,
        source_context: u64,
        starting_line: u32,
        flags: u32,
        result: *mut VARIANT,
        excep: *mut EXCEPINFO,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IActiveScriptSiteVtbl {
    pub base: IUnknownVtbl,
    pub GetLCID: unsafe extern "system" fn(this: *mut c_void, lcid: *mut u32) -> HRESULT,
    pub GetItemInfo: unsafe extern "system" fn(this: *mut c_void, name: *const u16, mask: u32, item: *mut *mut c_void, info: *mut *mut c_void) -> HRESULT,
    pub GetDocVersionString: unsafe extern "system" fn(this: *mut c_void, version: *mut *mut u16) -> HRESULT,
    pub OnScriptTerminate: unsafe extern "system" fn(this: *mut c_void, result: *const VARIANT, excep: *const EXCEPINFO) -> HRESULT,
    pub OnStateChange: unsafe extern "system" fn(this: *mut c_void, state: i32) -> HRESULT,
    pub OnScriptError: unsafe extern "system" fn(this: *mut c_void, error: *mut c_void) -> HRESULT,
    pub OnEnterScript: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub OnLeaveScript: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
}

#[repr(C)]
pub struct IActiveScriptErrorVtbl {
    pub base: IUnknownVtbl,
    pub GetExceptionInfo: unsafe extern "system" fn(this: *mut c_void, excep: *mut EXCEPINFO) -> HRESULT,
    pub GetSourcePosition: unsafe extern "system" fn(this: *mut c_void, context: *mut u32, line: *mut u32, column: *mut i32) -> HRESULT,
    pub GetSourceLineText: unsafe extern "system" fn(this: *mut c_void, text: *mut *mut u16) -> HRESULT,
}

#[repr(C)]
pub struct IClassFactoryVtbl {
    pub base: IUnknownVtbl,
    pub CreateInstance: unsafe extern "system" fn(this: *mut c_void, outer: *mut c_void, riid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub LockServer: unsafe extern "system" fn(this: *mut c_void, lock: i32) -> HRESULT,
}

/// Calls through a COM object's function table.
///
/// # Safety
/// `obj` must be a live COM object whose table starts with `V`.
pub unsafe fn vtbl<'a, V>(obj: *mut c_void) -> &'a V {
    unsafe { &**(obj as *const *const V) }
}

/// A UTF-16, NUL-terminated copy of a string, for passing to COM.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A NUL-terminated UTF-16 string from COM, as Rust text.
///
/// # Safety
/// `p` must be null or point to a NUL-terminated UTF-16 string.
pub unsafe fn from_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe {
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    }
}
