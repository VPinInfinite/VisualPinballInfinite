//! A PinUP pack: its layouts, the one in place, and how complete it is.
//!
//! A pack offers its layouts as option files at its top (`Option 2 - FullDMD on Backglass.bat`),
//! each copying one of its own folders' files to the top (`xcopy /y "PuP-Pack_Options\Option 2 -
//! FullDMD on Backglass\*"`), deleting what that layout doesn't use (`del /Q PuPInit.bat`) and now
//! and then writing a line to a file (`echo 2 > ScreenType.txt`). They're read here, never run: the
//! app makes their copies, deletes and lines itself, inside the pack's folder alone, and a file
//! asking for anything else — another program, a place outside the pack — is refused, with its
//! line, for the user to run themselves. A file replaced or deleted that the pack keeps no copy of
//! goes to the Recycle Bin. The layout in place is the one whose files are all there as it would
//! leave them. Some packs offer their layouts as small zips at their top instead
//! (`munsters_2screen.zip`), each holding the files that layout puts there (`screens.pup`,
//! `triggers.pup`, `playlists.pup`): a layout whose step is unzipping it into the top, read from the
//! zip, never taken for a pack still to unpack. The check is of what the player reads to show a
//! pack (`player::pup`): `screens.pup`, which screen shows what, at the pack's top, and the folders
//! its `playlists.pup` names. Ours.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

/// An option file's numbering: `Option 2 - `, `OPTION2_`, `opt1_ `, `1 - `.
static NUMBERING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(?:option|opt)\s*\d+\s*[-_.:)]*\s*|^\s*\d+\s*[-_.:)]+\s*").unwrap());

/// The helper files packs carry that aren't layouts: PinUP's pack editor, its start for the desktop
/// (`PuPInit.bat`, which PinUP runs), and tools for a pack's videos and sound.
const TOOLS: [&str; 6] = ["editthispuppack", "pupinit", "getcodec", "getlen", "normalizeaudio", "pupstandardallmedia"];

/// Files compared byte for byte up to this size; past it, by size and time (a copy keeps both).
const COMPARED: u64 = 64 << 20;

/// One of a pack's layouts.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// Its option file, as the pack names it.
    pub file: String,
    /// Its name as its author gave it, the file's numbering taken off.
    pub name: String,
    /// Why the app won't apply it, in words; None: it can.
    pub refused: Option<String>,
    /// How it's applied where its file names a folder that isn't there and another plainly is the
    /// one meant.
    pub note: Option<String>,
    #[serde(skip)]
    steps: Vec<Step>,
}

/// What a line of an option file does.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    /// The files in the folder `from` matching `pattern` copied into the folder `to` (one file under
    /// another name where `rename` gives one), and its subfolders' where `deep` (the empty ones too
    /// where `empty`), hidden files too where `hidden`. Folders are the pack's own, from its top.
    Copy { from: PathBuf, pattern: String, to: PathBuf, rename: Option<String>, deep: bool, empty: bool, hidden: bool },
    /// The files in `at` matching `pattern` deleted, in its subfolders too where `deep`.
    Delete { at: PathBuf, pattern: String, deep: bool },
    /// A file written with a line (`echo … > file`), or the line added to its end (`>>`).
    Write { file: PathBuf, text: String, append: bool },
    /// A file renamed where it is.
    Rename { file: PathBuf, to: String },
    /// A folder made.
    Folder { at: PathBuf },
    /// A layout's zip at the pack's top unzipped into it: its files, by name, with their bytes.
    Unzip { zip: PathBuf, files: Vec<(String, Vec<u8>)> },
}

/// A zip at the pack's top that's a layout: small, holding only files for its top (no folders),
/// one of them the pack's screens, triggers or playlists. Its files with their bytes.
fn layout_zip(path: &Path) -> Option<Vec<(String, Vec<u8>)>> {
    use std::io::Read;
    const SMALL: u64 = 4 << 20;
    if std::fs::metadata(path).ok()?.len() > SMALL {
        return None;
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut files = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).ok()?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        // its top only, and nothing that climbs out of it
        if name.contains('/') || name.contains("..") || entry.size() > SMALL {
            return None;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).ok()?;
        files.push((name, bytes));
    }
    let pup = |n: &str| ["screens.pup", "triggers.pup", "playlists.pup"].iter().any(|k| n.eq_ignore_ascii_case(k));
    files.iter().any(|(n, _)| pup(n)).then_some(files)
}

/// The pack's layouts kept as zips at its top.
fn zip_layouts(pack: &Path) -> Vec<Layout> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(pack).into_iter().flatten().flatten() {
        let path = e.path();
        if !path.is_file() || !path.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) {
            continue;
        }
        if let Some(files) = layout_zip(&path) {
            let file = e.file_name().to_string_lossy().into_owned();
            out.push(Layout { name: name_of(&file), file, refused: None, note: None, steps: vec![Step::Unzip { zip: PathBuf::from(e.file_name()), files }] });
        }
    }
    out
}

/// A pack's layouts, in their authors' order, and the one in place.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layouts {
    pub layouts: Vec<Layout>,
    /// The layout in place, by its file.
    pub current: Option<String>,
}

impl Layouts {
    /// The layout in place, if one is.
    pub fn in_place(&self) -> Option<&Layout> {
        self.current.as_deref().and_then(|f| self.layouts.iter().find(|l| l.file == f))
    }
}

/// The pack at `pack`'s layouts, read from its option files, and the one in place.
pub fn layouts(pack: &Path) -> Layouts {
    layouts_with(pack, None)
}

/// The pack's layouts and the one in place, the layout last chosen for it (`chosen`, its file)
/// counting first wherever its files are in place: some packs carry two layouts whose files are
/// the same, which their files alone can't tell apart.
pub fn layouts_with(pack: &Path, chosen: Option<&str>) -> Layouts {
    let mut found: Vec<Layout> = option_files(pack).into_iter().filter_map(|(file, text)| read_layout(pack, &file, &text)).collect();
    found.extend(zip_layouts(pack));
    found.sort_by_key(|l| natural(&l.file));
    // of those in place, the one chosen, else the one leaving the most files (the others' are part
    // of it), the first as their authors number them where two leave as many
    let mut current: Option<(usize, &Layout)> = None;
    for l in &found {
        if let Some(n) = in_place(pack, l) {
            if chosen.is_some_and(|c| c.eq_ignore_ascii_case(&l.file)) {
                current = Some((usize::MAX, l));
            } else if current.is_none_or(|(best, _)| n > best) {
                current = Some((n, l));
            }
        }
    }
    let current = current.map(|(_, l)| l.file.clone());
    Layouts { layouts: found, current }
}

/// The batch files at the pack's top that aren't its tools, with their text.
fn option_files(pack: &Path) -> Vec<(String, String)> {
    std::fs::read_dir(pack)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let low = file.to_lowercase();
            if !(low.ends_with(".bat") || low.ends_with(".cmd")) || TOOLS.iter().any(|t| low.starts_with(t)) {
                return None;
            }
            Some((file, text_of(&std::fs::read(e.path()).ok()?)))
        })
        .collect()
}

