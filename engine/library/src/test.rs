//! A table run with no window, as the compatibility dashboard runs it, for the app's "Test
//! it" on a table's page and the full test of new tables as they're added (off by default, a
//! setting): the headless runner plays its balls
//! and flippers for 3 s twice, which must end the same, then the table with its script for 8 s in
//! real time — a coin, start, the plunger, then the flippers each second — reporting its script
//! errors, the members our engine doesn't answer yet, the objects it refused, and for a ROM table
//! whether its machine started in PinMAME. Ours.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// How long the script runs, in ms of real time.
pub const SCRIPT_MS: &str = "8000";

/// What the script run found.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Script {
    /// None when it ran; else why not.
    pub failed: Option<String>,
    pub errors: u32,
    /// The members not answered yet, as `Interface.Member`.
    pub missing: Vec<String>,
    /// The objects it asked for that the player doesn't provide yet (`CreateObject`), which its
    /// errors usually follow from.
    pub refused: Vec<String>,
    /// For a ROM table: the machine that started.
    pub machine: Option<String>,
    /// For a ROM table: why its machine didn't start.
    pub machine_problem: Option<String>,
    /// What the cabinet's toys were told during the run (the runner's `--toys`), when asked.
    pub toys: Option<String>,
}

/// A test's findings, and what they come to.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Report {
    /// None when the physics repeated; else why not.
    pub physics: Option<String>,
    pub script: Script,
    /// `good`, `warn` or `bad`, and in words.
    pub look: String,
    pub text: String,
    pub seconds: f64,
}

/// Where the headless runner is: beside this program, else the repository's build (developing).
pub fn runner() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let beside = dir.join("vpi-headless.exe");
    if beside.exists() {
        return Some(beside);
    }
    dir.ancestors().take(4).map(|up| up.join(r"target\release\vpi-headless.exe")).find(|p| p.exists())
}

fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// The table at `table` tested with the runner at `runner`.
pub fn run(runner: &Path, table: &Path) -> Report {
    let started = Instant::now();
    let physics = physics(runner, table).err();
    let script = script(runner, table, false);
    let (look, text) = verdict(&physics, &script);
    Report { physics, script, look: look.to_string(), text, seconds: started.elapsed().as_secs_f64() }
}

