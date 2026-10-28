// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its DMDUtil plugin's
// choice of display and its life with the game (plugins/dmdutil/DMDUtilPlugin.cpp, onDmdSrcChanged, onGameEnd), and
// the order VPX offers its displays in (src/core/VPXPluginAPIImpl.cpp, ControllerOnGetDMDSrc, then the plugins).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A cabinet's real DMD panel: `vpi-dmd` runs libdmdutil beside the
//! player — a program of its own, as the toys and the colour files run —
//! started with the first display a panel can show, as VPX 5436's DMDUtil
//! plugin makes its DMD then, and handed that display's frames. Of the
//! displays offered, in the order VPX offers them — the script's own
//! (`DMDPixels`, `DMDColoredPixels`), AlphaDMD's picture of a segment game's
//! digits (`alphadmd`), each FlexDMD's, the machine's, its colour file's —
//! the panel takes the widest in colour, else the widest in
//! brightness, none larger than libdmdutil takes (256 × 64 dots), each frame
//! in the form its source made it: a brightness, or colours in 24 or 16
//! bits. On when the settings say `[Plugin.DMDUtil]` `Enable = 1`, as in
//! VPX, whose plugins are off until enabled; the panels themselves (ZeDMD,
//! Pixelcade, PIN2DMD, or a DMD server) are switched on in the same section,
//! under VPX's names, and handed to `vpi-dmd` as they are.

use std::path::Path;
use std::process::Child;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::dmd::DmdFrame;

/// How long the player waits for `vpi-dmd` to end (libdmdutil blanking the panel) before ending it.
const STOP_TIMEOUT: Duration = Duration::from_millis(2000);
/// How long it waits for the last picture asked of the virtual panel.
const PICTURE_TIMEOUT: Duration = Duration::from_millis(1000);

/// Numbers the links this player makes, so each is new.
static NEXT_LINK: AtomicU32 = AtomicU32::new(0);

/// A display offered to the panel, in VPX's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Script,
    AlphaDmd,
    Flex(usize),
    Machine,
    Coloured,
}

/// `vpi-dmd` at work for one game.
pub struct DmdPanel {
    link: dmd_link::Link,
    child: Child,
    /// The display and frame last handed over.
    last: Option<(Source, u64)>,
    /// Frames handed over, numbering them on the link.
    sent: u32,
    dots: Vec<u32>,
    /// The newest picture asked of the virtual panel (a check).
    picture_asked: u32,
}

/// Whether the settings' `[Plugin.DMDUtil]` switch the panel on (`Enable`; off unless set, as
/// VPX's plugins are).
pub fn enabled(settings: &[(String, String)]) -> bool {
    let enable = settings.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case("Enable")).map(|(_, v)| crate::settings::vpx_trim(v).to_string());
    enable.as_deref().and_then(crate::settings::vpx_bool) == Some(true)
}

/// The display a panel takes now, as VPX 5436's plugin picks it: the widest in colour, else the
/// widest in brightness, of those libdmdutil takes, the first offered among equals.
fn chosen(w: &mut crate::world::World) -> Option<(Source, Arc<DmdFrame>)> {
    let mut offered: Vec<(Source, Arc<DmdFrame>)> = Vec::new();
    if let Some(f) = &w.dmd_frame {
        offered.push((Source::Script, f.clone()));
    }
    if let Some(f) = alpha_frame(w) {
        offered.push((Source::AlphaDmd, f));
    }
    for (i, flex) in w.flexdmds.iter().enumerate() {
        if let Some(f) = flex.borrow().display_frame() {
            offered.push((Source::Flex(i), f));
        }
    }
    if let Some(f) = crate::dmd::machine_own_frame(w) {
        offered.push((Source::Machine, f));
    }
    if let Some(f) = crate::dmd::coloured_frame(w) {
        offered.push((Source::Coloured, f));
    }
    // libdmdutil's update buffers hold 256 × 64 dots (VPX skips larger displays)
    offered.retain(|(_, f)| (f.width * f.height) as usize <= dmd_link::MAX_DOTS);
    let widest = |colour: bool| {
        let mut best: Option<&(Source, Arc<DmdFrame>)> = None;
        for o in offered.iter().filter(|(_, f)| f.luminance != colour) {
            if o.1.width > best.map_or(0, |b| b.1.width) {
                best = Some(o);
            }
        }
        best.cloned()
    };
    widest(true).or_else(|| widest(false))
}

/// AlphaDMD's picture of the game's segment displays, drawn again only when they change, while
/// `[Plugin.AlphaDMD]` leaves it on (as VPX 5436's plugins are, theirs offering it once its
/// layout matched).
fn alpha_frame(w: &mut crate::world::World) -> Option<Arc<DmdFrame>> {
    if !crate::colour::enabled(&w.alphadmd_settings) {
        return None;
    }
    let now = w.machine_segments.as_ref()?.1.clone();
    if let Some((id, f)) = &w.alpha_render
        && *id == now.id
    {
        return Some(f.clone());
    }
    let dots = crate::alphadmd::render(&now)?;
    w.dmd_frames += 1;
    let frame = Arc::new(DmdFrame::with_dots(128, 32, dots.iter().map(|&l| [l, l, l, 1.0]), true, w.dmd_frames));
    w.alpha_render = Some((now.id, frame.clone()));
    Some(frame)
}

