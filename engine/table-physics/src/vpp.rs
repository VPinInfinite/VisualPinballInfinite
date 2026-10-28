// Ported from Visual Pinball X build 5436 (10.8.1 pre-release, commit af26b2d): PinTable::ImportVPP and its
// call in PinTable::LoadGameFromFilename (src/parts/pintable.cpp), the setters it calls (PinTable's
// put_Gravity, put_Friction, put_Elasticity, put_ElasticityFalloff, put_Scatter, put_DefaultScatter,
// put_SlopeMin and put_SlopeMax; Flipper's put_Mass, put_Strength, put_Elasticity, put_Return,
// put_ElasticityFalloff, put_Friction, put_RampUp, put_Scatter, put_EOSTorque and put_EOSTorqueAngle, in
// src/parts/pintable.cpp, flipper.cpp and flipper.h), sz2f (src/core/def.cpp) and GRAVITYCONST and
// DEFAULT_TABLE_MIN_SLOPE and _MAX_SLOPE (src/physics/physconst.h) (license:GPLv3+).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A physics settings file left beside a table, as VPX 5436 loads one: `<table>.vpp`, else
//! `autovpp.vpp`, beside the table. It sets the table's gravity, friction, elasticity,
//! scatter and slopes, and every flipper's physics, as VPX's editor exports them ("Export
//! physics"), before play begins. VPX's quirks kept: a file without the slopes puts them back
//! to VPX's default (6°), and a flipper value the file leaves out becomes 0.

use std::path::{Path, PathBuf};

use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;

/// VPX's `VPPelementNames`: the table's eight, then the flipper's ten.
pub const ELEMENTS: [&str; 18] = [
    "gravityConstant",
    "contactFriction",
    "elasticity",
    "elasticityFalloff",
    "playfieldScatter",
    "defaultElementScatter",
    "playfieldminslope",
    "playfieldmaxslope",
    // flippers
    "speed",
    "strength",
    "elasticity",
    "scatter",
    "eosTorque",
    "eosTorqueAngle",
    "returnStrength",
    "elasticityFalloff",
    "friction",
    "coilRampUp",
];

const GRAVITYCONST: f32 = 1.81751;
const DEFAULT_TABLE_MIN_SLOPE: f32 = 6.0;
const DEFAULT_TABLE_MAX_SLOPE: f32 = 6.0;

/// The file VPX would read for `table`: `<table>.vpp` beside it, else `autovpp.vpp` there.
pub fn find(table: &Path) -> Option<PathBuf> {
    let own = table.with_extension("vpp");
    if own.is_file() {
        return Some(own);
    }
    let auto = table.parent()?.join("autovpp.vpp");
    auto.is_file().then_some(auto)
}

/// VPX's `sz2f`: the number `from_chars` reads from the text's first character — a minus sign,
/// digits, a decimal point and digits, an exponent when it has digits — 0 when there's none.
fn sz2f(text: &str) -> f32 {
    let b = text.as_bytes();
    let digits = |from: usize| b[from.min(b.len())..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = usize::from(b.first() == Some(&b'-'));
    let whole = digits(i);
    i += whole;
    let mut fraction = 0;
    if b.get(i) == Some(&b'.') {
        fraction = digits(i + 1);
        i += 1 + fraction;
    }
    if whole + fraction == 0 {
        return 0.0;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(b.get(i + 1), Some(b'+' | b'-')));
        let exponent = digits(i + 1 + sign);
        if exponent > 0 {
            i += 1 + sign + exponent;
        }
    }
    text[..i].parse().unwrap_or(0.0)
}

/// tinyxml2's `FirstChildElement`: the first child element of that name.
fn child<'a, 'input>(node: roxmltree::Node<'a, 'input>, name: &str) -> Option<roxmltree::Node<'a, 'input>> {
    node.children().find(|n| n.is_element() && n.tag_name().name() == name)
}

/// VPX's `saturate`.
fn saturate(x: f32) -> f32 {
    let x = if x < 1.0 { x } else { 1.0 };
    if x < 0.0 { 0.0 } else { x }
}

/// VPX's defaults for the flipper values a table may not hold (`[DefaultPropsFlipper]`
/// `Scatter`, `EOSTorque`, `EOSTorqueAngle` in its settings' definitions).
const FLIPPER_SCATTER: f32 = 0.0;
const FLIPPER_EOS_TORQUE: f32 = 0.75;
const FLIPPER_EOS_TORQUE_ANGLE: f32 = 6.0;

