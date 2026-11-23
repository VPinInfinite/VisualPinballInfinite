//! Changes written into the settings file in place, as a person would make
//! them in a text editor, so a file kept by hand — or copied from VPX's own —
//! keeps its comments, order and spelling.

/// A change to the settings file: a value set, or the setting taken out (None).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub section: String,
    pub name: String,
    pub value: Option<String>,
}

impl Change {
    pub fn set(section: &str, name: &str, value: impl ToString) -> Change {
        Change { section: section.to_string(), name: name.to_string(), value: Some(value.to_string()) }
    }

    pub fn remove(section: &str, name: &str) -> Change {
        Change { section: section.to_string(), name: name.to_string(), value: None }
    }
}

/// One line of the file, as `parse_ini` reads it.
struct Line {
    text: String,
    /// The section it lies in (none before the first header).
    section: Option<String>,
    /// For a setting's line: its name.
    name: Option<String>,
    header: bool,
}

/// The file's text with the changes made, as a person would make them: a setting already there
/// keeps its line — its name's spelling and the spaces around its `=` — with its value replaced
/// (its last copy, the one that counts; earlier copies, which count for nothing, stay); one not
/// there goes at the end of its section, after the section's last setting, or into a new
/// section at the end of the file; a setting taken out loses every copy. Comments, blank lines
/// and everything else stay as they were, and so do the file's own line ends (Windows' for a
/// new file).
pub fn edit(text: &str, changes: &[Change]) -> String {
    let newline = if text.contains("\r\n") || !text.contains('\n') { "\r\n" } else { "\n" };
    let mut lines: Vec<Line> = Vec::new();
    let mut section: Option<String> = None;
    for raw in text.lines() {
        let t = raw.trim();
        if let Some(s) = t.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = Some(s.trim().to_string());
            lines.push(Line { text: raw.to_string(), section: section.clone(), name: None, header: true });
        } else if let Some((k, _)) = t.split_once('=')
            && !t.starts_with(';')
        {
            lines.push(Line { text: raw.to_string(), section: section.clone(), name: Some(k.trim().to_string()), header: false });
        } else {
            lines.push(Line { text: raw.to_string(), section: section.clone(), name: None, header: false });
        }
    }
    let same = |a: &Option<String>, b: &str| a.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(b));
    for change in changes {
        let is_it = |l: &Line| same(&l.section, &change.section) && l.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&change.name));
        match &change.value {
            None => lines.retain(|l| !is_it(l)),
            Some(value) => {
                if let Some(at) = lines.iter().rposition(is_it) {
                    // the name and what comes before the value kept as written
                    let old = &lines[at].text;
                    let eq = old.find('=').unwrap_or(old.len());
                    let after = &old[eq + 1..];
                    let gap = &after[..after.len() - after.trim_start().len()];
                    let new = format!("{}={gap}{value}", &old[..eq]);
                    lines[at].text = new;
                } else {
                    let line = Line { text: format!("{} = {value}", change.name), section: Some(change.section.clone()), name: Some(change.name.clone()), header: false };
                    // after the section's last setting, else right after its last header
                    let last_setting = lines.iter().rposition(|l| same(&l.section, &change.section) && l.name.is_some());
                    let last_header = lines.iter().rposition(|l| l.header && same(&l.section, &change.section));
                    match last_setting.or(last_header) {
                        Some(at) => lines.insert(at + 1, line),
                        None => {
                            if lines.last().is_some_and(|l| !l.text.trim().is_empty()) {
                                lines.push(Line { text: String::new(), section: None, name: None, header: false });
                            }
                            lines.push(Line { text: format!("[{}]", change.section), section: Some(change.section.clone()), name: None, header: true });
                            lines.push(line);
                        }
                    }
                }
            }
        }
    }
    let mut out = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(newline);
    if !out.is_empty() {
        out.push_str(newline);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get, parse_ini};

    #[test]
    fn a_setting_there_keeps_its_line() {
        let text = "; my cabinet\r\n[Player]\r\nPlayfieldDisplay=Wide Screen\r\nplayfieldfullscreen   =  0\r\n\r\n[Backglass]\r\nBackglassOutput = 1\r\n";
        let out = edit(text, &[Change::set("player", "PlayfieldFullScreen", 1), Change::set("Player", "PlayfieldDisplay", "Big Screen")]);
        assert_eq!(out, "; my cabinet\r\n[Player]\r\nPlayfieldDisplay=Big Screen\r\nplayfieldfullscreen   =  1\r\n\r\n[Backglass]\r\nBackglassOutput = 1\r\n");
    }

    #[test]
    fn a_new_setting_goes_at_the_end_of_its_section() {
        let text = "[Player]\nPlayfieldDisplay = Wide Screen\n\n[Backglass]\nBackglassOutput = 1\n";
        let out = edit(text, &[Change::set("Player", "PlayfieldWndX", 10), Change::set("Player", "PlayfieldWndY", 20), Change::set("Topper", "TopperOutput", 1)]);
        assert_eq!(out, "[Player]\nPlayfieldDisplay = Wide Screen\nPlayfieldWndX = 10\nPlayfieldWndY = 20\n\n[Backglass]\nBackglassOutput = 1\n\n[Topper]\nTopperOutput = 1\n");
        // an empty section takes it right after its header
        assert_eq!(edit("[VPI]\n; nothing yet\n", &[Change::set("VPI", "FloatingScore", 0)]), "[VPI]\nFloatingScore = 0\n; nothing yet\n");
        // a new file
        assert_eq!(edit("", &[Change::set("Player", "BGSet", 1)]), "[Player]\r\nBGSet = 1\r\n");
    }

    #[test]
    fn the_copy_that_counts_changes_and_a_removal_takes_every_copy() {
        let text = "[Player]\nBGSet = 0\n[player]\nBGSet = 2\nSyncMode = 1\n";
        let out = edit(text, &[Change::set("Player", "BGSet", 1)]);
        assert_eq!(get(&parse_ini(&out), "Player", "BGSet"), Some("1"));
        assert_eq!(out, "[Player]\nBGSet = 0\n[player]\nBGSet = 1\nSyncMode = 1\n");
        let out = edit(text, &[Change::remove("Player", "bgset")]);
        assert_eq!(out, "[Player]\n[player]\nSyncMode = 1\n");
        // a comment that looks like a setting is left alone
        assert_eq!(edit(";BGSet = 2\n[Player]\n", &[Change::remove("Player", "BGSet")]), ";BGSet = 2\n[Player]\n");
    }
}
