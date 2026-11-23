// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its plugins' event
// stream, plugins/plugins/B2SPluginEventStream.cpp. Copyright (C) 2000-2026 Visual Pinball development team and
// contributors. This port is also GPL-3.0-or-later.

//! The game's event stream (VPX 5436's `B2SPluginEventStream`): what
//! happens in the game as a letter, a number and a value — the machine's
//! lamps (`L`), solenoids (`S`), lighting strings (`G`), the switches the
//! script set (`W`, 0 or 1), mechanisms (`N`) and segment digits (`D`),
//! polled 60 times a second, and, as they come, the script's B2S data
//! (`E`) and scores (`B`, `C`) and a colour file's triggers (`D`). Its
//! listeners are a table's PinUP pack and the cabinet's toys (DOF); each
//! hears every output's changes from its own first poll on, its first
//! value an event too, as VPX gives each plugin a stream of its own. A
//! PinUP pack also hears its own pictures of the score display matched,
//! which its stream alone looks for.

use crate::world::World;

/// When the stream next polls the machine.
#[derive(Default)]
pub struct EventStream {
    next_ms: f64,
}

/// The machine's outputs 60 times a second, to each listener (a frame's work):
/// its lamps, solenoids, lighting strings, the switches the script set, the
/// mechanisms it set up, and its segment displays' digits.
pub fn poll(w: &mut World, now: u64) {
    if w.pup.is_none() && w.dof.is_none() && w.toys_record.is_none() && !crate::achievements::active() {
        return;
    }
    if (now as f64) < w.events.next_ms {
        return;
    }
    w.events.next_ms = (w.events.next_ms + 1000.0 / 60.0).max(now as f64);
    let Some(c) = w.controller.as_mut() else { return };
    let switches: Vec<(char, i32, i32)> = c.state.switches.iter().map(|(&n, &on)| ('W', n, i32::from(on))).collect();
    let mechs: Vec<i32> = c.state.mechs.keys().copied().collect();
    let Some(m) = c.state.machine() else { return };
    // D: the score display's pictures, first, for a PinUP pack's own pictures of it
    if let Some(p) = w.pup.as_mut() {
        let mut frame = std::mem::take(&mut p.identify_buffer);
        if let Some((width, height, bits, number)) = m.dmd_identify(&mut frame) {
            p.pup.dmd_frame(width as usize, height as usize, bits, number, &frame);
        }
        p.identify_buffer = frame;
    }
    use pinmame_link::OutputKind;
    let (lamps, solenoids, gis) = m.output_counts();
    let mut states: Vec<(char, i32, i32)> = Vec::with_capacity((lamps + solenoids + gis) as usize + switches.len());
    for (kind, letter, count) in [(OutputKind::Lamp, 'L', lamps), (OutputKind::Solenoid, 'S', solenoids), (OutputKind::Gi, 'G', gis)] {
        for n in 0..=count as i32 {
            states.push((letter, n, m.output(kind, n)));
        }
    }
    states.extend(switches);
    states.extend(mechs.into_iter().map(|n| ('N', n, m.mech_position(n))));
    if let Some((layout, _)) = m.segments() {
        let digits = layout.digits().count() as i32;
        states.extend((0..digits).map(|d| ('D', d, m.output(OutputKind::Led, d))));
    }
    if let Some(p) = w.pup.as_mut() {
        p.pup.machine_states(&states);
    }
    if let Some(d) = w.dof.as_mut() {
        d.machine_states(&states);
    }
    if let Some(r) = w.toys_record.as_mut() {
        r.machine_states(&states);
    }
    crate::achievements::machine_states(&states);
}

/// An event as it comes (the script's B2S data and scores, a colour file's
/// triggers): to each listener.
pub fn queue(w: &mut World, kind: char, number: i32, value: i32) {
    // a script table's game, as its script tells the backglass, where a check asks for it
    crate::game_state::on_b2s(kind, number, value);
    if let Some(p) = w.pup.as_mut() {
        p.pup.queue_dof_event(kind, number, value);
    }
    if let Some(d) = w.dof.as_mut() {
        d.queue(kind, number, value);
    }
    if let Some(r) = w.toys_record.as_mut() {
        r.queue(kind, number, value);
    }
}
