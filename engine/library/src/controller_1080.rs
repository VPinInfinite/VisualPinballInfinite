//! A game controller in VPX 10.8.0, as VPX 10.8.1's settings map it. VPX 10.8.0 keeps settings of
//! its own, where a controller's buttons are numbers (`[Player]` `Joy…`, none by default) read
//! through DirectInput unless its settings choose XInput (`InputApi`), so a controller set up for
//! 10.8.1 does nothing there. Where 10.8.0's own settings map no controller button and 10.8.1's map
//! a controller SDL reads through Windows' XInput, a game in 10.8.0 is given that controller as
//! 10.8.1's settings have it — its buttons, its nudge and plunger sticks — through XInput, in the
//! copy of 10.8.0's settings made for the game alone.
//!
//! VPX 10.8.0 reads no button from a trigger (its `PinInput::ProcessJoystick` turns only buttons into
//! actions): flippers 10.8.1 has on the triggers go on the shoulder buttons, and what those pressed
//! on the sticks' clicks. A controller of another kind (a cabinet's board, a controller SDL reads
//! another way) is left to 10.8.0's own settings: its buttons are numbered otherwise there. Ours.

use vpi_settings::Change;

/// The settings VPX 10.8.0 maps a controller's buttons with (its `PinInput::LoadSettings`).
const OWN_BUTTONS: &[&str] = &[
    "JoyLFlipKey",
    "JoyRFlipKey",
    "JoyStagedLFlipKey",
    "JoyStagedRFlipKey",
    "JoyPlungerKey",
    "JoyAddCreditKey",
    "JoyAddCredit2Key",
    "JoyLMagnaSave",
    "JoyRMagnaSave",
    "JoyStartGameKey",
    "JoyFrameCount",
    "JoyExitGameKey",
    "JoyVolumeUp",
    "JoyVolumeDown",
    "JoyLTiltKey",
    "JoyCTiltKey",
    "JoyRTiltKey",
    "JoyPMBuyIn",
    "JoyPMCoin3",
    "JoyPMCoin4",
    "JoyPMCoinDoor",
    "JoyPMCancel",
    "JoyPMDown",
    "JoyPMUp",
    "JoyPMEnter",
    "JoyCustom1",
    "JoyCustom2",
    "JoyCustom3",
    "JoyCustom4",
    "JoyMechTiltKey",
    "JoyDebugKey",
    "JoyDebuggerKey",
    "JoyLockbarKey",
    "JoyPauseKey",
    "JoyTweakKey",
    "JoyTableRecenterKey",
    "JoyTableUpKey",
    "JoyTableDownKey",
];

/// Each action VPX 10.8.1's `[Input]` `Mapping.<action>` names, beside the setting VPX 10.8.0 maps
/// it with; the first found of two for one setting kept. 10.8.1's in-game menu goes where 10.8.0's
/// close menu is (Esc's, from which a game goes on or ends), its Service 1 to 4 on 10.8.0's PinMAME
/// menu buttons (the keys 7, 8, 9 and 0).
const ACTIONS: &[(&str, &str)] = &[
    ("LeftFlipper", "JoyLFlipKey"),
    ("RightFlipper", "JoyRFlipKey"),
    ("LeftStagedFlipper", "JoyStagedLFlipKey"),
    ("RightStagedFlipper", "JoyStagedRFlipKey"),
    ("LaunchBall", "JoyPlungerKey"),
    ("Credit1", "JoyAddCreditKey"),
    ("Credit2", "JoyAddCredit2Key"),
    ("Credit3", "JoyPMCoin3"),
    ("Credit4", "JoyPMCoin4"),
    ("ExtraBall", "JoyPMBuyIn"),
    ("CoinDoor", "JoyPMCoinDoor"),
    ("LeftMagna", "JoyLMagnaSave"),
    ("RightMagna", "JoyRMagnaSave"),
    ("Start", "JoyStartGameKey"),
    ("ExitGame", "JoyExitGameKey"),
    ("InGameUI", "JoyExitGameKey"),
    ("Pause", "JoyPauseKey"),
    ("PerfOverlay", "JoyFrameCount"),
    ("VolumeUp", "JoyVolumeUp"),
    ("VolumeDown", "JoyVolumeDown"),
    ("LeftNudge", "JoyLTiltKey"),
    ("CenterNudge", "JoyCTiltKey"),
    ("RightNudge", "JoyRTiltKey"),
    ("Tilt", "JoyMechTiltKey"),
    ("Lockbar", "JoyLockbarKey"),
    ("DebugBalls", "JoyDebugKey"),
    ("Debugger", "JoyDebuggerKey"),
    ("Service1", "JoyPMCancel"),
    ("Service2", "JoyPMDown"),
    ("Service3", "JoyPMUp"),
    ("Service4", "JoyPMEnter"),
    ("VRCenter", "JoyTableRecenterKey"),
    ("VRUp", "JoyTableUpKey"),
    ("VRDown", "JoyTableDownKey"),
];

