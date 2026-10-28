//! A table opened for the table inspector, the editor's first stage: the playfield's size and
//! each of its parts — its name, its kind, its outline seen from above (a wall's or a flasher's
//! drag points, a ramp's or a rubber's centre line and width, a flipper from its pivot to its tip,
//! a bumper's or a kicker's circle, a gate's or a spinner's bar, a primitive's place) — and every
//! setting it has, as the table reader writes them out. Read only: nothing in the table changes.
//! Ours.

use std::path::Path;

use serde::Serialize;
use vpin::vpx::gameitem::GameItemEnum;

/// A part's outline seen from above, in the table's own units.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Outline {
    /// A closed outline.
    Polygon { points: Vec<[f32; 2]> },
    /// A path with a width: a ramp, a rubber, a flipper, a gate.
    Path { points: Vec<[f32; 2]>, width: f32 },
    Circle { at: [f32; 2], radius: f32 },
    /// Not on the playfield (a timer, a text box on the backdrop).
    None,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub name: String,
    /// Its kind, as VPX's editor names it.
    pub kind: &'static str,
    pub outline: Outline,
    pub visible: bool,
    /// Its editor layer, where it has one.
    pub layer: Option<String>,
    /// What achievements can count of it: the events the player tells of it, as a pack names them
    /// (`hit`, `spin`, `slingshot`) — a part the table has its hits off for watched by the player
    /// where a pack names it.
    pub counts: Vec<&'static str>,
    /// Every setting it has, as the table reader writes them out.
    pub settings: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspected {
    /// The playfield's corners: left, top, right, bottom.
    pub bounds: [f32; 4],
    pub parts: Vec<Part>,
}

fn kind_of(item: &GameItemEnum) -> &'static str {
    match item {
        GameItemEnum::Wall(_) => "Wall",
        GameItemEnum::Flipper(_) => "Flipper",
        GameItemEnum::Timer(_) => "Timer",
        GameItemEnum::Plunger(_) => "Plunger",
        GameItemEnum::TextBox(_) => "Text box",
        GameItemEnum::Bumper(_) => "Bumper",
        GameItemEnum::Trigger(_) => "Trigger",
        GameItemEnum::Light(_) => "Light",
        GameItemEnum::Kicker(_) => "Kicker",
        GameItemEnum::Decal(_) => "Decal",
        GameItemEnum::Gate(_) => "Gate",
        GameItemEnum::Spinner(_) => "Spinner",
        GameItemEnum::Ramp(_) => "Ramp",
        GameItemEnum::Reel(_) => "Reel",
        GameItemEnum::LightSequencer(_) => "Light sequencer",
        GameItemEnum::Primitive(_) => "Primitive",
        GameItemEnum::Flasher(_) => "Flasher",
        GameItemEnum::Rubber(_) => "Rubber",
        GameItemEnum::HitTarget(_) => "Target",
        GameItemEnum::Ball(_) => "Ball",
        GameItemEnum::PartGroup(_) => "Part group",
        GameItemEnum::Generic(..) => "Other",
    }
}

/// A bar `length` long through `at`, turned `degrees` as VPX turns a gate or a spinner.
fn bar(at: [f32; 2], length: f32, degrees: f32) -> Vec<[f32; 2]> {
    let a = degrees.to_radians();
    let (dx, dy) = (a.cos() * length / 2.0, a.sin() * length / 2.0);
    vec![[at[0] - dx, at[1] - dy], [at[0] + dx, at[1] + dy]]
}

