// Ported from Visual Pinball X build 5436 (10.8.1 pre-release, commit af26b2d): src/input/SensorMapping.h and
// src/input/PhysicsSensor.cpp (a sensor's mapping, its value and its text in the settings), the nudge and plunger
// sensors' settings of src/physics/cabinet/NudgeHandler.cpp, GamepadNudge.cpp, CabinetNudgeSensor.cpp and
// src/input/PlungerHandler.cpp, the default layouts' sensors of src/input/SDLInputHandler.h, and the helpers
// trim_string, try_parse_int and try_parse_float of src/core/def.h and def.cpp (license:GPLv3+).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A cabinet's analog sensors as VPX 5436 maps them: any joystick
//! axis — a gamepad's stick, a cabinet board's accelerometer or plunger — read through a mapping
//! (dead zone, limit, scale), each reading kept with its time on the device's clock, as VPX's
//! mappings keep their last one. The nudge and plunger sensors come from the settings under VPX's
//! names — `[Input]` `NudgeSensorCount`, `Mapping.Nudge<n>.Type` (0 a gamepad's way, 1 a cabinet
//! sensor read as intent, 2 applied directly), `.Strength`, `.CabWeight`, a gamepad nudge's `.X`
//! and `.Y`, a cabinet sensor's `.VelX`, `.VelY`, `.AccX` and `.AccY`; `PlungerSensorCount`,
//! `Mapping.Plunger<n>.Position` and `.Velocity`; each mapping `device;axis;P|V|A;dead zone;scale;
//! limit` — then from VPX's default layouts as devices connect: a gamepad's left stick nudging and
//! right stick the plunger, a Pinscape board's accelerometer and plunger, an Arcade2TV's stick and
//! plunger. VPX adds a layout's sensors to its lists (and its settings); here they fill slots the
//! physics reads from the start, in the order the devices came, after the settings' own. The
//! keyboard's nudge model and strength come from the settings too
//! (`[Player]` `KeyboardNudgeMode`, `KeyboardNudgeStrength`): VPX 5436's three models, its "10.8"
//! box the default here as in the released VPX, where 5436 defaults to its "10.9".

use std::sync::Mutex;

use physics_compat::math::Vertex2D;

use crate::input_map::{AXIS, GamepadElements, InputMap, Kind as DeviceKind};
use crate::settings::Settings;

/// How a sensor's axis is read (VPX's `SensorMapping::Type`): what the mapping says it measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Position,
    Velocity,
    Acceleration,
}

/// An axis's mapping (VPX's `SensorMapping`): its dead zone, limit and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisMapping {
    pub dead_zone: f32,
    pub scale: f32,
    pub limit: f32,
}

impl AxisMapping {
    /// VPX's default mapping for an axis: no dead zone, scale 1, limit 1.
    pub const VPX_DEFAULT: AxisMapping = AxisMapping { dead_zone: 0.0, scale: 1.0, limit: 1.0 };

    /// VPX's `SensorMapping::UpdateValue`.
    pub fn value(&self, raw: f32) -> f32 {
        let mut value = raw;
        if self.dead_zone > 0.0 {
            value = if value < -self.dead_zone {
                (value + self.dead_zone) / (1.0 - self.dead_zone)
            } else if value > self.dead_zone {
                (value - self.dead_zone) / (1.0 - self.dead_zone)
            } else {
                0.0
            };
        }
        // VPX's clamp, max(min(x, limit), -limit), as SSE's minss and maxss
        let m = if value < self.limit { value } else { self.limit };
        let c = if m > -self.limit { m } else { -self.limit };
        c * self.scale
    }
}

/// One device's axis read through a mapping (VPX's `PhysicsSensor` with its `SensorMapping`):
/// the device as the input map numbers it, the axis as a joystick element (`AXIS` and its number).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub device: u16,
    pub element: u16,
    pub kind: Kind,
    pub mapping: AxisMapping,
}

impl Axis {
    pub fn new(device: u16, element: u16, kind: Kind) -> Axis {
        Axis { device, element, kind, mapping: AxisMapping::VPX_DEFAULT }
    }