/// Where 10.8.0 reads a shoulder button and the stick's click beside it: left, then right.
const SHOULDERS: [(&str, &str, u32, u32); 2] = [("LeftFlipper", "JoyLFlipKey", 5, 9), ("RightFlipper", "JoyRFlipKey", 6, 10)];

/// Whether a device VPX 10.8.1 names (`SDLJoy_<SDL's GUID>_<n>`) is a controller SDL reads through
/// Windows' XInput: its GUID's fifteenth byte, SDL's driver mark, `x`.
fn reads_through_xinput(device: &str) -> bool {
    device.strip_prefix("SDLJoy_").and_then(|rest| rest.split('_').next()).is_some_and(|guid| guid.len() == 32 && guid.get(28..30).is_some_and(|b| b.eq_ignore_ascii_case("78")))
}

/// The ways one of 10.8.1's mappings presses an action, each a device's element alone (several held
/// together left out): the device, the element's number, and whether an axis is pressed past a point
/// (`;o;<point>`) rather than a button.
fn ways(value: &str) -> impl Iterator<Item = (&str, u32, bool)> {
    value.split('|').map(str::trim).filter(|w| !w.contains('&')).filter_map(|w| {
        let mut parts = w.split(';').map(str::trim);
        let device = parts.next().filter(|d| !d.is_empty())?;
        let element = parts.next()?.parse().ok()?;
        Some((device, element, parts.next().is_some()))
    })
}

/// VPX 10.8.0's number for an XInput controller's button (its `HandleInputXI`, from 1: A, B, X, Y,
/// the shoulders, Back, Start, the sticks' clicks, then the pad's left, right, up and down), from
/// the element 10.8.1 names through SDL, which lists XInput's buttons in that order from 0 and the
/// pad as a hat (0x100 on: left, right, up, down). None for one 10.8.0 reads no button from.
fn button_1080(element: u32) -> Option<u32> {
    match element {
        0..=9 => Some(element + 1),
        0x100..=0x103 => Some(element - 0x100 + 11),
        _ => None,
    }
}

/// VPX 10.8.0's number for an XInput controller's axis (`LRAxis`, `UDAxis`, `PlungerAxis`: X, Y, Z,
/// RX, RY, RZ from 1 — the left stick, the left trigger, the right stick, the right trigger), from
/// the element 10.8.1 names through SDL, which lists XInput's axes in that order (0x200 on).
fn axis_1080(element: u32) -> Option<u32> {
    (0x200..=0x205).contains(&element).then(|| element - 0x200 + 1)
}

/// The axis 10.8.1's sensor `name` reads on `device`, as 10.8.0 numbers it.
fn sensor(newer: &[(String, String, String)], name: &str, device: &str) -> Option<u32> {
    let value = vpi_settings::get(newer, "Input", name)?;
    ways(value).find(|(d, _, _)| *d == device).and_then(|(_, element, _)| axis_1080(element))
}

