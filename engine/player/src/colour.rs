// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): where its Serum and VNI
// plugins look for a game's colour files, and how they hand on what those files give (plugins/serum/serum.cpp,
// plugins/vni/vni.cpp). Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A ROM game's score display in colour: the game's Serum file, or
//! PIN2DMD's older palette and animation files, coloured by `vpi-colour`
//! beside the player — libserum and libvni are under version 2 of the GPL
//! alone, so they run in a program of their own. The game's raw shades go
//! over (PinMAME's identify frames); the coloured frame comes back
//! and takes the machine's own frame's place wherever the display shows, as
//! VPX's plugins override PinMAME's display; the triggers the file gives
//! frames go to the table's PinUP pack, as VPX's event stream fires them.

use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::dmd::DmdFrame;
use crate::pinmame::Machine;

/// A game's colour file, as found.
#[derive(Clone, Debug, PartialEq)]
pub enum Files {
    /// The folder holding `<rom>\<rom>.cROMc` or `.cRZ`.
    Serum { folder: PathBuf },
    Vni { pal: PathBuf, vni: Option<PathBuf> },
    /// PIN2DMD's locked file, with the user's key to open it.
    Pac { pac: PathBuf, key: PacKey },
}

/// A PIN2DMD key, as the user set it (`[Plugin.VNI]` `PacKey`, DMD Extensions' `vni.key`): kept out of
/// anything printed, and handed to `vpi-colour` privately.
#[derive(Clone, PartialEq)]
pub struct PacKey(String);

impl std::fmt::Debug for PacKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PacKey(…)")
    }
}

/// Where `rom`'s colour file is, as VPX 5436's plugins look: Serum's first —
/// `serum\<rom>\` beside the table, then `pinmame\altcolor\<rom>\` there,
/// then the folder `[Plugin.Serum]` `SerumPath` names — then PIN2DMD's
/// (`<rom>.pal` or `pin2dmd.pal`, with its `.vni`): `vni\<rom>\` beside the
/// table, `pinmame\altcolor\<rom>\`, then `[Plugin.VNI]` `VniPath`. Then
/// ours: VPinMAME's `altcolor\<rom>\`, where VPinMAME and VPin Studio keep
/// them. Each kind is on unless its `Enable` is 0 (VPX's switch for the
/// plugin, on by default here). Without an open pair, PIN2DMD's locked
/// `<rom>.pac` or `pin2dmd.pac` in the same places, when the user has set the
/// key that opens it (`[Plugin.VNI]` `PacKey`; VPX 5436 reads none).
pub fn find(table_dir: Option<&Path>, rom: &str, serum: &[(String, String)], vni: &[(String, String)]) -> Option<Files> {
    if rom.is_empty() {
        return None;
    }
    let file = |dir: &Path, name: &str| crate::pup::find_case_insensitive(&dir.join(name), false);
    let serum_in = |base: &Path| {
        let dir = base.join(rom);
        file(&dir, &format!("{rom}.cROMc")).or_else(|| file(&dir, &format!("{rom}.cRZ")))
    };
    // `<rom>.pal`, then PIN2DMD's own name where VPX takes it (not in `VniPath`), each with its `.vni`
    let vni_in = |base: &Path, pin2dmd: bool| {
        let dir = base.join(rom);
        let pair = |stem: &str| file(&dir, &format!("{stem}.pal")).map(|pal| (pal, file(&dir, &format!("{stem}.vni"))));
        pair(rom).or_else(|| if pin2dmd { pair("pin2dmd") } else { None })
    };
    let vpinmame = crate::pinmame::vpinmame().join("altcolor");
    if enabled(serum) {
        let mut bases: Vec<PathBuf> = table_dir.map(|d| vec![d.join("serum"), d.join("pinmame").join("altcolor")]).unwrap_or_default();
        bases.extend(setting(serum, "SerumPath").filter(|p| !p.is_empty()).map(PathBuf::from));
        bases.push(vpinmame.clone());
        if let Some(base) = bases.into_iter().find(|b| serum_in(b).is_some()) {
            return Some(Files::Serum { folder: base });
        }
    }
    if enabled(vni) {
        let mut bases: Vec<(PathBuf, bool)> = table_dir.map(|d| vec![(d.join("vni"), true), (d.join("pinmame").join("altcolor"), true)]).unwrap_or_default();
        bases.extend(setting(vni, "VniPath").filter(|p| !p.is_empty()).map(|p| (PathBuf::from(p), false)));
        bases.push((vpinmame, true));
        if let Some((pal, vni)) = bases.iter().find_map(|(b, pin2dmd)| vni_in(b, *pin2dmd)) {
            return Some(Files::Vni { pal, vni });
        }
        if let Some(key) = setting(vni, "PacKey").filter(|k| !k.is_empty()) {
            let pac_in = |base: &Path, pin2dmd: bool| {
                let dir = base.join(rom);
                file(&dir, &format!("{rom}.pac")).or_else(|| if pin2dmd { file(&dir, "pin2dmd.pac") } else { None })
            };
            if let Some(pac) = bases.iter().find_map(|(b, pin2dmd)| pac_in(b, *pin2dmd)) {
                return Some(Files::Pac { pac, key: PacKey(key) });
            }
        }
    }
    None
}

