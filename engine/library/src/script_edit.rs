//! A table's script as the app's script editor sees it: its option lines — the settings a table's
//! authors leave near the top of its script for players to change, a constant or a variable given a
//! plain value, explained in a comment — found, and changed in place, every other byte as it was.
//! Ours.
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

/// An option line of a script.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptOption {
    /// Its name in the script.
    pub name: String,
    /// Its value as the script writes it: a number, True or False, or a string in quotes.
    pub value: String,
    /// `number`, `bool` or `text`.
    pub kind: &'static str,
    /// Its line, from 0.
    pub line: usize,
    /// The heading it sits under, without its stars and dashes.
    pub group: String,
    /// What its comment says, the comment lines just below it too.
    pub note: String,
    /// The values its comment names, with what each does, where it names two or more.
    pub choices: Vec<(String, String)>,
}

/// A plain value: a number, True or False, or a string in quotes (VBScript doubles a quote inside).
const VALUE: &str = r#"-?(?:\d+\.?\d*|\.\d+)|True|False|"(?:[^"]|"")*""#;

/// `Const Name = value`, or `Dim Name : Name = value`, and its comment.
static OPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"(?i)^\s*(?:(?:Public|Private)\s+)?(?:Const\s+(\w+)|Dim\s+(\w+)\s*:\s*(\w+))\s*=\s*({VALUE})\s*(?:'(.*))?$"#)).unwrap()
});
/// Where the script's routines begin: its option lines come before.
static ROUTINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(?:(?:Public|Private)\s+)?(?:Sub|Function|Class|Property\s+(?:Get|Let|Set))\s+\w").unwrap());
/// A comment line alone.
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*'(.*)$").unwrap());
/// Where a comment names a value and goes on to say what it does: `0 = off`, `1 - Minimal room`,
/// `2: slow`.
static CHOICE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|[\s,;|(])(-?\d+(?:\.\d+)?)\s*(?:=|-|:)\s*").unwrap());
/// A comment line saying what a value does first, then the value: `Cabinet Mode On = 1`.
static CHOICE_AFTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*([A-Za-z][^=]*?)\s*=\s*(-?\d+(?:\.\d+)?)\s*$").unwrap());

/// The names scripts give their machine's wiring, through VPinMAME's `controller.vbs`, and their
/// own bookkeeping, which aren't settings for a player.
const WIRING: &[&str] = &[
    "cgamename", "ccredits", "usesolenoids", "uselamps", "usegi", "usesync", "handlemech", "usevpmmodsol", "usevpmdmd", "usevpmcoloreddmd", "usevpmnvram", "ssolenoidon", "ssolenoidoff",
    "sflipperon", "sflipperoff", "scoin", "sknocker", "csinglelflip", "csinglerflip", "tnob", "lob", "bipl", "tablewidth", "tableheight", "desktopmode", "b2scgamename",
];

/// The characters a heading is drawn with.
const RULE: &str = "*-=#_~+/\\";

/// A comment's words without the stars, dashes, slashes and equals signs a heading is drawn with.
fn heading_words(text: &str) -> String {
    text.trim_matches(|c: char| c.is_whitespace() || RULE.contains(c)).trim().to_string()
}

/// Whether a comment line is only a rule: three or more stars, dashes, slashes and the like.
fn decoration(text: &str) -> bool {
    let t = text.trim();
    t.len() >= 3 && t.chars().all(|c| RULE.contains(c))
}

/// Whether a comment line draws a heading: words with a rule of three or more before them, or a
/// rule on both sides (`----- Sound Options -----`).
fn heading(text: &str) -> bool {
    let t = text.trim();
    let before = t.chars().take_while(|c| RULE.contains(*c)).count();
    let after = t.chars().rev().take_while(|c| RULE.contains(*c)).count();
    !heading_words(t).is_empty() && (before >= 3 || (before >= 1 && after >= 1))
}

