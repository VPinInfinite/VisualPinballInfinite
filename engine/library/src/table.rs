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
    out
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
    let ScriptFacts { game_name, pinmame, pup_names, helpers, music, folders, saved_under, uses_pup } = script_facts(&data.code.string);
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
    use std::io::Read;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut stg = cfb::open(path).map_err(fail)?;
    let mut out = Vec::new();
    for i in 0.. {
        let Ok(stream) = stg.open_stream(format!("/GameStg/Image{i}")) else { break };
        let mut head = Vec::new();
        stream.take(4096).read_to_end(&mut head).map_err(fail)?;
        if let Some(name) = biff_string(&head, b"NAME") {
            out.push(name);
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
    ScriptFacts { game_name, pinmame, pup_names, helpers, music, folders, saved_under, uses_pup }
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
}
