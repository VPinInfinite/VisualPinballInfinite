//! The settings pages made of plain settings — Picture (the screens' display timing among it), General,
//! PinUP packs — each setting read from the settings file
//! as the file has it, by VPX 5436's names, an unset one left to the player's own default; saved as
//! every settings page saves: only what changed, each file before kept beside it as `<name>.previous.ini`
//! (VPX's as `VPinballX.previous.ini`).
//! The pages name their settings; only those in the sections these pages hold are written, and a
//! PIN2DMD key is never read out, only whether there is one. The Machines page's are the classic
//! VPinMAME's for every game, where it keeps them (`machines`, its `default`; named `Machine.default`
//! here); the Advanced page's, every setting the settings files hold, any of them changed or added
//! by name but the key.

use serde::{Deserialize, Serialize};
use vpi_settings::Change;

/// The sections these pages may write.
const SECTIONS: [&str; 16] = ["Player", "PlayerVR", "TableOverride", "VPI", "Plugin.UpscaleDMD", "Plugin.ScoreView", "Plugin.Nixie", "Plugin.Serum", "Plugin.VNI", "Plugin.DMDUtil", "Alpha", "DMD", "Plugin.PUP", "Backglass", "ScoreView", "Plugin.PinMAME"];

/// An add-on VPX's later builds call by another name: the score view's, `ScoreView` before, `Nixie`
/// after. The pages name the earlier; its setting is read from the later where that's set, and
/// written to both, so either build reads it.
const RENAMED: (&str, &str) = ("Plugin.ScoreView", "Plugin.Nixie");

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

/// VPinMAME's settings for every game: `Machine.default` here, its `default` where it keeps them.
const MACHINES: &str = "Machine.default";

pub fn load(keys: &[Key]) -> Result<View, String> {
    let path = vpi_settings::vpx_settings_path().ok_or("no APPDATA folder")?;
    let entries = vpi_settings::read_all();
    let machines = crate::machines::entries(&["default"]);
    let values = keys
        .iter()
        .map(|k| {
            if k.section.eq_ignore_ascii_case(MACHINES) {
                return vpi_settings::get(&machines, "default", &k.name).map(|v| vpi_settings::vpx_trim(v).to_string());
            }
            let later = k.section.eq_ignore_ascii_case(RENAMED.0).then(|| vpi_settings::get(&entries, RENAMED.1, &k.name)).flatten();
            let v = later.or_else(|| vpi_settings::get(&entries, &k.section, &k.name)).map(|v| vpi_settings::vpx_trim(v).to_string());
            if secret(&k.section, &k.name) { v.filter(|v| !v.is_empty()).map(|_| "set".to_string()) } else { v }
        })
        .collect();
    Ok(View { values, file: path.display().to_string(), exists: path.exists() })
}

/// Every setting the settings file holds, as written, for the Advanced page: section, name, value
/// (the PIN2DMD key only as set); and VPinMAME's for every game, from their own file, as
/// `Machine.default`.
pub fn all() -> Result<Vec<(String, String, String)>, String> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for (section, name, value) in vpi_settings::read_all() {
        let value = if secret(&section, &name) { "set".to_string() } else { vpi_settings::vpx_trim(&value).to_string() };
        // a setting written twice counts by its last
        out.retain(|(s, n, _)| !(s.eq_ignore_ascii_case(&section) && n.eq_ignore_ascii_case(&name)));
        out.push((section, name, value));
    }
    for (section, name, value) in crate::machines::entries(&["default"]) {
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
    // the renamed add-on's settings written under its later name too
    let mirrored: Vec<Set> = sets.iter().filter(|s| s.section.eq_ignore_ascii_case(RENAMED.0)).map(|s| Set { section: RENAMED.1.to_string(), ..s.clone() }).collect();
    sets.iter()
        .chain(mirrored.iter())
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
    // VPinMAME's for every game, where it keeps them
    let (machine, rest): (Vec<Set>, Vec<Set>) = sets.iter().cloned().partition(|s| s.section.eq_ignore_ascii_case(MACHINES));
    if !machine.is_empty() {
        let changes: Vec<Change> = machine
            .iter()
            .map(|s| match &s.value {
                Some(v) => Change::set("default", &s.name, v),
                None => Change::remove("default", &s.name),
            })
            .collect();
        crate::machines::apply(&changes)?;
    }
    if rest.is_empty() {
        return Ok(());
    }
    vpi_settings::save(&changes(&rest, any_section)?)
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
