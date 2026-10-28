//! Lists what a table puts on its backdrop besides its picture — text boxes, score reels and
//! lights — where each sits on VPX's 1000 × 750 grid and what decides how it's drawn:
//! `cargo run -p player --example backdrop -- table.vpx`. For finding what a spot at the top
//! of the desktop view is.

use vpin::vpx::gameitem::GameItemEnum;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: backdrop <table.vpx>");
        std::process::exit(2);
    };
    let vpx = match vpin::vpx::read(std::path::Path::new(&path)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("could not read {path}: {e}");
            std::process::exit(1);
        }
    };
    let g = &vpx.gamedata;
    println!("backdrop: desktop image \"{}\", colour {:?}, decals drawn {}, reels drawn {}", g.backglass_image_full_desktop, g.backdrop_color, g.render_decals, g.render_em_reels);
    for item in &vpx.gameitems {
        match item {
            GameItemEnum::TextBox(t) => println!(
                "textbox\t{}\t{:.0},{:.0} to {:.0},{:.0}\ttransparent {}\tdmd {:?}\tback {:?}\tfont {:?}\ttext {:?}",
                t.name, t.ver1.x, t.ver1.y, t.ver2.x, t.ver2.y, t.is_transparent, t.is_dmd, t.back_color, t.font_color, t.text
            ),
            GameItemEnum::Reel(r) => println!(
                "reel\t{}\tat {:.0},{:.0} size {:.0}x{:.0}\treels {}\timage \"{}\"\ttransparent {}\tvisible {}\tback {:?}",
                r.name, r.ver1.x, r.ver1.y, r.width, r.height, r.reel_count, r.image, r.is_transparent, r.is_visible, r.back_color
            ),
            GameItemEnum::Light(l) if l.is_backglass => println!("light\t{}\tat {:.0},{:.0}\timage \"{}\"", l.name, l.center.x, l.center.y, l.image),
            _ => {}
        }
    }
}
