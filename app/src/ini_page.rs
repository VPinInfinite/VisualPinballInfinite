//! The settings pages made of plain settings — Picture (the screens' display timing among it), General,
//! PinUP packs — each setting read from the settings file
//! as the file has it, by VPX 5436's names, an unset one left to the player's own default; saved as
//! every settings page saves: only what changed, the file before kept as `Settings.previous.ini`.
//! The pages name their settings; only those in the sections these pages hold are written, and a
//! PIN2DMD key is never read out, only whether there is one. The Machines page's are VPinMAME's for
//! every game, the `[default]` section of `Machine settings.ini` beside the settings file (named
//! `Machine.default` here); the Advanced page's, every setting the settings file holds, any of them
//! changed or added by name but the key.

use serde::{Deserialize, Serialize};
use vpi_settings::Change;

/// The sections these pages may write.
const SECTIONS: [&str; 13] = ["Player", "TableOverride", "VPI", "Plugin.UpscaleDMD", "Plugin.ScoreView", "Plugin.Serum", "Plugin.VNI", "Plugin.DMDUtil", "Alpha", "DMD", "Plugin.PUP", "Backglass", "ScoreView"];

/// A key the pages may know is there, never what it is.
const SECRET: (&str, &str) = ("Plugin.VNI", "PacKey");

