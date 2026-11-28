//! The keyboard taken for the app's own menu while it shows over a game that keeps the focus: Future
//! Pinball's, whose full screen gives way — the screen's mode changed back, blank for a moment, and
//! the sound a screen plays cut with it — whenever its window loses the focus. While the menu shows,
//! each key pressed on the keyboard is kept from the game by a low-level keyboard hook and handed to
//! the menu's page as the browser names it (`pause-menu-key`: its `key` and its `code`), which takes
//! it as its own key press. Windows' own keys pass: Alt with a key (Alt+Tab), the Windows keys, the
//! volume and media keys, Print Screen; so do the keys the app itself presses, and a key let go that
//! was held before the menu showed. The hook is there only while the menu shows. Ours.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// A key press as the browser names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Key {
    pub key: String,
    pub code: String,
}

/// Whether the keyboard is the menu's: the hook's thread runs while it's so.
static WANTED: AtomicBool = AtomicBool::new(false);

/// The hook's thread, while it runs: its ID, to end it.
static THREAD: AtomicU32 = AtomicU32::new(0);

/// Where the hook hands the keys it keeps: the thread telling the menu's page.
static TO_PAGE: Mutex<Option<mpsc::Sender<Key>>> = Mutex::new(None);

/// The keys whose presses the hook kept, by their virtual key, so their letting go is kept too.
static HELD: Mutex<[bool; 256]> = Mutex::new([false; 256]);

/// The keyboard the menu's, for the window `label`, until `stop`.
pub fn start(app: &AppHandle, label: &'static str) {
    if WANTED.swap(true, Ordering::SeqCst) {
        return;
    }
    // a hook from before ended first
    wait_until(|| THREAD.load(Ordering::SeqCst) == 0);
    *HELD.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = [false; 256];
    let (send, heard) = mpsc::channel::<Key>();
    *TO_PAGE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(send);
    let app = app.clone();
    let _ = std::thread::Builder::new().name("menu-keys-page".into()).spawn(move || {
        // until the hook's end lets its sender go
        for key in heard {
            let _ = app.emit_to(label, "pause-menu-key", key);
        }
    });
    let _ = std::thread::Builder::new().name("menu-keys".into()).spawn(hook_thread);
}

/// The keyboard the game's again.
pub fn stop() {
    if !WANTED.swap(false, Ordering::SeqCst) {
        return;
    }
    *TO_PAGE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    // the hook's thread told to end once it's there to be told (it ends by itself where it comes
    // after this, seeing the keyboard isn't the menu's)
    wait_until(|| THREAD.load(Ordering::SeqCst) != 0);
    let thread = THREAD.load(Ordering::SeqCst);
    if thread != 0 {
        post_quit(thread);
    }
}

