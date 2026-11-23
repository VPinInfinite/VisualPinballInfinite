//! What a VPX table file says about itself, for matching it to VPS: its table
//! info (name, authors, version) and the game name its script gives, which is
//! a PinMAME table's ROM. Read without the table's pictures. Ours.

use std::io::Write;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::archive::{Entry, Listing, Source};
use crate::matcher::Clues;

/// The script's game name, as the script census reads it (comments left out).
static GAME_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\b(?:cgamename|gamename)\s*=\s*"([^"]+)""#).unwrap());

/// A script that drives PinMAME: core.vbs's loader called with VPinMAME's version, as ROM tables
/// do — some with none, `LoadVPM "", "S7.VBS", 2.0` (Defender) — where originals that only borrow
/// core.vbs's helpers, or create VPinMAME's controller in code they never use, don't.
static LOAD_VPM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bloadvpm\w*\s*\(?\s*"(\d*)""#).unwrap());

/// What a script hands PinUP as its pack's name: the last argument of `PuPlayer.Init` or
/// `PuPlayer.B2SInit`, a string or a variable.
static PUP_INIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bPuPlayer\.(?:B2SInit|Init)\b[^\n]*,\s*("[^"\n]*"|\w+)\s*$"#).unwrap());

/// A script's PinUP object, made as PinUP Player's own is: the script uses a PinUP pack.
static PUP_OBJECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)"PinUpPlayer\.PinDisplay""#).unwrap());

/// A script making the backglass server its machine's controller itself.
static B2S_CONTROLLER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bset\s+controller\s*=\s*createobject\s*\(\s*"B2S\.Server"\s*\)"#).unwrap());

/// A script turning off the backglass server `controller.vbs` would make its controller.
static B2S_OFF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?im)^\s*(?:const\s+)?b2soff\s*=\s*(?:true|1|-1)\b").unwrap());

/// The pack names scripts commonly keep in a constant of their own.
static PUP_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\b(?:pGameName|cPuPPack)\s*=\s*"([^"]+)""#).unwrap());

/// A script file named in a script, as the ones it loads are (`ExecuteGlobal GetTextFile("core.vbs")`,
/// `LoadVPM`'s system script `"WPC.VBS"`, KISS's own `"KISSCore.vbs"`): a quoted name ending in
/// `.vbs`, with no spaces — a message such as `"Can't open core.vbs"` names none.
static VBS_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)"([^"\\/:*?<>|\s]+\.vbs)""#).unwrap());

/// A string a script gives a name of its own: `MusicSubDirectory = "TNA\"`, `Const cGameName = "diablo"`.
static ASSIGNED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?im)\b([a-z_]\w*)\s*=\s*"((?:[^"\r\n]|"")*)"\s*(?::|$)"#).unwrap());

/// Pieces joined by `&` ending in a string, as scripts build file names: `"Music80\Song" &
/// Int(rnd*numSongs) + 1 & ".mp3"`, `curDir & "\" & cGameName & ".UltraDMD"`; a string alone is
/// one too. The first piece a string, a name (with what a call or an array takes) or a number; the
/// others any sum without a string, comma or colon; the pieces before the last string the first
/// capture, each ending in its `&`.
static CHAIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:((?:"(?:[^"\r\n]|"")*"|[a-z_][\w.]*(?:\([^()\r\n]*\))?|\d+)\s*&(?:\s*(?:"(?:[^"\r\n]|"")*"|[^&"\r\n,:]+?)\s*&)*)\s*)?"((?:[^"\r\n]|"")*)""#).unwrap()
});

/// A piece that's a string, or a name alone.
static STRING_PIECE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^"((?:[^"\r\n]|"")*)"$"#).unwrap());
static NAME_PIECE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[a-z_]\w*$").unwrap());

/// Scorbit's start (`Scorbit.DoInit(4156, "SPONGEQR", …)`), naming the folder its code keeps its
/// pairing pictures in, beside the table, which a FlexDMD of its own shows.
static SCORBIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bScorbit\.DoInit\s*\(\s*[^,]*,\s*"([^"\r\n]+)""#).unwrap());

/// What a script hands `SaveValue` or `LoadValue` as the name its values are kept under: a string,
/// or a name it gives one (`Const TableName = "Goonies"`).
static SAVED_UNDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\b(?:SaveValue|LoadValue)\b\s*\(?\s*("(?:[^"\r\n]|"")*"|[a-z_]\w*)\s*,"#).unwrap());

/// The names a script keeps its values under (`SaveValue`, `LoadValue`), as VPX files them: each
/// string it hands them, or the strings it gives the name it hands them; the most used first.
pub fn saved_under(script: &str) -> Vec<String> {
    let unquote = |s: &str| s.replace("\"\"", "\"");
    let mut given: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for c in ASSIGNED.captures_iter(script) {
        let values = given.entry(c[1].to_lowercase()).or_default();
        let v = unquote(&c[2]);
        if !values.contains(&v) {
            values.push(v);
        }
    }
    let mut counted: Vec<(String, usize)> = Vec::new();
    for c in SAVED_UNDER.captures_iter(script) {
        let names = match STRING_PIECE.captures(&c[1]) {
            Some(s) => vec![unquote(&s[1])],
            None => given.get(&c[1].to_lowercase()).cloned().unwrap_or_default(),
        };
        for name in names.into_iter().filter(|n| !n.trim().is_empty()) {
            match counted.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                Some(e) => e.1 += 1,
                None => counted.push((name, 1)),
            }
        }
    }
    counted.sort_by_key(|c| std::cmp::Reverse(c.1));
    counted.into_iter().map(|(n, _)| n).collect()
}

/// The pieces before a chain's last string, split at each `&` outside a string.
fn pieces(before: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut in_string) = (0, false);
    for (i, ch) in before.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '&' if !in_string => {
                out.push(before[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out
}

/// A sub handling the table's keys, as every table's own script has (`Sub Table1_KeyDown(ByVal
/// keycode)`) and the scripts tables load don't.
static KEY_SUB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?im)^\s*(?:public\s+|private\s+)?sub\s+\w+_key(?:down|up)\b").unwrap());

/// Whether a script is a table's own — it gives its game name, loads VPinMAME's system, or handles
/// the table's keys — rather than one tables load: a machine's system script (`sys80.vbs`), a
/// helper (ours: Beavis and Butt-head's system script came named for its table).
pub fn is_table_script(script: &str) -> bool {
    let script = strip_comments(script);
    GAME_NAME.is_match(&script) || LOAD_VPM.is_match(&script) || KEY_SUB.is_match(&script)
}

/// The kinds of file `PlayMusic` plays.
const MUSIC: [&str; 8] = ["mp3", "ogg", "wav", "flac", "m4a", "wma", "aac", "opus"];

/// The script files a script names — those it loads, from beside the table or VPX's Scripts folder —
/// each once, as written.
pub fn helper_scripts(script: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in VBS_NAME.captures_iter(script) {
        let n = c[1].trim().to_string();
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&n)) {
            out.push(n);
        }
    }
    out
}