/// The option lines of a script: before its routines begin, a constant or a variable given a plain
/// value — under a heading that says options or settings, or explained in a comment of its own, or
/// just after one that is — leaving out a machine's wiring.
pub fn options(script: &str) -> Vec<ScriptOption> {
    let lines: Vec<&str> = script.lines().collect();
    let mut out: Vec<ScriptOption> = Vec::new();
    let mut group = String::new();
    // whether the last line read was an option line taken (a block of them goes together)
    let mut last_taken: Option<usize> = None;
    // the comment lines just above, which explain the line below them
    let mut above: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if ROUTINE.is_match(line) {
            break;
        }
        if let Some(c) = COMMENT.captures(line) {
            let text = c.get(1).map_or("", |m| m.as_str());
            // a heading: ruled, or words between rules
            let between = i.checked_sub(1).is_some_and(|p| COMMENT.captures(lines[p]).is_some_and(|c| decoration(&c[1])));
            if heading(text) || (between && !heading_words(text).is_empty()) {
                group = heading_words(text);
                above.clear();
            } else if decoration(text) {
                above.clear();
            } else {
                let words = text.trim_start_matches('/').trim();
                if !words.is_empty() {
                    above.push(words.to_string());
                }
            }
            i += 1;
            continue;
        }
        let Some(c) = OPTION.captures(line) else {
            if !line.trim().is_empty() {
                last_taken = None;
            }
            above.clear();
            i += 1;
            continue;
        };
        let name = c.get(1).or(c.get(2)).map_or("", |m| m.as_str());
        // `Dim A : B = value` sets another variable: not an option line
        if c.get(3).is_some_and(|m| !m.as_str().eq_ignore_ascii_case(name)) || WIRING.contains(&name.to_lowercase().as_str()) {
            i += 1;
            continue;
        }
        let value = c[4].to_string();
        let mut note = c.get(5).map_or(String::new(), |m| m.as_str().trim().to_string());
        // what's said above it, where nothing's said beside it
        let said_above = std::mem::take(&mut above);
        if note.is_empty() {
            note = said_above.join(" ");
        }
        // the comment lines just below, indented to its comment, carry it on
        let mut j = i + 1;
        while j < lines.len() {
            let Some(more) = COMMENT.captures(lines[j]) else { break };
            let text = more.get(1).map_or("", |m| m.as_str());
            let indented = lines[j].starts_with([' ', '\t']) || text.starts_with('\t') || text.starts_with("  ");
            let words = text.trim_start().trim_start_matches('\'').trim();
            if !indented || words.is_empty() || decoration(text) {
                break;
            }
            note.push(' ');
            note.push_str(words);
            j += 1;
        }
        let wanted = group.to_lowercase().contains("option")
            || ["setting", "user", "config", "choice", "preference"].iter().any(|w| group.to_lowercase().contains(w))
            || !note.is_empty()
            || last_taken.is_some_and(|l| l + 1 == i);
        if wanted {
            let kind = if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
                "bool"
            } else if value.starts_with('"') {
                "text"
            } else {
                "number"
            };
            let choices = if kind != "number" {
                Vec::new()
            } else {
                // values said first, else said after what they do, a line each
                let first = choices(&note);
                if first.is_empty() { choices_after(&said_above) } else { first }
            };
            out.push(ScriptOption { name: name.to_string(), value, kind, line: i, group: group.clone(), note, choices });
            last_taken = Some(j - 1);
        } else {
            last_taken = None;
        }
        i = j;
    }
    out
}

/// The values a comment names, with what each does, where it names two or more: each value's
/// words running to where the next value begins.
fn choices(note: &str) -> Vec<(String, String)> {
    let starts: Vec<(usize, usize, String)> = CHOICE.captures_iter(note).map(|c| (c.get(0).unwrap().start(), c.get(0).unwrap().end(), c[1].to_string())).collect();
    let mut found = Vec::new();
    for (k, (_, end, value)) in starts.iter().enumerate() {
        let until = starts.get(k + 1).map_or(note.len(), |next| next.0);
        let words = note[*end..until].trim().trim_end_matches([',', ';', '|', '.', ' ']).trim();
        // words, not another number (a range, `0 - 100`)
        if words.starts_with(|c: char| c.is_alphabetic() || c == '(' || c == '"') {
            found.push((value.clone(), words.to_string()));
        }
    }
    let distinct = found.iter().map(|(v, _)| v.as_str()).collect::<std::collections::HashSet<_>>().len();
    if found.len() >= 2 && distinct == found.len() { found } else { Vec::new() }
}