/// Waits, a few milliseconds at a time, up to a fifth of a second, until `done`.
fn wait_until(done: impl Fn() -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_millis(200);
    while !done() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// What the hook does with a key, by its virtual key, scan code and flags as Windows hands them
/// (`KBDLLHOOKSTRUCT`), pressed (`down`) or let go: kept from the game, handed to the page when
/// pressed — `Some(Some(key))` — kept and nothing said (its letting go) — `Some(None)` — or passed on.
fn keep(vk: u32, scan: u32, flags: u32, down: bool, held: &mut [bool; 256]) -> Option<Option<Key>> {
    const INJECTED: u32 = 0x10;
    const ALT_DOWN: u32 = 0x20;
    const EXTENDED: u32 = 0x01;
    let slot = (vk & 0xFF) as usize;
    if !down {
        // let go: kept only where its press was
        return std::mem::take(&mut held[slot]).then_some(None);
    }
    if flags & INJECTED != 0 || flags & ALT_DOWN != 0 || windows_own(vk) {
        return None;
    }
    held[slot] = true;
    Some(Some(named(vk, scan, flags & EXTENDED != 0)))
}

/// Windows' own keys, which pass whatever shows: the Windows keys and the menu key beside them, Alt
/// itself, Print Screen, the volume, media and launch keys.
fn windows_own(vk: u32) -> bool {
    matches!(vk, 0x5B | 0x5C | 0x5D | 0x12 | 0xA4 | 0xA5 | 0x2C | 0xAD..=0xB7)
}

/// A key as the browser names it: its `code` from its place on the keyboard (Windows' scan code, as
/// Chromium names them), its `key` as the menu's page reads it — the arrows, Enter, the space,
/// Escape, Backspace, Tab and the F keys by name, a letter or digit by itself, the rest by its code.
fn named(vk: u32, scan: u32, extended: bool) -> Key {
    let code = code_of(scan, extended).map(str::to_string).unwrap_or_else(|| format!("Unidentified{vk:02X}"));
    let key = match code.as_str() {
        "Enter" | "NumpadEnter" => "Enter".to_string(),
        "Space" => " ".to_string(),
        "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Escape" | "Backspace" | "Tab" => code.clone(),
        "ShiftLeft" | "ShiftRight" => "Shift".to_string(),
        "ControlLeft" | "ControlRight" => "Control".to_string(),
        c if c.len() == 4 && c.starts_with("Key") => c[3..].to_lowercase(),
        c if c.len() == 6 && c.starts_with("Digit") => c[5..].to_string(),
        c if c.starts_with('F') && c[1..].parse::<u8>().is_ok() => c.to_string(),
        _ => code.clone(),
    };
    Key { key, code }
}

/// The browser's code for a key by Windows' scan code, `extended` for those Windows marks so (its
/// second key of a name: the arrows' block, the right Ctrl and Alt, the keypad's Enter and /).
fn code_of(scan: u32, extended: bool) -> Option<&'static str> {
    const LETTERS: [(u32, &str); 26] = [
        (0x1E, "KeyA"), (0x30, "KeyB"), (0x2E, "KeyC"), (0x20, "KeyD"), (0x12, "KeyE"), (0x21, "KeyF"), (0x22, "KeyG"), (0x23, "KeyH"), (0x17, "KeyI"),
        (0x24, "KeyJ"), (0x25, "KeyK"), (0x26, "KeyL"), (0x32, "KeyM"), (0x31, "KeyN"), (0x18, "KeyO"), (0x19, "KeyP"), (0x10, "KeyQ"), (0x13, "KeyR"),
        (0x1F, "KeyS"), (0x14, "KeyT"), (0x16, "KeyU"), (0x2F, "KeyV"), (0x11, "KeyW"), (0x2D, "KeyX"), (0x15, "KeyY"), (0x2C, "KeyZ"),
    ];
    if let Some(&(_, c)) = LETTERS.iter().find(|(s, _)| *s == scan).filter(|_| !extended) {
        return Some(c);
    }
    const DIGITS: [&str; 10] = ["Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0"];
    if (0x02..=0x0B).contains(&scan) && !extended {
        return Some(DIGITS[(scan - 0x02) as usize]);
    }
    const F_KEYS: [&str; 10] = ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10"];
    if (0x3B..=0x44).contains(&scan) && !extended {
        return Some(F_KEYS[(scan - 0x3B) as usize]);
    }
    Some(match (scan, extended) {
        (0x01, false) => "Escape",
        (0x0C, false) => "Minus",
        (0x0D, false) => "Equal",
        (0x0E, false) => "Backspace",
        (0x0F, false) => "Tab",
        (0x1A, false) => "BracketLeft",
        (0x1B, false) => "BracketRight",
        (0x1C, false) => "Enter",
        (0x1C, true) => "NumpadEnter",
        (0x1D, false) => "ControlLeft",
        (0x1D, true) => "ControlRight",
        (0x27, false) => "Semicolon",
        (0x28, false) => "Quote",
        (0x29, false) => "Backquote",
        (0x2A, false) => "ShiftLeft",
        (0x2B, false) => "Backslash",
        (0x33, false) => "Comma",
        (0x34, false) => "Period",
        (0x35, false) => "Slash",
        (0x35, true) => "NumpadDivide",
        (0x36, _) => "ShiftRight",
        (0x37, false) => "NumpadMultiply",
        (0x39, false) => "Space",
        (0x3A, false) => "CapsLock",
        (0x45, false) => "Pause",
        (0x45, true) => "NumLock",
        (0x46, false) => "ScrollLock",
        (0x47, false) => "Numpad7",
        (0x47, true) => "Home",
        (0x48, false) => "Numpad8",
        (0x48, true) => "ArrowUp",
        (0x49, false) => "Numpad9",
        (0x49, true) => "PageUp",
        (0x4A, false) => "NumpadSubtract",
        (0x4B, false) => "Numpad4",
        (0x4B, true) => "ArrowLeft",
        (0x4C, false) => "Numpad5",
        (0x4D, false) => "Numpad6",
        (0x4D, true) => "ArrowRight",
        (0x4E, false) => "NumpadAdd",
        (0x4F, false) => "Numpad1",
        (0x4F, true) => "End",
        (0x50, false) => "Numpad2",
        (0x50, true) => "ArrowDown",
        (0x51, false) => "Numpad3",
        (0x51, true) => "PageDown",
        (0x52, false) => "Numpad0",
        (0x52, true) => "Insert",
        (0x53, false) => "NumpadDecimal",
        (0x53, true) => "Delete",
        (0x56, false) => "IntlBackslash",
        (0x57, false) => "F11",
        (0x58, false) => "F12",
        _ => return None,
    })
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct KbdLlHook {
        pub vk: u32,
        pub scan: u32,
        pub flags: u32,
        pub time: u32,
        pub extra: usize,
    }

    #[repr(C)]
    pub struct Msg {
        pub hwnd: *mut c_void,
        pub message: u32,
        pub wparam: usize,
        pub lparam: isize,
        pub time: u32,
        pub pt: [i32; 2],
        pub private: u32,
    }

    pub type HookProc = unsafe extern "system" fn(i32, usize, isize) -> isize;

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn SetWindowsHookExW(kind: i32, hook: HookProc, module: *mut c_void, thread: u32) -> *mut c_void;
        pub fn UnhookWindowsHookEx(hook: *mut c_void) -> i32;
        pub fn CallNextHookEx(hook: *mut c_void, code: i32, wparam: usize, lparam: isize) -> isize;
        pub fn GetMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32) -> i32;
        pub fn PeekMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32, remove: u32) -> i32;
        pub fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        pub fn GetCurrentThreadId() -> u32;
    }

    pub const WH_KEYBOARD_LL: i32 = 13;
    pub const WM_QUIT: u32 = 0x0012;
    pub const WM_KEYDOWN: usize = 0x0100;
    pub const WM_SYSKEYDOWN: usize = 0x0104;
}

