//! The programs running now, by their files' names, from Windows' list of processes (Tool Help's
//! snapshot): for what mustn't be changed while a program holds it. Ours.

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct ProcessEntry {
        pub size: u32,
        pub usage: u32,
        pub process_id: u32,
        pub default_heap_id: usize,
        pub module_id: u32,
        pub threads: u32,
        pub parent_process_id: u32,
        pub priority: i32,
        pub flags: u32,
        pub exe_file: [u16; 260],
    }

    pub const SNAPPROCESS: u32 = 0x0000_0002;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
        pub fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry) -> i32;
        pub fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry) -> i32;
        pub fn CloseHandle(handle: *mut c_void) -> i32;
    }
}

/// The running programs' files' names (`PinUpMenu.exe` and the like).
#[cfg(windows)]
pub fn running() -> Vec<String> {
    use windows::*;
    let mut out = Vec::new();
    // SAFETY: Tool Help's snapshot, read entry by entry as its documentation lays out, then closed
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(SNAPPROCESS, 0);
        if snapshot.is_null() || snapshot as isize == -1 {
            return out;
        }
        let mut entry: ProcessEntry = std::mem::zeroed();
        entry.size = std::mem::size_of::<ProcessEntry>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let len = entry.exe_file.iter().position(|&c| c == 0).unwrap_or(entry.exe_file.len());
            out.push(String::from_utf16_lossy(&entry.exe_file[..len]));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    out
}

#[cfg(not(windows))]
pub fn running() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn this_program_is_among_them() {
        let names = super::running();
        let me = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().to_lowercase();
        assert!(names.iter().any(|n| n.to_lowercase() == me), "{me} not in {} names", names.len());
    }
}