    fn scaled(self, scale: f32) -> Axis {
        Axis { mapping: AxisMapping { scale, ..self.mapping }, ..self }
    }

    /// Its latest reading through its mapping: its time on the device's clock (SDL's, in
    /// nanoseconds) and its value; (0, 0) before the first, as VPX's mapping starts.
    pub fn reading(&self) -> (u64, f32) {
        let readings = READINGS.lock().unwrap_or_else(|e| e.into_inner());
        match readings.iter().find(|r| r.0 == self.device && r.1 == self.element) {
            Some(&(_, _, timestamp, raw)) => (timestamp, self.mapping.value(raw)),
            None => (0, 0.0),
        }
    }
}

/// Every joystick axis's latest reading, by device and element: its time on SDL's clock and its
/// raw value, -1 to 1 (written by the window thread as SDL's events come, read by the game thread
/// each millisecond).
static READINGS: Mutex<Vec<(u16, u16, u64, f32)>> = Mutex::new(Vec::new());

/// A joystick axis moved (VPX's `PushAxisEvent` to its sensor mappings): `value` from -1 to 1
/// (SDL's axis over 32768, as VPX scales a joystick's).
pub fn set_axis(device: u16, element: u16, timestamp_ns: u64, value: f32) {
    let mut readings = READINGS.lock().unwrap_or_else(|e| e.into_inner());
    match readings.iter_mut().find(|r| r.0 == device && r.1 == element) {
        Some(r) => (r.2, r.3) = (timestamp_ns, value),
        None => readings.push((device, element, timestamp_ns, value)),
    }
}

/// VPX's `try_parse_int` (`std::from_chars` on the trimmed text): an optional minus and at least
/// one digit, whatever follows.
fn parse_int(text: &str) -> Option<i32> {
    let t = text.trim_matches([' ', '\t', '\r', '\n']);
    let digits = t.strip_prefix('-').unwrap_or(t);
    let n = digits.bytes().take_while(u8::is_ascii_digit).count();
    if n == 0 {
        return None;
    }
    t[..t.len() - digits.len() + n].parse().ok()
}

/// VPX's `try_parse_float` (`std::from_chars`): the longest number at the start of the trimmed
/// text, in plain or exponent form.
fn parse_float(text: &str) -> Option<f32> {
    let t = text.trim_matches([' ', '\t', '\r', '\n']).as_bytes();
    let mut i = usize::from(t.first() == Some(&b'-'));
    let int_digits = t[i..].iter().take_while(|b| b.is_ascii_digit()).count();
    i += int_digits;
    let mut frac_digits = 0;
    if t.get(i) == Some(&b'.') {
        frac_digits = t[i + 1..].iter().take_while(|b| b.is_ascii_digit()).count();
        if int_digits + frac_digits > 0 {
            i += 1 + frac_digits;
        }
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    if matches!(t.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(t.get(j), Some(b'-' | b'+')) {
            j += 1;
        }
        let exp_digits = t[j..].iter().take_while(|b| b.is_ascii_digit()).count();
        if exp_digits > 0 {
            i = j + exp_digits;
        }
    }
    std::str::from_utf8(&t[..i]).ok()?.parse().ok()
}

/// A sensor's mapping as the settings write it (VPX's `PhysicsSensor::SetMapping`):
/// `device;axis;P|V|A;dead zone;scale;limit`, the device by its name in the settings (known from
/// now on if it wasn't, as VPX's `GetDeviceId` makes it). None when empty, or when it doesn't read
/// (said once, as VPX logs it).
pub fn parse_mapping(text: &str, map: &mut InputMap) -> Option<Axis> {
    let fields: Vec<&str> = text.split(';').map(|f| f.trim_matches([' ', '\t', '\r', '\n'])).collect();
    let read = || -> Option<(&str, i32, Kind, f32, f32, f32)> {
        let kind = match fields.get(2).copied()? {
            "P" => Kind::Position,
            "V" => Kind::Velocity,
            "A" => Kind::Acceleration,
            _ => return None,
        };
        Some((fields[0], parse_int(fields.get(1)?)?, kind, parse_float(fields.get(3)?)?, parse_float(fields.get(4)?)?, parse_float(fields.get(5)?)?))
    };
    match read() {
        Some((device, axis, kind, dead_zone, scale, limit)) => {
            Some(Axis { device: map.device(device), element: axis as u16, kind, mapping: AxisMapping { dead_zone, scale, limit } })
        }
        None => {
            if !text.trim().is_empty() {
                crate::script::log::once(&format!("input: invalid sensor mapping: '{text}'"));
            }
            None
        }
    }
}

/// A nudge sensor from the settings.
#[derive(Clone, Debug, PartialEq)]
pub enum NudgeSensor {
    /// VPX's `GamepadNudge` (type 0): two axes read as the player's intent, as a gamepad's stick.
    Gamepad { x: Option<Axis>, y: Option<Axis>, strength: f32 },
    /// VPX's `CabinetNudgeSensor`: a real cabinet's motion, read as intent (type 1) or applied
    /// directly (type 2), the cabinet's mass in kg.
    Cabinet { velocity: [Option<Axis>; 2], acceleration: [Option<Axis>; 2], intent: bool, strength: f32, cabinet_mass: f32 },
}

/// A plunger sensor from the settings (VPX's `PlungerSensor`): its position axis, and a speed
/// axis for the moment the plunger hits the ball, if the hardware measures it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlungerSensor {
    pub position: Option<Axis>,
    pub velocity: Option<Axis>,
}

