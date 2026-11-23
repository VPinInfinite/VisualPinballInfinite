// Follows PinMAME's VPinMAME (license:BSD-3-Clause, the PinMAME project), from the commit VPX 5436 builds: what it
// hands a display device and when (src/win32com/ControllerDmdDevice.cpp, and core_dmd_send_dmddevice and
// core_seg_send_dmddevice in src/wpc/core.c); and FlexDMD 1.9.1 (Apache-2.0, Copyright (c) 2019 Vincent
// Bousquet), for a table without a machine: what it hands one (FlexDMD/FlexDMD.cs). Ours otherwise.

//! A cabinet's real DMD panel through a VPinMAME display device:
//! `vpi-dmddevice` loads DMD Extensions' `DmdDevice64.dll`, or a panel
//! maker's own, beside the player and drives it as VPinMAME drives one in
//! VPX 10.8.0's cabinets, the device itself doing the rest as its own
//! settings say (`DmdDevice.ini`: which panels, colour files, a picture on
//! screen). It's handed what VPinMAME hands it: the machine's DMD frames with
//! PinMAME's raw shades beside their brightness (our PinMAME build's identify
//! frames), a segment game's segment words with PinMAME's layout of
//! them, and, for tables without a machine, a FlexDMD's or UltraDMD's picture
//! or segments as FlexDMD hands them itself, under its name for the game and
//! with its colour. On when the settings name a device
//! (`[VPI]` `DmdDevice`: its path, or 1 for VPinMAME's folder's
//! `DmdDevice64.dll`); `[VPI]` `DmdDeviceConfig` names its settings file, as
//! DMD Extensions' `DMDDEVICE_CONFIG` does. libdmdutil's way (`dmd_panel`,
//! VPX 5436's) is the other; both may run.

use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// How long the player waits for `vpi-dmddevice` to end (the device blanking the panel).
const STOP_TIMEOUT: Duration = Duration::from_millis(3000);

/// Numbers the links this player makes, so each is new.
static NEXT_LINK: AtomicU32 = AtomicU32::new(0);

/// The device the settings name, and its settings file when they name one.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub library: PathBuf,
    pub config: Option<PathBuf>,
}

/// The display device the settings switch on (`[VPI]` `DmdDevice`: a path to it, or a yes for
/// VPinMAME's), when it's there.
pub fn setup(settings: &crate::settings::Settings) -> Option<Setup> {
    let value = settings.get("VPI", "DmdDevice")?.trim();
    let library = match crate::settings::vpx_bool(value) {
        Some(true) => crate::pinmame::vpinmame().join("DmdDevice64.dll"),
        Some(false) => return None,
        None => PathBuf::from(value),
    };
    if !library.is_file() {
        crate::script::log::once(&format!("DmdDevice: {} isn't there; no panel is driven through it", library.display()));
        return None;
    }
    let config = settings.get("VPI", "DmdDeviceConfig").map(str::trim).filter(|c| !c.is_empty()).map(PathBuf::from);
    Some(Setup { library, config })
}

/// What's handed to the device, so a frame goes over once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    /// The machine's DMD: its brightness frame's number and its identify frame's.
    Dmd(u64, u32),
    /// The machine's segments, by their frame count.
    Segments(u64),
    /// A FlexDMD's picture, by its number.
    Flex(usize, u64),
    /// A FlexDMD's segments.
    FlexSegments(usize),
}

/// `vpi-dmddevice` at work for one game.
pub struct DmdDevice {
    link: dmd_link::Link,
    child: Child,
    last: Option<Shown>,
    sent: u32,
    dots: Vec<u32>,
    raw: Vec<u8>,
    words: Vec<u8>,
    /// VPinMAME's `seg_data2`: kept between frames, as VPinMAME keeps it.
    seg_data2: [u16; dmd_link::SEG_WORDS],
    /// A FlexDMD's two segment buffers, kept between calls as FlexDMD keeps them.
    flex_segs: [[u16; dmd_link::SEG_WORDS]; 2],
}

