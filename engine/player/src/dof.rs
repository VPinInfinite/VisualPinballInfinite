// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its DOF plugin,
// plugins/dof/DOFPlugin.cpp. Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The cabinet's toys: `vpi-dof` runs libdof beside the player — a
//! program of its own, so a fault in libdof never ends a game — started
//! when the game starts and fed the game's event stream (`events.rs`), as
//! VPX 5436's DOF plugin feeds libdof on a thread of its own. The toys are
//! named by the ROM, as VPX 5436 names them, or, for a table running no
//! ROM, by the name it gives B2S (its `cGameName`), as VPX 10.8.0's B2S
//! names them, so every table's toys work. The configuration is
//! the cabinet's DOF Config Tool files, read where the user keeps them: our
//! settings folder first, then VPX's, the whole set from the first folder
//! holding one (`directoutputconfig\` in it). Without one, nothing starts.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use dof_link::Event;

/// How long the player waits for `vpi-dof` to end (libdof turning the toys off) before ending it.
const STOP_TIMEOUT: Duration = Duration::from_millis(2000);

/// Numbers the links this player makes, so each is new.
static NEXT_LINK: AtomicU32 = AtomicU32::new(0);

/// `vpi-dof` at work for one game.
pub struct Dof {
    link: dof_link::Link,
    child: Child,
    /// Each output's value as the toys last heard it (VPX's event stream starts each at -1, so
    /// its first value is an event too).
    last: HashMap<(char, i32), i32>,
    /// Said once if the link was full.
    full_said: bool,
}

/// A configuration set in `dir`: DOF's global configuration, or a table file
/// (`directoutputconfig*.ini`, the Config Tool's one per output board).
fn holds_set(dir: &Path) -> bool {
    dir.join("GlobalConfig_B2SServer.xml").is_file()
        || std::fs::read_dir(dir).is_ok_and(|entries| {
            entries.flatten().any(|e| {
                let name = e.file_name().to_string_lossy().to_ascii_lowercase();
                name.starts_with("directoutputconfig") && name.ends_with(".ini")
            })
        })
}

/// The folder holding the cabinet's configuration in its `directoutputconfig\`:
/// ours, then VPX's, the first holding a set. Read only.
pub fn config_folder() -> Option<PathBuf> {
    let appdata = PathBuf::from(std::env::var_os("APPDATA")?);
    [appdata.join("Visual Pinball Infinite"), appdata.join("VPinballX")].into_iter().find(|base| holds_set(&base.join("directoutputconfig")))
}

/// Whether the settings' `[Plugin.DOF]` leave the toys on (`Enable`; on unless 0).
fn enabled(settings: &[(String, String)]) -> bool {
    let enable = settings.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case("Enable")).map(|(_, v)| crate::settings::vpx_trim(v).to_string());
    enable.as_deref().and_then(crate::settings::vpx_bool) != Some(false)
}

/// The game named `name` started — a ROM, or a table naming itself through
/// B2S: its toys, when the settings leave them on and a configuration exists.
pub fn on_game_start(w: &mut crate::world::World, name: &str) {
    if name.is_empty() || w.dof.is_some() || !enabled(&w.dof_settings) {
        return;
    }
    let Some(folder) = w.dof_folder.clone().or_else(config_folder) else {
        crate::script::log::once("DOF: no cabinet configuration (directoutputconfig in Visual Pinball Infinite's or VPX's settings folder): the toys are off");
        return;
    };
    let table = w.table_path.clone().unwrap_or_default();
    match Dof::start(&table, name, &folder) {
        Ok(d) => {
            crate::script::log::once(&format!("DOF: {name}'s toys from {}", folder.join("directoutputconfig").display()));
            w.dof = Some(d);
        }
        Err(e) => crate::script::log::once(&format!("DOF: {e}")),
    }
}

impl Dof {
    /// Starts `vpi-dof` on a new link for the table at `table`, its toys named
    /// `name`, its configuration in `folder`'s `directoutputconfig\`.
    pub fn start(table: &Path, name: &str, folder: &Path) -> Result<Dof, String> {
        let host = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("vpi-dof.exe");
        if !host.exists() {
            return Err(format!("{} is missing", host.display()));
        }
        let link_name = format!(r"Local\VPI-DOF-{}-{}", std::process::id(), NEXT_LINK.fetch_add(1, Ordering::Relaxed));
        let link = dof_link::Link::create(&link_name).map_err(|e| format!("the link to vpi-dof: {e}"))?;
        let child = std::process::Command::new(&host)
            .args(["--link", &link_name, "--parent", &std::process::id().to_string(), "--name", name])
            .arg("--table")
            .arg(table)
            .arg("--config")
            .arg(folder)
            .spawn()
            .map_err(|e| format!("start {}: {e}", host.display()))?;
        Ok(Dof { link, child, last: HashMap::new(), full_said: false })
    }