/// Windows' low-level keyboard hook, on this thread, until `stop` posts it its end: Windows calls
/// the hook on the thread that set it, through its messages.
#[cfg(windows)]
fn hook_thread() {
    use win::*;
    let mut msg = Msg { hwnd: std::ptr::null_mut(), message: 0, wparam: 0, lparam: 0, time: 0, pt: [0, 0], private: 0 };
    // SAFETY: the thread's own message queue made, nothing taken from it
    unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 0) };
    // SAFETY: no arguments
    let own = unsafe { GetCurrentThreadId() };
    THREAD.store(own, Ordering::SeqCst);
    // SAFETY: the hook below lives as long as the program; the program's own module
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, on_key, GetModuleHandleW(std::ptr::null()), 0) };
    if hook.is_null() {
        eprintln!("the menu couldn't take the keyboard: Windows refused its hook");
    } else {
        // not where the keyboard is the game's already, its end asked before it was there
        if WANTED.load(Ordering::SeqCst) {
            // SAFETY: a message to fill; the loop ends with WM_QUIT
            while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {}
        }
        // SAFETY: the hook set above
        unsafe { UnhookWindowsHookEx(hook) };
    }
    // its ID no longer this thread's to be told
    let _ = THREAD.compare_exchange(own, 0, Ordering::SeqCst, Ordering::SeqCst);
}

