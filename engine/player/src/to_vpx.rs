//! A table a front end starts with this program as it starts VPX — VPX's own command line,
//! `-Play <table>`, the program's name changed in the front end — handed to VPX itself, as the app
//! plays every table: the VPX the library plays it in (the line chosen for the table on its page,
//! else the settings' `[VPI]` `PlayWith`, else the newest), started in VPX's folder with the front
//! end's own command line and waited for, this program ending as VPX ends. VPX 10.8.0 is started
//! minimized where the command line doesn't say, as only then does it quit after the game. VPX
//! runs in a job closed with this program, so a front end that ends this program to end a game
//! ends VPX with it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use library::vpx::Vpx;

/// The table a command line plays where it's VPX's own (`-Play <table>`, or `/play`), as front
/// ends start VPX.
pub fn table_in(args: &[OsString]) -> Option<PathBuf> {
    let at = args.iter().position(|a| switch(a).as_deref() == Some("play"))?;
    args.get(at + 1).map(PathBuf::from)
}

/// Hands `table` to VPX with this program's own command line, `args`, and ends as VPX ends.
pub fn run(table: &Path, args: Vec<OsString>) -> ExitCode {
    match hand_over(table, args) {
        Ok(code) => u8::try_from(code).map_or(ExitCode::FAILURE, ExitCode::from),
        Err(e) => {
            eprintln!("vpi-player: {e}");
            tell(&format!("{e}.\n\nVisual Pinball Infinite plays tables in Visual Pinball X: set the front end to start VPX itself."));
            ExitCode::FAILURE
        }
    }
}