/// Each frame: the machine's newest identify frame to `vpi-colour` (a
/// segment game's digits as VPX 5436's AlphaDMD draws them, unless
/// `[Plugin.AlphaDMD]` `Enable` is 0), and the triggers its file gave frames
/// to the table's PinUP pack, as `D` triggers switched on and off, as VPX's
/// event stream fires Serum's.
pub fn poll(w: &mut crate::world::World) {
    let segments = w.machine_segments.as_ref().filter(|_| enabled(&w.alphadmd_settings)).map(|(_, s)| s.clone());
    let Some(c) = w.controller.as_mut() else { return };
    let (Some(colour), Some(machine)) = (c.state.colour.as_mut(), c.state.machine.as_ref()) else { return };
    colour.send(machine);
    // a segment game: its digits as VPX 5436's AlphaDMD draws them for colouring
    if machine.dmd_identify_count().is_none()
        && let Some(s) = segments
    {
        colour.send_segments(&s);
    }
    let mut triggers = Vec::new();
    colour.take_triggers(&mut triggers);
    // to the PinUP pack and the toys, as Serum's plugin broadcasts them
    for id in triggers {
        crate::events::queue(w, 'D', id as i32, 1);
        crate::events::queue(w, 'D', id as i32, 0);
    }
}

/// A plugin's setting, the last given winning.
fn setting(settings: &[(String, String)], key: &str) -> Option<String> {
    settings.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| crate::settings::vpx_trim(v).to_string())
}

pub(crate) fn enabled(settings: &[(String, String)]) -> bool {
    setting(settings, "Enable").as_deref().and_then(crate::settings::vpx_bool) != Some(false)
}

/// How long the player waits for `vpi-colour` to end before ending it.
const STOP_TIMEOUT: Duration = Duration::from_millis(1000);

/// Numbers the links this player makes, so each is new.
static NEXT_LINK: AtomicU32 = AtomicU32::new(0);

/// `vpi-colour` at work for one game.
pub struct Colour {
    link: colour_link::Link,
    child: Child,
    /// The identify frame last handed over (its number on PinMAME's link).
    sent: Option<u32>,
    shades: Vec<u8>,
    dots: Vec<u32>,
    /// The frame shown, and the plane and frame it was made from.
    shown: Option<((usize, u32), Arc<DmdFrame>)>,
    /// Each plane's frame number when last looked at, and the plane showing: the one that last
    /// changed.
    seen: [u32; 2],
    showing: Option<usize>,
    /// How far the triggers have been read.
    triggers: u32,
    /// Said once if `vpi-colour` couldn't load the file.
    failed_said: bool,
}