/// The panel's frame for now, handed to `vpi-dmddevice` when the settings name a device: the
/// program started with the first frame there is, for the game it's for.
pub fn update(w: &mut crate::world::World) {
    let Some(setup) = w.dmd_device_setup.clone() else { return };
    if w.dmd_device.is_none() {
        // the game it's for, once there's something to show: the machine's, else a FlexDMD's
        let flex = w.flexdmds.iter().find_map(|f| {
            let f = f.borrow();
            (f.display_frame().is_some() || f.display_segments().is_some()).then(|| (f.game_name.clone(), f.color))
        });
        let has_frame = crate::dmd::machine_own_frame(w).is_some() || w.machine_segments.is_some() || flex.is_some();
        if !has_frame {
            return;
        }
        // a game whose VPinMAME setting says no real DMD (`showpindmd` = 0)
        if w.controller.as_ref().is_some_and(|c| c.state.machine.is_some() && c.state.setting("showpindmd") == Some(0)) {
            crate::script::log::once("DmdDevice: off for this game (its `showpindmd` is 0)");
            w.dmd_device_setup = None;
            return;
        }
        let machine = w.controller.as_ref().and_then(|c| c.state.machine.as_ref().map(|m| (&c.state, m)));
        let (game, hardware, options) = match (machine, flex) {
            // the ROM PinMAME runs, as VPinMAME names it to the device (`g_szGameName`), its hardware and display options
            (Some((c, m)), _) => (if c.rom_name.is_empty() { c.game_name.clone() } else { c.rom_name.clone() }, m.hardware_gen(), c.display_options()),
            // FlexDMD's name for the game and its options from its colour
            (None, Some((name, colour))) => (if name.is_empty() { flexdmd_game_name(&w.table_file) } else { name }, 0, flexdmd_options(colour)),
            (None, None) => return,
        };
        match DmdDevice::start(&setup, &game, hardware, &options) {
            Ok(d) => {
                crate::script::log::once(&format!("DmdDevice: the panel's frames go to vpi-dmddevice ({})", setup.library.display()));
                w.dmd_device = Some(d);
            }
            Err(e) => {
                crate::script::log::once(&format!("DmdDevice: {e}"));
                w.dmd_device_setup = None;
                return;
            }
        }
    }
    // the machine's DMD, with PinMAME's raw shades, as VPinMAME hands it
    if let Some(frame) = crate::dmd::machine_own_frame(w) {
        let Some(machine) = w.controller.as_ref().and_then(|c| c.state.machine.as_ref()) else { return };
        let Some(d) = w.dmd_device.as_mut() else { return };
        let identify = machine.dmd_identify(&mut d.raw).filter(|&(iw, ih, _, _)| iw as u32 == frame.width && ih as u32 == frame.height);
        let shown = Shown::Dmd(frame.id, identify.map_or(0, |i| i.3));
        if d.last == Some(shown) {
            return;
        }
        d.last = Some(shown);
        let n = (frame.width * frame.height) as usize;
        let bits = match identify {
            Some((_, _, bits, _)) => bits,
            None => {
                // a PinMAME without identify frames (VPX's own build): shades from the brightness
                d.raw.clear();
                d.raw.extend(frame.dots.iter().map(|l| (l[0].clamp(0.0, 1.0) * 3.0).round() as u8));
                2
            }
        };
        d.dots.clear();
        d.dots.extend(frame.dots.iter().map(|l| l[0].to_bits()));
        d.sent = d.sent.wrapping_add(1).max(1);
        if d.raw.len() >= n && d.link.shared().send_lum_raw(d.sent, frame.width, frame.height, &d.dots, &d.raw[..n], bits) {
            d.link.wake_host();
        }
        return;
    }
    // a segment game's segments, as PinMAME lays them out for a device
    if let Some((count, now)) = w.machine_segments.clone() {
        let Some(machine) = w.controller.as_ref().and_then(|c| c.state.machine.as_ref()) else { return };
        let Some(d) = w.dmd_device.as_mut() else { return };
        if d.last == Some(Shown::Segments(count)) {
            return;
        }
        d.last = Some(Shown::Segments(count));
        let rom = w.controller.as_ref().map_or(String::new(), |c| if c.state.rom_name.is_empty() { c.state.game_name.clone() } else { c.state.rom_name.clone() });
        let counts: Vec<usize> = now.layout.displays.iter().map(|s| s.digits.len()).collect();
        let layout = crate::segments::pinmame_layout(machine.hardware_gen(), &counts, &rom);
        // PinMAME's segment words (`drawSeg`): digit after digit in the order its layout declares
        // them, which a joined display keeps — one display's digits can come from several of
        // PinMAME's (a ball-in-play pair is two single digits)
        let mut seg_data: Vec<u16> = Vec::with_capacity(dmd_link::SEG_WORDS);
        let mut read = None;
        for (_, digit) in now.layout.digits() {
            if read != Some(digit.display) {
                machine.display_frame(digit.display, &mut d.words);
                read = Some(digit.display);
            }
            let word = d.words.get(2 * digit.index..2 * digit.index + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
            seg_data.push(word);
        }
        seg_data.resize(dmd_link::SEG_WORDS, 0);
        // VPinMAME's copy for Medusa's third row, made for every game (`core_seg_send_dmddevice`)
        d.seg_data2[..6].copy_from_slice(&seg_data[4 * 7 + 2 * 2..4 * 7 + 2 * 2 + 6]);
        d.sent = d.sent.wrapping_add(1).max(1);
        if d.link.shared().send_alphanumeric(d.sent, layout, &seg_data, &d.seg_data2) {
            d.link.wake_host();
        }
        return;
    }
    // a table without a machine: its FlexDMD's or UltraDMD's segments, as FlexDMD hands them
    // (`Segments`): its buffers filled as far as its mode reaches, always as 2 × 16 alphanumerics
    let segments = w.flexdmds.iter().enumerate().find_map(|(i, f)| f.borrow().display_segments().map(|(mode, words)| (i, mode, words.to_vec())));
    if let Some((index, mode, words)) = segments {
        let Some(d) = w.dmd_device.as_mut() else { return };
        let (size1, size2) = flexdmd_segment_sizes(mode);
        let mut buffers = d.flex_segs;
        for (k, &word) in words.iter().enumerate().take(size1 + size2) {
            if k < size1 {
                buffers[0][k] = word;
            } else {
                buffers[1][k - size1] = word;
            }
        }
        if d.last == Some(Shown::FlexSegments(index)) && buffers == d.flex_segs {
            return;
        }
        d.last = Some(Shown::FlexSegments(index));
        d.flex_segs = buffers;
        d.sent = d.sent.wrapping_add(1).max(1);
        if d.link.shared().send_alphanumeric(d.sent, FLEXDMD_SEGMENT_LAYOUT, &buffers[0], &buffers[1]) {
            d.link.wake_host();
        }
        return;
    }
    // or its picture, as FlexDMD hands one: 4 or 16 shades of grey as its mode says, else colour
    let flex = w.flexdmds.iter().enumerate().find_map(|(i, f)| {
        let f = f.borrow();
        f.display_frame().map(|frame| (i, frame, f.render_mode))
    });
    let Some((index, frame, mode)) = flex else { return };
    let Some(d) = w.dmd_device.as_mut() else { return };
    if d.last == Some(Shown::Flex(index, frame.id)) || (frame.width * frame.height) as usize > dmd_link::MAX_DOTS {
        return;
    }
    d.last = Some(Shown::Flex(index, frame.id));
    let (format, levels) = match (frame.luminance, mode) {
        (true, crate::flexdmd::mode::GRAY_2) => (dmd_link::FORMAT_SHADES4, 3.0),
        (true, _) => (dmd_link::FORMAT_SHADES16, 15.0),
        (false, _) => (dmd_link::FORMAT_RGB888, 0.0),
    };
    d.dots.clear();
    d.dots.extend(frame.dots.iter().map(|c| {
        if frame.luminance {
            // the shade FlexDMD made the brightness from
            (c[0] * levels).round() as u32
        } else {
            let [r, g, b] = [c[0], c[1], c[2]].map(|v| u32::from(crate::dmd::linear_to_srgb_byte(v)));
            (r << 16) | (g << 8) | b
        }
    }));
    d.sent = d.sent.wrapping_add(1).max(1);
    if d.link.shared().send(d.sent, frame.width, frame.height, format, &d.dots) {
        d.link.wake_host();
    }
}

/// The layout FlexDMD 1.9.1 hands a device with every segment mode's words: 2 × 16 alphanumerics,
/// whatever the mode (`RenderAlphaNumeric(NumericalLayout.__2x16Alpha, ...)`), kept as it does.
const FLEXDMD_SEGMENT_LAYOUT: u32 = 1;

/// How many of the script's words FlexDMD puts in its first and second segment buffers in a
/// segment mode (its `Segments` setter).
fn flexdmd_segment_sizes(mode: i32) -> (usize, usize) {
    match mode {
        3 => (2 * 16, 0),
        4 => (2 * 20, 0),
        5 => (2 * 7 + 2 * 7, 0),
        6 | 7 | 9 | 10 => (2 * 7 + 2 * 7 + 4, 0),
        8 => (2 * 7 + 2 * 7 + 4, 6),
        11 | 12 => (2 * 6 + 2 * 6 + 4, 0),
        13 => (4 * 7, 0),
        14 => (6 * 4 + 4, 0),
        15 => (2 * 7 + 4 + 1, 0),
        16 => (1 + 1 + 7, 0),
        _ => (0, 0),
    }
}

/// FlexDMD's name for a game whose script named none: the table's name (VPX's window title) with
/// a version at its end taken off, as FlexDMD takes it (`[\s_vV][\d_\.]+[a-z]?(-DOF)?\*?$`).
fn flexdmd_game_name(title: &str) -> String {
    let s = title.trim();
    let mut end = s.len();
    for tail in ["*", "-DOF"] {
        if s[..end].ends_with(tail) {
            end -= tail.len();
        }
    }
    let b = s.as_bytes();
    if end > 0 && b[end - 1].is_ascii_lowercase() {
        end -= 1;
    }
    let in_run = |c: u8| c.is_ascii_digit() || c == b'_' || c == b'.';
    let mut run = end;
    while run > 0 && in_run(b[run - 1]) {
        run -= 1;
    }
    // the leftmost place the expression can start: a mark just before the run, else an
    // underscore in it with something after
    let mark = |c: u8| c.is_ascii_whitespace() || c == b'_' || c == b'v' || c == b'V';
    let start = if run > 0 && run < end && mark(b[run - 1]) { Some(run - 1) } else { (run..end.saturating_sub(1)).find(|&i| b[i] == b'_') };
    match start {
        Some(i) => s[..i].trim().to_string(),
        None => s.to_string(),
    }
}

/// The display options FlexDMD hands a device: its colour, and its shades at 66, 33 and 0 percent
/// of it (`tPMoptions`, in its order).
fn flexdmd_options(colour: [u8; 4]) -> [i32; 19] {
    let [_, r, g, b] = colour.map(i32::from);
    let (p66, p33, p0) = (66, 33, 0);
    [r, g, b, p66, p33, p0, 0, 0, 0, 0, p66 * r / 100, p66 * g / 100, p66 * b / 100, p33 * r / 100, p33 * g / 100, p33 * b / 100, p0 * r / 100, p0 * g / 100, p0 * b / 100]
}

impl DmdDevice {
    fn start(setup: &Setup, game: &str, hardware: u64, options: &[i32; 19]) -> Result<DmdDevice, String> {
        let host = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("vpi-dmddevice.exe");
        if !host.exists() {
            return Err(format!("{} is missing", host.display()));
        }
        let link_name = format!(r"Local\VPI-DMDDEV-{}-{}", std::process::id(), NEXT_LINK.fetch_add(1, Ordering::Relaxed));
        let link = dmd_link::Link::create(&link_name).map_err(|e| format!("the link to vpi-dmddevice: {e}"))?;
        let mut command = std::process::Command::new(&host);
        command.args(["--link", &link_name, "--parent", &std::process::id().to_string()]);
        command.arg("--library").arg(&setup.library).args(["--game", game, "--hardware", &hardware.to_string()]);
        let options: Vec<String> = options.iter().map(i32::to_string).collect();
        command.args(["--options", &options.join(",")]);
        if let Some(config) = &setup.config {
            command.arg("--config").arg(config);
        }
        let child = command.spawn().map_err(|e| format!("start {}: {e}", host.display()))?;
        Ok(DmdDevice {
            link,
            child,
            last: None,
            sent: 0,
            dots: Vec::new(),
            raw: Vec::new(),
            words: Vec::new(),
            seg_data2: [0; dmd_link::SEG_WORDS],
            flex_segs: [[0; dmd_link::SEG_WORDS]; 2],
        })
    }

    /// Ends `vpi-dmddevice`, waiting a little for the device to blank the panel and close.
    pub fn stop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(None)) {
            return;
        }
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

impl Drop for DmdDevice {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_device_is_named_by_the_settings() {
        let s = |text: &str| crate::settings::Settings::from_text(text);
        assert_eq!(setup(&s("")), None);
        assert_eq!(setup(&s("[VPI]\nDmdDevice = 0\n")), None);
        // a path that isn't there: none
        assert_eq!(setup(&s("[VPI]\nDmdDevice = C:\\nowhere\\DmdDevice64.dll\n")), None);
        // one that is: this test's own program stands in for it
        let exe = std::env::current_exe().unwrap();
        let named = setup(&s(&format!("[VPI]\nDmdDevice = {}\nDmdDeviceConfig = C:\\cab\\DmdDevice.ini\n", exe.display()))).expect("named");
        assert_eq!((named.library, named.config), (exe, Some(PathBuf::from(r"C:\cab\DmdDevice.ini"))));
    }

    #[test]
    fn flexdmd_names_a_game_from_the_table_without_its_version() {
        for (title, name) in [
            ("Diablo Pinball v600", "Diablo Pinball"),
            ("Leprechaun King (Orbital 2020)", "Leprechaun King (Orbital 2020)"),
            ("Table 1.2.3", "Table"),
            ("Table_1.2a-DOF*", "Table"),
            ("Abc_12", "Abc"),
            ("Abc 2_1.0", "Abc"),
            ("Star Gazer V1.1", "Star Gazer"),
            ("Tablev", "Tablev"),
            ("Flex 1_", "Flex"),
        ] {
            assert_eq!(flexdmd_game_name(title), name, "{title}");
        }
    }

    #[test]
    fn flexdmd_hands_its_colour_and_shades_of_it() {
        let o = flexdmd_options([0xFF, 0xFF, 0x58, 0x20]);
        assert_eq!(o[..6], [255, 88, 32, 66, 33, 0]);
        assert_eq!(o[10..], [168, 58, 21, 84, 29, 10, 0, 0, 0]);
        assert_eq!(flexdmd_segment_sizes(8), (32, 6));
        assert_eq!(flexdmd_segment_sizes(2), (0, 0));
    }
}