/// The values named a line each, after what they do (`Cabinet Mode On = 1`), where two or more are.
fn choices_after(lines: &[String]) -> Vec<(String, String)> {
    let found: Vec<(String, String)> = lines.iter().filter_map(|l| CHOICE_AFTER.captures(l)).map(|c| (c[2].to_string(), c[1].trim().to_string())).collect();
    let distinct = found.iter().map(|(v, _)| v.as_str()).collect::<std::collections::HashSet<_>>().len();
    if found.len() >= 2 && distinct == found.len() { found } else { Vec::new() }
}

/// The script with the option lines named in `changes` given their new values, written as the
/// script writes values; every other byte as it was. A name it has no option line for, or a value
/// of the wrong kind, refused.
pub fn with_options(script: &str, changes: &[(String, String)]) -> Result<String, String> {
    let found = options(script);
    // the lines as they are, their own endings kept
    let mut lines: Vec<String> = script.split_inclusive('\n').map(str::to_string).collect();
    for (name, value) in changes {
        let option = found.iter().find(|o| o.name.eq_ignore_ascii_case(name)).ok_or_else(|| format!("Its script has no option line for {name}."))?;
        let written = match option.kind {
            "bool" if value.eq_ignore_ascii_case("true") => "True".to_string(),
            "bool" if value.eq_ignore_ascii_case("false") => "False".to_string(),
            "number" if Regex::new(r"^-?(?:\d+\.?\d*|\.\d+)$").unwrap().is_match(value.trim()) => value.trim().to_string(),
            // the words alone, given their quotes here
            "text" if !value.contains(['\r', '\n']) => format!("\"{}\"", value.replace('"', "\"\"")),
            _ => return Err(format!("{value} can't be {name}'s value.")),
        };
        let line = lines.get_mut(option.line).ok_or("The script changed while it was read.")?;
        let c = OPTION.captures(line.trim_end_matches(['\r', '\n'])).ok_or("The script changed while it was read.")?;
        let at = c.get(4).unwrap();
        line.replace_range(at.start()..at.end(), &written);
    }
    Ok(lines.concat())
}

