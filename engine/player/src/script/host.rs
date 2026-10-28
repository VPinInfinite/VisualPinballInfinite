//! One table's script, running in libwinevbs: started the way VPX's
//! `ScriptInterpreter` starts it, given the table's objects by name, and
//! called by name when something happens.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};

use super::com::*;
use super::object::Obj;
use super::site::Site;
use super::value::{Ret, write};
use super::winevbs::{self, WineVbs};

pub struct ScriptHost {
    lib: &'static WineVbs,
    script: *mut c_void,
    parse: *mut c_void,
    _site: Box<Site>,
    dispatch: *mut IDispatch,
    /// Script routines' ids, by lowercase name; `None` where the script has
    /// no such routine (looked up again after any `ExecuteGlobal`).
    ids: RefCell<HashMap<String, Option<i32>>>,
}

/// One named object: its name, the object, and whether its members are
/// global (the `Global` object's are).
pub struct Item {
    pub name: String,
    pub obj: *mut Obj,
    pub global: bool,
}

impl ScriptHost {
    /// Starts a script engine with the table's objects, as VPX's
    /// `ScriptInterpreter::Start` does.
    pub fn new(items: &[Item]) -> Result<ScriptHost, String> {
        let lib = winevbs::get()?;
        let site = Site::new(items.iter().map(|i| (i.name.to_lowercase(), i.obj)).collect());
        unsafe {
            let mut parse: *mut c_void = std::ptr::null_mut();
            let hr = (lib.co_create_instance)(&CLSID_VBSCRIPT, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, &IID_IACTIVESCRIPTPARSE64, &mut parse);
            if failed(hr) || parse.is_null() {
                return Err(format!("could not start the VBScript engine ({hr:#x})"));
            }
            let unknown: &IUnknownVtbl = vtbl(parse);
            let mut script: *mut c_void = std::ptr::null_mut();
            (unknown.QueryInterface)(parse, &IID_IACTIVESCRIPT, &mut script);
            if script.is_null() {
                (unknown.Release)(parse);
                return Err("the VBScript engine has no IActiveScript".into());
            }
            let host = ScriptHost {
                lib,
                script,
                parse,
                _site: site,
                dispatch: std::ptr::null_mut(),
                ids: RefCell::new(HashMap::new()),
            };
            let s: &IActiveScriptVtbl = vtbl(script);
            let p: &IActiveScriptParse64Vtbl = vtbl(parse);
            (s.SetScriptSite)(script, (&*host._site as *const Site).cast_mut().cast());
            (p.InitNew)(parse);
            (s.SetScriptState)(script, SCRIPTSTATE_INITIALIZED);
            for item in items {
                let mut flags = SCRIPTITEM_ISVISIBLE;
                if item.global {
                    flags |= SCRIPTITEM_GLOBALMEMBERS;
                }
                (s.AddNamedItem)(script, wide(&item.name).as_ptr(), flags);
            }
            Ok(host)
        }
    }

    /// Runs the table's script (its top-level code runs now), then connects
    /// it, as VPX's `Evaluate` does.
    pub fn load(&mut self, code: &str) -> Result<(), String> {
        unsafe {
            let p: &IActiveScriptParse64Vtbl = vtbl(self.parse);
            let s: &IActiveScriptVtbl = vtbl(self.script);
            let mut excep = EXCEPINFO::default();
            let hr = (p.ParseScriptText)(
                self.parse,
                wide(code).as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                1000, // VPX's compile context cookie
                0,
                SCRIPTTEXT_ISVISIBLE,
                std::ptr::null_mut(),
                &mut excep,
            );
            (s.SetScriptState)(self.script, SCRIPTSTATE_CONNECTED);
            let mut dispatch: *mut IDispatch = std::ptr::null_mut();
            (s.GetScriptDispatch)(self.script, std::ptr::null(), &mut dispatch);
            self.dispatch = dispatch;
            if failed(hr) { Err(format!("the table's script did not compile ({hr:#x})")) } else { Ok(()) }
        }
    }