/// A table's own physics as VPX's editor exports them (`PinTable::ExportPhysics`): the table's
/// eight, in the units the file holds them — gravity as its script's `Gravity` — then its first
/// flipper's ten; None for a table with no flipper, which VPX won't export either.
pub fn own(game: &vpin::vpx::gamedata::GameData, items: &[GameItemEnum]) -> Option<[f32; 18]> {
    let f = items.iter().find_map(|i| match i {
        GameItemEnum::Flipper(f) => Some(f),
        _ => None,
    })?;
    Some([
        game.gravity / GRAVITYCONST,
        game.friction,
        game.elasticity,
        game.elastic_falloff,
        game.scatter,
        game.default_scatter,
        game.angle_tilt_min,
        game.angle_tilt_max,
        f.mass,
        f.strength,
        f.elasticity,
        f.scatter.unwrap_or(FLIPPER_SCATTER),
        f.torque_damping.unwrap_or(FLIPPER_EOS_TORQUE),
        f.torque_damping_angle.unwrap_or(FLIPPER_EOS_TORQUE_ANGLE),
        f.return_,
        f.elasticity_falloff,
        f.friction,
        f.ramp_up,
    ])
}

/// A physics settings file's name and values, read as `apply` reads them: None for a value it
/// doesn't hold.
pub fn read(text: &str) -> Result<(Option<String>, [Option<f32>; 18]), String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("Error parsing VPP XML file ({e})"))?;
    let root = Some(doc.root_element()).filter(|r| r.tag_name().name() == "physics").ok_or("the file has no physics element")?;
    let table = child(root, "table").ok_or("the file has no table element")?;
    let flipper = child(root, "flipper").ok_or("the file has no flipper element")?;
    let name = child(root, "name").and_then(|n| n.text()).map(str::to_string);
    let mut values = [None; 18];
    for (i, element) in ELEMENTS.iter().enumerate() {
        values[i] = child(if i <= 7 { table } else { flipper }, element).and_then(|el| el.children().next().and_then(|n| n.text())).map(sz2f);
    }
    Ok((name, values))
}

/// A value as VPX's `f2sz` writes it: C's `%f`, its trailing zeros dropped but for one after the point.
fn f2sz(value: f32) -> String {
    let mut s = format!("{:.6}", f64::from(value));
    if let Some(point) = s.find('.') {
        let last = s.trim_end_matches('0').len().max(point + 2);
        s.truncate(last);
    }
    s
}

/// The physics settings file VPX's editor writes for these values (`ExportPhysics`), printed as
/// its tinyxml2 prints: the declaration, then four spaces a level.
pub fn write(name: &str, values: &[f32; 18]) -> String {
    let escape = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<physics>\n");
    out += &format!("    <name>{}</name>\n", escape(name));
    for (part, range) in [("table", 0..8), ("flipper", 8..18)] {
        out += &format!("    <{part}>\n");
        for i in range {
            out += &format!("        <{0}>{1}</{0}>\n", ELEMENTS[i], f2sz(values[i]));
        }
        out += &format!("    </{part}>\n");
    }
    out += "</physics>\n";
    out
}