/// Each file name a script builds, `*` standing for each piece it works out as it plays: its pieces
/// joined — strings as written, a name given one string (or several) by the script its string, any
/// other piece `*` — with whether a line naming `ProjectFolder` holds it (FlexDMD's and UltraDMD's
/// folders). At most eight readings of a chain whose names were given several strings.
fn built_names(script: &str) -> Vec<(String, bool)> {
    let unquote = |s: &str| s.replace("\"\"", "\"");
    let mut given: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for c in ASSIGNED.captures_iter(script) {
        let values = given.entry(c[1].to_lowercase()).or_default();
        let v = unquote(&c[2]);
        if !values.contains(&v) {
            values.push(v);
        }
    }
    let mut out: Vec<(String, bool)> = Vec::new();
    for line in script.lines() {
        let low = line.to_lowercase();
        // an object's name (`CreateObject("FlexDMD.FlexDMD")`) is no file's
        if low.contains("createobject") {
            continue;
        }
        let folder_line = low.contains("projectfolder");
        for c in CHAIN.captures_iter(line) {
            // a chain going on past a piece of another kind isn't whole
            if line[c.get(0).map_or(0, |m| m.end())..].trim_start().starts_with('&') {
                continue;
            }
            let mut readings = vec![String::new()];
            let pieces: Vec<Vec<String>> = pieces(c.get(1).map_or("", |m| m.as_str()))
                .into_iter()
                .map(|p| {
                    if let Some(s) = STRING_PIECE.captures(p) {
                        vec![unquote(&s[1])]
                    } else if NAME_PIECE.is_match(p) {
                        given.get(&p.to_lowercase()).cloned().unwrap_or_else(|| vec!["*".into()])
                    } else {
                        vec!["*".into()]
                    }
                })
                .chain(std::iter::once(vec![unquote(&c[2])]))
                .collect();
            for options in &pieces {
                readings = readings.iter().flat_map(|r| options.iter().map(move |o| format!("{r}{o}"))).take(8).collect();
            }
            for r in readings {
                // `*` once where several stand together
                let mut name = String::with_capacity(r.len());
                for ch in r.chars() {
                    if !(ch == '*' && name.ends_with('*')) {
                        name.push(ch);
                    }
                }
                if !out.iter().any(|(n, _)| *n == name) {
                    out.push((name, folder_line));
                }
            }
        }
    }
    out
}

/// The music files a script plays (`PlayMusic`), each a name below a music folder: the names it
/// builds that end in a kind of music file, but none it builds from an unknown start (`name &
/// ".ogg"`, a PinUP pack's playlist), which would stand for every such file. PinUP packs' own
/// sound files are named here too; only those found in a music folder count.
pub fn music_names(script: &str) -> Vec<String> {
    let music = |n: &str| n.rsplit_once('.').is_some_and(|(_, ext)| MUSIC.iter().any(|m| ext.eq_ignore_ascii_case(m)));
    built_names(script)
        .into_iter()
        .map(|(n, _)| n)
        .filter(|n| music(n) && !n.starts_with('*'))
        // a name that's only a kind of file (`".mp3"`) is a chain's end, not a file
        .filter(|n| !n.rsplit(['\\', '/']).next().unwrap_or("").starts_with('.'))
        .collect()
}

/// The folders beside the table a script names for FlexDMD's or UltraDMD's pictures and fonts:
/// what a `ProjectFolder` line builds, or a name ending in `.UltraDMD` or `.FlexDMD`, below any
/// piece it works out as it plays (the table's folder: `curDir & "\" & cGameName & ".UltraDMD"`).
pub fn folder_names(script: &str) -> Vec<String> {
    // Scorbit's (the online scores' pairing pictures, which its code makes when it's set up), not the DMD's
    let scorbit: Vec<String> = SCORBIT.captures_iter(script).map(|c| c[1].to_lowercase()).collect();
    let mut out: Vec<String> = Vec::new();
    for (name, folder_line) in built_names(script) {
        let low = name.to_lowercase();
        let low = low.trim_end_matches(['\\', '/']);
        if !(folder_line || low.ends_with(".ultradmd") || low.ends_with(".flexdmd")) {
            continue;
        }
        let tail = name.rsplit('*').next().unwrap_or("");
        let tail = tail.trim_start_matches("./").trim_start_matches(".\\").trim_matches(['\\', '/']);
        if tail.is_empty() || tail == "." || tail.contains([':', '*', '?', '"', '<', '>', '|']) || tail.split(['\\', '/']).any(|c| c == "..") {
            continue;
        }
        if scorbit.contains(&tail.to_lowercase()) {
            continue;
        }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(tail)) {
            out.push(tail.to_string());
        }
    }
    out.retain(|f| folder_used(script, f));
    out
}

/// UltraDMD's calls that show a picture or video from its project folder, and which of their
/// arguments names it.
const ULTRA_PICTURES: [(&str, usize); 7] = [
    ("displayscene00exwithid", 2),
    ("displayscene00ex", 0),
    ("displayscene00", 0),
    ("displayscene01", 1),
    ("scrollingcredits", 0),
    ("setscoreboardbackgroundimage", 0),
    ("createanimationfromimages", 2),
];

/// A script's UltraDMD object, by the name it's kept under: UltraDMD's own, or FlexDMD's stand-in.
static ULTRA_OBJECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bset\s+(\w+)\s*=\s*(?:createobject\s*\(\s*"UltraDMD\.DMDObject"\s*\)|\w+\.NewUltraDMD\b)"#).unwrap());
/// A routine's first line, and its last.
static ROUTINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(?:public\s+|private\s+)?(?:default\s+)?(sub|function)\s+\w+").unwrap());
static ROUTINE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*end\s+(?:sub|function)\b").unwrap());
/// A routine left at once on a condition: `If turnonultradmd = 0 Then Exit Sub`.
static GUARD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*if\s+(.+?)\s+then\s+exit\s+(sub|function)\b").unwrap());

/// Whether a script shows anything from a DMD folder it names, as the table comes: not where every
/// line naming the folder is in a routine that leaves first while an option the script sets is off
/// (`turnonultradmd = 0` and `If turnonultradmd = 0 Then Exit Sub`), nor an UltraDMD folder where
/// none of the script's UltraDMD scenes shows a picture or video (each scene's picture an empty
/// string: the folder holds nothing it shows).
fn folder_used(script: &str, folder: &str) -> bool {
    let lines: Vec<&str> = script.lines().collect();
    let low = folder.to_ascii_lowercase();
    let ending = [".ultradmd", ".flexdmd"].into_iter().find(|e| low.ends_with(e));
    let naming: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let l = l.to_ascii_lowercase();
            l.contains("projectfolder") || l.contains(&low) || ending.is_some_and(|e| l.contains(e))
        })
        .map(|(n, _)| n)
        .collect();
    if !naming.is_empty() && naming.iter().all(|&n| left_before(script, &lines, n)) {
        return false;
    }
    !(ending == Some(".ultradmd") && !ultra_shows_pictures(script))
}