#[derive(Clone, Debug, Deserialize)]
pub struct Key {
    pub section: String,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// Each key's value as the file has it, in the order asked; None: not set.
    pub values: Vec<Option<String>>,
    pub file: String,
    pub exists: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Set {
    pub section: String,
    pub name: String,
    /// None: taken out, the player's own default again.
    pub value: Option<String>,
}

fn secret(section: &str, name: &str) -> bool {
    section.eq_ignore_ascii_case(SECRET.0) && name.eq_ignore_ascii_case(SECRET.1)
}

/// VPinMAME's settings for every game: `Machine.default` here, `[default]` in their own file.
const MACHINES: &str = "Machine.default";

fn machine_file() -> Option<std::path::PathBuf> {
    Some(vpi_settings::settings_path()?.parent()?.join("Machine settings.ini"))
}

fn read(path: &std::path::Path) -> String {
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

pub fn load(keys: &[Key]) -> Result<View, String> {
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    let entries = vpi_settings::parse_ini(&read(&path));
    let machines = vpi_settings::parse_ini(&machine_file().map(|f| read(&f)).unwrap_or_default());
    let values = keys
        .iter()
        .map(|k| {
            if k.section.eq_ignore_ascii_case(MACHINES) {
                return vpi_settings::get(&machines, "default", &k.name).map(|v| vpi_settings::vpx_trim(v).to_string());
            }
            let v = vpi_settings::get(&entries, &k.section, &k.name).map(|v| vpi_settings::vpx_trim(v).to_string());
            if secret(&k.section, &k.name) { v.filter(|v| !v.is_empty()).map(|_| "set".to_string()) } else { v }
        })
        .collect();
    Ok(View { values, file: path.display().to_string(), exists: path.exists() })
}

/// Every setting the settings file holds, as written, for the Advanced page: section, name, value
/// (the PIN2DMD key only as set); and VPinMAME's for every game, from their own file, as
/// `Machine.default`.
pub fn all() -> Result<Vec<(String, String, String)>, String> {
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    let mut out: Vec<(String, String, String)> = Vec::new();
    for (section, name, value) in vpi_settings::parse_ini(&read(&path)) {
        let value = if secret(&section, &name) { "set".to_string() } else { vpi_settings::vpx_trim(&value).to_string() };
        // a setting written twice counts by its last
        out.retain(|(s, n, _)| !(s.eq_ignore_ascii_case(&section) && n.eq_ignore_ascii_case(&name)));
        out.push((section, name, value));
    }
    let machines = vpi_settings::parse_ini(&machine_file().map(|f| read(&f)).unwrap_or_default());
    for (section, name, value) in machines {
        if section.eq_ignore_ascii_case("default") {
            out.retain(|(s, n, _)| !(s == MACHINES && n.eq_ignore_ascii_case(&name)));
            out.push((MACHINES.to_string(), name, vpi_settings::vpx_trim(&value).to_string()));
        }
    }
    Ok(out)
}

/// The changes to make, each checked: a section these pages hold (any, for the Advanced page),
/// never the key, one line each.
fn changes(sets: &[Set], any_section: bool) -> Result<Vec<Change>, String> {
    sets.iter()
        .map(|s| {
            let section_ok = any_section && !s.section.trim().is_empty() && !s.section.contains(['[', ']', '\r', '\n']) && !s.section.eq_ignore_ascii_case(MACHINES);
            if !(section_ok || SECTIONS.iter().any(|x| x.eq_ignore_ascii_case(&s.section))) || secret(&s.section, &s.name) {
                return Err(format!("[{}] {} isn't this page's to change.", s.section, s.name));
            }
            let one_line = |t: &str| !t.contains(['\r', '\n', '[', ']', '=']) && !t.trim().is_empty();
            if !one_line(&s.name) || s.value.as_deref().is_some_and(|v| v.contains(['\r', '\n'])) {
                return Err(format!("[{}] {} can't be written that way.", s.section, s.name));
            }
            Ok(match &s.value {
                Some(v) => Change::set(&s.section, &s.name, v),
                None => Change::remove(&s.section, &s.name),
            })
        })
        .collect()
}

pub fn save(sets: &[Set]) -> Result<(), String> {
    save_sets(sets, false)
}

/// The Advanced page's changes: any setting but the key.
pub fn save_any(sets: &[Set]) -> Result<(), String> {
    save_sets(sets, true)
}

fn save_sets(sets: &[Set], any_section: bool) -> Result<(), String> {
    // VPinMAME's for every game, into their own file, the one before kept beside it
    let (machine, rest): (Vec<Set>, Vec<Set>) = sets.iter().cloned().partition(|s| s.section.eq_ignore_ascii_case(MACHINES));
    if !machine.is_empty() {
        let file = machine_file().ok_or("no APPDATA folder")?;
        let as_default: Vec<Set> = machine.into_iter().map(|s| Set { section: "default".into(), ..s }).collect();
        let changes = as_default
            .iter()
            .map(|s| {
                if s.name.contains(['[', ']', '=', '\r', '\n']) || s.value.as_deref().is_some_and(|v| v.contains(['\r', '\n'])) {
                    return Err(format!("{} can't be written that way.", s.name));
                }
                Ok(match &s.value {
                    Some(v) => Change::set("default", &s.name, v),
                    None => Change::remove("default", &s.name),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let before = read(&file);
        let after = vpi_settings::edit(&before, &changes);
        if after != before {
            if !before.is_empty() {
                std::fs::write(file.with_file_name("Machine settings.previous.ini"), &before).map_err(|e| format!("VPinMAME's settings couldn't be kept before changing them: {e}"))?;
            }
            let new = file.with_extension("ini.new");
            std::fs::write(&new, &after).map_err(|e| format!("VPinMAME's settings couldn't be written: {e}"))?;
            std::fs::rename(&new, &file).map_err(|e| format!("VPinMAME's settings couldn't be written: {e}"))?;
        }
    }
    if rest.is_empty() {
        return Ok(());
    }
    let changes = changes(&rest, any_section)?;
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    let before = read(&path);
    let after = vpi_settings::edit(&before, &changes);
    crate::screens::write_settings(&path, &before, &after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(section: &str, name: &str, value: Option<&str>) -> Set {
        Set { section: section.into(), name: name.into(), value: value.map(str::to_string) }
    }

    #[test]
    fn only_the_pages_own_sections_are_written_and_never_the_key() {
        let file = "[Player]\nMSAASamples = 1\n; my own note\n\n[Plugin.VNI]\nPacKey = secret\n\n[Input]\nMapping.LeftFlipper = K42\n";
        let made = changes(&[set("Player", "MSAASamples", Some("0")), set("TableOverride", "ToneMapper", Some("2")), set("Player", "ForceBloomOff", None)], false).unwrap();
        let out = vpi_settings::edit(file, &made);
        let e = vpi_settings::parse_ini(&out);
        assert_eq!(vpi_settings::get(&e, "Player", "MSAASamples"), Some("0"));
        assert_eq!(vpi_settings::get(&e, "TableOverride", "ToneMapper"), Some("2"));
        assert!(out.contains("; my own note") && out.contains("PacKey = secret") && out.contains("Mapping.LeftFlipper = K42"));
        // not the page's: the controls, the key
        assert!(changes(&[set("Input", "Mapping.LeftFlipper", Some("K1"))], false).is_err());
        assert!(changes(&[set("Plugin.VNI", "PacKey", Some("x"))], false).is_err());
        assert!(changes(&[set("Player", "MaxFramerate", Some("60\n[Input]"))], false).is_err());
        // the Advanced page: any section, but never the key
        assert!(changes(&[set("Input", "Mapping.LeftFlipper", Some("K1"))], true).is_ok());
        assert!(changes(&[set("Plugin.VNI", "PacKey", Some("x"))], true).is_err());
    }
}