/// How many devices' default layouts are kept: their sensors' slots.
pub const LAYOUT_SLOTS: usize = 4;

/// The cabinet's analog sensors the settings set up, the strengths of the default layouts', and
/// the keyboard's nudge model.
#[derive(Clone, Debug, PartialEq)]
pub struct Sensors {
    /// The keyboard's nudge model and strength (VPX's `[Player]` `KeyboardNudgeMode`, 0 to 2,
    /// and `KeyboardNudgeStrength`, 0 to 2, 1 by default).
    pub keyboard_mode: physics_compat::nudge::KeyboardNudgeMode,
    pub keyboard_strength: f32,
    pub nudge: Vec<NudgeSensor>,
    pub plunger: Vec<PlungerSensor>,
    /// Each default layout's nudge strength, slot by slot: the setting it would get in VPX,
    /// `Mapping.Nudge<n>.Strength` numbered after the settings' own sensors (1 by default).
    pub layout_strengths: [f32; LAYOUT_SLOTS],
}

impl Default for Sensors {
    fn default() -> Self {
        Sensors {
            keyboard_mode: physics_compat::nudge::KeyboardNudgeMode::VpxBoxModel,
            keyboard_strength: 1.0,
            nudge: Vec::new(),
            plunger: Vec::new(),
            layout_strengths: [1.0; LAYOUT_SLOTS],
        }
    }
}

