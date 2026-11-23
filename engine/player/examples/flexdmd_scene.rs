//! Runs a FlexDMD scene script through our FlexDMD and saves the picture it makes:
//! `cargo run --release -p player --example flexdmd_scene -- scene.vbs out.txt [steps]`. The
//! script's `Setup()` makes the FlexDMD (`CreateObject("FlexDMD.FlexDMD")`), builds its scene,
//! runs it and returns it; the FlexDMD then draws `steps` frames (10 by default) and its colours
//! (`DmdColoredPixels`, red in the lowest byte) are written as text: width and height, then a
//! row of hex values a line. `tools/flexdmd-oracle` runs the same script through the installed
//! FlexDMD and compares.

use player::script::com::VT_DISPATCH;
use player::script::host::{Item, ScriptHost};
use player::script::object::{Obj, Target};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: flexdmd_scene <scene.vbs> <out.txt> [steps]");
        std::process::exit(2);
    }
    let steps: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
    let code = std::fs::read_to_string(&args[0]).expect("read the scene script");
    let fence = Obj::new(Target::Fence, &player::script::fence::I_CREATE_OBJECT);
    let mut host = ScriptHost::new(&[Item { name: "CreateObject".into(), obj: fence, global: false }]).expect("a script engine");
    host.load(&code).expect("the scene script compiles");
    let v = host.evaluate("Setup()").expect("Setup() runs");
    assert_eq!(v.vt, VT_DISPATCH, "Setup() returns the FlexDMD");
    // SAFETY: the object the script handed back, alive while the engine is
    let flex = unsafe { player::script::flexdmd::flex_of(v.ptr()) }.expect("Setup() returns our FlexDMD");
    flex.borrow_mut().ask_colored();
    for i in 0..steps {
        player::flexdmd::advance(&flex, f64::from(i) * 1000.0 / 60.0 + 0.01);
    }
    let f = flex.borrow();
    // a scene may report what it found through the game name
    if !f.game_name.is_empty() {
        println!("{}", f.game_name);
    }
    let (w, h) = (usize::from(f.width), usize::from(f.height));
    let mut out = format!("{w} {h}\n");
    for row in f.colored.chunks(w).take(h) {
        out.push_str(&row.iter().map(|c| format!("{c:06X}")).collect::<Vec<_>>().join(" "));
        out.push('\n');
    }
    std::fs::write(&args[1], out).expect("write the picture");
}