impl Colour {
    /// Starts `vpi-colour` on a new link for `rom`'s file; it loads the file
    /// on its own, frames handed over meanwhile waiting for it. libserum's
    /// `.cROMc`, which it would write beside a `.cRZ` as a faster copy (as in
    /// VPX), isn't written: the player writes into no table's or VPinMAME's
    /// folder.
    pub fn start(rom: &str, files: &Files) -> Result<Colour, String> {
        let host = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("vpi-colour.exe");
        if !host.exists() {
            return Err(format!("{} is missing", host.display()));
        }
        let name = format!(r"Local\VPI-Colour-{}-{}", std::process::id(), NEXT_LINK.fetch_add(1, Ordering::Relaxed));
        let link = colour_link::Link::create(&name).map_err(|e| format!("the link to vpi-colour: {e}"))?;
        let mut process = std::process::Command::new(&host);
        process.args(["--link", &name, "--parent", &std::process::id().to_string()]);
        match files {
            Files::Serum { folder } => {
                process.arg("--serum").arg(folder).args(["--rom", rom, "--no-cache"]);
            }
            Files::Vni { pal, vni } => {
                process.arg("--vni-pal").arg(pal);
                if let Some(v) = vni {
                    process.arg("--vni").arg(v);
                }
            }
            // the key in the helper's own environment, not on its command line, which other
            // programs can read
            Files::Pac { pac, key } => {
                process.arg("--vni-pac").arg(pac).env("VPI_PAC_KEY", &key.0);
            }
        }
        let child = process.spawn().map_err(|e| format!("start {}: {e}", host.display()))?;
        Ok(Colour { link, child, sent: None, shades: Vec::new(), dots: Vec::new(), shown: None, seen: [0, 0], showing: None, triggers: 0, failed_said: false })
    }

    /// Hands over a segment game's display, drawn as VPX 5436's AlphaDMD draws it for
    /// colouring (`segments::alpha_identify`), when its digits change and its displays match
    /// one of AlphaDMD's layouts.
    pub fn send_segments(&mut self, segments: &crate::segments::SegmentsNow) {
        let number = segments.id as u32;
        if self.sent == Some(number) {
            return;
        }
        self.sent = Some(number);
        let Some(frame) = crate::segments::alpha_identify(segments) else { return };
        if self.link.shared().send_identify(number, 128, 32, 2, &frame.dots) {
            self.link.wake_host();
        }
    }

    /// Hands over the machine's newest identify frame, when it has a new one.
    pub fn send(&mut self, machine: &Machine) {
        let Some(count) = machine.dmd_identify_count() else { return };
        if self.sent == Some(count) || count == 0 {
            return;
        }
        let Some((width, height, bits, number)) = machine.dmd_identify(&mut self.shades) else { return };
        self.sent = Some(count);
        if self.link.shared().send_identify(number, width as u32, height as u32, bits, &self.shades) {
            self.link.wake_host();
        }
    }