/// Whether the routine holding line `n` leaves before it, as the script's options stand: a guard at
/// its top level (not inside a block of its own) whose condition holds.
fn left_before(script: &str, lines: &[&str], n: usize) -> bool {
    let Some(start) = (0..n).rev().take_while(|&k| !ROUTINE_END.is_match(lines[k])).find(|&k| ROUTINE.is_match(lines[k])) else {
        // outside any routine: it runs as the script loads
        return false;
    };
    let mut depth = 0i32;
    for line in &lines[start + 1..n] {
        let l = line.trim().to_ascii_lowercase();
        if depth == 0
            && let Some(c) = GUARD.captures(line)
            && guard_holds(script, &c[1])
        {
            return true;
        }
        let word = |w: &str| l == w || l.starts_with(&format!("{w} "));
        if (l.starts_with("if ") && l.ends_with(" then")) || word("for") || word("do") || word("while") || l.starts_with("select case") || word("with") {
            depth += 1;
        } else if l.starts_with("end if") || word("next") || word("loop") || word("wend") || l.starts_with("end select") || l.starts_with("end with") {
            depth -= 1;
        }
    }
    false
}

/// Whether a guard's condition holds as the script's options stand: one option compared with a
/// number (`= 0`, `<> 1`, `Not x = 1`) or taken alone (`x`, `Not x`); anything else can't be told.
fn guard_holds(script: &str, condition: &str) -> bool {
    let c = condition.trim();
    let (negated, c) = match c.get(..4) {
        Some(p) if p.eq_ignore_ascii_case("not ") => (true, c[4..].trim()),
        _ => (false, c),
    };
    let lower = c.to_ascii_lowercase();
    if lower.split_whitespace().any(|w| ["and", "or", "xor"].contains(&w)) {
        return false;
    }
    let compared = |op: &str| -> Option<(&str, i64)> {
        let (name, lit) = c.split_once(op)?;
        let name = name.trim();
        NAME_PIECE.is_match(name).then_some(())?;
        Some((name, literal(lit.trim())?))
    };
    let holds = if let Some((name, lit)) = compared("<>") {
        option_value(script, name).map(|v| v != lit)
    } else if c.contains(['<', '>']) {
        None
    } else if let Some((name, lit)) = compared("=") {
        option_value(script, name).map(|v| v == lit)
    } else if NAME_PIECE.is_match(c) {
        option_value(script, c).map(|v| v != 0)
    } else {
        None
    };
    holds.is_some_and(|h| h != negated)
}

/// A number as VBScript reads one, True and False among them.
fn literal(s: &str) -> Option<i64> {
    match s.to_ascii_lowercase().as_str() {
        "true" => Some(-1),
        "false" => Some(0),
        t => t.parse().ok(),
    }
}

/// An option's value: every statement giving `name` a value gives it one plain number, the same;
/// else (worked out, changed as it plays) none.
fn option_value(script: &str, name: &str) -> Option<i64> {
    let re = Regex::new(&format!(r"(?im)(?:^|:|\bthen\b|\belse\b)\s*(?:const\s+)?{}\s*=\s*([^:\r\n]+)", regex::escape(name))).ok()?;
    let mut value = None;
    for c in re.captures_iter(script) {
        let v = literal(c[1].trim())?;
        if value.is_some_and(|x| x != v) {
            return None;
        }
        value = Some(v);
    }
    value
}

/// Whether any of the script's UltraDMD scenes shows a picture or video: one whose picture is
/// anything but an empty string.
fn ultra_shows_pictures(script: &str) -> bool {
    let mut objects: Vec<String> = ULTRA_OBJECT.captures_iter(script).map(|c| c[1].to_ascii_lowercase()).collect();
    if !objects.iter().any(|o| o == "ultradmd") {
        objects.push("ultradmd".into());
    }
    for line in script.lines() {
        let low = line.to_ascii_lowercase();
        for obj in &objects {
            let dotted = format!("{obj}.");
            let mut from = 0;
            while let Some(at) = low[from..].find(&dotted) {
                let start = from + at;
                from = start + dotted.len();
                if low[..start].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                    continue;
                }
                let rest = &line[from..];
                let method: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                let Some(&(_, slot)) = ULTRA_PICTURES.iter().find(|(m, _)| m.eq_ignore_ascii_case(&method)) else { continue };
                if arguments(&rest[method.len()..]).get(slot).is_none_or(|a| a.trim() != "\"\"") {
                    return true;
                }
            }
        }
    }
    false
}

/// A call's arguments as written, split at the commas between them: in brackets, or to the end
/// of its statement.
fn arguments(s: &str) -> Vec<String> {
    let s = s.trim_start();
    let (s, bracketed) = match s.strip_prefix('(') {
        Some(r) => (r, true),
        None => (s, false),
    };
    let mut args = vec![String::new()];
    let (mut depth, mut quoted) = (0i32, false);
    for ch in s.chars() {
        let arg = args.last_mut().expect("one argument at least");
        if quoted {
            arg.push(ch);
            quoted = ch != '"';
            continue;
        }
        match ch {
            '"' => {
                quoted = true;
                arg.push(ch);
            }
            '(' => {
                depth += 1;
                arg.push(ch);
            }
            ')' if depth == 0 && bracketed => break,
            ')' => {
                depth -= 1;
                arg.push(ch);
            }
            ',' if depth == 0 => args.push(String::new()),
            ':' if depth == 0 => break,
            _ => arg.push(ch),
        }
    }
    args
}

/// The names a script may hand PinUP for its pack, the likeliest first: what it passes to
/// `PuPlayer.Init` (the string, or the variable's value), its pack-name constants, its game name.
pub fn pup_names(script: &str, game_name: Option<&str>) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut add = |n: &str| {
        let n = n.trim();
        if !n.is_empty() && !names.iter().any(|x| x.eq_ignore_ascii_case(n)) {
            names.push(n.to_string());
        }
    };
    for line in script.lines() {
        let Some(c) = PUP_INIT.captures(line.trim_end()) else { continue };
        let arg = &c[1];
        if let Some(literal) = arg.strip_prefix('"') {
            add(literal.trim_end_matches('"'));
        } else if let Ok(value) = Regex::new(&format!(r#"(?i)\b{}\s*=\s*"([^"]+)""#, regex::escape(arg))) {
            for v in value.captures_iter(script) {
                add(&v[1]);
            }
        }
    }
    for c in PUP_NAME.captures_iter(script) {
        add(&c[1]);
    }
    if let Some(g) = game_name {
        add(g);
    }
    names
}