impl Sensors {
    /// The settings' sensors (VPX's `NudgeHandler` and `PlungerHandler` loading theirs), their
    /// devices named in the input map.
    pub fn from_settings(settings: &Settings, map: &mut InputMap) -> Sensors {
        let mut s = Sensors::default();
        if let Some(mode) = physics_compat::nudge::KeyboardNudgeMode::from_setting(settings.int_or("Player", "KeyboardNudgeMode", 1, 0, 2)) {
            s.keyboard_mode = mode;
        }
        s.keyboard_strength = settings.float_or("Player", "KeyboardNudgeStrength", 1.0, 0.0, 2.0, 0.0);
        let mut mapping = |name: String| settings.get("Input", &format!("Mapping.{name}")).and_then(|t| parse_mapping(t, map));
        let strength = |i: usize| settings.float_or("Input", &format!("Mapping.Nudge{i}.Strength"), 1.0, 0.0, 2.0, 0.0);
        let nudges = settings.int_or("Input", "NudgeSensorCount", 0, 0, 100) as usize;
        for i in 0..nudges {
            match settings.enum_or("Input", &format!("Mapping.Nudge{i}.Type"), 0, 3) {
                0 => s.nudge.push(NudgeSensor::Gamepad { x: mapping(format!("Nudge{i}.X")), y: mapping(format!("Nudge{i}.Y")), strength: strength(i) }),
                kind => s.nudge.push(NudgeSensor::Cabinet {
                    velocity: [mapping(format!("Nudge{i}.VelX")), mapping(format!("Nudge{i}.VelY"))],
                    acceleration: [mapping(format!("Nudge{i}.AccX")), mapping(format!("Nudge{i}.AccY"))],
                    intent: kind == 1,
                    strength: strength(i),
                    cabinet_mass: settings.float_or("Input", &format!("Mapping.Nudge{i}.CabWeight"), 113.0, 0.0, 200.0, 0.0),
                }),
            }
        }
        for (k, layout) in s.layout_strengths.iter_mut().enumerate() {
            *layout = strength(nudges + k);
        }
        let plungers = settings.int_or("Input", "PlungerSensorCount", 0, 0, 100) as usize;
        for i in 0..plungers {
            s.plunger.push(PlungerSensor { position: mapping(format!("Plunger{i}.Position")), velocity: mapping(format!("Plunger{i}.Velocity")) });
        }
        s
    }

    /// The devices the settings' sensors read (VPX's `IsMappedToDevice`): set up already, so no
    /// default layout replaces their mappings.
    pub fn devices(&self) -> Vec<u16> {
        let mut axes: Vec<Axis> = Vec::new();
        for n in &self.nudge {
            match n {
                NudgeSensor::Gamepad { x, y, .. } => axes.extend(x.iter().chain(y)),
                NudgeSensor::Cabinet { velocity, acceleration, .. } => axes.extend(velocity.iter().chain(acceleration).flatten()),
            }
        }
        for p in &self.plunger {
            axes.extend(p.position.iter().chain(&p.velocity));
        }
        let mut devices: Vec<u16> = axes.iter().map(|a| a.device).collect();
        devices.sort_unstable();
        devices.dedup();
        devices
    }
}

/// The default layouts' sensors, slot by slot as devices came: a nudge sensor's two axes, a
/// plunger's position axis and its speed axis where the device has one.
static LAYOUT_NUDGE: Mutex<[Option<[Axis; 2]>; LAYOUT_SLOTS]> = Mutex::new([None; LAYOUT_SLOTS]);
static LAYOUT_PLUNGER: Mutex<[Option<PlungerSensor>; LAYOUT_SLOTS]> = Mutex::new([None; LAYOUT_SLOTS]);

/// The sensors of VPX's default layout for a device (`OnJoystickAdded`'s `MapNudge` and
/// `MapPlunger`): a gamepad's left stick nudging (as positions) and its right stick the plunger;
/// a Pinscape board's accelerometer (its first two axes) nudging and its third axis the plunger;
/// an Arcade2TV-XR's stick nudging at a fifth of its travel and its fifth axis the plunger; an
/// Open Pinball Device's nudge speeds nudging as a gamepad's stick and its plunger's position and
/// speed the plunger (VPX's `OpenPinDevHandler`). A gamepad without every element VPX's layout
/// asks for gets no layout, nor its sensors.
pub fn default_layout(kind: DeviceKind, device: u16, pad: Option<&GamepadElements>) -> (Option<[Axis; 2]>, Option<PlungerSensor>) {
    let axis = |element: u16, kind: Kind| Axis::new(device, element, kind);
    let plunger = |position: Axis| Some(PlungerSensor { position: Some(position), velocity: None });
    match kind {
        DeviceKind::Pinscape => (Some([axis(AXIS, Kind::Acceleration), axis(AXIS | 1, Kind::Acceleration)]), plunger(axis(AXIS | 2, Kind::Position))),
        DeviceKind::Arcade2TvPinball => (Some([axis(AXIS, Kind::Acceleration).scaled(0.2), axis(AXIS | 1, Kind::Acceleration).scaled(0.2)]), plunger(axis(AXIS | 4, Kind::Position))),
        DeviceKind::OpenPinDev => {
            use crate::opd::{NUDGE_SPEED_X, NUDGE_SPEED_Y, PLUNGER_POSITION, PLUNGER_SPEED};
            (
                Some([axis(NUDGE_SPEED_X, Kind::Velocity), axis(NUDGE_SPEED_Y, Kind::Velocity)]),
                Some(PlungerSensor { position: Some(axis(PLUNGER_POSITION, Kind::Position)), velocity: Some(axis(PLUNGER_SPEED, Kind::Velocity)) }),
            )
        }
        DeviceKind::Skip => (None, None),
        DeviceKind::Unknown => match pad {
            Some(p) if !crate::input_map::default_layout(kind, device, Some(p)).is_empty() => match (p.left_stick_x, p.left_stick_y, p.right_stick_y) {
                (Some(x), Some(y), Some(stick)) => (Some([axis(x, Kind::Position), axis(y, Kind::Position)]), plunger(axis(stick, Kind::Position))),
                _ => (None, None),
            },
            _ => (None, None),
        },
    }
}