/// A file's text: UTF-8, else each byte a character (as the player reads a pack's files).
fn text_of(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_string(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// An option file's name as its author gave it, without its numbering and extension:
/// `Option 2 - FullDMD on Backglass.bat` is `FullDMD on Backglass`, `1 - Desktop.bat` is `Desktop`,
/// and a name written with no spaces has its underscores for them (`OPTION5_FullDMD_Topper.bat`
/// is `FullDMD Topper`; `stwr_107 ROM` keeps its ROM's name).
pub fn name_of(file: &str) -> String {
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    let rest = NUMBERING.replace(stem, "");
    let rest = if rest.contains(' ') { rest.into_owned() } else { rest.replace('_', " ") };
    let tidy = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    let tidy = tidy.trim_matches(|c: char| c == '-' || c.is_whitespace());
    if tidy.is_empty() { stem.trim().to_string() } else { tidy.to_string() }
}

/// A name's order as people count: its runs of digits as numbers (`Option 2` before `Option 10`).
fn natural(name: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut digits = String::new();
    let mut text = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_digit() {
            if !text.is_empty() {
                out.push((u64::MAX, std::mem::take(&mut text)));
            }
            digits.push(c);
        } else {
            if !digits.is_empty() {
                out.push((digits.parse().unwrap_or(u64::MAX), String::new()));
                digits.clear();
            }
            text.push(c);
        }
    }
    if !digits.is_empty() {
        out.push((digits.parse().unwrap_or(u64::MAX), String::new()));
    }
    if !text.is_empty() {
        out.push((u64::MAX, text));
    }
    out
}

/// What a line comes to.
#[derive(Debug)]
enum Said {
    Nothing,
    Steps(Vec<Step>),
    /// The file ends here (`exit`, `goto :eof`).
    Stop,
}

/// An option file read: None where it copies none of the pack's own folders (a tool of the
/// pack's, not a layout).
fn read_layout(pack: &Path, file: &str, text: &str) -> Option<Layout> {
    let mut steps = Vec::new();
    let mut refused = None;
    let mut a_layout = false;
    for raw in text.lines() {
        let line = raw.trim().trim_start_matches('@').trim();
        if line.is_empty() {
            continue;
        }
        let (verb, rest) = verb_of(line);
        // a layout copies from one of the pack's folders, whether the app can make that copy or not
        if matches!(verb.as_str(), "xcopy" | "copy" | "robocopy") && args(rest).map_or(line.contains('\\'), |(words, _)| words.first().is_some_and(|w| w.contains(['\\', '/']))) {
            a_layout = true;
        }
        match said(pack, line) {
            Ok(Said::Nothing) => {}
            Ok(Said::Steps(s)) => steps.extend(s),
            Ok(Said::Stop) => break,
            Err(why) => {
                refused.get_or_insert(why);
            }
        }
    }
    if !a_layout {
        return None;
    }
    let mut note = None;
    for step in &mut steps {
        let Step::Copy { from, pattern, .. } = step else { continue };
        if from.as_os_str().is_empty() || pack.join(&*from).is_dir() {
            continue;
        }
        match stand_in(pack, file, from, pattern) {
            Some((found, why)) => {
                note.get_or_insert(why);
                *from = found;
            }
            None => {
                refused.get_or_insert(format!("It copies from {}, which isn't in the pack: the pack may be incomplete.", shown(from)));
            }
        }
    }
    Some(Layout { file: file.to_string(), name: name_of(file), refused, note, steps })
}

/// A folder in the pack to copy from in place of `from`, which isn't there, where one plainly is
/// the one meant: beside it, the folder named as the option file (`(4 Screens)` for `(4
/// Screens).bat`); else the folder above it, where it holds the option's own file and the files
/// asked for (its files packed a folder too high). With the note saying so.
fn stand_in(pack: &Path, file: &str, from: &Path, pattern: &str) -> Option<(PathBuf, String)> {
    let above = from.parent()?;
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    let named = above.join(stem);
    if pack.join(&named).is_dir() {
        return Some((named.clone(), format!("Its option file points to a folder that isn't there ({}). There's a folder named for this layout, {}, so the app uses that one.", shown(from), shown(&named))));
    }
    let entries: Vec<String> = std::fs::read_dir(pack.join(above)).ok()?.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    let own = entries.iter().any(|n| n.rsplit_once('.').is_some_and(|(s, _)| s.eq_ignore_ascii_case(stem)));
    let asked = entries.iter().any(|n| matches(pattern, n) && !n.to_lowercase().ends_with(".bat"));
    (own && asked).then(|| (above.to_path_buf(), format!("Packed slightly wrong by its author: its option file looks for its files in a folder that isn't there ({}), but they're one folder up. The app finds them there, so this layout works.", shown(from))))
}

/// A place in the pack as people read it.
fn shown(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\")
}

/// A line's command, lowercase, and what follows it.
fn verb_of(line: &str) -> (String, &str) {
    let end = line.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')).unwrap_or(line.len());
    (line[..end].to_lowercase(), &line[end..])
}

/// A line cut for a message.
fn quoted(line: &str) -> String {
    let short: String = line.chars().take(80).collect();
    if short.len() < line.len() { format!("“{short}…”") } else { format!("“{short}”") }
}

/// What a line of an option file does, as cmd would run it in the pack's folder; an error, in
/// words, for one the app doesn't make out or won't do.
fn said(pack: &Path, line: &str) -> Result<Said, String> {
    if line.starts_with(':') {
        // a comment (`::`) or a place to jump to
        return Ok(Said::Nothing);
    }
    let (verb, _) = verb_of(line);
    if verb == "rem" {
        return Ok(Said::Nothing);
    }
    let line = expand(pack, line)?;
    let (verb, rest) = verb_of(&line);
    let refuse = || format!("Its file runs {}, which the app doesn't do itself.", quoted(&line));
    match verb.as_str() {
        "echo" => echo(pack, rest),
        "pause" | "cls" | "title" | "color" | "setlocal" | "endlocal" | "chcp" | "timeout" | "popd" | "ver" => Ok(Said::Nothing),
        "exit" => Ok(Said::Stop),
        "goto" if rest.trim().eq_ignore_ascii_case(":eof") => Ok(Said::Stop),
        "cd" | "chdir" | "pushd" => {
            let (words, _) = args(rest)?;
            // the pack's own folder, where cmd starts it
            match words.as_slice() {
                [w] if place(pack, w)?.as_os_str().is_empty() => Ok(Said::Nothing),
                [] => Ok(Said::Nothing),
                _ => Err(format!("Its file moves to another folder ({}), which the app doesn't follow.", quoted(&line))),
            }
        }
        "xcopy" => xcopy(pack, rest),
        "copy" => copy(pack, rest),
        "del" | "erase" => del(pack, rest),
        "ren" | "rename" => {
            let (words, switches) = args(rest)?;
            match (words.as_slice(), switches.is_empty()) {
                ([from, to], true) if !to.contains(['\\', '/', '*', '?']) && !from.contains(['*', '?']) => Ok(Said::Steps(vec![Step::Rename { file: place(pack, from)?, to: to.clone() }])),
                _ => Err(refuse()),
            }
        }
        "md" | "mkdir" => {
            let (words, switches) = args(rest)?;
            if !switches.is_empty() || words.is_empty() {
                return Err(refuse());
            }
            Ok(Said::Steps(words.iter().map(|w| place(pack, w).map(|at| Step::Folder { at })).collect::<Result<_, _>>()?))
        }
        _ => Err(refuse()),
    }
}

/// The line with the pack's folder put in for cmd's own (`%~dp0`, the file's folder, and `%cd%`,
/// where it runs); a line using any other of Windows' settings refused.
fn expand(pack: &Path, line: &str) -> Result<String, String> {
    let folder = pack.to_string_lossy();
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let low = after.to_lowercase();
        if low.starts_with("%~dp0") {
            out.push_str(&folder);
            out.push('\\');
            rest = &after[5..];
        } else if low.starts_with("%cd%") {
            out.push_str(&folder);
            rest = &after[4..];
        } else {
            return Err(format!("Its file uses one of Windows' settings ({}), which the app doesn't fill in.", quoted(line)));
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// `echo`: a line shown, which does nothing here, or written to a file (`>`), or added to one (`>>`).
fn echo(pack: &Path, rest: &str) -> Result<Said, String> {
    let mut quote = false;
    let Some(at) = rest.char_indices().find(|&(_, c)| {
        if c == '"' {
            quote = !quote;
        }
        c == '>' && !quote
    }).map(|(i, _)| i) else {
        return Ok(Said::Nothing);
    };
    if rest[..at].contains(['|', '&', '<']) {
        return Err(format!("Its file runs {}, which the app doesn't do itself.", quoted(&format!("echo{rest}"))));
    }
    // cmd's text: all after the one character parting it from `echo`, blanks kept
    let text: String = rest[..at].chars().skip(1).collect();
    let (append, target) = match rest[at + 1..].strip_prefix('>') {
        Some(t) => (true, t),
        None => (false, &rest[at + 1..]),
    };
    let target = target.trim().trim_matches('"').trim();
    if target.eq_ignore_ascii_case("nul") {
        return Ok(Said::Nothing);
    }
    if target.is_empty() || target.contains(['*', '?', '>', '<', '|', '&']) {
        return Err(format!("Its file writes {}, which the app doesn't make out.", quoted(&format!("echo{rest}"))));
    }
    Ok(Said::Steps(vec![Step::Write { file: place(pack, target)?, text, append }]))
}

/// A command's arguments as cmd splits them: words apart at blanks, a quoted part kept whole and
/// its quotes taken off, and its switches (`/y`, lowercase; `/y/s` two) apart. Output sent to
/// `nul` is dropped; any other redirect, a pipe or a second command refused.
fn args(rest: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let refuse = || format!("Its file runs {}, which the app doesn't do itself.", quoted(rest.trim()));
    let mut words = Vec::new();
    let mut switches = Vec::new();
    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if matches!(c, '|' | '&' | '<') {
            return Err(refuse());
        }
        if c == '>' || (matches!(c, '1' | '2') && chars.get(i + 1) == Some(&'>')) {
            // a redirect: only to nul, or the errors to the output (`2>&1`)
            i += if c == '>' { 1 } else { 2 };
            if chars.get(i) == Some(&'>') {
                i += 1;
            }
            while chars.get(i).is_some_and(|c| c.is_whitespace()) {
                i += 1;
            }
            let start = i;
            while chars.get(i).is_some_and(|c| !c.is_whitespace()) {
                i += 1;
            }
            let target: String = chars[start..i].iter().collect();
            if !(target.eq_ignore_ascii_case("nul") || target == "&1") {
                return Err(refuse());
            }
            continue;
        }
        if c == '/' {
            let start = i + 1;
            i += 1;
            while chars.get(i).is_some_and(|c| !c.is_whitespace() && *c != '/' && *c != '"') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            if !s.is_empty() {
                switches.push(s.to_lowercase());
            }
            continue;
        }
        let mut word = String::new();
        let mut quote = false;
        while i < chars.len() {
            let c = chars[i];
            if c == '"' {
                quote = !quote;
                i += 1;
                // a switch straight after a closing quote is its own (`"a"/y`)
                if !quote && chars.get(i) == Some(&'/') {
                    break;
                }
                continue;
            }
            if !quote && (c.is_whitespace() || matches!(c, '|' | '&' | '<' | '>')) {
                break;
            }
            word.push(c);
            i += 1;
        }
        words.push(word);
    }
    Ok((words, switches))
}

/// A place the line names, inside the pack, as a path from its top; refused where it's outside.
fn place(pack: &Path, text: &str) -> Result<PathBuf, String> {
    let outside = || Err(format!("It reaches outside the pack's folder ({text}), which the app doesn't do."));
    let t = text.trim().replace('/', "\\");
    let folder = pack.to_string_lossy().replace('/', "\\");
    let folder = folder.trim_end_matches('\\');
    let rel = if t.starts_with('\\') {
        return outside();
    } else if t.as_bytes().get(1) == Some(&b':') {
        // a whole path: the pack's own folder, or a place in it, its names compared as Windows does
        let n = folder.chars().count();
        let head: String = t.chars().take(n).collect();
        let rest: String = t.chars().skip(n).collect();
        if head.to_lowercase() != folder.to_lowercase() || !(rest.is_empty() || rest.starts_with('\\')) {
            return outside();
        }
        rest
    } else {
        t.clone()
    };
    let mut parts: Vec<&str> = Vec::new();
    let pieces: Vec<&str> = rel.split('\\').collect();
    for (k, p) in pieces.iter().enumerate() {
        match *p {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return outside();
                }
            }
            // wildcards only in the last part, as cmd takes them
            p if p.contains(['*', '?']) && k + 1 < pieces.len() => return Err(format!("It names {text}, which the app doesn't make out.")),
            p if p.contains(':') => return outside(),
            p => parts.push(p),
        }
    }
    Ok(parts.iter().collect())
}