/// A table file's clues for matching, and its game name.
#[derive(Clone, Debug, Default)]
pub struct TableFacts {
    pub clues: Clues,
    pub game_name: Option<String>,
    /// Whether its script drives PinMAME, and so needs its game name's ROM.
    pub pinmame: bool,
    /// The names its script may hand PinUP for its pack, the likeliest first.
    pub pup_names: Vec<String>,
    /// The script files its script loads.
    pub helpers: Vec<String>,
    /// The music its script plays, `*` standing for what it works out as it plays.
    pub music: Vec<String>,
    /// The folders beside it its script names for FlexDMD's or UltraDMD's pictures.
    pub folders: Vec<String>,
    /// The names its script keeps its values under (`SaveValue`).
    pub saved_under: Vec<String>,
    /// Whether its script makes PinUP's object, and so plays a PinUP pack.
    pub uses_pup: bool,
    /// Whether the backglass server is its machine's controller (`ScriptFacts::b2s_controller`).
    pub b2s_controller: bool,
    /// The one of VPX's eight global physics sets it plays (its Physics set property, 1 to 8),
    /// where it plays one in its own values' place.
    pub physics_set: Option<u32>,
}

pub fn read(path: &Path) -> Result<TableFacts, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let made = file
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    read_from(file, &stem, made).map_err(|e| format!("{}: {e}", path.display()))
}

/// A table file in a drop: read where it lies, or, inside an archive, copied out to a passing file
/// first (a table file's parts are read out of order, which a packed entry can't be), its date the
/// archive's.
pub fn read_in(listing: &Listing, entry: &Entry) -> Result<TableFacts, String> {
    match &listing.source {
        Source::Folder(top) => return read(&top.join(&entry.path)),
        Source::File(path) => return read(path),
        Source::Archive(_) => {}
    }
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", entry.path);
    let passing = std::env::temp_dir().join(format!("vpi-library-{}.vpx", std::process::id()));
    let result = (|| -> Result<TableFacts, String> {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&passing).map_err(|e| fail(&e))?);
        listing.copy_out(&entry.path, &mut out, u64::MAX)?;
        out.flush().map_err(|e| fail(&e))?;
        drop(out);
        let file = std::fs::File::open(&passing).map_err(|e| fail(&e))?;
        let name = entry.path.rsplit('/').next().unwrap_or(&entry.path);
        let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
        read_from(file, stem, entry.modified).map_err(|e| fail(&e))
    })();
    let _ = std::fs::remove_file(&passing);
    result
}

fn read_from(file: std::fs::File, stem: &str, made: Option<i64>) -> Result<TableFacts, String> {
    let mut vpx = vpin::vpx::VpxFile::open(file).map_err(|e| e.to_string())?;
    let info = vpx.read_tableinfo().map_err(|e| e.to_string())?;
    let data = vpx.read_gamedata().map_err(|e| e.to_string())?;
    let ScriptFacts { game_name, pinmame, pup_names, helpers, music, folders, saved_under, uses_pup, b2s_controller } = script_facts(&data.code.string);
    let present = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    Ok(TableFacts {
        clues: Clues {
            file_name: stem.to_string(),
            info_name: present(info.table_name),
            info_authors: present(info.author_name),
            info_version: present(info.table_version),
            made,
            rom: game_name.clone(),
        },
        game_name,
        pinmame,
        pup_names,
        helpers,
        music,
        folders,
        saved_under,
        uses_pup,
        b2s_controller,
        physics_set: (data.override_physics > 0).then_some(data.override_physics),
    })
}


/// What a table's file sets for itself that a player may change for that table, as VPX 5436's
/// in-game menu starts from them (`TableMiscPage`): its tone mapper, exposure, light level and
/// difficulty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnLook {
    /// VPX's tone mappers: 0 Reinhard, 1 AgX, 2 Filmic, 3 Neutral, 4 AgX Punchy.
    pub tone_mapper: u32,
    /// 0 to 2.
    pub exposure: f32,
    /// 0 to 1.
    pub light_level: f32,
    /// 0 to 1.
    pub difficulty: f32,
}

pub fn own_look(path: &Path) -> Result<OwnLook, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut vpx = vpin::vpx::VpxFile::open(file).map_err(|e| e.to_string())?;
    let g = vpx.read_gamedata().map_err(|e| e.to_string())?;
    // a table keeping no tone mapper or exposure of its own is drawn as VPX draws it: Reinhard, 1
    Ok(OwnLook { tone_mapper: g.tone_mapper.as_ref().map_or(0, u32::from), exposure: g.exposure.unwrap_or(1.0), light_level: g.global_emission_scale, difficulty: g.global_difficulty })
}

/// What a script says of the table.
#[derive(Clone, Debug, Default)]
pub struct ScriptFacts {
    /// Its game name (a PinMAME table's ROM).
    pub game_name: Option<String>,
    /// Whether it drives PinMAME.
    pub pinmame: bool,
    /// The names it may hand PinUP for its pack, the likeliest first.
    pub pup_names: Vec<String>,
    /// The script files it loads.
    pub helpers: Vec<String>,
    /// The music it plays, `*` standing for what it works out as it plays.
    pub music: Vec<String>,
    /// The folders beside the table it names for FlexDMD's or UltraDMD's pictures.
    pub folders: Vec<String>,
    /// The names it keeps its values under (`SaveValue`): its own high scores, among them.
    pub saved_under: Vec<String>,
    /// Whether it makes PinUP's object, and so plays a PinUP pack.
    pub uses_pup: bool,
    /// Whether the backglass server (`B2S.Server`) is its machine's controller, PinMAME's machine
    /// played inside it: through VPX's `controller.vbs`, or made so by the script itself.
    pub b2s_controller: bool,
}

/// The script that plays for the table at `path`, as it's kept: the one beside it (`<table>.vbs`),
/// which VPX 5436 and the player play in its own's place, else its own (as UTF-8).
pub fn script_that_plays(path: &Path) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = std::fs::read(path.with_extension("vbs")) {
        return Ok(bytes);
    }
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut vpx = vpin::vpx::VpxFile::open(file).map_err(|e| format!("{}: {e}", path.display()))?;
    let data = vpx.read_gamedata().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(data.code.string.into_bytes())
}

/// A script's coils as its `SolCallback` and `SolModCallback` lines name them: each coil's number,
/// what the line hands it to (`SolKnocker`), and the comment after it (`Knocker`), lines commented
/// out left out.
pub fn coil_names(script: &str) -> Vec<(u32, String, Option<String>)> {
    let mut out: Vec<(u32, String, Option<String>)> = Vec::new();
    for line in script.lines() {
        let l = line.trim_start();
        let lower = l.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix("solcallback").or_else(|| lower.strip_prefix("solmodcallback")) else { continue };
        let at = l.len() - rest.len();
        let rest = l[at..].trim_start();
        let Some(rest) = rest.strip_prefix('(') else { continue };
        let Some((number, rest)) = rest.split_once(')') else { continue };
        let Ok(number) = number.trim().parse::<u32>() else { continue };
        let Some(rest) = rest.trim_start().strip_prefix('=') else { continue };
        let rest = rest.trim_start();
        // what it's handed to: a quoted string, its doubled quotes kept as one
        let Some(body) = rest.strip_prefix('"') else { continue };
        let mut name = String::new();
        let mut chars = body.char_indices().peekable();
        let mut end = body.len();
        while let Some((i, c)) = chars.next() {
            if c == '"' {
                if chars.peek().is_some_and(|(_, n)| *n == '"') {
                    chars.next();
                    name.push('"');
                    continue;
                }
                end = i + 1;
                break;
            }
            name.push(c);
        }
        let comment = body[end..].split_once('\'').map(|(_, c)| c.trim().to_string()).filter(|c| !c.is_empty());
        let name = name.trim().trim_end_matches(',').trim().to_string();
        if name.is_empty() {
            continue;
        }
        match out.iter_mut().find(|(n, ..)| *n == number) {
            // the last line for a coil is the one that holds
            Some(slot) => *slot = (number, name, comment),
            None => out.push((number, name, comment)),
        }
    }
    out.sort_by_key(|(n, ..)| *n);
    out
}