fn outline(item: &GameItemEnum) -> Outline {
    let points = |d: &[vpin::vpx::gameitem::dragpoint::DragPoint]| d.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>();
    match item {
        GameItemEnum::Wall(w) if w.drag_points.len() >= 3 => Outline::Polygon { points: points(&w.drag_points) },
        GameItemEnum::Flasher(f) if f.drag_points.len() >= 3 => Outline::Polygon { points: points(&f.drag_points) },
        GameItemEnum::Trigger(t) if t.drag_points.len() >= 3 => Outline::Polygon { points: points(&t.drag_points) },
        GameItemEnum::Trigger(t) => Outline::Circle { at: [t.center.x, t.center.y], radius: t.radius },
        GameItemEnum::Light(l) => Outline::Circle { at: [l.center.x, l.center.y], radius: l.mesh_radius.max(8.0) },
        GameItemEnum::Ramp(r) if r.drag_points.len() >= 2 => Outline::Path { points: points(&r.drag_points), width: r.width_bottom.max(10.0) },
        GameItemEnum::Rubber(r) if r.drag_points.len() >= 2 => Outline::Path { points: points(&r.drag_points), width: (r.thickness as f32).max(4.0) },
        GameItemEnum::Flipper(f) => {
            // VPX's flipper angle: 0 points up the table, clockwise
            let a = f.start_angle.to_radians();
            let tip = [f.center.x + a.sin() * f.flipper_radius_max, f.center.y - a.cos() * f.flipper_radius_max];
            Outline::Path { points: vec![[f.center.x, f.center.y], tip], width: f.base_radius * 2.0 }
        }
        GameItemEnum::Bumper(b) => Outline::Circle { at: [b.center.x, b.center.y], radius: b.radius },
        GameItemEnum::Kicker(k) => Outline::Circle { at: [k.center.x, k.center.y], radius: k.radius },
        GameItemEnum::Gate(g) => Outline::Path { points: bar([g.center.x, g.center.y], g.length, g.rotation), width: 6.0 },
        GameItemEnum::Spinner(s) => Outline::Path { points: bar([s.center.x, s.center.y], s.length, s.rotation), width: 8.0 },
        GameItemEnum::HitTarget(t) => Outline::Path { points: bar([t.position.x, t.position.y], t.size.x.max(10.0), t.rot_z), width: t.size.y.clamp(6.0, 40.0) },
        GameItemEnum::Plunger(p) => Outline::Path { points: vec![[p.center.x, p.center.y - p.height], [p.center.x, p.center.y]], width: p.width.max(10.0) },
        GameItemEnum::Primitive(p) => Outline::Circle { at: [p.position.x, p.position.y], radius: 10.0 },
        _ => Outline::None,
    }
}

/// The events the player tells of a part, as VPX fires them: a hit wherever the ball meets it (a
/// trigger rolled over, a kicker entered, a gate passed through) — where the table has its hits
/// off the player watches it itself, the table none the wiser, and a trigger or a kicker switched
/// off counts once the script switches it on — a spinner's spins, a wall's slingshots. Nothing of a
/// part the ball passes through, nor of a spinner held between two angles, which swings as a flap
/// and never spins.
fn counts(item: &GameItemEnum) -> Vec<&'static str> {
    match item {
        GameItemEnum::Wall(w) if w.is_collidable => {
            if w.drag_points.iter().any(|p| p.is_slingshot == Some(true)) { vec!["hit", "slingshot"] } else { vec!["hit"] }
        }
        GameItemEnum::Ramp(r) if r.is_collidable => vec!["hit"],
        GameItemEnum::Rubber(r) if r.is_collidable => vec!["hit"],
        GameItemEnum::Primitive(p) if p.is_collidable => vec!["hit"],
        GameItemEnum::Bumper(b) if b.is_collidable != Some(false) => vec!["hit"],
        GameItemEnum::HitTarget(t) if t.is_collidable => vec!["hit"],
        GameItemEnum::Trigger(_) | GameItemEnum::Kicker(_) => vec!["hit"],
        GameItemEnum::Gate(g) if g.is_collidable => vec!["hit"],
        GameItemEnum::Spinner(s) if s.angle_min == s.angle_max => vec!["spin"],
        _ => Vec::new(),
    }
}

/// The table at `path`, opened for the inspector.
pub fn inspect(path: &Path) -> Result<Inspected, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut vpx = vpin::vpx::VpxFile::open(file).map_err(|e| e.to_string())?;
    let data = vpx.read_gamedata().map_err(|e| e.to_string())?;
    let items = vpx.read_gameitems().map_err(|e| e.to_string())?;
    let parts = items
        .iter()
        .map(|item| {
            let mut settings = serde_json::to_value(item).unwrap_or(serde_json::Value::Null);
            // the setting's object inside the kind's name
            if let Some(obj) = settings.as_object_mut()
                && obj.len() == 1
            {
                settings = obj.values().next().cloned().unwrap_or_default();
            }
            Part {
                name: item.name().to_string(),
                kind: kind_of(item),
                outline: outline(item),
                visible: item.is_visible().unwrap_or(true),
                layer: item.editor_layer_name().clone(),
                counts: counts(item),
                settings,
            }
        })
        .collect();
    Ok(Inspected { bounds: [data.left, data.top, data.right, data.bottom], parts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gates_bar_turned_as_vpx_turns_it() {
        let b = bar([100.0, 100.0], 50.0, 90.0);
        assert!((b[0][0] - 100.0).abs() < 0.01 && (b[0][1] - 75.0).abs() < 0.01);
        assert!((b[1][1] - 125.0).abs() < 0.01);
    }
}