/// A name matched to a pattern as cmd matches one: `*` any run, `?` one character, case aside;
/// `*.*` every file.
fn matches(pattern: &str, name: &str) -> bool {
    let pattern = if pattern == "*.*" { "*" } else { pattern };
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    // the classic two-pointer match, going back to the last star on a miss
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|&c| c == '*')
}

/// A source split as the copy commands take it: a folder and the files in it it means.
fn source(pack: &Path, text: &str) -> Result<(PathBuf, String, bool), String> {
    let at = place(pack, text)?;
    let last = at.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if last.contains(['*', '?']) {
        Ok((at.parent().map(Path::to_path_buf).unwrap_or_default(), last, true))
    } else if at.as_os_str().is_empty() || pack.join(&at).is_dir() {
        // a folder: all of it
        Ok((at, "*".into(), true))
    } else {
        Ok((at.parent().map(Path::to_path_buf).unwrap_or_default(), last, false))
    }
}

/// `xcopy source [destination] [switches]`.
fn xcopy(pack: &Path, rest: &str) -> Result<Said, String> {
    let (words, switches) = args(rest)?;
    for s in &switches {
        if !matches!(s.as_str(), "y" | "-y" | "q" | "f" | "s" | "e" | "i" | "h" | "r" | "k" | "c" | "v" | "z" | "j" | "l") {
            return Err(format!("Its file copies with xcopy's /{s}, which the app doesn't do itself."));
        }
    }
    if switches.iter().any(|s| s == "l") {
        // only lists what it would copy
        return Ok(Said::Nothing);
    }
    let (from, pattern, many) = match words.as_slice() {
        [src] | [src, _] => source(pack, src)?,
        _ => return Err(format!("Its file runs {}, which the app doesn't do itself.", quoted(&format!("xcopy{rest}")))),
    };
    let (to, rename) = match words.get(1) {
        None => (PathBuf::new(), None),
        Some(dest) => {
            let at = place(pack, dest)?;
            if dest.ends_with(['\\', '/']) || at.as_os_str().is_empty() || pack.join(&at).is_dir() || many {
                (at, None)
            } else if pack.join(&at).is_file() {
                (at.parent().map(Path::to_path_buf).unwrap_or_default(), at.file_name().map(|n| n.to_string_lossy().into_owned()))
            } else {
                return Err(format!("Its file copies to {dest}, which xcopy would ask about (a file or a folder?): run it yourself."));
            }
        }
    };
    let deep = switches.iter().any(|s| s == "s" || s == "e");
    if deep && to.starts_with(&from) && to != from {
        return Err("Its file copies a folder into itself.".into());
    }
    let (empty, hidden) = (switches.iter().any(|s| s == "e"), switches.iter().any(|s| s == "h"));
    Ok(Said::Steps(vec![Step::Copy { from, pattern, to, rename, deep, empty, hidden }]))
}