    /// The newest coloured frame, once there's one: of the file's two sizes (Serum's 32 and 64
    /// dots high), the one that changed last — the larger where both did, as VPX's default
    /// display picks the largest — since a file may colour some scenes at one size only (Attack
    /// from Mars's: its start-up tests at 64, its game at 32), and the other size's last frame
    /// is then stale. `next_id` numbers a new frame for the renderer.
    pub fn frame(&mut self, next_id: impl FnOnce() -> u64) -> Option<Arc<DmdFrame>> {
        let shared = self.link.shared();
        let (state, _) = shared.state();
        if state == colour_link::STATE_FAILED && !self.failed_said {
            self.failed_said = true;
            crate::script::log::once("vpi-colour couldn't load the game's colour file; its own display shows");
        }
        let seqs = [shared.plane_seq(0), shared.plane_seq(1)];
        let changed = |k: usize| seqs[k] > 0 && seqs[k] != self.seen[k];
        let plane = if changed(1) {
            1
        } else if changed(0) {
            0
        } else {
            match self.showing {
                Some(k) => k,
                None if seqs[1] > 0 => 1,
                None if seqs[0] > 0 => 0,
                None => return None,
            }
        };
        self.seen = seqs;
        self.showing = Some(plane);
        let key = (plane, seqs[plane]);
        if let Some((k, f)) = &self.shown
            && *k == key
        {
            return Some(f.clone());
        }
        let plane = shared.read_plane(key.0, &mut self.dots)?;
        let frame = Arc::new(DmdFrame::from_coloured(plane.width, plane.height, plane.kind, &self.dots, next_id()));
        self.shown = Some(((key.0, plane.seq), frame.clone()));
        Some(frame)
    }

    /// The triggers the file gave frames since the last look.
    pub fn take_triggers(&mut self, out: &mut Vec<u32>) {
        self.link.shared().triggers_since(&mut self.triggers, out);
    }

    /// Ends `vpi-colour`, waiting a little for it to go.
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

impl Drop for Colour {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_files_are_found_as_vpx_looks_and_then_in_vpinmames_folder() {
        let dir = std::env::temp_dir().join(format!("vpi-colour-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let table = dir.join("table");
        // made-up games, so the user's own colour files in VPinMAME's folder never answer for them
        // PIN2DMD's files in the table's pinmame folder, by PIN2DMD's names
        std::fs::create_dir_all(table.join(r"pinmame\altcolor\vpi_test_dmd")).unwrap();
        std::fs::write(table.join(r"pinmame\altcolor\vpi_test_dmd\pin2dmd.pal"), b"").unwrap();
        std::fs::write(table.join(r"pinmame\altcolor\vpi_test_dmd\pin2dmd.vni"), b"").unwrap();
        let found = find(Some(&table), "vpi_test_dmd", &[], &[]);
        assert_eq!(found, Some(Files::Vni { pal: table.join(r"pinmame\altcolor\vpi_test_dmd\pin2dmd.pal"), vni: Some(table.join(r"pinmame\altcolor\vpi_test_dmd\pin2dmd.vni")) }));
        // a Serum file beside them comes first, its letters' case as it likes
        std::fs::write(table.join(r"pinmame\altcolor\vpi_test_dmd\VPI_TEST_DMD.CRZ"), b"").unwrap();
        assert_eq!(find(Some(&table), "vpi_test_dmd", &[], &[]), Some(Files::Serum { folder: table.join(r"pinmame\altcolor") }));
        // each kind switched off on its own
        let off = [("Enable".to_string(), "0".to_string())];
        assert!(matches!(find(Some(&table), "vpi_test_dmd", &off, &[]), Some(Files::Vni { .. })));
        assert_eq!(find(Some(&table), "vpi_test_dmd", &off, &off), None);
        // a game with no file of its own
        assert_eq!(find(Some(&table), "zzz_never", &[], &[]), None);
        // PIN2DMD's locked file, used only once the user has set the key, and its key never printed
        std::fs::create_dir_all(table.join(r"pinmame\altcolor\vpi_test_pac")).unwrap();
        std::fs::write(table.join(r"pinmame\altcolor\vpi_test_pac\pin2dmd.pac"), b"").unwrap();
        assert_eq!(find(Some(&table), "vpi_test_pac", &[], &[]), None);
        let key = [("PacKey".to_string(), "00112233445566778899aabbccddeeff".to_string())];
        let found = find(Some(&table), "vpi_test_pac", &[], &key);
        assert!(matches!(&found, Some(Files::Pac { pac, .. }) if *pac == table.join(r"pinmame\altcolor\vpi_test_pac\pin2dmd.pac")));
        assert!(!format!("{found:?}").contains("0011"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