/// A device's default layout's sensors, each in the next free slot unless a slot has the same
/// already (VPX's `HasSensor`); none once the slots are full.
pub fn add_layout(nudge: Option<[Axis; 2]>, plunger: Option<PlungerSensor>) {
    if let Some(n) = nudge {
        let mut slots = LAYOUT_NUDGE.lock().unwrap_or_else(|e| e.into_inner());
        if !slots.contains(&Some(n))
            && let Some(free) = slots.iter_mut().find(|s| s.is_none())
        {
            *free = Some(n);
        }
    }
    if let Some(p) = plunger {
        let mut slots = LAYOUT_PLUNGER.lock().unwrap_or_else(|e| e.into_inner());
        if !slots.contains(&Some(p))
            && let Some(free) = slots.iter_mut().find(|s| s.is_none())
        {
            *free = Some(p);
        }
    }
}

/// A default layout's plunger, by slot.
pub fn layout_plunger(slot: usize) -> Option<PlungerSensor> {
    LAYOUT_PLUNGER.lock().unwrap_or_else(|e| e.into_inner())[slot]
}

/// What the app's Controls page shows of the sensors (`vpi-player --input-test`): the nudge's two
/// values — a stick's position, or a cabinet board's acceleration — and the plunger's position,
/// each through its mapping, the settings' sensors first, else a default layout's; None for none.
pub fn live(sensors: &Sensors) -> (Option<[f32; 2]>, Option<f32>) {
    let value = |a: Option<Axis>| a.map_or(0.0, |a| a.reading().1);
    let nudge = sensors
        .nudge
        .iter()
        .find_map(|n| match *n {
            NudgeSensor::Gamepad { x, y, .. } => (x.is_some() || y.is_some()).then(|| [value(x), value(y)]),
            NudgeSensor::Cabinet { acceleration, .. } => acceleration.iter().any(Option::is_some).then(|| [value(acceleration[0]), value(acceleration[1])]),
        })
        .or_else(|| LAYOUT_NUDGE.lock().unwrap_or_else(|e| e.into_inner()).iter().flatten().next().map(|[x, y]| [x.reading().1, y.reading().1]));
    let plunger = sensors
        .plunger
        .iter()
        .find_map(|p| p.position.map(|a| a.reading().1))
        .or_else(|| LAYOUT_PLUNGER.lock().unwrap_or_else(|e| e.into_inner()).iter().flatten().find_map(|p| p.position.map(|a| a.reading().1)));
    (nudge, plunger)
}

/// Two axes as VPX's gamepad nudge reads them: their values, 0 for an unmapped one.
#[derive(Debug)]
struct Stick {
    x: Option<Axis>,
    y: Option<Axis>,
}

impl physics_compat::nudge::NudgeAxes for Stick {
    fn values(&self) -> Vertex2D {
        Vertex2D::new(self.x.map_or(0.0, |a| a.reading().1), self.y.map_or(0.0, |a| a.reading().1))
    }
}