/// `copy [switches] source [destination]`.
fn copy(pack: &Path, rest: &str) -> Result<Said, String> {
    let (words, switches) = args(rest)?;
    for s in &switches {
        if !matches!(s.as_str(), "y" | "-y" | "b" | "a" | "v" | "z") {
            return Err(format!("Its file copies with copy's /{s}, which the app doesn't do itself."));
        }
    }
    let (src, dest) = match words.as_slice() {
        [src] => (src, None),
        [src, dest] => (src, Some(dest)),
        _ => return Err(format!("Its file runs {}, which the app doesn't do itself.", quoted(&format!("copy{rest}")))),
    };
    if src.contains('+') {
        return Err(format!("Its file joins files together ({}), which the app doesn't do itself.", quoted(&format!("copy{rest}"))));
    }
    let (from, pattern, many) = source(pack, src)?;
    let (to, rename) = match dest {
        None => (PathBuf::new(), None),
        Some(dest) => {
            let at = place(pack, dest)?;
            if dest.ends_with(['\\', '/']) || at.as_os_str().is_empty() || pack.join(&at).is_dir() {
                (at, None)
            } else if many {
                return Err(format!("Its file joins files together into {dest}, which the app doesn't do itself."));
            } else {
                (at.parent().map(Path::to_path_buf).unwrap_or_default(), at.file_name().map(|n| n.to_string_lossy().into_owned()))
            }
        }
    };
    Ok(Said::Steps(vec![Step::Copy { from, pattern, to, rename, deep: false, empty: false, hidden: false }]))
}

/// `del [switches] names…`: a folder named means the files in it.
fn del(pack: &Path, rest: &str) -> Result<Said, String> {
    let (words, switches) = args(rest)?;
    for s in &switches {
        if !matches!(s.as_str(), "q" | "f" | "s" | "p") {
            return Err(format!("Its file deletes with del's /{s}, which the app doesn't do itself."));
        }
    }
    if words.is_empty() {
        return Ok(Said::Nothing);
    }
    let deep = switches.iter().any(|s| s == "s");
    let mut steps = Vec::new();
    for w in &words {
        let at = place(pack, w)?;
        if at.as_os_str().is_empty() || pack.join(&at).is_dir() {
            steps.push(Step::Delete { at, pattern: "*".into(), deep });
        } else {
            let pattern = at.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            steps.push(Step::Delete { at: at.parent().map(Path::to_path_buf).unwrap_or_default(), pattern, deep });
        }
    }
    Ok(Said::Steps(steps))
}

/// Whether a file's hidden or the system's, which the copy and delete commands pass over unless
/// asked.
fn hidden(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        std::fs::metadata(path).is_ok_and(|m| m.file_attributes() & 0x6 != 0)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// The files in `folder` (from the pack's top) matching `pattern`, its subfolders' too where
/// `deep`, each with its path from `folder`.
fn files_in(pack: &Path, folder: &Path, pattern: &str, deep: bool, with_hidden: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![PathBuf::new()];
    while let Some(sub) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(pack.join(folder).join(&sub)) else { continue };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            if !with_hidden && hidden(&path) {
                continue;
            }
            let rel = sub.join(e.file_name());
            if path.is_dir() {
                if deep {
                    todo.push(rel);
                }
            } else if matches(pattern, &e.file_name().to_string_lossy()) {
                out.push(rel);
            }
        }
    }
    out
}

/// A copy step's files, as the pack holds them now: each from where to where (from its top).
fn pairs(pack: &Path, step: &Step) -> Vec<(PathBuf, PathBuf)> {
    let Step::Copy { from, pattern, to, rename, deep, hidden, .. } = step else { return Vec::new() };
    let found = files_in(pack, from, pattern, *deep, *hidden);
    let one = found.len() == 1;
    found
        .into_iter()
        .map(|rel| {
            let dest = match rename {
                Some(name) if one => to.join(name),
                _ => to.join(&rel),
            };
            (from.join(rel), dest)
        })
        .filter(|(a, b)| a != b)
        .collect()
}

/// A path's key for comparing places: lowercase, as Windows' folders compare names.
fn key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase().replace('/', "\\")
}

/// What a file should be once a layout is in place.
enum Should {
    /// The same as this file of the pack's.
    Same(PathBuf),
    /// Holding this text.
    Hold(String),
    /// Holding these bytes (a layout's zip's file).
    Bytes(Vec<u8>),
    Gone,
}

/// Two files the same: byte for byte, or past `COMPARED`, by size and modified time.
fn same(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else { return false };
    if ma.len() != mb.len() {
        return false;
    }
    if ma.len() > COMPARED {
        return ma.modified().ok() == mb.modified().ok();
    }
    matches!((std::fs::read(a), std::fs::read(b)), (Ok(x), Ok(y)) if x == y)
}

/// How the pack's files should be once `layout` is applied, from the files as they are now: by
/// each file's key, the file (from the pack's top) and what it should be.
fn wanted(pack: &Path, layout: &Layout) -> BTreeMap<String, (PathBuf, Should)> {
    let mut want: BTreeMap<String, (PathBuf, Should)> = BTreeMap::new();
    for step in &layout.steps {
        match step {
            Step::Copy { .. } => {
                for (from, to) in pairs(pack, step) {
                    want.insert(key(&to), (to, Should::Same(from)));
                }
            }
            Step::Delete { at, pattern, deep } => {
                for rel in files_in(pack, at, pattern, *deep, false) {
                    let file = at.join(rel);
                    want.insert(key(&file), (file, Should::Gone));
                }
                // what an earlier line put there goes too
                for (_, (file, should)) in want.iter_mut() {
                    let inside = file.parent().is_some_and(|p| if *deep { p.starts_with(at) } else { p == at.as_path() });
                    if inside && file.file_name().is_some_and(|n| matches(pattern, &n.to_string_lossy())) {
                        *should = Should::Gone;
                    }
                }
            }
            Step::Write { file, text, append: false } => {
                want.insert(key(file), (file.clone(), Should::Hold(format!("{text}\r\n"))));
            }
            Step::Unzip { files, .. } => {
                for (name, bytes) in files {
                    let file = PathBuf::from(name);
                    want.insert(key(&file), (file, Should::Bytes(bytes.clone())));
                }
            }
            Step::Write { append: true, .. } | Step::Rename { .. } | Step::Folder { .. } => {}
        }
    }
    want
}