/// What VPX 10.8.0's settings (`own`) are changed by for a game so that the controller VPX
/// 10.8.1's settings (`newer`) map works there too: none where 10.8.0's own map a controller button
/// already, where 10.8.1's map no XInput controller, or where they map another controller as well,
/// which 10.8.0 would stop reading through DirectInput.
pub fn changes(newer: &[(String, String, String)], own: &[(String, String, String)]) -> Vec<Change> {
    if OWN_BUTTONS.iter().any(|name| vpi_settings::get(own, "Player", name).and_then(vpi_settings::vpx_int).is_some_and(|n| n != 0)) {
        return Vec::new();
    }
    let Some(device) = vpi_settings::get(newer, "Input", "Devices").and_then(|all| all.split(';').map(str::trim).find(|d| reads_through_xinput(d))) else {
        return Vec::new();
    };
    let mapped = || newer.iter().filter(|(s, n, _)| s.eq_ignore_ascii_case("Input") && n.starts_with("Mapping.")).flat_map(|(_, _, v)| ways(v));
    if mapped().any(|(d, _, _)| d.starts_with("SDLJoy_") && d != device) || !mapped().any(|(d, _, _)| d == device) {
        return Vec::new();
    }
    let mapping = |action: &str| vpi_settings::get(newer, "Input", &format!("Mapping.{action}"));
    let mut buttons: Vec<(&str, u32)> = Vec::new();
    for &(action, name) in ACTIONS {
        if buttons.iter().any(|(n, _)| *n == name) {
            continue;
        }
        if let Some(button) = mapping(action).and_then(|v| ways(v).filter(|(d, _, axis)| *d == device && !axis).find_map(|(_, element, _)| button_1080(element))) {
            buttons.push((name, button));
        }
    }
    // flippers on the triggers: on the shoulders, what those pressed on the sticks' clicks
    for (action, name, shoulder, click) in SHOULDERS {
        let on_trigger = mapping(action).is_some_and(|v| ways(v).any(|(d, element, _)| d == device && axis_1080(element).is_some()));
        if !on_trigger || buttons.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let to = if buttons.iter().any(|(_, b)| *b == click) { None } else { Some(click) };
        buttons = buttons.into_iter().filter_map(|(n, b)| if b == shoulder { to.map(|c| (n, c)) } else { Some((n, b)) }).collect();
        buttons.push((name, shoulder));
    }
    let nudge = (0..4).map(|n| (sensor(newer, &format!("Mapping.Nudge{n}.X"), device), sensor(newer, &format!("Mapping.Nudge{n}.Y"), device))).find(|(x, y)| x.is_some() || y.is_some());
    let (across, along) = nudge.unwrap_or((None, None));
    let plunger = (0..4).find_map(|n| sensor(newer, &format!("Mapping.Plunger{n}.Position"), device));
    let speed = (0..4).find_map(|n| sensor(newer, &format!("Mapping.Plunger{n}.Velocity"), device));
    let mut changes = vec![Change::set("Player", "InputApi", 1)];
    changes.extend(buttons.into_iter().map(|(name, button)| Change::set("Player", name, button)));
    // the axes 10.8.1 reads on it, and none else: through XInput, Z is the left trigger
    for (name, axis) in [("LRAxis", across), ("UDAxis", along), ("PlungerAxis", plunger), ("PlungerSpeedAxis", speed)] {
        changes.push(Change::set("Player", name, axis.unwrap_or(0)));
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A controller SDL reads through XInput (driver mark `x`, 78) and one through its HID driver (`h`).
    const PAD: &str = "SDLJoy_030000005e040000130b000009057801_1";
    const HID_PAD: &str = "SDLJoy_030000005e040000130b000009056801_1";

    /// 10.8.1's settings as its own gamepad layout writes them: flippers on the triggers, the magna
    /// saves on the shoulders, A launches, B starts, Y adds a coin, Back opens the in-game menu, the
    /// pad the PinMAME menu's buttons, the left stick nudging, the right one the plunger.
    fn newer(pad: &str) -> Vec<(String, String, String)> {
        vpi_settings::parse_ini(&format!(
            "[Input]\nDevices = Key;{pad}\n\
             Mapping.LeftFlipper = Key;225 | {pad};514;o;-0.300000\n\
             Mapping.RightFlipper = Key;229 | {pad};517;o;-0.300000\n\
             Mapping.LeftStagedFlipper = Key;225 | {pad};514;o;0.300000\n\
             Mapping.LeftMagna = Key;224 | {pad};4\n\
             Mapping.RightMagna = Key;228 | {pad};5\n\
             Mapping.LaunchBall = Key;40 | {pad};0\n\
             Mapping.Credit1 = Key;34 | {pad};3\n\
             Mapping.Start = Key;30 | {pad};1\n\
             Mapping.ExitGame = Key;41\n\
             Mapping.InGameUI = Key;69 | {pad};6\n\
             Mapping.Service1 = Key;36 | {pad};256\n\
             Mapping.Service2 = Key;37 | {pad};259\n\
             Mapping.Service3 = Key;38 | {pad};258\n\
             Mapping.Service4 = Key;39 | {pad};257\n\
             Mapping.GenTournament = Key;226 & Key;30\n\
             Mapping.Plunger0.Position = {pad};516;P;0.000000;1.000000;1.000000\n\
             Mapping.Nudge0.X = {pad};512;P;0.000000;1.000000;1.000000\n\
             Mapping.Nudge0.Y = {pad};513;P;0.000000;1.000000;1.000000\n"
        ))
    }

    /// 10.8.0's own settings as it writes them first: no controller button mapped, DirectInput.
    const OWN: &str = "[Player]\nInputApi = 0\nLRAxis = 1\nUDAxis = 2\nPlungerAxis = 3\nJoyLFlipKey = 0\nJoyRFlipKey = 0\nJoyCustom1Key = 200\nJoyCustom2 =  0\n";

    fn set(changes: &[Change]) -> Vec<(String, String)> {
        changes.iter().map(|c| (c.name.clone(), c.value.clone().unwrap_or_default())).collect()
    }

    #[test]
    fn a_controller_set_up_for_10_8_1_works_in_10_8_0() {
        let changes = changes(&newer(PAD), &vpi_settings::parse_ini(OWN));
        let mut got = set(&changes);
        got.sort();
        let mut want: Vec<(String, String)> = [
            ("InputApi", "1"),
            // the flippers on the shoulders, the magna saves they had there on the sticks' clicks
            ("JoyLFlipKey", "5"),
            ("JoyRFlipKey", "6"),
            ("JoyLMagnaSave", "9"),
            ("JoyRMagnaSave", "10"),
            ("JoyPlungerKey", "1"),
            ("JoyAddCreditKey", "4"),
            ("JoyStartGameKey", "2"),
            ("JoyExitGameKey", "7"),
            ("JoyPMCancel", "11"),
            ("JoyPMDown", "14"),
            ("JoyPMUp", "13"),
            ("JoyPMEnter", "12"),
            ("LRAxis", "1"),
            ("UDAxis", "2"),
            ("PlungerAxis", "5"),
            ("PlungerSpeedAxis", "0"),
        ]
        .iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();
        want.sort();
        assert_eq!(got, want);
        assert!(changes.iter().all(|c| c.section == "Player"));
        // written into 10.8.0's own lines, the rest of its file as it was
        let text = vpi_settings::edit(OWN, &changes);
        assert!(text.contains("InputApi = 1") && text.contains("JoyLFlipKey = 5") && text.contains("PlungerAxis = 5") && text.contains("JoyCustom1Key = 200"));
    }

    #[test]
    fn ten_eight_zeros_own_controller_setup_is_kept() {
        let own = vpi_settings::parse_ini("[Player]\nInputApi = 0\nJoyLFlipKey = 0\nJoyRFlipKey = 7\n");
        assert!(changes(&newer(PAD), &own).is_empty());
        // keyboard keys under a controller-like name aren't a controller's
        assert!(!changes(&newer(PAD), &vpi_settings::parse_ini(OWN)).is_empty());
    }

    #[test]
    fn controllers_ten_eight_zero_numbers_otherwise_are_left_to_it() {
        let own = vpi_settings::parse_ini(OWN);
        // one SDL reads another way: its buttons are numbered otherwise
        assert!(changes(&newer(HID_PAD), &own).is_empty());
        // a cabinet's board mapped beside it, which 10.8.0 reads through DirectInput
        let mut both = newer(PAD);
        both.push(("Input".into(), "Mapping.Plunger1.Position".into(), "SDLJoy_03000000d2090000eaf8000000006800_1;514;P;0;1;1".into()));
        assert!(changes(&both, &own).is_empty());
        // none mapped at all
        assert!(changes(&vpi_settings::parse_ini("[Input]\nDevices = Key\nMapping.Start = Key;30\n"), &own).is_empty());
        let unused = vpi_settings::parse_ini(&format!("[Input]\nDevices = Key;{PAD}\nMapping.Start = Key;30\n"));
        assert!(changes(&unused, &own).is_empty());
    }

    /// `VPI_NEWER_SETTINGS=<10.8.1's VPinballX.ini> VPI_OWN_SETTINGS=<10.8.0's> cargo test -p library
    /// --lib a_real_pair -- --ignored --nocapture`: what a game in 10.8.0 is given, printed (settings'
    /// names and numbers alone), nothing written.
    #[test]
    #[ignore]
    fn a_real_pair() {
        let read = |var: &str| std::fs::read(std::env::var_os(var).unwrap_or_else(|| panic!("{var}"))).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap();
        let changes = changes(&vpi_settings::parse_ini(&read("VPI_NEWER_SETTINGS")), &vpi_settings::parse_ini(&read("VPI_OWN_SETTINGS")));
        println!("{} changes", changes.len());
        for c in changes {
            println!("[{}] {} = {}", c.section, c.name, c.value.unwrap_or_default());
        }
    }

    #[test]
    fn elements_numbered_as_10_8_0_reads_them() {
        assert!(reads_through_xinput(PAD));
        assert!(!reads_through_xinput(HID_PAD) && !reads_through_xinput("Key") && !reads_through_xinput("SDLJoy_78_1"));
        assert_eq!((button_1080(0), button_1080(9), button_1080(10)), (Some(1), Some(10), None));
        assert_eq!((button_1080(0x100), button_1080(0x103), button_1080(0x200)), (Some(11), Some(14), None));
        assert_eq!((axis_1080(0x200), axis_1080(0x202), axis_1080(0x205), axis_1080(0x206), axis_1080(5)), (Some(1), Some(3), Some(6), None, None));
        let w: Vec<_> = ways("Key;225 | SDLJoy_x_1;514;o;-0.3 | Key;226 & Key;30").collect();
        assert_eq!(w, [("Key", 225, false), ("SDLJoy_x_1", 514, true)]);
    }
}
