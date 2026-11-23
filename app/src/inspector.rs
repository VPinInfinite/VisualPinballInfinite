//! The table inspector, the table editor's first stage: a table's parts as the library reads them
//! (`library::inspect`), each part's settings made plain for the page — a number rounded, a point
//! written out, a short list joined, a long text cut short, what doesn't fit on a line left out (a
//! wall's drag points: its outline shows them). Read only: nothing in the table changes.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

/// A part as the inspector shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartView {
    name: String,
    kind: &'static str,
    outline: library::inspect::Outline,
    visible: bool,
    layer: Option<String>,
    /// What achievements can count of it (`hit`, `spin`, `slingshot`).
    counts: Vec<&'static str>,
    /// Its settings as the page shows them: each one's name and value, in the table file's order.
    settings: Vec<(String, Value)>,
}

#[derive(Serialize)]
pub struct Inspected {
    /// The table file's name.
    file: String,
    /// The playfield's corners: left, top, right, bottom.
    bounds: [f32; 4],
    parts: Vec<PartView>,
}

/// The table at `path`, its parts' settings made plain.
pub fn open(path: &Path) -> Result<Inspected, String> {
    let got = library::inspect::inspect(path)?;
    Ok(Inspected {
        file: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        bounds: got.bounds,
        parts: got
            .parts
            .into_iter()
            .map(|p| PartView { name: p.name, kind: p.kind, outline: p.outline, visible: p.visible, layer: p.layer, counts: p.counts, settings: plain(&p.settings) })
            .collect(),
    })
}

/// A number with at most `places` decimals, a table file's single-precision noise gone (0.699999988
/// is 0.7).
fn rounded(n: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    let r = (n * f).round() / f;
    // never a "-0"
    if r == 0.0 { 0.0 } else { r }
}

/// A value as one line shows it, or None where it doesn't fit on one.
fn shown(v: &Value) -> Option<Value> {
    let number = |n: &serde_json::Number, places| match n.as_f64() {
        Some(f) if n.is_f64() => rounded(f, places).to_string(),
        _ => n.to_string(),
    };
    match v {
        Value::Null => None,
        Value::Number(n) if n.is_f64() => serde_json::Number::from_f64(rounded(n.as_f64()?, 2)).map(Value::Number),
        // a point: x, y and z written out
        Value::Object(o) if !o.is_empty() && o.keys().all(|k| matches!(k.as_str(), "x" | "y" | "z")) => {
            let at: Vec<String> = ["x", "y", "z"].iter().filter_map(|a| o.get(*a)).filter_map(|v| v.as_number().map(|n| number(n, 1))).collect();
            Some(Value::String(at.join(", ")))
        }
        Value::Object(_) => None,
        Value::Array(list) if list.len() <= 4 && list.iter().all(|x| !x.is_object() && !x.is_array()) => Some(Value::String(
            list.iter()
                .map(|x| match x {
                    Value::Number(n) => number(n, 2),
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(", "),
        )),
        Value::Array(_) => None,
        Value::String(s) if s.chars().count() > 80 => Some(Value::String(format!("{}…", s.chars().take(77).collect::<String>()))),
        other => Some(other.clone()),
    }
}

/// A part's settings as the page shows them, in the table file's order.
fn plain(settings: &Value) -> Vec<(String, Value)> {
    let Some(o) = settings.as_object() else { return Vec::new() };
    o.iter().filter(|(k, _)| !matches!(k.as_str(), "drag_points" | "name")).filter_map(|(k, v)| Some((k.clone(), shown(v)?))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_made_plain_for_a_line_each() {
        let got = plain(&json!({
            "name": "sw65",
            "elasticity": 0.699999988079071,
            "is_visible": true,
            "center": { "x": 120.25, "y": 1588.77 },
            "drag_points": [{ "x": 1.0, "y": 2.0 }],
            "colors": [1.5, 2.25],
            "image": "",
            "notes": "a".repeat(100),
            "sounds": [{ "a": 1 }],
            "timer": null,
        }));
        let get = |k: &str| got.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        assert_eq!(get("elasticity"), Some(json!(0.7)));
        assert_eq!(get("is_visible"), Some(json!(true)));
        assert_eq!(get("center"), Some(json!("120.3, 1588.8")));
        assert_eq!(get("colors"), Some(json!("1.5, 2.25")));
        assert_eq!(get("image"), Some(json!("")));
        assert_eq!(get("notes").and_then(|v| v.as_str().map(|s| s.chars().count())), Some(78));
        for gone in ["name", "drag_points", "sounds", "timer"] {
            assert!(get(gone).is_none(), "{gone}");
        }
    }
}