    /// Evaluates an expression and hands back its value, which the caller
    /// then owns (the fence's own engine, which makes libwinevbs's objects).
    pub fn evaluate(&self, expression: &str) -> Option<VARIANT> {
        unsafe {
            let p: &IActiveScriptParse64Vtbl = vtbl(self.parse);
            let mut result = VARIANT::default();
            let mut excep = EXCEPINFO::default();
            let hr = (p.ParseScriptText)(
                self.parse,
                wide(expression).as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
                0,
                SCRIPTTEXT_ISEXPRESSION,
                &mut result,
                &mut excep,
            );
            for b in [excep.bstrSource, excep.bstrDescription, excep.bstrHelpFile] {
                if !b.is_null() {
                    (self.lib.sys_free_string)(b);
                }
            }
            if failed(hr) {
                (self.lib.variant_clear)(&mut result);
                None
            } else {
                Some(result)
            }
        }
    }

    /// The id of a script routine, if the script has one by that name.
    fn id_of(&self, name: &str) -> Option<i32> {
        if self.dispatch.is_null() {
            return None;
        }
        let key = name.to_lowercase();
        if let Some(&id) = self.ids.borrow().get(&key) {
            return id;
        }
        let w = wide(name);
        let names = [w.as_ptr()];
        let mut id = DISPID_UNKNOWN;
        let found = unsafe {
            let vt: &IDispatchVtbl = vtbl(self.dispatch.cast());
            !failed((vt.GetIDsOfNames)(self.dispatch.cast(), &IID_IUNKNOWN, names.as_ptr(), 1, 0, &mut id))
        };
        let id = found.then_some(id);
        // routines a script adds later (ExecuteGlobal) are found on the next call
        if id.is_some() {
            self.ids.borrow_mut().insert(key, id);
        }
        id
    }

    /// Whether the script has a routine by this name.
    pub fn has(&self, name: &str) -> bool {
        self.id_of(name).is_some()
    }

    /// Calls a script routine, as VPX fires an event: by name, with these
    /// arguments. Returns false if the script has no such routine.
    pub fn call(&self, name: &str, args: Vec<Ret>) -> bool {
        let Some(id) = self.id_of(name) else { return false };
        CALLS.fetch_add(1, Ordering::Relaxed);
        unsafe {
            // arguments go in reverse order
            let mut vars: Vec<VARIANT> = args
                .into_iter()
                .rev()
                .map(|a| {
                    let mut v = VARIANT::default();
                    write(self.lib, a, &mut v);
                    v
                })
                .collect();
            let mut params =
                DISPPARAMS { rgvarg: vars.as_mut_ptr(), rgdispidNamedArgs: std::ptr::null_mut(), cArgs: vars.len() as u32, cNamedArgs: 0 };
            let mut result = VARIANT::default();
            let mut excep = EXCEPINFO::default();
            let mut arg_err = 0u32;
            let vt: &IDispatchVtbl = vtbl(self.dispatch.cast());
            (vt.Invoke)(self.dispatch.cast(), id, &IID_IUNKNOWN, 0, DISPATCH_METHOD, &mut params, &mut result, &mut excep, &mut arg_err);
            (self.lib.variant_clear)(&mut result);
            for v in &mut vars {
                (self.lib.variant_clear)(v);
            }
            for b in [excep.bstrSource, excep.bstrDescription, excep.bstrHelpFile] {
                if !b.is_null() {
                    (self.lib.sys_free_string)(b);
                }
            }
        }
        true
    }
}

impl Drop for ScriptHost {
    fn drop(&mut self) {
        unsafe {
            if !self.dispatch.is_null() {
                let vt: &IUnknownVtbl = vtbl(self.dispatch.cast());
                (vt.Release)(self.dispatch.cast());
            }
            let s: &IActiveScriptVtbl = vtbl(self.script);
            (s.Close)(self.script);
            (s.base.Release)(self.script);
            let p: &IUnknownVtbl = vtbl(self.parse);
            (p.Release)(self.parse);
        }
    }
}

/// Calls into the script so far, each event or timer one: what the script
/// asks between two of them, it asks within one call.
static CALLS: AtomicU64 = AtomicU64::new(0);

/// How many calls into the script have been made.
pub fn calls() -> u64 {
    CALLS.load(Ordering::Relaxed)
}