/// A script's fingerprint, to tell whether a table's own script changed (FNV-1a over its bytes).
pub fn digest(script: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in script {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "Option Explicit\r\n\
        Const Ballsize = 50\r\n\
        \r\n\
        '********************\r\n\
        ' Table Options\r\n\
        '********************\r\n\
        \r\n\
        '----- Target Bouncer Levels -----\r\n\
        Const TargetBouncerEnabled = 1\t  ' 0 = normal standup targets, 1 = bouncy targets\r\n\
        Const TargetBouncerFactor = 0.7\t  ' Level of bounces. Recommended value of 0.7\r\n\
        \r\n\
        '----- Ball Shadow Options -----\r\n\
        Const AmbientBallShadowOn = 1\t\t' 0 = Static shadow under ball\r\n\
        '\t\t\t\t\t\t\t\t\t' 1 = Moving ball shadow\r\n\
        Const BallMass = 1\r\n\
        \r\n\
        Const bRotateSaucers = False 'change this if you want the small saucers to rotate\r\n\
        Const bRotateBigUFO = False\r\n\
        Dim VRRoom : VRRoom = \"Minimal\" ' the room's name\r\n\
        Const cGameName = \"xenon\" ' its ROM\r\n\
        Const UseSolenoids = 2\r\n\
        Dim DesktopMode: DesktopMode = Table1.ShowDT\r\n\
        Dim BIPL : BIPL = False ' Ball in plunger lane\r\n\
        \r\n\
        Sub Table1_Init\r\n\
        \x20   Const Inside = 3 ' not an option\r\n\
        End Sub\r\n";

    #[test]
    fn a_scripts_option_lines_found_with_their_headings_and_choices() {
        let found = options(SCRIPT);
        let names: Vec<&str> = found.iter().map(|o| o.name.as_str()).collect();
        // the plain Ballsize at the top is under no heading and explains nothing; the wiring and
        // bookkeeping, a value worked out, and what's inside a routine are left out
        assert_eq!(names, ["TargetBouncerEnabled", "TargetBouncerFactor", "AmbientBallShadowOn", "BallMass", "bRotateSaucers", "bRotateBigUFO", "VRRoom"]);
        let bouncer = &found[0];
        assert_eq!((bouncer.group.as_str(), bouncer.kind, bouncer.line), ("Target Bouncer Levels", "number", 8));
        assert_eq!(bouncer.choices, [("0".to_string(), "normal standup targets".to_string()), ("1".to_string(), "bouncy targets".to_string())]);
        // a comment carried on below it, and its choices from both lines
        assert_eq!(found[2].note, "0 = Static shadow under ball 1 = Moving ball shadow");
        assert_eq!(found[2].choices.len(), 2);
        assert!(found[1].choices.is_empty());
        assert_eq!((found[4].kind, found[6].kind, found[6].value.as_str()), ("bool", "text", "\"Minimal\""));
    }

    #[test]
    fn headings_drawn_with_slashes_and_comments_above_an_option() {
        let script = "' Options\r\n\
            \r\n\
            '///////////-----VR Room-----///////////\r\n\
            Const VRRoom = 0 ' 0 - VR Room off, 1 - Minimal Room, 2 - Ultra Minimal\r\n\
            \r\n\
            '/////////-----Cabinet Mode-----////////\r\n\
            'Will hide the rails and scale the side panels higher\r\n\
            '\tCabinet Mode Off = 0\r\n\
            '\tCabinet Mode On = 1\r\n\
            Const CabinetMode = 0\r\n\
            \r\n\
            '///////////----- General Sound Options -----///////////\r\n\
            '// VolumeDial:\r\n\
            '// Recommended values should be no greater than 1.\r\n\
            Const VolumeDial = 0.8\r\n";
        let found = options(script);
        let got: Vec<(&str, &str)> = found.iter().map(|o| (o.name.as_str(), o.group.as_str())).collect();
        assert_eq!(got, [("VRRoom", "VR Room"), ("CabinetMode", "Cabinet Mode"), ("VolumeDial", "General Sound Options")]);
        assert_eq!(found[0].choices.len(), 3);
        assert_eq!(found[1].choices, [("0".to_string(), "Cabinet Mode Off".to_string()), ("1".to_string(), "Cabinet Mode On".to_string())]);
        assert_eq!(found[2].note, "VolumeDial: Recommended values should be no greater than 1.");
    }

    #[test]
    fn option_lines_changed_in_place_every_other_byte_kept() {
        let changed = with_options(SCRIPT, &[("TargetBouncerEnabled".into(), "0".into()), ("bRotateSaucers".into(), "true".into()), ("VRRoom".into(), "Mega \"room\"".into())]).unwrap();
        assert!(changed.contains("Const TargetBouncerEnabled = 0\t  ' 0 = normal standup targets, 1 = bouncy targets\r\n"));
        assert!(changed.contains("Const bRotateSaucers = True 'change this"));
        assert!(changed.contains("Dim VRRoom : VRRoom = \"Mega \"\"room\"\"\" ' the room's name\r\n"));
        // False to True one shorter, the room's name six longer
        assert_eq!(changed.len(), SCRIPT.len() - 1 + 6);
        assert!(with_options(SCRIPT, &[("TargetBouncerFactor".into(), "loud".into())]).is_err());
        assert!(with_options(SCRIPT, &[("Inside".into(), "4".into())]).is_err());
        assert_eq!(with_options(SCRIPT, &[]).unwrap(), SCRIPT);
    }
}
