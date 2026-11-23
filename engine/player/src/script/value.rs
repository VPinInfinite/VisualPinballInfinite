//! Values crossing between the script and our objects: reading a call's
//! arguments out of COM's `DISPPARAMS`, and writing results back, converted
//! the way VBScript converts them (through libwinevbs's own functions).

use std::ffi::c_void;

use super::com::*;
use super::winevbs::WineVbs;

/// A result handed back to the script.
#[derive(Clone, Debug, Default)]
pub enum Ret {
    #[default]
    Empty,
    Bool(bool),
    Int(i32),
    Float(f32),
    Double(f64),
    Str(String),
    /// An object, which the result takes a reference to.
    Object(*mut IDispatch),
    /// An array of values (`GetBalls`), as VBScript's arrays are.
    Array(Vec<Ret>),
    /// A two-dimensional array, rows of equal length: VPinMAME's lists of
    /// changes, which scripts read as `ChgLamp(row, 0)` and `ChgLamp(row, 1)`.
    Rows(Vec<Vec<Ret>>),
    /// A new object whose one reference the result takes over (an enumerator).
    Unknown(*mut c_void),
    /// A new script object whose one reference the result takes over (one
    /// `CreateObject` made).
    NewObject(*mut IDispatch),
    /// A value kept from the script itself (a `UserValue`), copied out.
    Kept(Kept),
}

/// A VARIANT copied from the script and owned by us, freed when dropped.
#[derive(Debug)]
pub struct Kept(pub VARIANT);

impl Clone for Kept {
    fn clone(&self) -> Self {
        let mut v = VARIANT::default();
        if let Ok(lib) = super::winevbs::get() {
            // SAFETY: copying a VARIANT we own into a fresh one
            unsafe { (lib.variant_copy)(&mut v, &self.0) };
        }
        Kept(v)
    }
}

impl Kept {
    /// Whether the value reads as the number 0 (`Settings.Value("sound") = 0`).
    pub fn is_zero(&self) -> bool {
        self.number() == Some(0.0)
    }

    /// The value as a number, if it reads as one.
    pub fn number(&self) -> Option<f64> {
        super::winevbs::get().ok().and_then(|lib| number(lib, &self.0))
    }
}

impl Drop for Kept {
    fn drop(&mut self) {
        if let Ok(lib) = super::winevbs::get() {
            // SAFETY: a VARIANT we own
            unsafe { (lib.variant_clear)(&mut self.0) };
        }
    }
}

impl std::fmt::Debug for VARIANT {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VARIANT(vt {})", self.vt)
    }
}

/// A call's arguments, in the order the script wrote them.
pub struct Args<'a> {
    lib: &'static WineVbs,
    /// Pointers into the caller's `DISPPARAMS`, by-reference values followed.
    args: Vec<*const VARIANT>,
    /// Whether each argument came by reference: a variable the call may set.
    by_ref: Vec<bool>,
    /// The new value of a property assignment.
    pub put: Option<*const VARIANT>,
    _params: std::marker::PhantomData<&'a DISPPARAMS>,
}

/// Follows a by-reference VARIANT (`VT_BYREF | VT_VARIANT`) to its value.
///
/// # Safety
/// `v` must point to a live VARIANT.
unsafe fn deref(mut v: *const VARIANT) -> *const VARIANT {
    unsafe {
        while (*v).vt == VT_BYREF | VT_VARIANT {
            v = (*v).ptr::<VARIANT>();
        }
        v
    }
}