/// A default layout's nudge slot: nothing until a device fills it.
#[derive(Debug)]
struct LayoutStick(usize);

impl physics_compat::nudge::NudgeAxes for LayoutStick {
    fn values(&self) -> Vertex2D {
        let slot = LAYOUT_NUDGE.lock().unwrap_or_else(|e| e.into_inner())[self.0];
        slot.map_or(Vertex2D::new(0.0, 0.0), |[x, y]| Vertex2D::new(x.reading().1, y.reading().1))
    }
}

/// A cabinet sensor's channel.
#[derive(Debug)]
struct Channel(Axis);

impl physics_compat::nudge::SensorChannel for Channel {
    fn reading(&self) -> Option<(u64, f32)> {
        Some(self.0.reading())
    }
}

/// The settings' nudge sensors, then the default layouts' slots, added to the physics' nudge
/// handler in VPX's order (the first active one moves the cabinet).
pub fn add_nudge_sensors(handler: &mut physics_compat::nudge::NudgeHandler, sensors: &Sensors) {
    use std::rc::Rc;
    let channel = |a: Option<Axis>| a.map(|a| Rc::new(Channel(a)) as Rc<dyn physics_compat::nudge::SensorChannel>);
    for n in &sensors.nudge {
        match *n {
            NudgeSensor::Gamepad { x, y, strength } => handler.add_gamepad_sensor(Rc::new(Stick { x, y }), strength),
            NudgeSensor::Cabinet { velocity, acceleration, intent, strength, cabinet_mass } => handler.add_cabinet_sensor(
                physics_compat::nudge::CabinetChannels {
                    velocity_x: channel(velocity[0]),
                    velocity_y: channel(velocity[1]),
                    acceleration_x: channel(acceleration[0]),
                    acceleration_y: channel(acceleration[1]),
                },
                intent,
                strength,
                cabinet_mass,
            ),
        }
    }
    for (slot, &strength) in sensors.layout_strengths.iter().enumerate() {
        handler.add_gamepad_sensor(Rc::new(LayoutStick(slot)), strength);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_axis_mapping_is_vpxs() {
        let m = AxisMapping { dead_zone: 0.1, scale: 2.0, limit: 0.5 };
        assert_eq!((m.value(0.05), m.value(1.0), m.value(-0.55)), (0.0, 1.0, -1.0));
        assert_eq!(AxisMapping::VPX_DEFAULT.value(-0.25), -0.25);
    }

    #[test]
    fn numbers_read_as_vpx_reads_them() {
        assert_eq!((parse_int(" 512 "), parse_int("-3"), parse_int("12abc"), parse_int("+1"), parse_int("x")), (Some(512), Some(-3), Some(12), None, None));
        assert_eq!((parse_float("0.100000"), parse_float("-1e-3"), parse_float("2."), parse_float(".5"), parse_float("19.6133x")), (Some(0.1), Some(-0.001), Some(2.0), Some(0.5), Some(19.6133)));
        assert_eq!((parse_float("+1"), parse_float("."), parse_float("e3")), (None, None, None));
    }

    #[test]
    fn mappings_read_from_the_settings_text() {
        let mut map = InputMap::new(&Settings::default());
        let a = parse_mapping("SDLJoy_0300abc_1;514;A;0.000000;19.613300;1.000000", &mut map).expect("reads");
        assert_eq!((a.device, a.element, a.kind), (map.device("SDLJoy_0300abc_1"), 514, Kind::Acceleration));
        assert_eq!(a.mapping, AxisMapping { dead_zone: 0.0, scale: 19.6133, limit: 1.0 });
        // a field missing, or a type VPX doesn't know: no mapping
        assert_eq!(parse_mapping("SDLJoy_0300abc_1;514;A;0;1", &mut map), None);
        assert_eq!(parse_mapping("SDLJoy_0300abc_1;514;X;0;1;1", &mut map), None);
        assert_eq!(parse_mapping("", &mut map), None);
    }

    #[test]
    fn the_settings_sensors_in_vpxs_names() {
        let settings = Settings::from_text(concat!(
            "[Player]\nKeyboardNudgeMode = 2\nKeyboardNudgeStrength = 0.8\n",
            "[Input]\nNudgeSensorCount = 2\nPlungerSensorCount = 1\n",
            "Mapping.Nudge0.Type = 0\nMapping.Nudge0.Strength = 0.5\nMapping.Nudge0.X = Pad_1;512;P;0;1;1\nMapping.Nudge0.Y = Pad_1;513;P;0;1;1\n",
            "Mapping.Nudge1.Type = 2\nMapping.Nudge1.CabWeight = 90\nMapping.Nudge1.AccX = Board_1;512;A;0;19.6;1\nMapping.Nudge1.VelY = Board_1;516;V;0;1;1\n",
            "Mapping.Nudge2.Strength = 1.5\n",
            "Mapping.Plunger0.Position = Board_1;514;P;0;1;1\nMapping.Plunger0.Velocity = Board_1;517;V;0;2;1\n",
        ));
        let mut map = InputMap::new(&Settings::default());
        let s = Sensors::from_settings(&settings, &mut map);
        let (pad, board) = (map.device("Pad_1"), map.device("Board_1"));
        let NudgeSensor::Gamepad { x: Some(x), y: Some(_), strength } = s.nudge[0] else { panic!("{:?}", s.nudge[0]) };
        assert_eq!((x.device, x.element, strength), (pad, 512, 0.5));
        let NudgeSensor::Cabinet { velocity, acceleration, intent, strength, cabinet_mass } = s.nudge[1] else { panic!("{:?}", s.nudge[1]) };
        assert!(velocity[0].is_none() && velocity[1].is_some() && acceleration[0].is_some() && acceleration[1].is_none());
        assert_eq!((intent, strength, cabinet_mass), (false, 1.0, 90.0));
        // a default layout's sensor takes the strength VPX would number it with: after the settings' two
        assert_eq!(s.layout_strengths, [1.5, 1.0, 1.0, 1.0]);
        assert_eq!(s.plunger[0].velocity.map(|v| (v.device, v.element, v.mapping.scale)), Some((board, 517, 2.0)));
        assert_eq!(s.devices(), [pad.min(board), pad.max(board)]);
        assert_eq!((s.keyboard_mode, s.keyboard_strength), (physics_compat::nudge::KeyboardNudgeMode::VpxCabModel, 0.8));
        // unset: the released VPX's model
        let none = Sensors::from_settings(&Settings::default(), &mut map);
        assert_eq!((none.keyboard_mode, none.keyboard_strength), (physics_compat::nudge::KeyboardNudgeMode::VpxBoxModel, 1.0));
    }

    #[test]
    fn a_reading_goes_through_its_mapping_and_a_layout_is_added_once() {
        set_axis(900, AXIS | 1, 5_000, 0.5);
        let a = Axis::new(900, AXIS | 1, Kind::Acceleration).scaled(0.2);
        assert_eq!(a.reading(), (5_000, 0.1));
        assert_eq!(Axis::new(900, AXIS | 2, Kind::Position).reading(), (0, 0.0), "no reading yet");
        let pinscape = default_layout(DeviceKind::Pinscape, 901, None);
        add_layout(pinscape.0, pinscape.1);
        add_layout(pinscape.0, pinscape.1);
        let plungers: Vec<Option<PlungerSensor>> = (0..LAYOUT_SLOTS).map(layout_plunger).collect();
        assert_eq!(plungers.iter().filter(|p| p.and_then(|p| p.position).is_some_and(|a| a.device == 901)).count(), 1);
        // an Open Pinball Device's plunger has its speed axis too, and its nudge reads speeds
        let (nudge, plunger) = default_layout(DeviceKind::OpenPinDev, 902, None);
        assert_eq!(plunger.and_then(|p| p.velocity).map(|v| (v.element, v.kind)), Some((crate::opd::PLUNGER_SPEED, Kind::Velocity)));
        assert_eq!(nudge.map(|[x, _]| (x.element, x.kind)), Some((crate::opd::NUDGE_SPEED_X, Kind::Velocity)));
    }
}