/// The table's own script, inside its file, whatever a script beside it says.
pub fn own_script(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut vpx = vpin::vpx::VpxFile::open(file).map_err(|e| format!("{}: {e}", path.display()))?;
    let data = vpx.read_gamedata().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(data.code.string.into_bytes())
}

/// The names of a table's pictures, as VPX's image manager lists them: each picture's `NAME`
/// record, the first VPX writes, read from the start of its stream (`GameStg/Image0` and on) alone,
/// so a table of hundreds of megabytes is read in a moment.
pub fn picture_names(path: &Path) -> Result<Vec<String>, String> {
    Ok(picture_heads(path)?.into_iter().map(|(name, ..)| name).collect())
}

/// One of a table's own pictures, by its name as VPX's image manager lists it: its file as the table
/// keeps it (a PNG, JPEG or WebP and the rest, the `DATA` of its `JPEG` block); none for a picture
/// kept as VPX's own bitmap (`BITS`), or a name no picture has. Only that picture's stream is read
/// whole.
pub fn picture_file(path: &Path, name: &str) -> Result<Option<Vec<u8>>, String> {
    use std::io::Read;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut stg = cfb::open(path).map_err(fail)?;
    for i in 0.. {
        let Ok(mut stream) = stg.open_stream(format!("/GameStg/Image{i}")) else { break };
        let mut data = Vec::new();
        (&mut stream).take(4096).read_to_end(&mut data).map_err(fail)?;
        if !biff_string(&data, b"NAME").is_some_and(|n| n.trim().eq_ignore_ascii_case(name.trim())) {
            continue;
        }
        stream.read_to_end(&mut data).map_err(fail)?;
        return Ok(biff_data(&data, b"DATA").map(<[u8]>::to_vec));
    }
    Ok(None)
}

/// What a table's file says for a game's pause menu, its table info and game data read once.
pub struct OwnFacts {
    /// Its rules, as `rules` gives them.
    pub rules: Option<String>,
    /// Its own script, inside its file.
    pub script: String,
    /// Its own levels for its sounds and its music, 0 to 1 (VPX's `TableSoundVolume` and
    /// `TableMusicVolume`).
    pub sound_volume: f32,
    pub music_volume: f32,
}

/// What a table's file says for a game's pause menu.
pub fn own_facts(path: &Path) -> Result<OwnFacts, String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::fs::File::open(path).map_err(|e| fail(&e))?;
    let mut vpx = vpin::vpx::VpxFile::open(file).map_err(|e| fail(&e))?;
    let info = vpx.read_tableinfo().map_err(|e| fail(&e))?;
    let data = vpx.read_gamedata().map_err(|e| fail(&e))?;
    Ok(OwnFacts { rules: info.table_rules.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()), sound_volume: data.table_sound_volume, music_volume: data.table_music_volume, script: data.code.string })
}

/// Whether a script plays music with VPX's `PlayMusic`, comments left out: `Some(true)` where every
/// call leaves the volume at its default, `Some(false)` where one gives its own (a script's own
/// music level, which nothing outside it can read); none where it plays none.
pub fn plays_music(script: &str) -> Option<bool> {
    static CALL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bplaymusic\b").expect("a plain pattern"));
    let code = strip_comments(script);
    let mut plain = None;
    for line in code.lines() {
        for m in CALL.find_iter(line) {
            // a comma outside quotes before the statement's end: a volume given
            let mut in_string = false;
            let mut given = false;
            for c in line[m.end()..].chars() {
                match c {
                    '"' => in_string = !in_string,
                    ':' if !in_string => break,
                    ',' if !in_string => given = true,
                    _ => {}
                }
            }
            plain = Some(plain.unwrap_or(true) && !given);
        }
    }
    plain
}

/// Whether a picture's name, among a table's own, says it's a rules or instruction card: not a
/// price or coin card, a ruler, a sign, a sticker or a score display's.
pub fn rule_card_name(name: &str) -> bool {
    let low = name.to_lowercase();
    if ["cardboard", "ruler", "sign", "dmd", "stkr", "sticker", "discard"].iter().any(|w| low.contains(w)) {
        return false;
    }
    low.contains("instruct") || low.contains("rule") || (low.contains("card") && !low.contains("price") && !low.contains("coin"))
}

/// A table's own pictures that show its rules: those named as a rules or instruction card, at most
/// eight; else the picture of its apron, where a machine's cards are printed. Only the start of each
/// picture's stream is read.
pub fn rule_pictures(path: &Path) -> Result<Vec<String>, String> {
    let heads = picture_heads(path)?;
    let named: Vec<String> = heads.iter().filter(|(name, ..)| rule_card_name(name)).take(8).map(|(name, ..)| name.clone()).collect();
    if !named.is_empty() {
        return Ok(named);
    }
    Ok(heads.into_iter().find(|(name, w, h)| apron_picture(name, *w, *h)).map(|(name, ..)| name).into_iter().collect())
}

/// Whether a picture is an apron's art laid out flat, its cards readable on it: named for the apron
/// and wider than tall by half as much again (square or tall, it's a sheet of textures).
fn apron_picture(name: &str, width: i32, height: i32) -> bool {
    let low = name.to_lowercase();
    low.contains("apron") && !["gradient", "scorbit"].iter().any(|w| low.contains(w)) && height > 0 && f64::from(width) >= 1.5 * f64::from(height)
}

/// A table's own playfield picture, the one its playfield shows (its table's Image), as the table
/// keeps it; none where it names none, or keeps it as VPX's own bitmap.
pub fn playfield_picture(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut vpx = vpin::vpx::VpxFile::open(file).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = vpx.read_gamedata().map_err(|e| format!("{}: {e}", path.display()))?.image;
    if name.trim().is_empty() {
        return Ok(None);
    }
    picture_file(path, &name)
}

/// A table's rules, as VPX's table info keeps them for its Rules page (`TableRules`, Markdown, the
/// table's own pictures named in it as `![…](name)`), where it has any. Its table info alone is
/// read.
pub fn rules(path: &Path) -> Result<Option<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut vpx = vpin::vpx::VpxFile::open(file).map_err(|e| format!("{}: {e}", path.display()))?;
    let info = vpx.read_tableinfo().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(info.table_rules.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()))
}