impl<'a> Args<'a> {
    /// # Safety
    /// `params` must be the caller's live `DISPPARAMS`.
    pub unsafe fn new(lib: &'static WineVbs, params: &'a DISPPARAMS) -> Args<'a> {
        unsafe {
            let n = params.cArgs as usize;
            let raw: Vec<*const VARIANT> = (0..n).map(|i| params.rgvarg.add(i) as *const VARIANT).collect();
            let all: Vec<*const VARIANT> = raw.iter().map(|&v| deref(v)).collect();
            let refs: Vec<bool> = raw.iter().map(|&v| (*v).vt == VT_BYREF | VT_VARIANT).collect();
            // a property assignment's value is the named argument DISPID_PROPERTYPUT, first in the array
            let named_put = params.cNamedArgs > 0 && !params.rgdispidNamedArgs.is_null() && *params.rgdispidNamedArgs == DISPID_PROPERTYPUT;
            let first = usize::from(named_put && n > 0);
            Args {
                lib,
                args: all[first..].iter().rev().copied().collect(),
                by_ref: refs[first..].iter().rev().copied().collect(),
                put: (first == 1).then(|| all[0]),
                _params: std::marker::PhantomData,
            }
        }
    }

    pub fn len(&self) -> usize {
        self.args.len()
    }

    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// Argument `i`, unless the script left it out.
    pub fn get(&self, i: usize) -> Option<&VARIANT> {
        // SAFETY: pointers into the caller's live DISPPARAMS
        let v = unsafe { &**self.args.get(i)? };
        let missing = v.vt == VT_ERROR && v.i32() == DISP_E_PARAMNOTFOUND;
        (!missing).then_some(v)
    }

    /// The value assigned to a property, or the last argument when the
    /// engine passed it positionally.
    pub fn value(&self) -> Option<&VARIANT> {
        // SAFETY: pointer into the caller's live DISPPARAMS
        match self.put {
            Some(p) => Some(unsafe { &*p }),
            None => self.get(self.len().checked_sub(1)?),
        }
    }

    pub fn f64(&self, i: usize) -> Option<f64> {
        self.get(i).and_then(|v| number(self.lib, v))
    }

    pub fn f32(&self, i: usize) -> Option<f32> {
        self.f64(i).map(|v| v as f32)
    }

    pub fn i32(&self, i: usize) -> Option<i32> {
        self.get(i).and_then(|v| integer(self.lib, v))
    }

    pub fn bool(&self, i: usize) -> Option<bool> {
        self.f64(i).map(|v| v != 0.0)
    }

    pub fn string(&self, i: usize) -> Option<String> {
        self.get(i).map(|v| text(self.lib, v))
    }

    pub fn value_f64(&self) -> Option<f64> {
        self.value().and_then(|v| number(self.lib, v))
    }

    pub fn value_f32(&self) -> Option<f32> {
        self.value().and_then(|v| number(self.lib, v)).map(|v| v as f32)
    }

    pub fn value_i32(&self) -> Option<i32> {
        self.value().and_then(|v| integer(self.lib, v))
    }

    pub fn value_bool(&self) -> Option<bool> {
        self.value().and_then(|v| number(self.lib, v)).map(|v| v != 0.0)
    }

    pub fn value_string(&self) -> Option<String> {
        self.value().map(|v| text(self.lib, v))
    }

    /// The assigned value as an array of whole numbers ([`array_u32`]).
    pub fn value_array_u32(&self) -> Option<Vec<u32>> {
        self.value().and_then(|v| array_u32(self.lib, v))
    }

    /// The assigned value, kept for reading back later (`UserValue`).
    pub fn value_kept(&self) -> Option<Kept> {
        let v = self.value()?;
        let mut out = VARIANT::default();
        // SAFETY: copying the caller's VARIANT into one we own
        unsafe { (self.lib.variant_copy_ind)(&mut out, v) };
        Some(Kept(out))
    }

    /// Sets argument `i` when the script passed a variable (by reference):
    /// VPX's out parameters (`GetMaterial`). Returns whether it could.
    pub fn set(&self, i: usize, ret: Ret) -> bool {
        if !self.by_ref.get(i).copied().unwrap_or(false) {
            return false;
        }
        let Some(&p) = self.args.get(i) else { return false };
        let p = p as *mut VARIANT;
        // SAFETY: a by-reference argument points at the caller's live variable
        unsafe {
            (self.lib.variant_clear)(p);
            write(self.lib, ret, p);
        }
        true
    }

    /// How many items argument `i` holds when it's a one-dimensional array.
    pub fn array_len(&self, i: usize) -> Option<usize> {
        let v = self.get(i)?;
        // SAFETY: reading the bounds of a SAFEARRAY the caller's VARIANT holds
        unsafe {
            let array = if v.vt & VT_BYREF != 0 { *v.ptr::<*mut c_void>() } else { v.ptr::<c_void>() };
            if v.vt & VT_ARRAY == 0 || array.is_null() || (self.lib.safe_array_get_dim)(array) != 1 {
                return None;
            }
            let (mut low, mut high) = (0i32, -1i32);
            if failed((self.lib.safe_array_get_lbound)(array, 1, &mut low)) || failed((self.lib.safe_array_get_ubound)(array, 1, &mut high)) {
                return None;
            }
            usize::try_from(high - low + 1).ok()
        }
    }

    /// Argument `i`'s items as text, when it's a one-dimensional array.
    pub fn array_texts(&self, i: usize) -> Option<Vec<String>> {
        self.get(i).and_then(|v| array_text(self.lib, v))
    }

    /// Argument `i` as an object, if it is one.
    pub fn object(&self, i: usize) -> Option<*mut IDispatch> {
        let v = self.get(i)?;
        match v.vt {
            VT_DISPATCH | VT_UNKNOWN => Some(v.ptr()).filter(|p: &*mut IDispatch| !p.is_null()),
            t if t == VT_BYREF | VT_DISPATCH => {
                // SAFETY: a by-reference object points to the object pointer
                let p = unsafe { *v.ptr::<*mut IDispatch>() };
                (!p.is_null()).then_some(p)
            }
            _ => None,
        }
    }
}

