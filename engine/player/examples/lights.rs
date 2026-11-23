//! Prints a table's lights, for checking how the player draws them:
//! `cargo run -p player --example lights -- table.vpx`.

use vpin::vpx::gameitem::GameItemEnum;

fn main() {
    let path = std::env::args().nth(1).expect("usage: lights <table.vpx>");
    let vpx = vpin::vpx::read(std::path::Path::new(&path)).expect("read the table");
    for (index, item) in vpx.gameitems.iter().enumerate() {
        if let GameItemEnum::Light(l) = item {
            println!(
                "{:<12} #{} at ({:.0}, {:.0}) state {:?}/{} bulb {} show_mesh {} falloff {:.0} power {:.2} intensity {:.1} color {:?} color2 {:?} image {:?} visible {:?} halo {:.0} surface {:?} transmission {:.2} modulate_vs_add {:.4} fader {:?} fade {:.3}/{:.3} blink {:?}/{} shape {} height {:?} depth_bias {:.1}",
                l.name,
                index,
                l.center.x,
                l.center.y,
                l.state,
                l.state_u32,
                l.is_bulb_light,
                l.show_bulb_mesh,
                l.falloff_radius,
                l.falloff_power,
                l.intensity,
                (l.color.r, l.color.g, l.color.b),
                (l.color2.r, l.color2.g, l.color2.b),
                l.image,
                l.visible,
                l.bulb_halo_height,
                l.surface,
                l.transmission_scale,
                l.bulb_modulate_vs_add,
                l.fader,
                l.fade_speed_up,
                l.fade_speed_down,
                l.blink_pattern,
                l.blink_interval,
                shape(&l.drag_points),
                l.height,
                l.depth_bias
            );
        }
    }
}

/// The light's outline, which bounds its glow: how many points and how far they reach.
fn shape(points: &[vpin::vpx::gameitem::dragpoint::DragPoint]) -> String {
    if points.is_empty() {
        return "none".into();
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    format!("{} points x {x0:.0}-{x1:.0} y {y0:.0}-{y1:.0}", points.len())
}
