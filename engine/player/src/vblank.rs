//! Frame pacing: draw each frame just before the screen
//! refreshes, so it is shown at that refresh with the newest input, without
//! tearing and without a queue of frames waiting their turn.
//!
//! A thread of its own waits for each vertical blank of the screen the
//! window is on (DXGI's `IDXGIOutput::WaitForVBlank`) and notes when it came
//! and how far apart they are. The window thread then sleeps until just
//! before the next one, reads the game's newest state, draws and presents
//! through the classic vsync queue, which then never holds more than that
//! one frame.

use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sdl3_sys::everything::{SDL_DelayPrecise, SDL_GetTicksNS};

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// `IID_IDXGIFactory1`.
const IID_IDXGI_FACTORY1: Guid = Guid { data1: 0x770A_AE78, data2: 0xF26F, data3: 0x4DBA, data4: [0xA8, 0x29, 0x25, 0x3C, 0x83, 0xD1, 0xB3, 0x87] };

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// `DXGI_OUTPUT_DESC`.
#[repr(C)]
struct OutputDesc {
    device_name: [u16; 32],
    desktop_coordinates: Rect,
    attached_to_desktop: i32,
    rotation: u32,
    monitor: *mut c_void,
}

#[link(name = "dxgi")]
unsafe extern "system" {
    fn CreateDXGIFactory1(riid: *const Guid, factory: *mut *mut c_void) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn MonitorFromWindow(hwnd: *mut c_void, flags: u32) -> *mut c_void;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThread() -> *mut c_void;
    fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
}

const MONITOR_DEFAULTTONEAREST: u32 = 2;
const THREAD_PRIORITY_HIGHEST: i32 = 2;

// entries of the DXGI interfaces' tables (dxgi.h's order)
const RELEASE: usize = 2;
const FACTORY1_ENUM_ADAPTERS1: usize = 12;
const ADAPTER_ENUM_OUTPUTS: usize = 7;
const OUTPUT_GET_DESC: usize = 7;
const OUTPUT_WAIT_FOR_VBLANK: usize = 10;

/// Entry `n` of a COM object's table.
///
/// # Safety
/// `obj` must be a live COM object whose table has at least `n + 1` entries.
unsafe fn entry(obj: *mut c_void, n: usize) -> usize {
    unsafe { *(*(obj as *const *const usize)).add(n) }
}

/// # Safety
/// `obj` must be a live COM object.
unsafe fn release(obj: *mut c_void) {
    unsafe {
        let f: unsafe extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(entry(obj, RELEASE));
        f(obj);
    }
}

/// The DXGI output (screen) that shows `hwnd`, with a reference held.
fn output_for(hwnd: *mut c_void) -> Option<*mut c_void> {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut factory = null_mut();
        if CreateDXGIFactory1(&IID_IDXGI_FACTORY1, &mut factory) < 0 || factory.is_null() {
            return None;
        }
        let enum_adapters: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32 = std::mem::transmute(entry(factory, FACTORY1_ENUM_ADAPTERS1));
        let mut found = None;
        let mut a = 0;
        loop {
            let mut adapter = null_mut();
            if enum_adapters(factory, a, &mut adapter) < 0 || adapter.is_null() {
                break;
            }
            let enum_outputs: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32 = std::mem::transmute(entry(adapter, ADAPTER_ENUM_OUTPUTS));
            let mut o = 0;
            loop {
                let mut output = null_mut();
                if enum_outputs(adapter, o, &mut output) < 0 || output.is_null() {
                    break;
                }
                let get_desc: unsafe extern "system" fn(*mut c_void, *mut OutputDesc) -> i32 = std::mem::transmute(entry(output, OUTPUT_GET_DESC));
                let mut desc: OutputDesc = std::mem::zeroed();
                if found.is_none() && get_desc(output, &mut desc) >= 0 && desc.monitor == monitor {
                    found = Some(output);
                } else {
                    release(output);
                }
                o += 1;
            }
            release(adapter);
            a += 1;
        }
        release(factory);
        found
    }
}

struct Shared {
    stop: AtomicBool,
    /// When the newest refresh came (SDL's clock, ns), 0 before the first.
    last_ns: AtomicU64,
    /// The time between refreshes (ns), smoothed.
    period_ns: AtomicU64,
}