/// What a table's own script reads beyond the flippers, plunger, start, coins and nudges, by the
/// names VPX hands scripts for the keys (`LeftMagnaSave` and the rest), comments left out — each
/// with VPX's actions for it, by their names in VPX's settings (`[Input]` `Mapping.<action>`); a ROM
/// machine's coin door and service buttons where it starts one (`LoadVPM`), which VPX's shared
/// scripts give the machine.
pub fn own_controls(script: &str) -> Vec<(&'static str, &'static [&'static str])> {
    let code = strip_comments(script).to_lowercase();
    let has = |names: &[&str]| names.iter().any(|n| code.contains(n));
    let mut out = Vec::new();
    for (names, label, actions) in [
        (&["leftmagnasave", "rightmagnasave"][..], "Magna-saves", &["LeftMagna", "RightMagna"][..]),
        (&["lockbarkey"][..], "Fire button", &["Lockbar"][..]),
        (&["stagedleftflipperkey", "stagedrightflipperkey"][..], "Upper flippers", &["LeftStagedFlipper", "RightStagedFlipper"][..]),
        (&["mechanicaltilt"][..], "Tilt", &["Tilt"][..]),
        (&["addcreditkey2"][..], "Second coin", &["Credit2"][..]),
        (&["loadvpm"][..], "Coin door", &["CoinDoor"][..]),
        (&["loadvpm"][..], "Service buttons", &["Service1", "Service2", "Service3", "Service4"][..]),
    ] {
        if has(names) {
            out.push((label, actions));
        }
    }
    out
}

/// A record's data among VPX's BIFF records, as `biff_string` walks them: what follows its tag, to
/// its length. A `JPEG` block's records follow its own tag, so a picture's file is found inside it;
/// VPX's own bitmap (`BITS`) isn't a record, so the walk stops there.
fn biff_data<'a>(data: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let u32_at = |at: usize| data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let mut at = 0;
    while at + 8 <= data.len() {
        let len = u32_at(at)?;
        let here = &data[at + 4..at + 8];
        if here == tag {
            return data.get(at + 8..at + 4 + len);
        }
        if here == b"BITS" || len < 4 {
            return None;
        }
        at += 4 + len;
    }
    None
}

/// Each of a table's pictures' name, width and height, as VPX's image manager lists them, read from
/// the start of its stream alone.
fn picture_heads(path: &Path) -> Result<Vec<(String, i32, i32)>, String> {
    use std::io::Read;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut stg = cfb::open(path).map_err(fail)?;
    let mut out = Vec::new();
    for i in 0.. {
        let Ok(stream) = stg.open_stream(format!("/GameStg/Image{i}")) else { break };
        let mut head = Vec::new();
        stream.take(4096).read_to_end(&mut head).map_err(fail)?;
        if let Some(name) = biff_string(&head, b"NAME") {
            let size = |tag: &[u8; 4]| biff_data(&head, tag).and_then(|d| d.get(..4)).map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            out.push((name, size(b"WDTH"), size(b"HGHT")));
        }
    }
    Ok(out)
}

/// A string record's value among VPX's BIFF records: each a length (counting its tag), its tag and
/// its data, a string's data its length and then its bytes.
fn biff_string(data: &[u8], tag: &[u8; 4]) -> Option<String> {
    let u32_at = |at: usize| data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let mut at = 0;
    while at + 8 <= data.len() {
        let len = u32_at(at)?;
        if &data[at + 4..at + 8] == tag {
            let n = u32_at(at + 8)?;
            return data.get(at + 12..at + 12 + n).map(|s| String::from_utf8_lossy(s).into_owned());
        }
        if len < 4 {
            return None;
        }
        at += 4 + len;
    }
    None
}

/// What a script says of the table, read without its comments.
pub fn script_facts(script: &str) -> ScriptFacts {
    let script = strip_comments(script);
    let game_name = GAME_NAME.captures(&script).map(|c| c[1].to_string());
    let pinmame = game_name.is_some() && LOAD_VPM.is_match(&script);
    let pup_names = pup_names(&script, game_name.as_deref());
    let helpers = helper_scripts(&script);
    let music = music_names(&script);
    let folders = folder_names(&script);
    let saved_under = saved_under(&script);
    let uses_pup = PUP_OBJECT.is_match(&script);
    // VPX's `controller.vbs` makes the backglass server the controller, unless the script turns it
    // off; a script may make it so itself
    let b2s_controller = (helpers.iter().any(|h| h.eq_ignore_ascii_case("controller.vbs")) && !B2S_OFF.is_match(&script)) || B2S_CONTROLLER.is_match(&script);
    ScriptFacts { game_name, pinmame, pup_names, helpers, music, folders, saved_under, uses_pup, b2s_controller }
}

/// The game name a script gives, if any.
pub fn game_name_in(script: &str) -> Option<String> {
    GAME_NAME.captures(&strip_comments(script)).map(|c| c[1].to_string())
}

