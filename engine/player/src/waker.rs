//! Waking the game thread: at a set time (the next physics step) or at once
//! (an input arrived, a frame is wanted, PinMAME changed a solenoid). On
//! Windows, a high-resolution waitable timer gives sub-millisecond wake-ups
//! without spinning.

use std::time::Duration;

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicPtr, Ordering};

    type Handle = *mut c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateEventW(attributes: *mut c_void, manual_reset: i32, initial_state: i32, name: *const u16) -> Handle;
        fn SetEvent(event: Handle) -> i32;
        fn CreateWaitableTimerExW(attributes: *mut c_void, name: *const u16, flags: u32, access: u32) -> Handle;
        fn SetWaitableTimer(timer: Handle, due: *const i64, period: i32, routine: *mut c_void, arg: *mut c_void, resume: i32) -> i32;
        fn WaitForMultipleObjects(count: u32, handles: *const Handle, wait_all: i32, milliseconds: u32) -> u32;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetCurrentProcess() -> Handle;
        fn DuplicateHandle(source_process: Handle, source: Handle, target_process: Handle, target: *mut Handle, access: u32, inherit: i32, options: u32) -> i32;
    }

    const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION: u32 = 0x2;
    const TIMER_ALL_ACCESS: u32 = 0x1F_0003;
    const INFINITE: u32 = 0xFFFF_FFFF;
    const DUPLICATE_SAME_ACCESS: u32 = 0x2;

    pub struct Waker {
        event: Handle,
        timer: Handle,
        /// An event of another program's that also wakes the thread
        /// ([`Waker::watch`]): the waker's own copy of its handle, or null.
        watched: AtomicPtr<c_void>,
    }

    // SAFETY: Windows event and timer handles may be used from any thread.
    unsafe impl Send for Waker {}
    unsafe impl Sync for Waker {}

    impl Waker {
        pub fn new() -> Waker {
            // SAFETY: plain Windows calls; null handles are checked where used
            unsafe {
                let event = CreateEventW(null_mut(), 0, 0, std::ptr::null());
                let mut timer = CreateWaitableTimerExW(null_mut(), std::ptr::null(), CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS);
                if timer.is_null() {
                    // before Windows 10 1803: an ordinary timer
                    timer = CreateWaitableTimerExW(null_mut(), std::ptr::null(), 0, TIMER_ALL_ACCESS);
                }
                Waker { event, timer, watched: AtomicPtr::new(null_mut()) }
            }
        }

        pub fn wake(&self) {
            // SAFETY: a valid event handle owned by self
            unsafe {
                SetEvent(self.event);
            }
        }

        /// Also wakes for `event`, an auto-reset event another program sets
        /// (PinMAME's helper, when solenoids change), until [`Waker::unwatch`].
        /// The waker keeps its own copy of the handle.
        ///
        /// # Safety
        /// `event` is a live event handle of this process.
        pub unsafe fn watch(&self, event: *mut c_void) {
            let mut copy = null_mut();
            // SAFETY: copies a live handle within this process
            let copied = unsafe { DuplicateHandle(GetCurrentProcess(), event, GetCurrentProcess(), &mut copy, 0, 0, DUPLICATE_SAME_ACCESS) } != 0;
            self.replace_watched(if copied { copy } else { null_mut() });
        }

        pub fn unwatch(&self) {
            self.replace_watched(null_mut());
        }

        fn replace_watched(&self, handle: Handle) {
            let old = self.watched.swap(handle, Ordering::AcqRel);
            if !old.is_null() {
                // SAFETY: the waker's own copy, closed once (only the waiting thread watches,
                // so no wait is using it)
                unsafe { CloseHandle(old) };
            }
        }

        pub fn wait(&self, timeout: Option<std::time::Duration>) {
            let watched = self.watched.load(Ordering::Acquire);
            // SAFETY: valid handles owned by self
            unsafe {
                match timeout {
                    Some(t) if !self.timer.is_null() => {
                        let due = -((t.as_nanos() / 100).max(1) as i64); // relative, in 100 ns
                        SetWaitableTimer(self.timer, &due, 0, null_mut(), null_mut(), 0);
                        let handles = [self.event, self.timer, watched];
                        WaitForMultipleObjects(if watched.is_null() { 2 } else { 3 }, handles.as_ptr(), 0, INFINITE);
                    }
                    Some(t) => {
                        let handles = [self.event, watched];
                        WaitForMultipleObjects(if watched.is_null() { 1 } else { 2 }, handles.as_ptr(), 0, t.as_millis().max(1) as u32);
                    }
                    None => {
                        let handles = [self.event, watched];
                        WaitForMultipleObjects(if watched.is_null() { 1 } else { 2 }, handles.as_ptr(), 0, INFINITE);
                    }
                }
            }
        }
    }

    impl Drop for Waker {
        fn drop(&mut self) {
            self.unwatch();
            // SAFETY: handles owned by self, closed once
            unsafe {
                for h in [self.event, self.timer] {
                    if !h.is_null() {
                        CloseHandle(h);
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::{Condvar, Mutex};

    pub struct Waker {
        woken: Mutex<bool>,
        signal: Condvar,
    }

    impl Waker {
        pub fn new() -> Waker {
            Waker { woken: Mutex::new(false), signal: Condvar::new() }
        }

        pub fn wake(&self) {
            *self.woken.lock().unwrap_or_else(|e| e.into_inner()) = true;
            self.signal.notify_one();
        }

        /// Other programs' events are Windows's (PinMAME's helper runs only there).
        ///
        /// # Safety
        /// None needed here; Windows's takes a live event handle.
        pub unsafe fn watch(&self, _event: *mut std::ffi::c_void) {}

        pub fn unwatch(&self) {}

        pub fn wait(&self, timeout: Option<std::time::Duration>) {
            let mut woken = self.woken.lock().unwrap_or_else(|e| e.into_inner());
            if !*woken {
                woken = match timeout {
                    Some(t) => self.signal.wait_timeout(woken, t).unwrap_or_else(|e| e.into_inner()).0,
                    None => self.signal.wait(woken).unwrap_or_else(|e| e.into_inner()),
                };
            }
            *woken = false;
        }
    }
}

pub use imp::Waker;

impl Default for Waker {
    fn default() -> Self {
        Waker::new()
    }
}

/// How long until a moment on the clock, in nanoseconds (zero if past).
pub fn until(now_ns: u64, at_ns: u64) -> Duration {
    Duration::from_nanos(at_ns.saturating_sub(now_ns))
}