/// The panel's frame for now, handed to `vpi-dmd` when the settings switch the panel on: the
/// program started with the first display, as VPX's plugin makes its DMD then.
pub fn update(w: &mut crate::world::World) {
    if !w.dmd_panel_on {
        return;
    }
    let Some((source, frame)) = chosen(w) else { return };
    if w.dmd_panel.is_none() {
        let name = if w.table_file.is_empty() { "panel".to_string() } else { w.table_file.clone() };
        match DmdPanel::start(&w.dmdutil_settings, w.dmd_panel_check.as_deref(), &name) {
            Ok(p) => {
                crate::script::log::once("DMDUtil: the panel's frames go to vpi-dmd (libdmdutil)");
                w.dmd_panel = Some(p);
            }
            Err(e) => {
                crate::script::log::once(&format!("DMDUtil: {e}"));
                w.dmd_panel_on = false;
                return;
            }
        }
    }
    if let Some(p) = w.dmd_panel.as_mut() {
        p.send(source, &frame);
    }
}

/// A picture of what the virtual panel shows, for a check (the headless runner's `--dmd-panel`),
/// saved by `vpi-dmd` as `<table> at <ms> ms, panel.png`.
pub fn ask_picture(w: &mut crate::world::World, ms: u32) {
    if let Some(p) = w.dmd_panel.as_mut() {
        p.picture_asked = ms.max(1);
        p.link.shared().ask_picture(p.picture_asked);
        p.link.wake_host();
    }
}

impl DmdPanel {
    /// Starts `vpi-dmd` on a new link with the panel settings (`[Plugin.DMDUtil]`), and with
    /// libdmdutil's virtual panel when `check` names a folder for its pictures.
    fn start(settings: &[(String, String)], check: Option<&Path>, name: &str) -> Result<DmdPanel, String> {
        let host = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("vpi-dmd.exe");
        if !host.exists() {
            return Err(format!("{} is missing", host.display()));
        }
        let link_name = format!(r"Local\VPI-DMD-{}-{}", std::process::id(), NEXT_LINK.fetch_add(1, Ordering::Relaxed));
        let link = dmd_link::Link::create(&link_name).map_err(|e| format!("the link to vpi-dmd: {e}"))?;
        let mut command = std::process::Command::new(&host);
        command.args(["--link", &link_name, "--parent", &std::process::id().to_string()]);
        for (key, value) in settings.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("Enable")) {
            command.arg("--set").arg(format!("{key}={}", crate::settings::vpx_trim(value)));
        }
        if let Some(folder) = check {
            command.arg("--check").arg(folder).args(["--name", name]);
        }
        let child = command.spawn().map_err(|e| format!("start {}: {e}", host.display()))?;
        Ok(DmdPanel { link, child, last: None, sent: 0, dots: Vec::new(), picture_asked: 0 })
    }

    /// The display's frame, handed over when it's new: a brightness as it is, colours as the
    /// bytes or 16-bit values they were made from.
    fn send(&mut self, source: Source, frame: &DmdFrame) {
        if self.last == Some((source, frame.id)) {
            return;
        }
        self.last = Some((source, frame.id));
        let format = if frame.luminance {
            dmd_link::FORMAT_LUM
        } else if frame.rgb565 {
            dmd_link::FORMAT_RGB565
        } else {
            dmd_link::FORMAT_RGB888
        };
        self.dots.clear();
        self.dots.extend(frame.dots.iter().map(|d| match format {
            dmd_link::FORMAT_LUM => d[0].to_bits(),
            _ => {
                let [r, g, b] = [d[0], d[1], d[2]].map(|c| u32::from(crate::dmd::linear_to_srgb_byte(c)));
                if format == dmd_link::FORMAT_RGB565 { ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3) } else { (r << 16) | (g << 8) | b }
            }
        }));
        self.sent = self.sent.wrapping_add(1).max(1);
        if self.link.shared().send(self.sent, frame.width, frame.height, format, &self.dots) {
            self.link.wake_host();
        }
    }

    /// Ends `vpi-dmd`, once it has saved the picture last asked for, waiting a little for
    /// libdmdutil to let the panel go.
    pub fn stop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(None)) {
            return;
        }
        let shared = self.link.shared();
        if self.picture_asked != 0 {
            let asked = Instant::now();
            while shared.pictures_saved() != self.picture_asked && asked.elapsed() < PICTURE_TIMEOUT && matches!(self.child.try_wait(), Ok(None)) {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        shared.request_stop();
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

impl Drop for DmdPanel {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_is_off_unless_the_settings_switch_it_on() {
        assert!(!enabled(&[]));
        assert!(!enabled(&[("Enable".into(), "0".into())]));
        assert!(enabled(&[("enable".into(), " 1 ".into())]));
        assert!(enabled(&[("Enable".into(), "true".into())]));
    }

    /// A colour file's 16-bit dots reach the panel as the values they were.
    #[test]
    fn sixteen_bit_colours_go_back_to_what_they_were() {
        for v in [0u32, 0x0821, 0xF800, 0x07E0, 0x001F, 0xFFFF, 0x1234, 0xABCD] {
            let f = DmdFrame::from_coloured(1, 1, colour_link::DOTS_RGB565, &[v], 1);
            assert!(f.rgb565);
            let [r, g, b] = [f.dots[0][0], f.dots[0][1], f.dots[0][2]].map(|c| u32::from(crate::dmd::linear_to_srgb_byte(c)));
            assert_eq!(((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3), v, "{v:04x}");
        }
    }
}
