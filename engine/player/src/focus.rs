//! Where the keyboard is: the player hears keys only while one of its windows has them (SDL's raw
//! keyboard, as VPX's), so when another program's window takes them, play pauses until the table's
//! window has them again, as VPX's `IsPlaying` holds play while its window isn't focused. Which
//! window took them is said in the log, as VPX's `OnFocusChanged` says it. Ours.

use std::ffi::c_void;

/// The notice the table's window shows while play waits for the keyboard.
pub const NOTICE: &str = "Paused: another window has the keyboard. Click the table to play on";

/// The program and title of the window that has the keyboard now, for the log: "Discord.exe
/// (“Friends”)", or "no window" when none has it.
pub fn foreground_window() -> String {
    unsafe extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
        fn GetWindowTextW(hwnd: *mut c_void, text: *mut u16, max: i32) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(process: *mut c_void, flags: u32, name: *mut u16, size: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    // SAFETY: plain queries of the window that has the keyboard and of its program, each buffer
    // as long as said, the process's handle closed after
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return "no window".into();
        }
        let mut title = [0u16; 256];
        let n = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32).max(0) as usize;
        let title = String::from_utf16_lossy(&title[..n]);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut program = String::from("a program");
        if pid == std::process::id() {
            program = "this player".into();
        } else if pid != 0 {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !process.is_null() {
                let mut name = [0u16; 520];
                let mut size = name.len() as u32;
                if QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut size) != 0 {
                    let path = String::from_utf16_lossy(&name[..size as usize]);
                    program = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
                }
                CloseHandle(process);
            }
        }
        if title.is_empty() { program } else { format!("{program} (\u{201c}{title}\u{201d})") }
    }
}
