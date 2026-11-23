//! Add-ons' settings, kept in our app: what tables' add-ons keep in
//! the Windows registry — UltraDMD's colour and full-colour settings, which
//! Diablo's script reads and writes itself — answered from a file of ours
//! instead, `%APPDATA%\Visual Pinball Infinite\Add-on settings.ini`, a
//! section an add-on and a line a setting. Only settings an add-on is known
//! to keep are answered (the script's registry calls, `script::shell`).

use std::path::PathBuf;
use std::sync::Mutex;

/// The settings: (section, name, value), in the order first set.
struct Store {
    path: Option<PathBuf>,
    entries: Vec<(String, String, String)>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VPI_ADDON_SETTINGS") {
        return Some(PathBuf::from(p));
    }
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("Add-on settings.ini"))
}

fn with<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = guard.get_or_insert_with(|| {
        let path = default_path();
        let entries = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| crate::settings::parse_ini(&t)).unwrap_or_default();
        Store { path, entries }
    });
    f(store)
}

/// A setting, any case.
pub fn get(section: &str, name: &str) -> Option<String> {
    with(|s| s.entries.iter().find(|(sec, n, _)| sec.eq_ignore_ascii_case(section) && n.eq_ignore_ascii_case(name)).map(|e| e.2.clone()))
}

/// Sets a setting and writes the file.
pub fn set(section: &str, name: &str, value: &str) {
    with(|s| {
        match s.entries.iter_mut().find(|(sec, n, _)| sec.eq_ignore_ascii_case(section) && n.eq_ignore_ascii_case(name)) {
            Some(e) => e.2 = value.to_string(),
            None => s.entries.push((section.to_string(), name.to_string(), value.to_string())),
        }
        let Some(path) = &s.path else { return };
        let mut sections: Vec<&str> = Vec::new();
        for (sec, _, _) in &s.entries {
            if !sections.iter().any(|x| x.eq_ignore_ascii_case(sec)) {
                sections.push(sec);
            }
        }
        let mut text = String::from("; Settings tables' add-ons keep, kept by Visual Pinball Infinite instead of the registry\n");
        for sec in sections {
            text.push_str(&format!("\n[{sec}]\n"));
            for (_, n, v) in s.entries.iter().filter(|e| e.0.eq_ignore_ascii_case(sec)) {
                text.push_str(&format!("{n}={v}\n"));
            }
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    });
}

/// For tests: settings held in memory only, starting from these.
#[cfg(test)]
pub fn in_memory(entries: &[(&str, &str, &str)]) {
    *STORE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Store { path: None, entries: entries.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect() });
}

#[cfg(test)]
mod tests {
    #[test]
    fn settings_read_back_any_case() {
        let parsed = crate::settings::parse_ini("; a note\n[UltraDMD]\ncolor = Tomato\nfullcolor=True\n\n[Other]\nx=1\n");
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], ("UltraDMD".into(), "color".into(), "Tomato".into()));
        super::in_memory(&[("UltraDMD", "color", "Tomato")]);
        assert_eq!(super::get("ultradmd", "COLOR").as_deref(), Some("Tomato"));
        super::set("UltraDMD", "fullcolor", "True");
        assert_eq!(super::get("UltraDMD", "fullcolor").as_deref(), Some("True"));
        assert_eq!(super::get("UltraDMD", "hideVDMD"), None);
    }
}