fn hand_over(table: &Path, args: Vec<OsString>) -> Result<i32, String> {
    let folders = vpi_settings::library_folders_now().ok_or("No table library is set up in Visual Pinball Infinite")?;
    let mut layout = library::plan::Layout::from_folders(&folders);
    if let Some(line) = line_for(table) {
        layout.play_with = Some(line);
    }
    let vpx = library::vpx::to_play(&layout).ok_or("Visual Pinball X wasn't found beside the library's tables")?;
    // VPX starts in its own folder: a path the front end gave from its own made whole first
    let args = match std::env::current_dir() {
        Ok(from) => with_whole_paths(args, &from),
        Err(_) => args,
    };
    #[cfg(windows)]
    os::let_go_of_console();
    let mut child = command(&vpx, args).spawn().map_err(|e| format!("Visual Pinball X couldn't be started ({}): {e}", vpx.program.display()))?;
    #[cfg(windows)]
    let _job = os::Job::holding(&child);
    #[cfg(windows)]
    os::let_take_focus(child.id());
    let status = child.wait().map_err(|e| format!("Visual Pinball X couldn't be waited for: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

/// A VPX switch's word, lower case (`-Play`, `/play`: `play`).
fn switch(arg: &OsString) -> Option<String> {
    arg.to_str().and_then(crate::vpx_switch)
}

/// VPX's switches followed by a file or a folder, by their words, lower case, with how many paths
/// follow each (`-TournamentFile` takes the table's and the tournament file's).
const PATH_SWITCHES: [(&str, usize); 9] = [("play", 1), ("edit", 1), ("povedit", 1), ("pov", 1), ("extractvbs", 1), ("ini", 1), ("tableini", 1), ("tournamentfile", 2), ("prefpath", 1)];

/// A front end's command line with each path after one of VPX's switches made whole from `from`,
/// the folder the front end started this program in: VPX is started in its own folder, and puts the
/// folder it was started in before any path not starting with a drive's letter, so a path the front
/// end gave from its own folder would point into VPX's. The rest left as it came.
fn with_whole_paths(args: Vec<OsString>, from: &Path) -> Vec<OsString> {
    let mut out = Vec::with_capacity(args.len());
    let mut paths_left = 0usize;
    for arg in args {
        if paths_left > 0 {
            paths_left -= 1;
            let path = Path::new(&arg);
            // a path with its drive's letter, or a network share's, as it came; else made whole
            let whole = path.is_absolute() || arg.to_string_lossy().starts_with("\\\\");
            out.push(if whole { arg } else { std::path::absolute(from.join(path)).unwrap_or_else(|_| from.join(path)).into_os_string() });
            continue;
        }
        if let Some(word) = switch(&arg) {
            paths_left = PATH_SWITCHES.iter().find(|(w, _)| *w == word).map_or(0, |(_, n)| *n);
        }
        out.push(arg);
    }
    out
}

/// VPX started as the front end would start it: its own command line, in VPX's folder; 10.8.0
/// minimized where the command line doesn't say (`-ExtMinimized`, its pause menu kept), as only
/// then does it quit after the game.
fn command(vpx: &Vpx, args: Vec<OsString>) -> Command {
    let mut command = Command::new(&vpx.program);
    let minimized = args.iter().any(|a| matches!(switch(a).as_deref(), Some("minimized" | "extminimized")));
    if !vpx.has_addons() && !minimized {
        command.arg("-ExtMinimized");
    }
    command.args(args);
    if let Some(dir) = vpx.program.parent() {
        command.current_dir(dir);
    }
    // Microsoft's C++ runtime beside this program, for a PC without its own
    if let Some(path) = library::vpx::runtime_path() {
        command.env("PATH", path);
    }
    command
}

/// The VPX line chosen for `table` on its page, where one is: the app's library
/// (`Library\library.json` in its data folder) keeps it by the table file's path, lower case.
fn line_for(table: &Path) -> Option<String> {
    let file = PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("Library").join("library.json");
    let user: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
    line_in(&user, table)
}

/// The line `user` (the app's library) keeps for `table`, by its path as given or made whole.
fn line_in(user: &serde_json::Value, table: &Path) -> Option<String> {
    let chosen = user.get("play_with")?.as_object()?;
    let whole = std::path::absolute(table).unwrap_or_else(|_| table.to_path_buf());
    [table, whole.as_path()].iter().find_map(|p| chosen.get(&p.to_string_lossy().to_lowercase())?.as_str().map(str::to_string))
}

/// Says why a table wasn't handed over, in a message box: a front end shows no console.
fn tell(text: &str) {
    #[cfg(windows)]
    os::message(text);
    #[cfg(not(windows))]
    let _ = text;
}

#[cfg(windows)]
mod os {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    #[derive(Default)]
    struct BasicLimits {
        per_process_user_time: i64,
        per_job_user_time: i64,
        limit_flags: u32,
        minimum_working_set: usize,
        maximum_working_set: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operations: u64,
        write_operations: u64,
        other_operations: u64,
        read_bytes: u64,
        write_bytes: u64,
        other_bytes: u64,
    }

    /// JOBOBJECT_EXTENDED_LIMIT_INFORMATION.
    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimits {
        basic: BasicLimits,
        io: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[cfg(target_pointer_width = "64")]
    const _: () = assert!(std::mem::size_of::<ExtendedLimits>() == 144);

    /// JobObjectExtendedLimitInformation.
    const EXTENDED_LIMITS: i32 = 9;
    /// JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.
    const KILL_ON_JOB_CLOSE: u32 = 0x2000;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
        fn SetInformationJobObject(job: *mut c_void, class: i32, info: *const c_void, length: u32) -> i32;
        fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
        fn FreeConsole() -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn AllowSetForegroundWindow(process_id: u32) -> i32;
        fn MessageBoxW(window: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    /// A job ending the processes in it when it's closed: when this program ends, however it ends.
    pub struct Job(*mut c_void);

    impl Job {
        /// A job holding `child`, None where Windows refuses one.
        pub fn holding(child: &std::process::Child) -> Option<Job> {
            // SAFETY: no attributes, no name: a new job of this process's own
            let job = Job(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) });
            if job.0.is_null() {
                return None;
            }
            let mut limits = ExtendedLimits::default();
            limits.basic.limit_flags = KILL_ON_JOB_CLOSE;
            let size = std::mem::size_of::<ExtendedLimits>() as u32;
            // SAFETY: the job just made, and limits of the size given, which outlive the call
            let set = unsafe { SetInformationJobObject(job.0, EXTENDED_LIMITS, (&raw const limits).cast(), size) };
            // SAFETY: the job, and the child's process handle, open while `child` lives
            (set != 0 && unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } != 0).then_some(job)
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the job's handle, closed once
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Lets the process `id` bring its window to the front, as the front end that started this
    /// program could.
    pub fn let_take_focus(id: u32) {
        // SAFETY: a plain call on a process ID
        unsafe { AllowSetForegroundWindow(id) };
    }

    /// Closes the console window Windows opened for this program, where it's this program's alone
    /// (as when a front end starts it), so none stands behind the game; a console it shares, as a
    /// command prompt's, is left.
    pub fn let_go_of_console() {
        let mut list = [0u32; 2];
        // SAFETY: room for two IDs, as said
        if unsafe { GetConsoleProcessList(list.as_mut_ptr(), 2) } == 1 {
            // SAFETY: a plain call
            unsafe { FreeConsole() };
        }
    }

    /// A message box over everything, with the app's name.
    pub fn message(text: &str) {
        let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (text, caption) = (wide(text), wide("Visual Pinball Infinite"));
        // MB_ICONWARNING, MB_SETFOREGROUND, MB_TOPMOST
        // SAFETY: NUL-ended texts, no owner window
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x30 | 0x1_0000 | 0x4_0000) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vpx(version: [u16; 4]) -> Vpx {
        Vpx { program: PathBuf::from(r"C:\VPX\VPinballX64.exe"), version }
    }

    fn args(of: &[&str]) -> Vec<OsString> {
        of.iter().map(OsString::from).collect()
    }

    /// A front end's paths given from its own folder made whole from it, as VPX starts in its own;
    /// whole paths, a network share's and the other switches' words as they came.
    #[test]
    fn a_front_ends_paths_made_whole_from_its_own_folder() {
        let from = Path::new(r"C:\vPinball");
        let out: Vec<String> = with_whole_paths(
            args(&["-minimized", "-play", r"VisualPinball\Tables\Attack from Mars.vpx", "-Ini", r"C:\vPinball\VisualPinball\VPinballX.ini", "-TableIni", r"\\cabinet\share\afm.ini", "-LessCPUthreads", "2"]),
            from,
        )
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
        assert_eq!(
            out,
            vec!["-minimized", "-play", r"C:\vPinball\VisualPinball\Tables\Attack from Mars.vpx", "-Ini", r"C:\vPinball\VisualPinball\VPinballX.ini", "-TableIni", r"\\cabinet\share\afm.ini", "-LessCPUthreads", "2"]
        );
        // the tournament switch takes two paths; a path from the drive's top keeps its drive
        let out: Vec<String> = with_whole_paths(args(&["/TournamentFile", "afm.vpx", r"Results\afm.txt", "-play", r"\Tables\x.vpx"]), from).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(out, vec!["/TournamentFile", r"C:\vPinball\afm.vpx", r"C:\vPinball\Results\afm.txt", "-play", r"C:\Tables\x.vpx"]);
    }

    fn given(command: &Command) -> Vec<String> {
        command.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn a_front_ends_command_line_names_its_table() {
        assert_eq!(table_in(&args(&["-minimized", "-Play", r"C:\Tables\Medieval Madness.vpx"])), Some(PathBuf::from(r"C:\Tables\Medieval Madness.vpx")));
        assert_eq!(table_in(&args(&["/play", r"C:\Tables\T.vpx", "-exit"])), Some(PathBuf::from(r"C:\Tables\T.vpx")));
        assert_eq!(table_in(&args(&[r"C:\Tables\T.vpx", "--shot", "t.png"])), None);
        assert_eq!(table_in(&args(&["-Play"])), None);
    }

    #[test]
    fn vpx_gets_the_front_ends_own_command_line() {
        let line = args(&["-minimized", "-Play", r"C:\Tables\T.vpx"]);
        // 10.8.1 as given; 10.8.0 too, as it says minimized
        assert_eq!(given(&command(&vpx([10, 8, 1, 6758]), line.clone())), ["-minimized", "-Play", r"C:\Tables\T.vpx"]);
        assert_eq!(given(&command(&vpx([10, 8, 0, 2051]), line)), ["-minimized", "-Play", r"C:\Tables\T.vpx"]);
        // 10.8.0 started minimized where the line doesn't say, so it quits after the game
        let bare = args(&["-Play", r"C:\Tables\T.vpx"]);
        assert_eq!(given(&command(&vpx([10, 8, 0, 2051]), bare.clone())), ["-ExtMinimized", "-Play", r"C:\Tables\T.vpx"]);
        assert_eq!(given(&command(&vpx([10, 8, 1, 6758]), bare)), ["-Play", r"C:\Tables\T.vpx"]);
        assert_eq!(command(&vpx([10, 8, 1, 6758]), Vec::new()).get_current_dir(), Some(Path::new(r"C:\VPX")));
    }

    #[test]
    fn a_tables_own_line_is_found_by_its_path() {
        let user = serde_json::json!({ "play_with": { r"c:\tables\jaws.vpx": "10.8.0" } });
        assert_eq!(line_in(&user, Path::new(r"C:\Tables\Jaws.vpx")).as_deref(), Some("10.8.0"));
        assert_eq!(line_in(&user, Path::new(r"C:\Tables\Other.vpx")), None);
        assert_eq!(line_in(&serde_json::json!({}), Path::new(r"C:\Tables\Jaws.vpx")), None);
    }
}