/// How many files the layout leaves where it's in place (all its copies' files the same as their
/// sources, its lines written, the files it deletes gone); None where it isn't, or leaves none
/// to tell by.
fn in_place(pack: &Path, layout: &Layout) -> Option<usize> {
    if layout.refused.is_some() {
        return None;
    }
    let want = wanted(pack, layout);
    let mut left = 0;
    for (file, should) in want.values() {
        let there = pack.join(file);
        let ok = match should {
            Should::Same(from) => same(&pack.join(from), &there),
            Should::Hold(text) => std::fs::read(&there).is_ok_and(|b| b == text.as_bytes()),
            Should::Bytes(bytes) => std::fs::read(&there).is_ok_and(|b| &b == bytes),
            Should::Gone => !there.exists(),
        };
        if !ok {
            return None;
        }
        if !matches!(should, Should::Gone) {
            left += 1;
        }
    }
    (left > 0).then_some(left)
}

/// The files at the pack's top the layout `file` would leave there, each with the file of the
/// pack's it would be copied from: a layout tried without applying it, the player reading those in
/// the pack's own files' place.
pub fn overlay(pack: &Path, file: &str) -> Result<Vec<(String, PathBuf)>, String> {
    let all = layouts(pack);
    let layout = all.layouts.iter().find(|l| l.file.eq_ignore_ascii_case(file)).ok_or_else(|| format!("{file} isn't one of the pack's layouts."))?;
    if let Some(why) = &layout.refused {
        return Err(why.clone());
    }
    // a zip's files written to a passing folder of their own, for the player to read
    let passing = std::env::temp_dir().join(format!("vpi-pup-layout-{}", std::process::id()));
    let mut out = Vec::new();
    for (to, should) in wanted(pack, layout).into_values() {
        if !to.parent().is_none_or(|p| p.as_os_str().is_empty()) {
            continue;
        }
        match should {
            Should::Same(from) => out.push((to.to_string_lossy().into_owned(), pack.join(from))),
            Should::Bytes(bytes) => {
                std::fs::create_dir_all(&passing).map_err(|e| format!("{}: {e}", passing.display()))?;
                let file = passing.join(&to);
                std::fs::write(&file, bytes).map_err(|e| format!("{}: {e}", file.display()))?;
                out.push((to.to_string_lossy().into_owned(), file));
            }
            _ => {}
        }
    }
    Ok(out)
}

/// What applying a layout did.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    /// Files copied or written.
    pub copied: usize,
    /// Files already as the layout has them.
    pub same: usize,
    /// Files deleted.
    pub deleted: usize,
    /// Files put in the Recycle Bin, replaced or deleted, that the pack kept no copy of.
    pub recycled: usize,
    /// What couldn't be done, in words; the rest done, as cmd goes on past a line that fails.
    pub problems: Vec<String>,
}

/// The layout `file` of the pack at `pack` applied, read from its file again first: each line in
/// turn, inside the pack's folder alone. A file it replaces with another, or deletes, is handed to
/// `recycle` (the Recycle Bin) unless the pack keeps the same file elsewhere — one of its layouts'
/// own — when it's only replaced or deleted.
pub fn apply(pack: &Path, file: &str, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<Applied, String> {
    let all = layouts(pack);
    let layout = all.layouts.iter().find(|l| l.file.eq_ignore_ascii_case(file)).ok_or_else(|| format!("{file} isn't one of the pack's layouts."))?;
    if let Some(why) = &layout.refused {
        return Err(why.clone());
    }
    let top = std::fs::canonicalize(pack).map_err(|e| format!("The pack's folder couldn't be read: {e}"))?;
    // every file the pack's layouts copy, by name, for telling a file kept elsewhere in it
    let mut kept: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for l in &all.layouts {
        for step in &l.steps {
            for (from, _) in pairs(pack, step) {
                let name = from.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                kept.entry(name).or_default().push(pack.join(from));
            }
        }
    }
    // and every file the pack's layout zips hold, by name
    let mut zipped: BTreeMap<String, Vec<&[u8]>> = BTreeMap::new();
    for l in &all.layouts {
        for step in &l.steps {
            if let Step::Unzip { files, .. } = step {
                for (name, bytes) in files {
                    zipped.entry(name.to_lowercase()).or_default().push(bytes);
                }
            }
        }
    }
    let kept_elsewhere = |path: &Path| {
        let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        kept.get(&name).is_some_and(|copies| copies.iter().any(|c| c != path && same(c, path)))
            || zipped.get(&name).is_some_and(|copies| std::fs::read(path).is_ok_and(|b| copies.iter().any(|c| *c == b.as_slice())))
    };
    let mut done = Applied::default();
    // a file out of the way: deleted where the pack keeps it elsewhere, else to the Recycle Bin
    let out_of_way = |path: &Path, done: &mut Applied| -> bool {
        let r = if kept_elsewhere(path) { std::fs::remove_file(path).map_err(|e| e.to_string()) } else { recycle(path).map(|()| done.recycled += 1) };
        match r {
            Ok(()) => true,
            Err(e) => {
                done.problems.push(format!("{} couldn't be moved out of the way: {e}", shown(path.strip_prefix(pack).unwrap_or(path))));
                false
            }
        }
    };
    // a folder to write in, made, and still inside the pack once made (no link out of it)
    let folder_for = |path: &Path| -> Result<(), String> {
        let parent = path.parent().unwrap_or(pack);
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let real = std::fs::canonicalize(parent).map_err(|e| e.to_string())?;
        if real.starts_with(&top) { Ok(()) } else { Err("it's outside the pack's folder".into()) }
    };
    for step in &layout.steps {
        match step {
            Step::Copy { from, to, deep, empty, .. } => {
                if *deep && *empty {
                    let mut todo = vec![PathBuf::new()];
                    while let Some(sub) = todo.pop() {
                        for e in std::fs::read_dir(pack.join(from).join(&sub)).into_iter().flatten().flatten() {
                            if e.path().is_dir() {
                                let rel = sub.join(e.file_name());
                                let made = pack.join(to).join(&rel);
                                if let Err(e) = folder_for(&made.join("x")) {
                                    done.problems.push(format!("{} couldn't be made: {e}", shown(&to.join(&rel))));
                                }
                                todo.push(rel);
                            }
                        }
                    }
                }
                for (src, dest) in pairs(pack, step) {
                    let (src, there) = (pack.join(&src), pack.join(&dest));
                    if there.exists() {
                        if same(&src, &there) {
                            done.same += 1;
                            continue;
                        }
                        if !out_of_way(&there, &mut done) {
                            continue;
                        }
                    }
                    match folder_for(&there).and_then(|()| std::fs::copy(&src, &there).map_err(|e| e.to_string())) {
                        Ok(_) => done.copied += 1,
                        Err(e) => done.problems.push(format!("{} couldn't be copied: {e}", shown(&dest))),
                    }
                }
            }
            Step::Delete { at, pattern, deep } => {
                for rel in files_in(pack, at, pattern, *deep, false) {
                    if out_of_way(&pack.join(at).join(rel), &mut done) {
                        done.deleted += 1;
                    }
                }
            }
            Step::Write { file, text, append } => {
                let there = pack.join(file);
                let line = format!("{text}\r\n");
                if let Err(e) = folder_for(&there) {
                    done.problems.push(format!("{} couldn't be written: {e}", shown(file)));
                    continue;
                }
                if *append {
                    use std::io::Write;
                    match std::fs::OpenOptions::new().create(true).append(true).open(&there).and_then(|mut f| f.write_all(line.as_bytes())) {
                        Ok(()) => done.copied += 1,
                        Err(e) => done.problems.push(format!("{} couldn't be written: {e}", shown(file))),
                    }
                    continue;
                }
                if std::fs::read(&there).is_ok_and(|b| b == line.as_bytes()) {
                    done.same += 1;
                    continue;
                }
                if there.exists() && !out_of_way(&there, &mut done) {
                    continue;
                }
                match std::fs::write(&there, &line) {
                    Ok(()) => done.copied += 1,
                    Err(e) => done.problems.push(format!("{} couldn't be written: {e}", shown(file))),
                }
            }
            Step::Rename { file, to } => {
                let there = pack.join(file);
                let named = there.with_file_name(to);
                if !there.is_file() {
                    continue;
                }
                if named.exists() {
                    done.problems.push(format!("{} couldn't be renamed: {to} is there already", shown(file)));
                } else if let Err(e) = std::fs::rename(&there, &named) {
                    done.problems.push(format!("{} couldn't be renamed: {e}", shown(file)));
                }
            }
            Step::Folder { at } => {
                if let Err(e) = folder_for(&pack.join(at).join("x")) {
                    done.problems.push(format!("{} couldn't be made: {e}", shown(at)));
                }
            }
            Step::Unzip { files, .. } => {
                for (name, bytes) in files {
                    let there = pack.join(name);
                    if std::fs::read(&there).is_ok_and(|b| &b == bytes) {
                        done.same += 1;
                        continue;
                    }
                    if there.exists() && !out_of_way(&there, &mut done) {
                        continue;
                    }
                    match std::fs::write(&there, bytes) {
                        Ok(()) => done.copied += 1,
                        Err(e) => done.problems.push(format!("{name} couldn't be written: {e}")),
                    }
                }
            }
        }
    }
    Ok(done)
}

/// A pack's check: how it looks — good, warn, bad, or none for an empty folder, which has nothing
/// to play — a word or two for a list, and what it says.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Check {
    pub look: &'static str,
    pub short: &'static str,
    pub text: String,
}

