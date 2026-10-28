// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! A crash's report (Windows): the exception, where it happened, the stack it happened on,
//! frame by frame — each frame's module and offset, and its function and source line where
//! Windows' symbol reader (dbghelp) finds them — and the last switch changes PinMAME was
//! sent, on the error output, before Windows ends the process as it would have. The
//! player's own report (`engine/player/src/crash.rs`), ours too, with the switches added.

use core::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

/// The last switch changes sent to PinMAME, for a crash's report: each its time in
/// milliseconds since the report was installed, its number and its state, in one word.
static RECENT: [AtomicU64; 32] = [const { AtomicU64::new(0) }; 32];
static NEXT: AtomicUsize = AtomicUsize::new(0);
static START: OnceLock<Instant> = OnceLock::new();

/// Notes a switch change about to be sent to PinMAME.
pub fn note_switch(number: i32, closed: bool) {
    let ms = START.get().map_or(0, |s| s.elapsed().as_millis() as u64);
    let word = (ms << 24) | (u64::from(number as i16 as u16) << 8) | 0x80 | u64::from(closed);
    RECENT[NEXT.fetch_add(1, Ordering::Relaxed) % RECENT.len()].store(word, Ordering::Relaxed);
}

/// The switch changes noted, oldest first.
fn recent_switches() -> Vec<(u64, i32, bool)> {
    let next = NEXT.load(Ordering::Relaxed);
    (0..RECENT.len())
        .map(|k| RECENT[(next + k) % RECENT.len()].load(Ordering::Relaxed))
        .filter(|w| w & 0x80 != 0)
        .map(|w| (w >> 24, i32::from((w >> 8) as u16 as i16), w & 1 != 0))
        .collect()
}

#[repr(C)]
struct ExceptionRecord {
    code: u32,
    flags: u32,
    record: *mut ExceptionRecord,
    address: *mut c_void,
    parameters: u32,
    information: [usize; 15],
}

#[repr(C)]
struct ExceptionPointers {
    record: *mut ExceptionRecord,
    context: *mut c_void,
}

/// The x64 `CONTEXT`, as bytes: its instruction and stack pointers are read
/// and written at their offsets.
#[repr(C, align(16))]
struct Context([u8; 1232]);
const RIP: usize = 0xF8;
const RSP: usize = 0x98;

impl Context {
    fn get(&self, at: usize) -> u64 {
        u64::from_le_bytes(self.0[at..at + 8].try_into().unwrap_or_default())
    }
    fn set(&mut self, at: usize, v: u64) {
        self.0[at..at + 8].copy_from_slice(&v.to_le_bytes());
    }
}

#[repr(C)]
struct SymbolInfoW {
    size_of_struct: u32,
    type_index: u32,
    reserved: [u64; 2],
    index: u32,
    size: u32,
    mod_base: u64,
    flags: u32,
    value: u64,
    address: u64,
    register: u32,
    scope: u32,
    tag: u32,
    name_len: u32,
    max_name_len: u32,
    name: [u16; 256],
}

#[repr(C)]
struct LineW64 {
    size_of_struct: u32,
    key: *mut c_void,
    line_number: u32,
    file_name: *mut u16,
    address: u64,
}

type Filter = unsafe extern "system" fn(*mut ExceptionPointers) -> i32;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetUnhandledExceptionFilter(filter: Option<Filter>) -> Option<Filter>;
    fn GetCurrentProcess() -> *mut c_void;
    fn GetModuleHandleExW(flags: u32, address: *const c_void, module: *mut *mut c_void) -> i32;
    fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
    fn RtlLookupFunctionEntry(pc: u64, image_base: *mut u64, history: *mut c_void) -> *mut c_void;
    fn RtlVirtualUnwind(
        kind: u32,
        image_base: u64,
        pc: u64,
        entry: *mut c_void,
        context: *mut Context,
        handler_data: *mut *mut c_void,
        establisher: *mut u64,
        pointers: *mut c_void,
    ) -> *mut c_void;
}

#[link(name = "dbghelp")]
unsafe extern "system" {
    fn SymInitializeW(process: *mut c_void, path: *const u16, invade: i32) -> i32;
    fn SymFromAddrW(process: *mut c_void, address: u64, displacement: *mut u64, symbol: *mut SymbolInfoW) -> i32;
    fn SymGetLineFromAddrW64(process: *mut c_void, address: u64, displacement: *mut u32, line: *mut LineW64) -> i32;
}

