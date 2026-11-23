//! Lists a table's solid parts — walls, primitives, targets, kickers, bumpers, gates, triggers,
//! spinners, rubbers and plungers: where each sits, how high, whether it's seen and whether the
//! ball hits it — for finding what a spot on the playfield is:
//! `cargo run --release -p player --example list_parts -- table.vpx`.

use vpin::vpx::gameitem::GameItemEnum;

fn main() {
    let path = std::env::args().nth(1).expect("usage: list_parts table.vpx");
    let vpx = vpin::vpx::read(std::path::Path::new(&path)).expect("read the table");
    println!("table {} x {} (left {} top {})", vpx.gamedata.right, vpx.gamedata.bottom, vpx.gamedata.left, vpx.gamedata.top);
    for item in &vpx.gameitems {
        match item {
            GameItemEnum::Wall(w) => {
                let xs = w.drag_points.iter().map(|p| p.x);
                let ys = w.drag_points.iter().map(|p| p.y);
                let (x0, x1) = (xs.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max));
                let (y0, y1) = (ys.clone().fold(f32::MAX, f32::min), ys.fold(f32::MIN, f32::max));
                println!(
                    "wall\t{}\tx {:.0}-{:.0}\ty {:.0}-{:.0}\theight {:.1}-{:.1}\tcollidable {}\ttop seen {}\tsides seen {}\tdroppable {}\timage {:?}\tside image {:?}\ttop material {:?}",
                    w.name, x0, x1, y0, y1, w.height_bottom, w.height_top, w.is_collidable, w.is_top_bottom_visible, w.is_side_visible, w.is_droppable, w.image, w.side_image, w.top_material
                );
            }
            GameItemEnum::Plunger(p) => println!(
                "plunger\t{}\tat {:.0},{:.0}\tmechanical {}\tauto {}\tvisible {}",
                p.name, p.center.x, p.center.y, p.is_mech_plunger, p.auto_plunger, p.is_visible
            ),
            GameItemEnum::HitTarget(t) => println!(
                "target\t{}\tat {:.0},{:.0},{:.0}\tvisible {}\tcollidable {}\tdropped {}",
                t.name, t.position.x, t.position.y, t.position.z, t.is_visible, t.is_collidable, t.is_dropped
            ),
            GameItemEnum::Kicker(k) => println!("kicker\t{}\tat {:.0},{:.0}\tradius {:.0}\tenabled {}", k.name, k.center.x, k.center.y, k.radius, k.is_enabled),
            GameItemEnum::Bumper(b) => println!("bumper\t{}\tat {:.0},{:.0}\tradius {:.0}", b.name, b.center.x, b.center.y, b.radius),
            GameItemEnum::Gate(g) => println!("gate\t{}\tat {:.0},{:.0}\tcollidable {}\tvisible {}", g.name, g.center.x, g.center.y, g.is_collidable, g.is_visible),
            GameItemEnum::Trigger(t) => println!("trigger\t{}\tat {:.0},{:.0}\tradius {:.0}\tenabled {}", t.name, t.center.x, t.center.y, t.radius, t.is_enabled),
            GameItemEnum::Spinner(s) => println!("spinner\t{}\tat {:.0},{:.0}\tvisible {}", s.name, s.center.x, s.center.y, s.is_visible),
            GameItemEnum::Rubber(r) => {
                let n = r.drag_points.len().max(1) as f32;
                let (cx, cy) = (r.drag_points.iter().map(|p| p.x).sum::<f32>() / n, r.drag_points.iter().map(|p| p.y).sum::<f32>() / n);
                println!("rubber\t{}\tat {:.0},{:.0}\theight {:.0}\tcollidable {}\tvisible {}", r.name, cx, cy, r.height, r.is_collidable, r.is_visible)
            }
            GameItemEnum::Primitive(p) => println!(
                "primitive\t{}\tat {:.0},{:.0},{:.0}\tsize {:.1},{:.1},{:.1}\tvisible {}\tcollidable {}\ttoy {}\timage {:?}",
                p.name, p.position.x, p.position.y, p.position.z, p.size.x, p.size.y, p.size.z, p.is_visible, p.is_collidable, p.is_toy, p.image
            ),
            _ => {}
        }
    }
}