    /// The machine's outputs now: each that changed since the toys last heard
    /// it goes to them.
    pub fn machine_states(&mut self, states: &[(char, i32, i32)]) {
        let mut sent = false;
        for &(kind, number, value) in states {
            let last = self.last.entry((kind, number)).or_insert(-1);
            if *last != value {
                *last = value;
                sent |= self.push(Event { kind, number, value });
            }
        }
        if sent {
            self.link.wake_host();
        }
    }

    /// An event as it comes, to the toys at once.
    pub fn queue(&mut self, kind: char, number: i32, value: i32) {
        if self.push(Event { kind, number, value }) {
            self.link.wake_host();
        }
    }

    fn push(&mut self, e: Event) -> bool {
        let pushed = self.link.shared().push(e);
        if !pushed && !self.full_said {
            self.full_said = true;
            crate::script::log::once("DOF: vpi-dof isn't keeping up; events dropped");
        }
        pushed
    }

    /// Ends `vpi-dof`, waiting a little for libdof to turn the toys off.
    pub fn stop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            self.link.shared().request_stop();
            self.link.wake_host();
            let stopping = Instant::now();
            while matches!(self.child.try_wait(), Ok(None)) {
                if stopping.elapsed() > STOP_TIMEOUT {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

impl Drop for Dof {
    fn drop(&mut self) {
        self.stop();
    }
}

/// What the toys were told during a check (the headless runner's `--toys`,
/// the dashboard's toys check): how many times each kind of event
/// switched something on, and the script's own toy numbers (`E`) — whether
/// a table's toys get their signals at all, without a cabinet or a
/// configuration.
#[derive(Default)]
pub struct ToysRecord {
    last: HashMap<(char, i32), i32>,
    on: BTreeMap<char, u32>,
    script_toys: BTreeSet<i32>,
}

impl ToysRecord {
    /// The machine's outputs now, heard as the toys hear them.
    pub fn machine_states(&mut self, states: &[(char, i32, i32)]) {
        for &(kind, number, value) in states {
            let last = self.last.entry((kind, number)).or_insert(-1);
            if *last != value {
                *last = value;
                self.note(kind, number, value);
            }
        }
    }

    /// An event as it comes.
    pub fn queue(&mut self, kind: char, number: i32, value: i32) {
        self.note(kind, number, value);
    }

    fn note(&mut self, kind: char, number: i32, value: i32) {
        if value == 0 {
            return;
        }
        *self.on.entry(kind).or_default() += 1;
        if kind == 'E' {
            self.script_toys.insert(number);
        }
    }

    /// One line: each kind's count of events switching something on, the
    /// script's toy numbers after `E`'s.
    pub fn summary(&self) -> String {
        if self.on.is_empty() {
            return "none".into();
        }
        let toys = |kind: char| {
            if kind != 'E' || self.script_toys.is_empty() {
                return String::new();
            }
            let numbers: Vec<String> = self.script_toys.iter().take(12).map(i32::to_string).collect();
            format!(" ({}{})", numbers.join(" "), if self.script_toys.len() > 12 { " ..." } else { "" })
        };
        self.on.iter().map(|(&k, n)| format!("{k} {n}{}", toys(k))).collect::<Vec<_>>().join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_is_a_global_configuration_or_a_boards_file() {
        let dir = std::env::temp_dir().join(format!("vpi-dof-set-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        assert!(!holds_set(&dir));
        std::fs::write(dir.join("DirectOutputConfig30.ini"), "").unwrap();
        assert!(holds_set(&dir));
        let _ = std::fs::remove_file(dir.join("DirectOutputConfig30.ini"));
        std::fs::write(dir.join("GlobalConfig_B2SServer.xml"), "").unwrap();
        assert!(holds_set(&dir));
        let _ = std::fs::remove_file(dir.join("GlobalConfig_B2SServer.xml"));
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn a_check_counts_what_switched_on() {
        let mut r = ToysRecord::default();
        assert_eq!(r.summary(), "none");
        // a first poll: each output's first value, those at 0 switching nothing on
        r.machine_states(&[('S', 1, 0), ('S', 2, 1), ('L', 5, 1)]);
        r.machine_states(&[('S', 1, 1), ('S', 2, 1), ('L', 5, 0)]);
        r.queue('E', 101, 1);
        r.queue('E', 101, 0);
        r.queue('E', 117, 1);
        assert_eq!(r.summary(), "E 2 (101 117), L 1, S 2");
    }

    #[test]
    fn the_settings_turn_the_toys_off() {
        assert!(enabled(&[]));
        assert!(!enabled(&[("Enable".into(), "0".into())]));
        assert!(enabled(&[("enable".into(), "1".into())]));
    }
}