/// Where an address lies: its module's file name and the offset in it.
fn module_of(pc: u64) -> String {
    let mut module: *mut c_void = core::ptr::null_mut();
    // FROM_ADDRESS | UNCHANGED_REFCOUNT
    // SAFETY: plain Windows calls on an address; a failure leaves the module null
    if unsafe { GetModuleHandleExW(4 | 2, pc as *const c_void, &mut module) } == 0 || module.is_null() {
        return "?".to_string();
    }
    let mut name = [0u16; 260];
    // SAFETY: the buffer's length is handed over
    let n = unsafe { GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) } as usize;
    let path = String::from_utf16_lossy(&name[..n.min(name.len())]);
    let file = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
    format!("{file}+{:#x}", pc - module as u64)
}

/// A frame's function and line, where the symbols are found.
fn symbol_of(process: *mut c_void, pc: u64) -> String {
    let mut symbol = SymbolInfoW {
        size_of_struct: 88,
        type_index: 0,
        reserved: [0; 2],
        index: 0,
        size: 0,
        mod_base: 0,
        flags: 0,
        value: 0,
        address: 0,
        register: 0,
        scope: 0,
        tag: 0,
        name_len: 0,
        max_name_len: 255,
        name: [0; 256],
    };
    let mut out = String::new();
    let mut displacement = 0u64;
    // SAFETY: the symbol buffer is as long as it says
    if unsafe { SymFromAddrW(process, pc, &mut displacement, &mut symbol) } != 0 {
        let n = (symbol.name_len as usize).min(255);
        out.push_str(&String::from_utf16_lossy(&symbol.name[..n]));
    }
    let mut line = LineW64 { size_of_struct: size_of::<LineW64>() as u32, key: core::ptr::null_mut(), line_number: 0, file_name: core::ptr::null_mut(), address: 0 };
    let mut column = 0u32;
    // SAFETY: the line's file name is a string dbghelp owns, read up to its end
    if unsafe { SymGetLineFromAddrW64(process, pc, &mut column, &mut line) } != 0 && !line.file_name.is_null() {
        let mut len = 0;
        // SAFETY: a null-terminated wide string
        while len < 1024 && unsafe { *line.file_name.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: as measured
        let file = String::from_utf16_lossy(unsafe { core::slice::from_raw_parts(line.file_name, len) });
        out.push_str(&format!(" at {file}:{}", line.line_number));
    }
    out
}

unsafe extern "system" fn report(pointers: *mut ExceptionPointers) -> i32 {
    // SAFETY: Windows hands the filter the exception's record and its context
    let (r, context) = unsafe { (&*(*pointers).record, (*pointers).context as *const Context) };
    let what = if r.code == 0xC000_0005 && r.parameters >= 2 {
        format!(" ({} {:#x})", if r.information[0] == 1 { "writing" } else { "reading" }, r.information[1])
    } else {
        String::new()
    };
    eprintln!("vpi-pinmame crashed: exception {:#010x} at {:p} in {}{what}", r.code, r.address, module_of(r.address as u64));
    // SAFETY: the context is the crashed thread's, copied before it's unwound
    let mut ctx = Context([0; 1232]);
    unsafe { core::ptr::copy_nonoverlapping(context as *const u8, ctx.0.as_mut_ptr(), 1232) };
    // SAFETY: the symbol reader for this process, its modules loaded
    let process = unsafe { GetCurrentProcess() };
    unsafe { SymInitializeW(process, core::ptr::null(), 1) };
    for k in 0..48 {
        let pc = ctx.get(RIP);
        if pc == 0 {
            break;
        }
        eprintln!("  {k:2}: {} {}", module_of(pc), symbol_of(process, pc));
        let mut base = 0u64;
        // SAFETY: Windows' own unwinding, on a copy of the context
        let entry = unsafe { RtlLookupFunctionEntry(pc, &mut base, core::ptr::null_mut()) };
        if entry.is_null() {
            // a leaf function: its return address is on top of the stack
            let rsp = ctx.get(RSP);
            // SAFETY: the crashed thread's stack, still there
            ctx.set(RIP, unsafe { *(rsp as *const u64) });
            ctx.set(RSP, rsp + 8);
        } else {
            let (mut handler, mut establisher) = (core::ptr::null_mut(), 0u64);
            // SAFETY: as above
            unsafe { RtlVirtualUnwind(0, base, pc, entry, &mut ctx, &mut handler, &mut establisher, core::ptr::null_mut()) };
        }
    }
    let since = START.get().map_or(0, |s| s.elapsed().as_millis());
    eprintln!("  the last switch changes sent to PinMAME (ms since the start; crash at {since}):");
    for (ms, number, closed) in recent_switches() {
        eprintln!("    {ms:>8} switch {number} {}", if closed { "closed" } else { "open" });
    }
    0 // EXCEPTION_CONTINUE_SEARCH
}

/// Reports a crash before Windows ends the process.
pub fn install() {
    let _ = START.set(Instant::now());
    // SAFETY: the filter is a plain function living as long as the process
    unsafe {
        SetUnhandledExceptionFilter(Some(report));
    }
}