fn verdict(look: &'static str, short: &'static str, text: impl Into<String>) -> Check {
    Check { look, short, text: text.into() }
}

/// The archives at the pack's top (zip, RAR, 7-Zip), in name order: a pack not unpacked yet. A
/// layout's zip isn't one.
pub fn archives(pack: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(pack)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| ["zip", "rar", "7z"].iter().any(|k| x.eq_ignore_ascii_case(k))))
        .filter(|p| !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) || layout_zip(p).is_none())
        .collect();
    found.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    found
}

/// The pack at `pack` checked as the player reads it, with its layouts (`layouts`).
pub fn check(pack: &Path, layouts: &Layouts) -> Check {
    let entries: Vec<std::fs::DirEntry> = match std::fs::read_dir(pack) {
        Ok(e) => e.flatten().collect(),
        Err(e) => return verdict("bad", "Can't be read", format!("Its PinUP pack's folder couldn't be read: {e}")),
    };
    if entries.is_empty() {
        return verdict("none", "Empty folder", "An empty folder, with nothing to play");
    }
    let has = |name: &str| entries.iter().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name) && e.path().is_file());
    if has("screens.pup") {
        let (named, missing) = playlists(pack);
        return match (named, missing) {
            (0, _) => verdict("good", "Complete", "Its screens are there"),
            (n, 0) => verdict("good", "Complete", format!("Its screens and all {n} of its folders are there")),
            (n, m) if m * 2 < n => verdict("good", "Complete", format!("{} of its {n} folders are there; the pack itself lacks {m}", n - m)),
            (n, m) => verdict("bad", "Files missing", format!("Its PinUP pack is missing {m} of the {n} folders it plays from, so most of it won't show: it may be incomplete, or an add-on to a full pack that isn't here (VPS lists the packs for its game).")),
        };
    }
    if has("scriptonly.txt") {
        return verdict("good", "Set by its script", "Its table's script sets up its screens, as the pack says");
    }
    if layouts.layouts.iter().any(|l| l.refused.is_none()) {
        return verdict("bad", "No layout chosen", "Its PinUP pack has no layout chosen yet, so it shows nothing: choose one for your screens with Layout.");
    }
    if !layouts.layouts.is_empty() {
        return verdict("bad", "Layout to run by hand", "Its PinUP pack has no layout in place, and the app can't apply its layouts itself: run one of its option files from the pack's folder.");
    }
    if let Some(sub) = entries.iter().find(|e| e.path().join("screens.pup").is_file()) {
        return verdict("bad", "A folder too deep", format!("Its PinUP pack's files are a folder down, in {}: the player reads them at the top of the pack's folder, so nothing shows.", sub.file_name().to_string_lossy()));
    }
    let packed = archives(pack);
    if !packed.is_empty() {
        let names: Vec<String> = packed.iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
        return verdict("bad", "Still zipped", format!("Its PinUP pack is still packed ({}): unpacked into its folder, it would play.", names.join(", ")));
    }
    verdict("warn", "No screens file", "Its PinUP pack has no screens.pup, the file saying which screen shows what, so it shows only what its table's script sets up itself.")
}

/// The folders the pack's `playlists.pup` names, and how many of them aren't there, as the player
/// reads them (`Playlist::from_csv`: a line of seven fields, its folder the second; one per folder).
fn playlists(pack: &Path) -> (usize, usize) {
    let Some(file) = std::fs::read_dir(pack).into_iter().flatten().flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("playlists.pup")) else {
        return (0, 0);
    };
    let Ok(bytes) = std::fs::read(file.path()) else { return (0, 0) };
    let mut seen = std::collections::HashSet::new();
    let (mut named, mut missing) = (0, 0);
    for line in text_of(&bytes).lines().skip(1) {
        let parts = csv_line(line);
        if parts.len() != 7 || !seen.insert(parts[1].to_lowercase()) {
            continue;
        }
        named += 1;
        if !pack.join(&parts[1]).is_dir() {
            missing += 1;
        }
    }
    (named, missing)
}

