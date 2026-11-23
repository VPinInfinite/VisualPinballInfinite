//! Lists the controllers connected — gamepads, joysticks, cabinet boards, Open Pinball Devices —
//! as the player sees them: each one's name, its name in the settings (`[Input]` mappings), and the
//! layout it gets;
//! then, for `--seconds N`, each press of a mapped button as the key the table's script hears.
//! No window. `cargo run --release -p player --example controllers [-- --seconds 10]`.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use player::input::InputSink;
use player::input_map::InputMap;
use player::settings::Settings;
use player::waker::Waker;
use sdl3_sys::everything::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seconds = args.iter().position(|a| a == "--seconds").and_then(|i| args.get(i + 1)).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let (tx, rx) = mpsc::channel();
    let settings = Settings::load();
    let sink = Arc::new(InputSink::new(tx, Arc::new(Waker::new()), InputMap::new(&settings)));
    // SAFETY: SDL started and used on this thread alone
    unsafe {
        if !SDL_Init(SDL_INIT_GAMEPAD) {
            eprintln!("SDL's controllers didn't start");
            return;
        }
        if !player::input::install(sink.clone()) {
            eprintln!("couldn't watch SDL's events");
            return;
        }
        let mut count = 0;
        let ids = SDL_GetJoysticks(&mut count);
        if count == 0 {
            println!("no controllers connected");
        }
        if !ids.is_null() {
            for k in 0..count.max(0) as usize {
                sink.joystick_added(*ids.add(k));
            }
            SDL_free(ids.cast());
        }
        // a board sending Open Pinball Device reports says so itself
        player::opd::start(sink.clone());
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(seconds) {
            let mut event: SDL_Event = std::mem::zeroed();
            while SDL_PollEvent(&mut event) {
                if event.r#type == SDL_EVENT_JOYSTICK_ADDED.0 {
                    sink.joystick_added(event.jdevice.which);
                } else if event.r#type == SDL_EVENT_JOYSTICK_REMOVED.0 {
                    sink.joystick_removed(event.jdevice.which);
                }
            }
            while let Ok(e) = rx.try_recv() {
                println!("key {:#04x} {}", e.dik, if e.down { "down" } else { "up" });
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        SDL_Quit();
    }
}