/// A VARIANT as a number, converted as VBScript converts.
pub fn number(lib: &WineVbs, v: &VARIANT) -> Option<f64> {
    Some(match v.vt {
        VT_EMPTY => 0.0,
        VT_I2 => f64::from(v.i16()),
        VT_I4 | VT_INT => f64::from(v.i32()),
        VT_R4 => f64::from(v.f32()),
        VT_R8 => v.f64(),
        VT_BOOL => f64::from(v.i16()),
        VT_UI1 => f64::from(v.data[0] as u8),
        _ => {
            let mut out = VARIANT::default();
            // SAFETY: converting into a fresh VARIANT with the library's rules
            unsafe {
                if failed((lib.variant_change_type)(&mut out, v, 0, VT_R8)) {
                    return None;
                }
            }
            out.f64()
        }
    })
}

/// A VARIANT as a whole number, rounded as VBScript's `CLng` rounds.
pub fn integer(lib: &WineVbs, v: &VARIANT) -> Option<i32> {
    match v.vt {
        VT_I2 => Some(i32::from(v.i16())),
        VT_I4 | VT_INT => Some(v.i32()),
        VT_BOOL => Some(i32::from(v.i16())),
        _ => {
            let mut out = VARIANT::default();
            // SAFETY: converting into a fresh VARIANT with the library's rules
            unsafe {
                if failed((lib.variant_change_type)(&mut out, v, 0, VT_I4)) {
                    return None;
                }
            }
            Some(out.i32())
        }
    }
}

/// A one-dimensional array the script passed (VBScript's arrays hold
/// VARIANTs; byte and whole-number arrays are read too), as whole numbers.
/// A colour's 32 bits read back as they were (`&HFFFFFF` stays positive).
pub fn array_u32(lib: &WineVbs, v: &VARIANT) -> Option<Vec<u32>> {
    // SAFETY: reading a SAFEARRAY the caller's VARIANT holds, by its own bounds
    unsafe {
        let (kind, array) = if v.vt & VT_BYREF != 0 {
            (v.vt & !VT_BYREF, *v.ptr::<*mut c_void>())
        } else {
            (v.vt, v.ptr::<c_void>())
        };
        if kind & VT_ARRAY == 0 || array.is_null() || (lib.safe_array_get_dim)(array) != 1 {
            return None;
        }
        let (mut low, mut high) = (0i32, -1i32);
        if failed((lib.safe_array_get_lbound)(array, 1, &mut low)) || failed((lib.safe_array_get_ubound)(array, 1, &mut high)) {
            return None;
        }
        let n = (high - low + 1).max(0) as usize;
        let mut data: *mut c_void = std::ptr::null_mut();
        if failed((lib.safe_array_access_data)(array, &mut data)) {
            return None;
        }
        let out = match kind & !VT_ARRAY {
            VT_VARIANT => (0..n).map(|i| integer(lib, &*(data as *const VARIANT).add(i)).unwrap_or(0) as u32).collect(),
            VT_UI1 => std::slice::from_raw_parts(data as *const u8, n).iter().map(|&b| u32::from(b)).collect(),
            VT_I4 | VT_UI4 | VT_INT | VT_UINT => std::slice::from_raw_parts(data as *const u32, n).to_vec(),
            _ => Vec::new(),
        };
        (lib.safe_array_unaccess_data)(array);
        Some(out)
    }
}

/// A one-dimensional array the script passed (of VARIANTs, as VBScript makes them, or of
/// strings), each item as text.
pub fn array_text(lib: &WineVbs, v: &VARIANT) -> Option<Vec<String>> {
    // SAFETY: reading a SAFEARRAY the caller's VARIANT holds, by its own bounds
    unsafe {
        let (kind, array) = if v.vt & VT_BYREF != 0 {
            (v.vt & !VT_BYREF, *v.ptr::<*mut c_void>())
        } else {
            (v.vt, v.ptr::<c_void>())
        };
        if kind & VT_ARRAY == 0 || array.is_null() || (lib.safe_array_get_dim)(array) != 1 {
            return None;
        }
        let (mut low, mut high) = (0i32, -1i32);
        if failed((lib.safe_array_get_lbound)(array, 1, &mut low)) || failed((lib.safe_array_get_ubound)(array, 1, &mut high)) {
            return None;
        }
        let n = (high - low + 1).max(0) as usize;
        let mut data: *mut c_void = std::ptr::null_mut();
        if failed((lib.safe_array_access_data)(array, &mut data)) {
            return None;
        }
        let out = match kind & !VT_ARRAY {
            VT_VARIANT => (0..n).map(|i| text(lib, &*(data as *const VARIANT).add(i))).collect(),
            VT_BSTR => (0..n).map(|i| lib.bstr_text(*(data as *const *mut u16).add(i))).collect(),
            _ => Vec::new(),
        };
        (lib.safe_array_unaccess_data)(array);
        Some(out)
    }
}