#[cfg(not(windows))]
fn hook_thread() {}

#[cfg(windows)]
fn post_quit(thread: u32) {
    // SAFETY: a thread's ID, its message queue made before the ID was kept
    unsafe { win::PostThreadMessageW(thread, win::WM_QUIT, 0, 0) };
}

#[cfg(not(windows))]
fn post_quit(_thread: u32) {}

/// Windows' call for each key, on the hook's thread: quick, as Windows drops a hook that's slow.
#[cfg(windows)]
unsafe extern "system" fn on_key(code: i32, wparam: usize, lparam: isize) -> isize {
    use win::*;
    if code == 0 && lparam != 0 && WANTED.load(Ordering::Relaxed) {
        // SAFETY: Windows hands the key's facts with HC_ACTION
        let k = unsafe { &*(lparam as *const KbdLlHook) };
        let down = wparam == WM_KEYDOWN || wparam == WM_SYSKEYDOWN;
        let mut held = HELD.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(said) = keep(k.vk, k.scan, k.flags, down, &mut held) {
            if let Some(key) = said
                && let Some(to) = TO_PAGE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref()
            {
                let _ = to.send(key);
            }
            return 1;
        }
    }
    // SAFETY: the call passed on as Windows handed it
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keys named as the browser names them, Windows' own and the app's own presses passed on, and a
    /// key let go kept only where its press was.
    #[test]
    fn keys_kept_for_the_menu_named_as_the_browser_names_them() {
        let mut held = [false; 256];
        // the arrows' block, extended: the up arrow
        assert_eq!(keep(0x26, 0x48, 0x01, true, &mut held), Some(Some(Key { key: "ArrowUp".into(), code: "ArrowUp".into() })));
        assert_eq!(keep(0x26, 0x48, 0x01, false, &mut held), Some(None));
        // the keypad's 8, not extended, is the keypad's own
        assert_eq!(keep(0x68, 0x48, 0x00, true, &mut held).unwrap().unwrap().code, "Numpad8");
        assert_eq!(keep(0x0D, 0x1C, 0x00, true, &mut held), Some(Some(Key { key: "Enter".into(), code: "Enter".into() })));
        assert_eq!(keep(0x0D, 0x1C, 0x01, true, &mut held).unwrap().unwrap().code, "NumpadEnter");
        assert_eq!(keep(0x20, 0x39, 0x00, true, &mut held).unwrap().unwrap().key, " ");
        assert_eq!(keep(0x1B, 0x01, 0x00, true, &mut held).unwrap().unwrap().key, "Escape");
        assert_eq!(keep(0x7B, 0x58, 0x00, true, &mut held), Some(Some(Key { key: "F12".into(), code: "F12".into() })));
        assert_eq!(keep(0x41, 0x1E, 0x00, true, &mut held), Some(Some(Key { key: "a".into(), code: "KeyA".into() })));
        assert_eq!(keep(0x31, 0x02, 0x00, true, &mut held), Some(Some(Key { key: "1".into(), code: "Digit1".into() })));
        // a flipper's Shift, by side
        assert_eq!(keep(0xA0, 0x2A, 0x00, true, &mut held), Some(Some(Key { key: "Shift".into(), code: "ShiftLeft".into() })));
        assert_eq!(keep(0xA1, 0x36, 0x01, true, &mut held).unwrap().unwrap().code, "ShiftRight");
        // pressed by the app itself, with Alt (Alt+Tab), the Windows key, the volume: passed
        assert_eq!(keep(0x13, 0x45, 0x10, true, &mut held), None);
        assert_eq!(keep(0x09, 0x0F, 0x20, true, &mut held), None);
        assert_eq!(keep(0x5B, 0x5B, 0x01, true, &mut held), None);
        assert_eq!(keep(0xAF, 0x30, 0x01, true, &mut held), None);
        // let go of a key held before the menu showed: passed, so the game sees it let go
        assert_eq!(keep(0x51, 0x10, 0x00, false, &mut held), None);
    }
}
