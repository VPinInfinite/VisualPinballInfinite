//! The fence's rule for files: where a table's script may read and
//! write through `Scripting.FileSystemObject`. Our build of libwinevbs
//! (`tools/libwinevbs`) asks [`allow`] about every path its file
//! object opens, creates, lists or looks up, after making it absolute
//! (`libwinevbs_set_file_fence`). Scripts write only in the player's data
//! folder for tables — the one `UserDirectory` names — and read there, in the
//! table's own folder and in the shared scripts' folders. A relative path is
//! taken from the data folder.

use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Where scripts may write, and where they may also read: each folder
/// lowercase, `\`-separated and ending in one.
#[derive(Debug, Default, PartialEq)]
struct Rule {
    write: Vec<String>,
    read: Vec<String>,
}

static RULE: RwLock<Option<Rule>> = RwLock::new(None);

/// Whether the loaded libwinevbs holds its file object to the rule. VPX's
/// own build of it can't, and then the fence refuses the file object.
static FENCED: AtomicBool = AtomicBool::new(false);

/// A folder as the rule keeps it.
fn folder(dir: &Path) -> String {
    let mut s = dir.to_string_lossy().replace('/', "\\").to_lowercase();
    if !s.ends_with('\\') {
        s.push('\\');
    }
    s
}

/// Whether `path` (absolute, lowercase, as libwinevbs hands it) is `folder`
/// or lies inside it.
fn within(path: &str, folder: &str) -> bool {
    path.starts_with(folder) || (path.len() + 1 == folder.len() && folder.starts_with(path))
}

impl Rule {
    fn new(data: &Path, read: &[PathBuf]) -> Rule {
        Rule { write: vec![folder(data)], read: read.iter().map(|d| folder(d)).collect() }
    }

    fn allows(&self, path: &str, write: bool) -> bool {
        // a table the user let outside its files reaches every file
        if super::fence::open() {
            return true;
        }
        let path = path.to_lowercase();
        self.write.iter().any(|f| within(&path, f)) || (!write && self.read.iter().any(|f| within(&path, f)))
    }
}

/// Sets the rule for a table's script: writing in `data` (its
/// `UserDirectory`), reading there and in `read` (the table's folder and the
/// shared scripts'). Returns whether libwinevbs holds its file object to it.
pub fn set(data: &Path, read: &[PathBuf]) -> bool {
    *RULE.write().unwrap_or_else(|e| e.into_inner()) = Some(Rule::new(data, read));
    let fenced = super::winevbs::get().ok().and_then(|lib| lib.set_file_fence).is_some_and(|set_fence| {
        let base: Vec<u16> = data.to_string_lossy().encode_utf16().chain(Some(0)).collect();
        // SAFETY: libwinevbs copies the base folder; `allow` lives as long as the program
        unsafe { set_fence(Some(allow), base.as_ptr()) };
        true
    });
    FENCED.store(fenced, Ordering::Release);
    fenced
}

/// Whether the file object is held to the rule (see [`set`]).
pub fn fenced() -> bool {
    FENCED.load(Ordering::Acquire)
}

/// Whether a file our own add-ons open for a table (FlexDMD's pictures and
/// fonts) lies where the table's script may read. With no rule set (a
/// table played without its script), the table's own folders are trusted.
pub fn readable(path: &Path) -> bool {
    let path = path.to_string_lossy().replace('/', "\\");
    RULE.read().unwrap_or_else(|e| e.into_inner()).as_ref().is_none_or(|r| r.allows(&path, false))
}