/// A VARIANT as text.
pub fn text(lib: &WineVbs, v: &VARIANT) -> String {
    // SAFETY: reading a BSTR, or converting into a fresh VARIANT and freeing it
    unsafe {
        if v.vt == VT_BSTR {
            return lib.bstr_text(v.ptr());
        }
        let mut out = VARIANT::default();
        if failed((lib.variant_change_type)(&mut out, v, 0, VT_BSTR)) {
            return String::new();
        }
        let s = lib.bstr_text(out.ptr());
        (lib.variant_clear)(&mut out);
        s
    }
}

/// Writes a result into the caller's VARIANT.
///
/// # Safety
/// `out` must be null or a VARIANT the caller expects to receive a result in.
pub unsafe fn write(lib: &WineVbs, ret: Ret, out: *mut VARIANT) {
    if out.is_null() {
        if let Ret::Object(p) = ret {
            // the caller doesn't want it: nothing to hand over
            let _ = p;
        }
        return;
    }
    unsafe {
        let o = &mut *out;
        *o = VARIANT::default();
        match ret {
            Ret::Empty => {}
            Ret::Bool(b) => {
                o.vt = VT_BOOL;
                o.data[0] = (if b { VARIANT_TRUE } else { VARIANT_FALSE }) as u16 as u64;
            }
            Ret::Int(i) => {
                o.vt = VT_I4;
                o.data[0] = i as u32 as u64;
            }
            Ret::Float(f) => {
                o.vt = VT_R4;
                o.data[0] = u64::from(f.to_bits());
            }
            Ret::Double(f) => {
                o.vt = VT_R8;
                o.data[0] = f.to_bits();
            }
            Ret::Str(s) => o.set_ptr(VT_BSTR, lib.bstr(&s)),
            Ret::Object(p) => {
                if p.is_null() {
                    o.set_ptr(VT_DISPATCH, p);
                } else {
                    let vt: &IUnknownVtbl = vtbl(p as *mut c_void);
                    (vt.AddRef)(p as *mut c_void);
                    o.set_ptr(VT_DISPATCH, p);
                }
            }
            Ret::Array(items) => {
                let array = (lib.safe_array_create_vector)(VT_VARIANT, 0, items.len() as u32);
                for (i, item) in items.into_iter().enumerate() {
                    let mut v = VARIANT::default();
                    write(lib, item, &mut v);
                    let index = i as i32;
                    (lib.safe_array_put_element)(array, &index, (&raw mut v).cast());
                    (lib.variant_clear)(&mut v);
                }
                o.set_ptr(VT_ARRAY | VT_VARIANT, array);
            }
            Ret::Rows(rows) => {
                // VPinMAME's layout: the rows are the first (left-most)
                // dimension, which a SAFEARRAY stores fastest-varying, so
                // element (row, column) sits at row + column × rows
                let (n, columns) = (rows.len(), rows.first().map_or(0, Vec::len));
                let bounds = [SAFEARRAYBOUND { elements: n as u32, lower_bound: 0 }, SAFEARRAYBOUND { elements: columns as u32, lower_bound: 0 }];
                let array = (lib.safe_array_create)(VT_VARIANT, 2, bounds.as_ptr());
                let mut data: *mut c_void = std::ptr::null_mut();
                if !array.is_null() && !failed((lib.safe_array_access_data)(array, &mut data)) {
                    let cells = data as *mut VARIANT;
                    for (r, row) in rows.into_iter().enumerate() {
                        for (c, item) in row.into_iter().take(columns).enumerate() {
                            write(lib, item, cells.add(r + c * n));
                        }
                    }
                    (lib.safe_array_unaccess_data)(array);
                }
                o.set_ptr(VT_ARRAY | VT_VARIANT, array);
            }
            Ret::Unknown(p) => o.set_ptr(VT_UNKNOWN, p),
            Ret::NewObject(p) => o.set_ptr(VT_DISPATCH, p),
            Ret::Kept(k) => {
                (lib.variant_copy)(o, &k.0);
            }
        }
    }
}
