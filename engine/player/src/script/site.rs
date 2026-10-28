//! The script site: what the script engine asks its host — the objects
//! behind the names it was given, and what to do with an error.

use std::collections::HashMap;
use std::ffi::c_void;

use super::com::*;
use super::object::Obj;
use super::winevbs;

#[repr(C)]
pub struct Site {
    vtbl: &'static IActiveScriptSiteVtbl,
    /// Named objects, by lowercase name.
    items: HashMap<String, *mut Obj>,
}

impl Site {
    pub fn new(items: HashMap<String, *mut Obj>) -> Box<Site> {
        Box::new(Site { vtbl: &SITE_VTBL, items })
    }
}

static SITE_VTBL: IActiveScriptSiteVtbl = IActiveScriptSiteVtbl {
    base: IUnknownVtbl { QueryInterface: query_interface, AddRef: add_ref, Release: release },
    GetLCID: get_lcid,
    GetItemInfo: get_item_info,
    GetDocVersionString: get_doc_version_string,
    OnScriptTerminate: on_script_terminate,
    OnStateChange: on_state_change,
    OnScriptError: on_script_error,
    OnEnterScript: on_enter_script,
    OnLeaveScript: on_leave_script,
};

unsafe extern "system" fn query_interface(this: *mut c_void, riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    unsafe {
        if out.is_null() {
            return E_POINTER;
        }
        if *riid == IID_IUNKNOWN || *riid == IID_IACTIVESCRIPTSITE {
            *out = this;
            S_OK
        } else {
            *out = std::ptr::null_mut();
            E_NOINTERFACE
        }
    }
}

// the site lives as long as the script engine: references aren't counted
unsafe extern "system" fn add_ref(_this: *mut c_void) -> u32 {
    2
}

unsafe extern "system" fn release(_this: *mut c_void) -> u32 {
    1
}

unsafe extern "system" fn get_lcid(_this: *mut c_void, _lcid: *mut u32) -> HRESULT {
    E_NOTIMPL
}

unsafe extern "system" fn get_item_info(this: *mut c_void, name: *const u16, mask: u32, item: *mut *mut c_void, info: *mut *mut c_void) -> HRESULT {
    unsafe {
        let site = &*(this as *const Site);
        if mask & SCRIPTINFO_IUNKNOWN != 0 && !item.is_null() {
            *item = std::ptr::null_mut();
        }
        if mask & SCRIPTINFO_ITYPEINFO != 0 && !info.is_null() {
            *info = std::ptr::null_mut();
        }
        let Some(&obj) = site.items.get(&from_wide(name).to_lowercase()) else { return TYPE_E_ELEMENTNOTFOUND };
        if mask & SCRIPTINFO_IUNKNOWN != 0 && !item.is_null() {
            let unknown = obj as *mut c_void;
            let vt: &IUnknownVtbl = vtbl(unknown);
            (vt.AddRef)(unknown);
            *item = unknown;
        }
        S_OK
    }
}

unsafe extern "system" fn get_doc_version_string(_this: *mut c_void, _version: *mut *mut u16) -> HRESULT {
    E_NOTIMPL
}

unsafe extern "system" fn on_script_terminate(_this: *mut c_void, _result: *const VARIANT, _excep: *const EXCEPINFO) -> HRESULT {
    S_OK
}

unsafe extern "system" fn on_state_change(_this: *mut c_void, _state: i32) -> HRESULT {
    S_OK
}

unsafe extern "system" fn on_script_error(_this: *mut c_void, error: *mut c_void) -> HRESULT {
    let Ok(lib) = winevbs::get() else { return S_OK };
    unsafe {
        let vt: &IActiveScriptErrorVtbl = vtbl(error);
        let mut info = EXCEPINFO::default();
        (vt.GetExceptionInfo)(error, &mut info);
        let (mut context, mut line, mut column) = (0u32, 0u32, 0i32);
        (vt.GetSourcePosition)(error, &mut context, &mut line, &mut column);
        let mut text: *mut u16 = std::ptr::null_mut();
        (vt.GetSourceLineText)(error, &mut text);
        let description = if info.bstrDescription.is_null() {
            format!("error {:#x}", info.scode)
        } else {
            lib.bstr_text(info.bstrDescription)
        };
        super::log::script_error(line + 1, column, &description, &lib.bstr_text(text));
        for b in [info.bstrSource, info.bstrDescription, info.bstrHelpFile, text] {
            if !b.is_null() {
                (lib.sys_free_string)(b);
            }
        }
    }
    S_OK
}

unsafe extern "system" fn on_enter_script(_this: *mut c_void) -> HRESULT {
    S_OK
}

unsafe extern "system" fn on_leave_script(_this: *mut c_void) -> HRESULT {
    S_OK
}