/// Applies a physics settings file's text to the table, as VPX's `ImportVPP`; returns what VPX
/// would have reported (its error boxes), the values it found missing among them. A file that
/// isn't one (no `physics`, `table` or `flipper` element) changes nothing: VPX would stop there.
pub fn apply(vpx: &mut VPX, text: &str) -> Result<Vec<String>, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("Error parsing VPP XML file ({e})"))?;
    let root = Some(doc.root_element()).filter(|r| r.tag_name().name() == "physics").ok_or("the file has no physics element")?;
    let table = child(root, "table").ok_or("the file has no table element")?;
    let flipper = child(root, "flipper").ok_or("the file has no flipper element")?;

    let mut reports = Vec::new();
    let mut flippers = [0.0f32; 10];
    let g = &mut vpx.gamedata;
    for (i, name) in ELEMENTS.iter().enumerate() {
        match child(if i <= 7 { table } else { flipper }, name) {
            Some(el) => {
                // an element without text sets nothing (VPX would leave a flipper value unset)
                let Some(t) = el.children().next().and_then(|n| n.text()) else { continue };
                let val = sz2f(t);
                match i {
                    0 => g.gravity = val.max(0.0) * GRAVITYCONST,
                    1 => g.friction = saturate(val),
                    2 => g.elasticity = val,
                    3 => g.elastic_falloff = val,
                    4 => g.scatter = val,
                    5 => g.default_scatter = val,
                    6 => g.angle_tilt_min = val,
                    7 => g.angle_tilt_max = val,
                    _ => flippers[i - 8] = val,
                }
            }
            None => match i {
                0..=5 => reports.push(format!("{name} is missing")),
                // added later on, so no error
                6 => g.angle_tilt_min = DEFAULT_TABLE_MIN_SLOPE,
                7 => g.angle_tilt_max = DEFAULT_TABLE_MAX_SLOPE,
                // the flipper's need a value: 0
                _ => reports.push(format!("flipper {name} is missing")),
            },
        }
    }

    // every flipper takes the file's
    let [mass, strength, elasticity, scatter, torque, torque_angle, return_strength, falloff, friction, ramp_up] = flippers;
    for item in &mut vpx.gameitems {
        if let GameItemEnum::Flipper(f) = item {
            f.mass = mass;
            f.strength = strength;
            f.elasticity = elasticity;
            f.return_ = saturate(return_strength);
            f.elasticity_falloff = falloff;
            f.friction = friction;
            f.ramp_up = ramp_up;
            f.scatter = Some(scatter);
            f.torque_damping = Some(torque);
            f.torque_damping_angle = Some(torque_angle);
        }
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table with two flippers, VPX's defaults otherwise.
    fn table() -> VPX {
        let flipper = || GameItemEnum::Flipper(vpin::vpx::gameitem::flipper::Flipper::default());
        VPX { gameitems: vec![flipper(), flipper()], ..VPX::default() }
    }

    #[test]
    fn an_exported_physics_file_sets_the_table_and_its_flippers_as_vpx_does() {
        let mut vpx = table();
        // as JP's physics sets come, without the slopes
        let text = r#"<?xml version="1.0" encoding="utf-8"?>
<physics>
	<name>a set</name>
	<table>
		<gravityConstant>0.980665</gravityConstant>
		<contactFriction>1.5</contactFriction>
		<elasticity>0.200000</elasticity>
		<elasticityFalloff>0.100000</elasticityFalloff>
		<defaultElementScatter>5.000000</defaultElementScatter>
		<playfieldScatter>4.000000</playfieldScatter>
	</table>
	<flipper>
		<speed>1.000000</speed>
		<strength>3600.000000</strength>
		<elasticity>0.850000</elasticity>
		<scatter>0.000000</scatter>
		<eosTorque>0.750000</eosTorque>
		<eosTorqueAngle>10.000000</eosTorqueAngle>
		<returnStrength>1.070000</returnStrength>
		<elasticityFalloff>0.550000</elasticityFalloff>
		<friction>0.250000</friction>
	</flipper>
</physics>"#;
        let reports = apply(&mut vpx, text).unwrap();
        let g = &vpx.gamedata;
        assert_eq!((g.gravity, g.friction, g.elasticity, g.elastic_falloff), (0.980665 * GRAVITYCONST, 1.0, 0.2, 0.1));
        assert_eq!((g.scatter, g.default_scatter, g.angle_tilt_min, g.angle_tilt_max), (4.0, 5.0, 6.0, 6.0));
        let flippers: Vec<_> = vpx.gameitems.iter().filter_map(|i| if let GameItemEnum::Flipper(f) = i { Some(f) } else { None }).collect();
        assert_eq!(flippers.len(), 2);
        for f in flippers {
            assert_eq!((f.mass, f.strength, f.elasticity, f.return_, f.elasticity_falloff, f.friction), (1.0, 3600.0, 0.85, 1.0, 0.55, 0.25));
            assert_eq!((f.scatter, f.torque_damping, f.torque_damping_angle), (Some(0.0), Some(0.75), Some(10.0)));
            // left out of the file: 0, as VPX sets it
            assert_eq!(f.ramp_up, 0.0);
        }
        assert_eq!(reports, ["flipper coilRampUp is missing"]);
    }

    #[test]
    fn a_file_that_isnt_one_changes_nothing() {
        let mut vpx = table();
        let before = (vpx.gamedata.gravity, vpx.gamedata.angle_tilt_min);
        assert!(apply(&mut vpx, "<physics><table/></physics>").is_err());
        assert!(apply(&mut vpx, "not xml").is_err());
        assert_eq!((vpx.gamedata.gravity, vpx.gamedata.angle_tilt_min), before);
        assert_eq!((sz2f("0.5"), sz2f(" 0.5"), sz2f("x")), (0.5, 0.0, 0.0));
    }

    #[test]
    fn a_tables_own_physics_written_as_vpx_writes_them_and_read_back() {
        let mut vpx = table();
        vpx.gamedata.gravity = 0.97 * GRAVITYCONST;
        let own = own(&vpx.gamedata, &vpx.gameitems).unwrap();
        // the flipper values a table may not hold: VPX's defaults
        assert_eq!((own[11], own[12], own[13]), (0.0, 0.75, 6.0));
        let text = write("A <table> & co", &own);
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<physics>
    <name>A &lt;table&gt; &amp; co</name>"));
        assert!(text.contains("        <gravityConstant>0.97</gravityConstant>
"));
        let (name, back) = read(&text).unwrap();
        assert_eq!(name.as_deref(), Some("A <table> & co"));
        for (a, b) in own.iter().zip(back) {
            assert!((a - b.unwrap()).abs() < 1e-5, "{a} {b:?}");
        }
        // a table with no flipper has none to export
        assert!(super::own(&VPX::default().gamedata, &[]).is_none());
        assert_eq!((f2sz(1.0), f2sz(3600.0), f2sz(0.058), f2sz(-0.5)), ("1.0".to_string(), "3600.0".to_string(), "0.058".to_string(), "-0.5".to_string()));
    }
}
