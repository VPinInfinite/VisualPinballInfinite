//! What the table's script and its engine report: script errors, the
//! engine's warnings, and the first use of anything not yet supported, each
//! said once.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// A script error shown on the playfield as VPX 5436 shows it (`Player::OnScriptError`'s
/// notification): "<kind> error: <description>", for five seconds, at most 200 in a game.
static NOTICE: Mutex<Option<(String, Instant)>> = Mutex::new(None);
static NOTICES: AtomicU32 = AtomicU32::new(0);
const NOTICE_FOR: Duration = Duration::from_secs(5);
const MOST_NOTICES: u32 = 200;

thread_local! {
    static SAID: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    /// Distinct script errors and unsupported members said so far.
    static COUNTS: Cell<(u32, u32)> = const { Cell::new((0, 0)) };
}

/// How many distinct script errors and unsupported members were said since
/// the last [`reset`].
pub fn counts() -> (u32, u32) {
    COUNTS.get()
}

/// Forgets what was said, for the next table run on this thread.
pub fn reset() {
    SAID.with(|s| s.borrow_mut().clear());
    COUNTS.set((0, 0));
    NOTICES.store(0, Ordering::Release);
    *NOTICE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The script error to show now, if any (the window thread).
pub fn notice() -> Option<String> {
    let shown = NOTICE.lock().unwrap_or_else(|e| e.into_inner());
    shown.as_ref().filter(|(_, until)| Instant::now() < *until).map(|(text, _)| text.clone())
}

/// Prints a line, once per distinct text.
pub fn once(text: &str) {
    let first = SAID.with(|s| s.borrow_mut().insert(text.to_string()));
    if first {
        eprintln!("[script] {text}");
    }
}

/// A message from libwinevbs: warnings and errors only.
pub fn engine_message(level: i32, text: &str) {
    // LIBWINEVBS_LOG_INFO, DEBUG, WARN, ERROR
    if level >= 2 && !text.is_empty() {
        once(&format!("engine: {text}"));
    }
}

/// An error in the table's script, said the first time it happens (a
/// timer can hit the same error a thousand times a second), and shown on the
/// playfield each time until 200 have been, as VPX does; `compile`, found
/// as the script was read rather than as it ran.
pub fn script_error(line: u32, column: i32, description: &str, source_line: &str, compile: bool) {
    if NOTICES.fetch_add(1, Ordering::AcqRel) < MOST_NOTICES {
        let kind = if compile { "Compile" } else { "Runtime" };
        let text: String = format!("{kind} error: {description}").chars().take(90).collect();
        *NOTICE.lock().unwrap_or_else(|e| e.into_inner()) = Some((text, Instant::now() + NOTICE_FOR));
    }
    let key = format!("error on line {line}, column {column}: {description}");
    let first = SAID.with(|s| s.borrow_mut().insert(key.clone()));
    if first {
        COUNTS.set((COUNTS.get().0 + 1, COUNTS.get().1));
        eprintln!("[script] {key}");
        if !source_line.trim().is_empty() {
            eprintln!("[script]     {}", source_line.trim());
        }
    }
}

/// A member the script used that we don't support yet: said once per kind
/// of object and member, naming the first part that used it.
pub fn unsupported(kind: &str, member: &str, part: Option<&str>) {
    let key = format!("not supported yet: {kind}.{member}");
    let first = SAID.with(|s| s.borrow_mut().insert(key.clone()));
    if first {
        COUNTS.set((COUNTS.get().0, COUNTS.get().1 + 1));
        match part {
            Some(p) => eprintln!("[script] {key} (first: {p})"),
            None => eprintln!("[script] {key}"),
        }
    }
}