/// Balls and flippers for 3 s, twice: the same end both times.
pub fn physics(runner: &Path, table: &Path) -> Result<(), String> {
    let out = quiet(Command::new(runner).arg(table).args(["--ms", "3000", "--repeat", "2"])).output().map_err(|e| format!("the runner didn't start: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let last = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or_default();
    if last.contains("could not read") {
        Err(last.split("could not read").nth(1).map_or_else(String::new, |r| format!("could not read{r}")))
    } else if last.contains("NOT REPEATABLE") {
        Err("two runs ended differently".into())
    } else if !out.status.success() {
        Err(format!("the runner stopped ({})", out.status))
    } else {
        Ok(())
    }
}

/// The table with its script, 8 s in real time; `toys`: what a cabinet's toys were told, too.
pub fn script(runner: &Path, table: &Path, toys: bool) -> Script {
    let mut cmd = Command::new(runner);
    cmd.arg(table).args(["--script", "--realtime"]);
    if toys {
        cmd.arg("--toys");
    }
    let out = match quiet(cmd.args(["--ms", SCRIPT_MS])).output() {
        Ok(o) => o,
        Err(e) => return Script { failed: Some(format!("the runner didn't start: {e}")), ..Script::default() },
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut s = Script::default();
    // the result line: its last columns are balls, errors, missing, fingerprint
    let last = stdout.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or_default();
    let columns: Vec<&str> = last.split_whitespace().rev().collect();
    match columns.get(2).and_then(|e| e.parse().ok()) {
        Some(errors) if !last.contains("could not read") => s.errors = errors,
        _ => s.failed = Some(if last.contains("could not read") { "could not read the table".into() } else { format!("the runner stopped ({})", out.status) }),
    }
    // PinMAME names each ROM file it can't find
    let rom_missing = stderr.lines().any(|l| l.trim_end().ends_with(" NOT FOUND"));
    for line in stderr.lines() {
        // the controller couldn't start the machine (its ROM missing, most often)
        if let Some(rest) = line.split("Controller.Run: PinMAME couldn't run ").nth(1)
            && s.machine.is_none()
        {
            let game = rest.split_whitespace().next().unwrap_or(rest);
            s.machine_problem = Some(if rom_missing { format!("its ROM ({game}) isn't installed, or is missing files") } else { format!("PinMAME couldn't run `{game}`") });
        }
        if let Some(rest) = line.split("not supported yet: ").nth(1) {
            let member = rest.split(" (").next().unwrap_or(rest).trim().to_string();
            if !s.missing.contains(&member) {
                s.missing.push(member);
            }
        }
        if let Some(rest) = line.split("CreateObject(\"").nth(1)
            && line.contains("refused")
        {
            let object = rest.split('"').next().unwrap_or(rest).to_string();
            if !s.refused.contains(&object) {
                s.refused.push(object);
            }
        }
        if let Some(rest) = line.strip_prefix("toys: ") {
            s.toys = Some(rest.trim().to_string());
        }
        if line.contains("the table's script can't run") {
            s.failed = Some("its script didn't start".into());
        }
        if let Some(rest) = line.strip_prefix("vpi-pinmame: ")
            && !rest.starts_with("PinMAME ")
        {
            if let Some(game) = rest.strip_suffix(')').and_then(|r| r.split(" running (").next()).filter(|_| rest.contains(" running (")) {
                s.machine = Some(game.to_string());
                s.machine_problem = None;
            } else if s.machine.is_none() && s.machine_problem.is_none() && !rest.contains("stopped after") && !rest.contains("the player is gone") {
                s.machine_problem = Some(rest.to_string());
            }
        }
    }
    s
}

/// What a test comes to, in words, and how it looks.
pub fn verdict(physics: &Option<String>, s: &Script) -> (&'static str, String) {
    let few = |v: &[String]| {
        let shown: Vec<&str> = v.iter().take(3).map(String::as_str).collect();
        if v.len() > 3 { format!("{}, and {} more", shown.join(", "), v.len() - 3) } else { shown.join(", ") }
    };
    if let Some(p) = physics {
        return ("bad", format!("Its physics didn't run: {p}"));
    }
    if let Some(f) = &s.failed {
        return ("bad", format!("Its script didn't run: {f}"));
    }
    if let Some(m) = &s.machine_problem {
        return ("bad", format!("Its machine didn't start: {m}"));
    }
    let mut asks = Vec::new();
    if !s.refused.is_empty() {
        asks.push(format!("objects our engine doesn't provide yet ({})", few(&s.refused)));
    }
    if !s.missing.is_empty() {
        asks.push(format!("script members it doesn't answer yet ({})", few(&s.missing)));
    }
    if s.errors > 0 {
        let errors = if s.errors == 1 { "1 script error".to_string() } else { format!("{} script errors", s.errors) };
        let why = if asks.is_empty() { String::new() } else { format!(": it asks for {}", asks.join(" and ")) };
        return ("bad", format!("{errors} in {} seconds{why}", SCRIPT_MS.trim_end_matches("000")));
    }
    if !asks.is_empty() {
        return ("warn", format!("It plays, but asks for {}", asks.join(" and ")));
    }
    let machine = s.machine.as_ref().map_or(String::new(), |m| format!(", its machine running ({m})"));
    ("good", format!("It plays — its script ran {} seconds without an error{machine}", SCRIPT_MS.trim_end_matches("000")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_test_comes_to() {
        let mut s = Script { machine: Some("afm_113b".into()), ..Default::default() };
        assert_eq!(verdict(&None, &s), ("good", "It plays — its script ran 8 seconds without an error, its machine running (afm_113b)".into()));
        s.missing = vec!["Light.FadeSpeedUp".into()];
        assert_eq!(verdict(&None, &s).0, "warn");
        s.errors = 2;
        assert_eq!(verdict(&None, &s).0, "bad");
        assert_eq!(verdict(&Some("two runs ended differently".into()), &Script::default()).0, "bad");
    }
}