/// A line of a pack's lists split into its fields, quotes kept together (the player's
/// `parse_csv_line`).
fn csv_line(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    for c in line.trim_matches([' ', '\t', '\r', '\n']).chars() {
        match (quoted, c) {
            (false, '"') => quoted = true,
            (false, ',') => parts.push(std::mem::take(&mut field)),
            (true, '"') => quoted = false,
            _ => field.push(c),
        }
    }
    parts.push(field);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vpi-pup-pack-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn an_option_files_name_without_its_numbering() {
        assert_eq!(name_of("Option 2 - FullDMD on Backglass.bat"), "FullDMD on Backglass");
        assert_eq!(name_of("opt1_ 3 screens_pup split dmd-BG.bat"), "3 screens_pup split dmd-BG");
        assert_eq!(name_of("1 - Desktop.bat"), "Desktop");
        assert_eq!(name_of("OPTION2_2_SCREENS_Topper&DMD_onBG.bat"), "2 SCREENS Topper&DMD onBG");
        assert_eq!(name_of("(2 Screens)  stwr_107 ROM, DMD WHEREVER.bat"), "(2 Screens) stwr_107 ROM, DMD WHEREVER");
        assert_eq!(name_of("2 Screen DMD on BG no Topper.bat"), "2 Screen DMD on BG no Topper");
        assert_eq!(name_of("3 or 4 screen - BG, FullDMD, Topper.bat"), "3 or 4 screen - BG, FullDMD, Topper");
        assert_eq!(name_of("(4 Screens).bat"), "(4 Screens)");
        assert_eq!(name_of("Option 3 -Pup Pack on Backglass.bat"), "Pup Pack on Backglass");
        let mut names = vec!["Option 10 - x.bat", "Option 2 - y.bat", "Option 1 - z.bat"];
        names.sort_by_key(|n| natural(n));
        assert_eq!(names, ["Option 1 - z.bat", "Option 2 - y.bat", "Option 10 - x.bat"]);
    }

    #[test]
    fn a_pattern_matched_as_cmd_matches_it() {
        assert!(matches("*", "screens.pup") && matches("*.pup", "Screens.PUP") && matches("*.*", "README"));
        assert!(matches("s?reens.*", "screens.pup") && !matches("*.pup", "screens.pupx") && !matches("a*b", "ac"));
    }

    #[test]
    fn its_lines_read_never_run() {
        let pack = scratch("lines");
        put(&pack.join("PuP-Pack_Options").join("Option 1 - Desktop").join("screens.pup"), "x");
        let p = |line: &str| said(&pack, line);
        // what cmd does in the pack's folder
        assert!(matches!(p("xcopy /y \"PuP-Pack_Options\\Option 1 - Desktop\\*\""), Ok(Said::Steps(s)) if s == vec![Step::Copy { from: "PuP-Pack_Options\\Option 1 - Desktop".into(), pattern: "*".into(), to: PathBuf::new(), rename: None, deep: false, empty: false, hidden: false }]));
        assert!(matches!(p("CD /d %~dp0"), Ok(Said::Nothing)));
        assert!(matches!(p("del /Q PuPDesktopPunch.exe"), Ok(Said::Steps(s)) if s == vec![Step::Delete { at: PathBuf::new(), pattern: "PuPDesktopPunch.exe".into(), deep: false }]));
        assert!(matches!(p("echo 2 > ScreenType.txt\""), Ok(Said::Steps(s)) if s == vec![Step::Write { file: "ScreenType.txt".into(), text: "2 ".into(), append: false }]));
        assert!(matches!(p("echo: "), Ok(Said::Nothing)));
        assert!(matches!(p("xcopy /y \"%~dp0PuP-Pack_Options\\Option 1 - Desktop\\*\" \"%~dp0\" > nul"), Ok(Said::Steps(_))));
        // anything else refused, with its line
        assert!(p("start PuPDesktopPunch.exe \"Visual Pinball Player\" 10").unwrap_err().contains("start PuPDesktopPunch.exe"));
        assert!(p("xcopy /y \"..\\other\\*\"").unwrap_err().contains("outside the pack"));
        assert!(p("xcopy /y \"C:\\Windows\\*\"").unwrap_err().contains("outside the pack"));
        assert!(p("del %TEMP%\\x").unwrap_err().contains("Windows' settings"));
        assert!(p("copy a.pup b.pup & del c").is_err());
        assert!(p("xcopy /d \"PuP-Pack_Options\\*\"").unwrap_err().contains("/d"));
        let _ = std::fs::remove_dir_all(&pack);
    }

    /// A pack as packs come: its layouts' files in an options folder, none chosen yet.
    fn a_pack(name: &str) -> PathBuf {
        let pack = scratch(name);
        let options = pack.join("PuP-Pack_Options");
        put(&options.join("Option 1 - Desktop").join("screens.pup"), "desktop screens");
        put(&options.join("Option 1 - Desktop").join("PuPInit.bat"), "start PuPDesktopPunch.exe");
        put(&options.join("Option 2 - FullDMD on Backglass").join("screens.pup"), "two screens");
        put(&pack.join("Option 1 - Desktop.bat"), "rem echo off\r\nxcopy /y \"PuP-Pack_Options\\Option 1 - Desktop\\*\"\r\n");
        put(&pack.join("Option 2 - FullDMD on Backglass.bat"), "rem echo off\r\nxcopy /y \"PuP-Pack_Options\\Option 2 - FullDMD on Backglass\\*\"\r\ndel /Q PuPInit.bat\r\necho 2 > ScreenType.txt\r\n");
        put(&pack.join("EditThisPuPPack.bat"), "start ..\\PinUpPackEditor.exe");
        put(&pack.join("playlists.pup"), "ScreenNum,Folder,Des,AlphaSort,RestSeconds,Volume,Priority\n2,Attract,,0,0,100,1\n2,Match,,0,0,100,1\n2,Gone,,0,0,100,1\n");
        std::fs::create_dir_all(pack.join("Attract")).unwrap();
        std::fs::create_dir_all(pack.join("Match")).unwrap();
        pack
    }

    #[test]
    fn a_layout_applied_and_found_in_place() {
        let pack = a_pack("apply");
        let all = layouts(&pack);
        assert_eq!(all.layouts.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Desktop", "FullDMD on Backglass"]);
        assert_eq!(all.current, None);
        assert!(check(&pack, &all).text.contains("no layout chosen yet"), "{:?}", check(&pack, &all));
        let bin = RefCell::new(Vec::new());
        let recycle = |p: &Path| {
            bin.borrow_mut().push(p.to_path_buf());
            std::fs::remove_file(p).map_err(|e| e.to_string())
        };
        let done = apply(&pack, "Option 1 - Desktop.bat", &recycle).unwrap();
        assert_eq!((done.copied, done.same, done.deleted), (2, 0, 0));
        assert_eq!(layouts(&pack).current.as_deref(), Some("Option 1 - Desktop.bat"));
        // tried without applying: its files, from where they'd be copied
        let tried = overlay(&pack, "Option 2 - FullDMD on Backglass.bat").unwrap();
        assert!(tried.iter().any(|(n, from)| n == "screens.pup" && from.ends_with("screens.pup") && from.parent().unwrap().ends_with("Option 2 - FullDMD on Backglass")), "{tried:?}");
        // the other: its copies over the first's, its delete and its line; nothing in the Recycle
        // Bin, the pack keeping the files it replaced in its options
        let done = apply(&pack, "option 2 - fulldmd on backglass.bat", &recycle).unwrap();
        assert_eq!((done.copied, done.deleted, done.recycled), (2, 1, 0), "{done:?}");
        assert!(bin.borrow().is_empty());
        assert_eq!(std::fs::read_to_string(pack.join("screens.pup")).unwrap(), "two screens");
        assert_eq!(std::fs::read_to_string(pack.join("ScreenType.txt")).unwrap(), "2 \r\n");
        assert!(!pack.join("PuPInit.bat").exists());
        assert_eq!(layouts(&pack).current.as_deref(), Some("Option 2 - FullDMD on Backglass.bat"));
        // a file of the user's own replaced: to the Recycle Bin
        std::fs::write(pack.join("screens.pup"), "edited by hand").unwrap();
        assert_eq!(layouts(&pack).current, None);
        let done = apply(&pack, "Option 2 - FullDMD on Backglass.bat", &recycle).unwrap();
        assert_eq!((done.copied, done.same, done.recycled), (1, 1, 1), "{done:?}");
        assert_eq!(bin.borrow().len(), 1);
        // the check: its screens there, and two of its three folders
        assert_eq!(check(&pack, &layouts(&pack)), verdict("good", "Complete", "2 of its 3 folders are there; the pack itself lacks 1"));
        let _ = std::fs::remove_dir_all(&pack);
    }

    #[test]
    fn a_layout_refused_or_its_folder_found() {
        let pack = scratch("refused");
        put(&pack.join("Desktop.bat"), "xcopy /y \"PuP-Pack_Options\\Desktop\\*\"\r\nstart PuPDesktopPunch.exe\r\n");
        // its folder named wrong; the one named as the layout beside it
        put(&pack.join("PuP-Pack_Options").join("(4 Screens)").join("screens.pup"), "four");
        put(&pack.join("(4 Screens).bat"), "xcopy /y \"PuP-Pack_Options\\(2 Screens) Overlay 1\\*.pup\"\r\n");
        // its files packed a folder too high, beside a copy of its own file
        put(&pack.join("PuP-Pack_Options").join("Option 7 - LTek.bat"), "x");
        put(&pack.join("PuP-Pack_Options").join("screens.pup"), "seven");
        put(&pack.join("Option 7 - LTek.bat"), "xcopy /y \"PuP-Pack_Options\\Option 7 - LTek\\*.pup\"\r\n");
        // its files not there at all
        put(&pack.join("Gone.bat"), "xcopy /y \"PuP-Pack_Options\\Gone\\*\"\r\n");
        // a tool, copying nothing of the pack's folders
        put(&pack.join("Tidy.bat"), "del /q *.log\r\n");
        let all = layouts(&pack);
        let by = |f: &str| all.layouts.iter().find(|l| l.file == f).unwrap();
        assert_eq!(all.layouts.len(), 4);
        assert!(by("Desktop.bat").refused.as_deref().unwrap().contains("start PuPDesktopPunch.exe"));
        assert!(by("(4 Screens).bat").refused.is_none() && by("(4 Screens).bat").note.as_deref().unwrap().contains("named for this layout"));
        assert!(by("Option 7 - LTek.bat").refused.is_none() && by("Option 7 - LTek.bat").note.as_deref().unwrap().contains("one folder up"));
        assert!(by("Gone.bat").refused.as_deref().unwrap().contains("isn't in the pack"));
        assert!(apply(&pack, "Desktop.bat", &|_: &Path| Ok(())).unwrap_err().contains("start"));
        apply(&pack, "(4 Screens).bat", &|p: &Path| std::fs::remove_file(p).map_err(|e| e.to_string())).unwrap();
        assert_eq!(std::fs::read_to_string(pack.join("screens.pup")).unwrap(), "four");
        let _ = std::fs::remove_dir_all(&pack);
    }

    /// A folder copied, all of it.
    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                copy_tree(&e.path(), &to.join(e.file_name()));
            } else {
                std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
            }
        }
    }

    /// Real packs' layouts, from the folder of packs `VPI_PUP_PACKS` names, read only: each pack's
    /// top files and the folders its layouts copy from (not its videos) copied to a scratch folder,
    /// then every layout the app can apply applied there in turn, each in place after.
    /// `cargo test -p library --lib real_packs -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_packs_layouts_applied_in_turn() {
        let Ok(from) = std::env::var("VPI_PUP_PACKS") else { return };
        let mut applied = 0;
        for e in std::fs::read_dir(&from).unwrap().flatten() {
            let src = e.path();
            let all = layouts(&src);
            if !src.is_dir() || all.layouts.iter().all(|l| l.refused.is_some()) {
                continue;
            }
            let pack = scratch(&format!("real-{}", e.file_name().to_string_lossy()));
            for f in std::fs::read_dir(&src).unwrap().flatten() {
                if f.path().is_file() && f.metadata().unwrap().len() < 64 << 20 {
                    std::fs::copy(f.path(), pack.join(f.file_name())).unwrap();
                }
            }
            for l in &all.layouts {
                for step in &l.steps {
                    if let Step::Copy { from, .. } = step
                        && !from.as_os_str().is_empty()
                    {
                        copy_tree(&src.join(from), &pack.join(from));
                    }
                }
            }
            for l in all.layouts.iter().filter(|l| l.refused.is_none()) {
                let done = apply(&pack, &l.file, &|p: &Path| std::fs::remove_file(p).map_err(|e| e.to_string())).unwrap();
                let now = layouts_with(&pack, Some(&l.file));
                assert_eq!(now.current.as_deref(), Some(l.file.as_str()), "{}: {} isn't in place after it's applied: {done:?}", e.file_name().to_string_lossy(), l.file);
                assert!(done.problems.is_empty(), "{}: {}: {:?}", e.file_name().to_string_lossy(), l.file, done.problems);
                let named = layouts(&pack);
                println!("{}: {} → in place; by its files alone: {}", e.file_name().to_string_lossy(), l.name, named.in_place().map_or("none", |x| x.name.as_str()));
                applied += 1;
            }
            let _ = std::fs::remove_dir_all(&pack);
        }
        println!("{applied} layouts applied");
    }

    #[test]
    fn a_pack_checked_as_the_player_reads_it() {
        let pack = scratch("check");
        let none = Layouts::default();
        assert_eq!(check(&pack, &none).short, "Empty folder");
        put(&pack.join("hpgof46.zip"), "x");
        assert!(check(&pack, &none).text.contains("still packed (hpgof46.zip)"));
        assert_eq!(archives(&pack).len(), 1);
        put(&pack.join("inner").join("screens.pup"), "x");
        assert!(check(&pack, &none).text.contains("a folder down, in inner"));
        put(&pack.join("scriptonly.txt"), "x");
        assert_eq!(check(&pack, &none).look, "good");
        put(&pack.join("screens.pup"), "x");
        put(&pack.join("playlists.pup"), "ScreenNum,Folder,Des,AlphaSort,RestSeconds,Volume,Priority\n2,a,,0,0,100,1\n2,b,,0,0,100,1\n2,\"c\",,0,0,100,1\n2,A,,0,0,100,1\n");
        assert_eq!(check(&pack, &none).short, "Files missing");
        assert!(check(&pack, &none).text.starts_with("Its PinUP pack is missing 3 of the 3 folders it plays from"));
        for f in ["a", "b", "c"] {
            std::fs::create_dir_all(pack.join(f)).unwrap();
        }
        assert_eq!(check(&pack, &none), verdict("good", "Complete", "Its screens and all 3 of its folders are there"));
        let _ = std::fs::remove_dir_all(&pack);
    }
}
