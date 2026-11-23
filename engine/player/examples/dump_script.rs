//! Prints a table's script as the table file holds it, for reading what a table does:
//! `cargo run --release -p player --example dump_script -- table.vpx > script.vbs`.

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump_script table.vpx");
    let vpx = vpin::vpx::read(std::path::Path::new(&path)).expect("read the table");
    print!("{}", vpx.gamedata.code.string);
}