/// A script without its comments: `'` outside a string to the end of the line, and `Rem` lines —
/// as the script census reads scripts (`tools/script-census`).
pub fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let trimmed = line.trim_start();
        // by bytes that may end inside a letter (Indiana Jones's "' © Williams 1993")
        if trimmed.get(..3).is_some_and(|w| w.eq_ignore_ascii_case("rem")) && trimmed.as_bytes().get(3).is_some_and(u8::is_ascii_whitespace) {
            out.push('\n');
            continue;
        }
        let mut in_string = false;
        let mut end = line.len();
        for (i, c) in line.char_indices() {
            match c {
                '"' => in_string = !in_string,
                '\'' if !in_string => {
                    end = i;
                    break;
                }
                _ => {}
            }
        }
        out.push_str(&line[..end]);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coils_named_as_the_script_names_them() {
        let script = "SolCallback(7) = \"SolKnocker\" \t\t' Knocker\n'SolCallback(8) = \"Old\"\nsolcallback( 15 )=\"SolUfoShake\"\nSolModCallback(18)=\"SolFlash18\"\t'Flasher: Jackpot\nSolCallback(2) = \"vpmSolSound SoundFX(\"\"fx_Knocker\"\",DOFKnocker),\"\n";
        let got = coil_names(script);
        assert_eq!(got.iter().map(|(n, ..)| *n).collect::<Vec<_>>(), [2, 7, 15, 18]);
        assert_eq!(got[1], (7, "SolKnocker".to_string(), Some("Knocker".to_string())));
        assert_eq!(got[0].1, "vpmSolSound SoundFX(\"fx_Knocker\",DOFKnocker)");
        assert_eq!(got[3].2.as_deref(), Some("Flasher: Jackpot"));
    }

    #[test]
    fn game_name_past_comments() {
        let script = "'Const cGameName = \"mtl_180hc\" ' the colour ROM\nRem Const cGameName = \"x\"\nConst cGameName = \"mtl_180h\" ' the one\n";
        assert_eq!(game_name_in(script).as_deref(), Some("mtl_180h"));
        assert!(LOAD_VPM.is_match(&strip_comments("LoadVPM \"03060000\", \"sam.vbs\", 3.10")));
        assert!(!LOAD_VPM.is_match(&strip_comments("' LoadVPM \"03060000\", \"sam.vbs\", 3.10")));
        assert!(LOAD_VPM.is_match(&strip_comments("LoadVPM \"\", \"S7.VBS\", 2.0")));
    }

    #[test]
    fn pack_names_as_scripts_give_them() {
        // The Goonies: its own constant, handed to B2SInit
        let goonies = strip_comments("Const cGameName = \"goonies\"\nDim pGameName : pGameName=\"TheGoonies\" 'pupvideos foldername\nPuPlayer.B2SInit \"\", pGameName\n");
        assert_eq!(pup_names(&goonies, Some("goonies")), ["TheGoonies", "goonies"]);
        // Leprechaun King: the game name itself
        let lep = strip_comments("Const cGameName = \"leprechaun\"\nPuPlayer.Init pBackglass,cGameName\n");
        assert_eq!(pup_names(&lep, Some("leprechaun")), ["leprechaun"]);
    }

    #[test]
    fn the_scripts_a_script_loads() {
        let kiss = "On Error Resume Next\nExecuteGlobal GetTextFile(\"controller.vbs\")\nLoadVPM \"01560000\", \"S11.VBS\", 3.26\n' ExecuteGlobal GetTextFile(\"old.vbs\")\nExecuteGlobal GetTextFile(\"KISSCore.vbs\") : ExecuteGlobal GetTextFile(\"kisscore.vbs\")\n";
        assert_eq!(script_facts(kiss).helpers, ["controller.vbs", "S11.VBS", "KISSCore.vbs"]);
        // a message naming one isn't loading it (21 of the test tables' scripts)
        assert!(script_facts("If Err Then MsgBox \"Can't open core.vbs\"\n").helpers.is_empty());
    }

    #[test]
    fn a_tables_own_script_or_one_tables_load() {
        assert!(is_table_script("Const cGameName = \"afm_113b\"\nLoadVPM \"03060000\", \"WPC.VBS\", 3.26\n"));
        assert!(is_table_script("Sub Table1_KeyDown(ByVal keycode)\n  If keycode = LeftFlipperKey Then LeftFlipper.RotateToEnd\nEnd Sub\n"));
        // a system script: it loads core.vbs and names keys, but none of the table's own
        let sys = "LoadCore\nPrivate Sub LoadCore\n  ExecuteGlobal GetTextFile(\"core.vbs\")\nEnd Sub\nConst swLRFlip = 82\n' Sub Table1_KeyDown\n";
        assert!(!is_table_script(sys));
    }

    #[test]
    fn the_music_a_script_plays() {
        // Tom and Jerry: one built with a number, one whole
        let tj = r#"song = "bgout_T&JbackgroundMusic" & currentsong & ".mp3"
PlayMusic song
PlayMusic "bgout_T&Jmultiballmadness.mp3"
"#;
        assert_eq!(script_facts(tj).music, ["bgout_T&JbackgroundMusic*.mp3", "bgout_T&Jmultiballmadness.mp3"]);
        // Total Nuclear Annihilation: in the folder the script names, or in none
        let tna = r#"dim MusicSubDirectory : MusicSubDirectory = "TNA\"
MusicSubDirectory = ""
m_main = MusicSubDirectory & "TNA" & songnro & ".mp3"
"#;
        assert_eq!(script_facts(tna).music, [r"TNA\TNA*.mp3", "TNA*.mp3"]);
        // Playboy's in a folder, its volume after; Canasta's built from a sum
        let more = r#"If musicNum = 0 Then PlayMusic "playboy78/01 - Stayin' Alive.mp3",MusicVol: End If
If Musicchoice = 1 then PlayMusic "Music80\Song" & Int(rnd*numSongs) + 1 & ".mp3"
"#;
        assert_eq!(script_facts(more).music, ["playboy78/01 - Stayin' Alive.mp3", r"Music80\Song*.mp3"]);
        // The Goonies' PinUP playlist file, built from an unknown name, stands for nothing
        assert!(script_facts(r#"PuPlayer.playlistplayex pMusic,"MUSIC",name&".ogg",100,10"#).music.is_empty());
    }

    #[test]
    fn the_folders_a_script_names_for_its_dmd() {
        // Diablo
        let s = r#"Const cGameName = "diablo"
DirName = curDir& "\" &cGameName& ".UltraDMD"
UltraDMD.SetProjectFolder DirName
UltraDMD.DisplayScene00 "intro.wmv", "", 15, "", 15, 14, 4000, 14
"#;
        assert_eq!(script_facts(s).folders, ["diablo.UltraDMD"]);
        // Game of Thrones
        let s = r#"Const myGameName = "gameofthrones" ' Used for FlexDMD
.ProjectFolder = ".\"&myGameName&".FlexDMD\"
"#;
        assert_eq!(script_facts(s).folders, ["gameofthrones.FlexDMD"]);
        // SpongeBob's, beside Scorbit's own folder, which isn't its DMD's
        let s = r#"Set FlexDMD = CreateObject("FlexDMD.FlexDMD")
.ProjectFolder = "./SpongebobDMD/"
If Scorbit.DoInit(4156, "SPONGEQR", TableVersion, "spongebob-vpin") then
.ProjectFolder = TablesDir & "\SPONGEQR"
"#;
        assert_eq!(script_facts(s).folders, ["SpongebobDMD"]);
    }

    #[test]
    fn a_dmd_folder_the_script_shows_nothing_from_isnt_asked_for() {
        // UltraDMD on, its scenes all text: the folder holds nothing it shows
        let text_only = r#"Const ScoreType = 1 ' 1=UltraDMD 2=PuPlayer DMD
Sub LoadUltraDMD
	If not ScoreType = 1 Then Exit Sub: End If
	Set UltraDMD = CreateObject("UltraDMD.DMDObject")
	UltraDMD.Init
	If Not UltraDMD.GetMajorVersion = 1 Then
		MsgBox "Incompatible Version of UltraDMD found."
		Exit Sub
	End If
	UltraDMD.SetProjectFolder curDir & "\Jaws.UltraDMD"
End Sub
Sub DMD_DisplayScene(toptext,bottomtext,animateIn,pauseTime,animateOut)
	If not ScoreType = 1 Then Exit Sub: End If
	UltraDMD.DisplayScene00 "", toptext, 13, bottomtext, 15, animateIn, pauseTime, animateOut
End Sub
Sub DMD_ScrollingCredits(text,animateIn,pauseTime,animateOut)
	UltraDMD.ScrollingCredits "", text, 15, animateIn, pauseTime, animateOut
End Sub
"#;
        assert!(script_facts(text_only).folders.is_empty());
        // a scene showing a video from it
        let with_video = text_only.replace("UltraDMD.ScrollingCredits \"\"", "UltraDMD.ScrollingCredits \"shark.wmv\"");
        assert_eq!(script_facts(&with_video).folders, ["Jaws.UltraDMD"]);
        // another option chosen: no UltraDMD at all, the folder never set
        assert!(script_facts(&with_video.replace("Const ScoreType = 1", "Const ScoreType = 2")).folders.is_empty());

        // UltraDMD only with an option the table comes with off
        let option_off = r#"Sub Table1_Init
	turnonultradmd = 0 ' change to 1 to turn on ultradmd
End Sub
Dim turnonultradmd
Const TableName = "jurassic"
Sub DisplayScene(background, toptext, bottomtext, duration)
	If turnonultradmd = 0 then exit sub
	UltraDMD.DisplayScene00 background, toptext, 15, bottomtext, 15, 14, duration, 14
End Sub
Sub DMD_Init
	If turnonultradmd = 0 then exit sub
	Set UltraDMD = CreateObject("UltraDMD.DMDObject")
	Dim DirName
	DirName = curDir& "\" &TableName& ".UltraDMD"
	If Not fso.FolderExists(DirName) Then _
			Msgbox "UltraDMD userfiles directory '" & DirName & "' does not exist."
	UltraDMD.SetProjectFolder DirName
End Sub
"#;
        assert!(script_facts(option_off).folders.is_empty());
        assert_eq!(script_facts(&option_off.replace("turnonultradmd = 0 '", "turnonultradmd = 1 '")).folders, ["jurassic.UltraDMD"]);
        // an option worked out as it plays can't be told: the folder's asked for
        let worked_out = option_off.replace("turnonultradmd = 0 '", "turnonultradmd = Table1.Option(\"UltraDMD\", 0, 1, 1, 0, 0) '");
        assert_eq!(script_facts(&worked_out).folders, ["jurassic.UltraDMD"]);
        // a guard inside a block of its own leaves only sometimes
        let nested = option_off.replace("Sub DMD_Init\n\tIf turnonultradmd = 0 then exit sub", "Sub DMD_Init\n\tIf ready Then\n\tIf turnonultradmd = 0 then exit sub\n\tEnd If");
        assert_eq!(script_facts(&nested).folders, ["jurassic.UltraDMD"]);
    }

    /// A picture's file found in its stream as VPX writes one: its name, its size, a `JPEG` block
    /// holding the file's own name, size and data; VPX's own bitmap has none.
    #[test]
    fn a_pictures_file_found_in_its_stream() {
        let record = |tag: &[u8; 4], data: &[u8]| {
            let mut v = ((data.len() + 4) as u32).to_le_bytes().to_vec();
            v.extend_from_slice(tag);
            v.extend_from_slice(data);
            v
        };
        let string = |text: &str| {
            let mut v = (text.len() as u32).to_le_bytes().to_vec();
            v.extend_from_slice(text.as_bytes());
            v
        };
        let mut stream = record(b"NAME", &string("rulescard"));
        stream.extend(record(b"WDTH", &640u32.to_le_bytes()));
        stream.extend(record(b"JPEG", &[]));
        stream.extend(record(b"NAME", &string("rulescard")));
        stream.extend(record(b"SIZE", &4u32.to_le_bytes()));
        stream.extend(record(b"DATA", b"\x89PNG"));
        stream.extend(record(b"ENDB", &[]));
        assert_eq!(biff_string(&stream, b"NAME").as_deref(), Some("rulescard"));
        assert_eq!(biff_data(&stream, b"DATA"), Some(&b"\x89PNG"[..]));
        let mut bitmap = record(b"NAME", &string("old"));
        bitmap.extend(record(b"BITS", &[]));
        bitmap.extend([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(biff_data(&bitmap, b"DATA"), None);
    }

    /// What real tables hold for a Rules page, printed: each table in the folder `VPI_RULES_TABLES`
    /// names, the length of its own rules and its pictures named as a rules card might be. `cargo
    /// test --release -p library --lib rules_of_real_tables -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn rules_of_real_tables() {
        let Ok(folder) = std::env::var("VPI_RULES_TABLES") else { return };
        let mut files: Vec<_> = std::fs::read_dir(&folder).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vpx"))).collect();
        files.sort();
        let (mut with_rules, mut with_cards) = (0, 0);
        for f in &files {
            let rules = rules(f);
            let names = picture_names(f).unwrap_or_default();
            let cards: Vec<&String> = names.iter().filter(|n| rule_card_name(n)).collect();
            with_rules += usize::from(matches!(rules, Ok(Some(_))));
            with_cards += usize::from(!cards.is_empty());
            println!("{:58} rules {:>6}  shows {:?}", f.file_stem().unwrap().to_string_lossy().chars().take(56).collect::<String>(), match &rules { Ok(Some(r)) => r.len().to_string(), Ok(None) => "-".into(), Err(_) => "error".into() }, rule_pictures(f).unwrap_or_default());
        }
        println!("{} tables, {with_rules} with rules of their own, {with_cards} with a picture named as a card", files.len());
    }

    /// Music a script plays, told from names that only look like it, and whether it gives a volume.
    #[test]
    fn whether_a_script_plays_music() {
        assert_eq!(plays_music("PlayMusic \"a, b.mp3\" : x = 1, 2\n"), Some(true));
        assert_eq!(plays_music("If y Then PlayMusic Song, bMusicVolume\n"), Some(false));
        assert_eq!(plays_music("PlayMusic \"a\"\nCase 0:PlayMusic \"b\", fVol :x\n"), Some(false));
        assert_eq!(plays_music("' PlayMusic \"a\"\nmyPlayMusicForMode(1)\nPlayMusicOnStartup = 1\nSub PlayMusic1(aLvl)\nEnd Sub\n"), None);
    }

    /// The names that say a rules card, and an apron's art laid out flat.
    #[test]
    fn a_tables_rules_pictures_by_their_names() {
        for yes in ["RulesCard", "InstructionCard1", "CardRight", "Instruction and Price Cards", "T2 Rules", "HelpCard_BadCats", "vpwinstrcard"] {
            assert!(rule_card_name(yes), "{yes}");
        }
        for no in ["cardboard_box_01", "PriceCard", "LIScoincard", "Ruler 100cm", "nachosrulesign", "DMDOptRulesCard", "DKInstructionSTKR", "playfield"] {
            assert!(!rule_card_name(no), "{no}");
        }
        assert!(apron_picture("Apron_new01", 1521, 652));
        assert!(!apron_picture("apron3", 987, 2048));
        assert!(!apron_picture("Apron_CP", 3584, 3584));
        assert!(!apron_picture("gradientapron", 1024, 256));
        assert!(!apron_picture("scorbitapron", 1024, 576));
        assert!(!apron_picture("plastics", 2048, 512));
    }

    /// The controls a table's script reads beyond the usual ones, comments left out.
    #[test]
    fn a_tables_own_controls_from_its_script() {
        let script = "Sub Table1_KeyDown(ByVal keycode)\n\tIf keycode = LeftMagnaSave Then Magnet.On\n\t' If keycode = LockBarKey Then Fire\nEnd Sub\nLoadVPM \"01560000\", \"WPC.VBS\", 3.26\n";
        let labels: Vec<&str> = own_controls(script).iter().map(|c| c.0).collect();
        assert_eq!(labels, ["Magna-saves", "Coin door", "Service buttons"]);
        assert!(own_controls("Sub Table1_Init\nEnd Sub\n").is_empty());
    }
}