/// Follows the refreshes of the screen showing a window.
pub struct VBlank {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl VBlank {
    /// Starts following the refreshes of the screen showing `hwnd`;
    /// `refresh_hz` is the screen's rate, as a first guess of their spacing.
    /// None when Windows can't say which screen that is.
    pub fn start(hwnd: *mut c_void, refresh_hz: f32) -> Option<VBlank> {
        if hwnd.is_null() {
            return None;
        }
        let output = output_for(hwnd)? as usize;
        let first_period = if refresh_hz > 1.0 { 1e9 / f64::from(refresh_hz) } else { 0.0 };
        let shared = Arc::new(Shared { stop: AtomicBool::new(false), last_ns: AtomicU64::new(0), period_ns: AtomicU64::new(first_period as u64) });
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("vblank".into())
            .spawn(move || {
                let output = output as *mut c_void;
                // SAFETY: a live output, whose reference this thread owns from here
                unsafe {
                    // waking late would move every frame late
                    SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
                    let wait: unsafe extern "system" fn(*mut c_void) -> i32 = std::mem::transmute(entry(output, OUTPUT_WAIT_FOR_VBLANK));
                    let mut period = first_period;
                    let mut last = 0u64;
                    while !thread_shared.stop.load(Ordering::Relaxed) {
                        let ok = wait(output) >= 0;
                        let now = SDL_GetTicksNS();
                        // a screen that's off can return at once: don't spin
                        if !ok || now.saturating_sub(last) < 1_000_000 {
                            SDL_DelayPrecise(1_000_000);
                            continue;
                        }
                        if last != 0 {
                            let d = (now - last) as f64;
                            if period == 0.0 {
                                period = d;
                            } else if d > period * 0.5 && d < period * 1.5 {
                                // a missed or doubled refresh says nothing about the spacing
                                period += (d - period) / 16.0;
                            }
                        }
                        last = now;
                        thread_shared.period_ns.store(period as u64, Ordering::Relaxed);
                        thread_shared.last_ns.store(now, Ordering::Release);
                    }
                    release(output);
                }
            })
            .ok()?;
        Some(VBlank { shared, thread: Some(thread) })
    }

    /// The next refresh after `now_ns`, and the time between refreshes;
    /// None until one has been seen.
    pub fn next_after(&self, now_ns: u64) -> Option<(u64, u64)> {
        let last = self.shared.last_ns.load(Ordering::Acquire);
        let period = self.shared.period_ns.load(Ordering::Relaxed);
        if last == 0 || period == 0 {
            return None;
        }
        let k = now_ns.saturating_sub(last) / period + 1;
        Some((last + k * period, period))
    }
}

impl Drop for VBlank {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        // the thread notices at the next refresh
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// When to start each frame: early enough before its refresh to be drawn
/// and handed over in time (the *lead*), learned from how long frames take.
pub struct Pacer {
    lead_ns: u64,
    costs: VecDeque<u64>,
    /// Frames paced, and those handed over after their refresh (shown one
    /// refresh late).
    pub frames: u32,
    pub late: u32,
}

impl Default for Pacer {
    fn default() -> Self {
        Pacer { lead_ns: 2_000_000, costs: VecDeque::new(), frames: 0, late: 0 }
    }
}

impl Pacer {
    /// Sleeps until the frame for the coming refresh should start. Returns
    /// that refresh's time and the time between refreshes.
    pub fn wait(&mut self, vblank: &VBlank) -> Option<(u64, u64)> {
        // SAFETY: SDL's clock and sleep
        unsafe {
            let now = SDL_GetTicksNS();
            let (mut next, period) = vblank.next_after(now)?;
            // too late for this refresh: the one after
            while next < now + self.lead_ns {
                next += period;
            }
            SDL_DelayPrecise(next - self.lead_ns - now);
            Some((next, period))
        }
    }

    /// A frame for the refresh at `target` started at `start` and was handed
    /// over at `done`: learn how long frames take.
    pub fn done(&mut self, start: u64, done: u64, target: u64, period: u64) {
        self.frames += 1;
        if done > target {
            self.late += 1;
        }
        self.costs.push_back(done.saturating_sub(start));
        if self.costs.len() > 120 {
            self.costs.pop_front();
        }
        // the slowest recent frame, a quarter more, and 1 ms for the graphics card's own work
        let worst = self.costs.iter().copied().max().unwrap_or(0);
        self.lead_ns = (worst + worst / 4 + 1_000_000).clamp(1_500_000, period * 6 / 10);
    }

    /// How far before each refresh frames start now.
    pub fn lead_ms(&self) -> f64 {
        self.lead_ns as f64 / 1e6
    }
}