/// libwinevbs's question: may the script reach `path`, for writing when
/// `write` isn't 0?
unsafe extern "C" fn allow(path: *const u16, write: i32) -> i32 {
    if path.is_null() {
        return 0;
    }
    // SAFETY: a NUL-terminated path from libwinevbs
    let path = unsafe { super::com::from_wide(path) };
    let allowed = RULE.read().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|r| r.allows(&path, write != 0));
    if !allowed {
        let what = if write != 0 { "writing" } else { "reading" };
        super::log::once(&format!("the fence kept a table's script from {what} {path}"));
    }
    i32::from(allowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule() -> Rule {
        Rule::new(
            Path::new(r"C:\Users\Someone\AppData\Roaming\Visual Pinball Infinite\User"),
            &[PathBuf::from(r"C:\vPinball\VisualPinball\Tables"), PathBuf::from(r"C:\vPinball\VisualPinball\Scripts\")],
        )
    }

    #[test]
    fn writes_only_in_the_data_folder() {
        let r = rule();
        assert!(r.allows(r"C:\Users\Someone\AppData\Roaming\Visual Pinball Infinite\User\scores.txt", true));
        assert!(r.allows(r"C:\USERS\someone\appdata\roaming\visual pinball infinite\user\Sub\x.txt", true));
        assert!(!r.allows(r"C:\vPinball\VisualPinball\Tables\scores.txt", true));
        assert!(!r.allows(r"C:\Windows\x.txt", true));
    }

    #[test]
    fn reads_in_the_table_and_scripts_folders_too() {
        let r = rule();
        assert!(r.allows(r"C:\vPinball\VisualPinball\Tables\Some Table.vpx", false));
        assert!(r.allows(r"C:\vPinball\VisualPinball\Scripts\core.vbs", false));
        assert!(!r.allows(r"C:\vPinball\VisualPinball\VPinMAME\nvram\x.nv", false));
        assert!(!r.allows(r"C:\Users\Someone\Documents\secret.txt", false));
    }

    #[test]
    fn a_folder_itself_counts_but_not_one_beginning_alike() {
        let r = rule();
        // FolderExists(UserDirectory): libwinevbs hands the folder without its separator
        assert!(r.allows(r"C:\Users\Someone\AppData\Roaming\Visual Pinball Infinite\User", true));
        assert!(!r.allows(r"C:\Users\Someone\AppData\Roaming\Visual Pinball Infinite\UserData\x.txt", true));
        assert!(!r.allows(r"C:\vPinball\VisualPinball\TablesOld\x.txt", false));
    }

    #[test]
    fn nothing_without_a_rule() {
        assert!(!Rule::default().allows(r"C:\x.txt", false));
    }

    /// A script's file object through our libwinevbs, behind the fence:
    /// throwaway folders under the temp folder stand for the data folder,
    /// the table's folder and somewhere else.
    #[test]
    fn a_script_writes_only_in_its_data_folder() {
        use super::super::com::{VT_BOOL, VT_BSTR, VT_I4};
        use super::super::host::ScriptHost;
        let root = std::env::temp_dir().join(format!("vpi-file-fence-{}", std::process::id()));
        let (data, table, outside) = (root.join("User"), root.join("Tables"), root.join("Outside"));
        for d in [&data, &table, &outside] {
            std::fs::create_dir_all(d).expect("make a test folder");
        }
        std::fs::write(table.join("table.txt"), "beside the table\n").expect("write a test file");
        std::fs::write(outside.join("secret.txt"), "elsewhere\n").expect("write a test file");
        if !set(&data, std::slice::from_ref(&table)) {
            eprintln!("skipped: the libwinevbs found has no file fence (build tools/libwinevbs)");
            return;
        }
        let mut host = ScriptHost::new(&[]).expect("a script engine");
        host.load(concat!(
            "Function TryWrite(path)\n On Error Resume Next\n Dim f\n Set f = CreateObject(\"Scripting.FileSystemObject\").CreateTextFile(path, True)\n",
            " If Err.Number = 0 Then f.WriteLine \"x\" : f.Close\n TryWrite = Err.Number\nEnd Function\n",
            "Function TryRead(path)\n On Error Resume Next\n Dim f\n Set f = CreateObject(\"Scripting.FileSystemObject\").OpenTextFile(path, 1)\n",
            " If Err.Number = 0 Then f.ReadLine : f.Close\n TryRead = Err.Number\nEnd Function\n",
            "Function Exists(path)\n Exists = CreateObject(\"Scripting.FileSystemObject\").FileExists(path)\nEnd Function\n",
            "Function Absolute(path)\n Absolute = CreateObject(\"Scripting.FileSystemObject\").GetAbsolutePathName(path)\nEnd Function\n",
        ))
        .expect("the test script compiles");
        let lib = super::super::winevbs::get().expect("libwinevbs");
        let quoted = |p: &Path| p.to_string_lossy().replace('"', "\"\"");
        let number = |expression: String| {
            let mut v = host.evaluate(&expression).expect("the call runs");
            let n = if v.vt == VT_I4 { v.i32() } else { i32::from(v.i16()) };
            // SAFETY: a VARIANT the engine handed us
            unsafe { (lib.variant_clear)(&mut v) };
            n
        };
        let truth = |expression: String| {
            let v = host.evaluate(&expression).expect("the call runs");
            assert_eq!(v.vt, VT_BOOL);
            v.i16() != 0
        };

        assert_eq!(number(format!("TryWrite(\"{}\")", quoted(&data.join("scores.txt")))), 0);
        assert!(data.join("scores.txt").exists());
        assert_ne!(number(format!("TryWrite(\"{}\")", quoted(&table.join("scores.txt")))), 0);
        assert!(!table.join("scores.txt").exists());
        assert_ne!(number(format!("TryWrite(\"{}\")", quoted(&data.join("..").join("Outside").join("climbed.txt")))), 0);
        assert!(!outside.join("climbed.txt").exists());
        // a relative path lands in the data folder
        assert_eq!(number("TryWrite(\"relative.txt\")".into()), 0);
        assert!(data.join("relative.txt").exists());

        assert_eq!(number(format!("TryRead(\"{}\")", quoted(&table.join("table.txt")))), 0);
        assert_ne!(number(format!("TryRead(\"{}\")", quoted(&outside.join("secret.txt")))), 0);
        assert!(truth(format!("Exists(\"{}\")", quoted(&table.join("table.txt")))));
        assert!(!truth(format!("Exists(\"{}\")", quoted(&outside.join("secret.txt")))));

        let mut v = host.evaluate("Absolute(\".\")").expect("the call runs");
        assert_eq!(v.vt, VT_BSTR);
        // SAFETY: a BSTR the engine made, freed with the VARIANT
        let here = unsafe { lib.bstr_text(v.ptr()) };
        unsafe { (lib.variant_clear)(&mut v) };
        assert!(here.eq_ignore_ascii_case(&data.to_string_lossy()), "{here}");

        drop(host);
        let _ = std::fs::remove_dir_all(&root);
    }
}
