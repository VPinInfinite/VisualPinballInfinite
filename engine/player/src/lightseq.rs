// Ported from Visual Pinball X: src/parts/lightseq.cpp and lightseq.h (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's light sequencer: light shows played over a collection's lights,
//! flashers and primitives — lines sweeping across the table, circles,
//! radar sweeps, blinking, random flicker. Each member's place is put on a
//! grid of 20-unit cells; a tracer walks the grid a step per update,
//! switching what it crosses on (a tail tracer, following, switches it off).
//! While a show plays, the members it touched ignore their own state; when
//! the queue of shows runs dry they get it back and the script hears
//! `<name>_PlayDone`.

use physics_compat::math::Mwc64x;
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::gameitem::lightsequencer::LightSequencer;

use crate::world::Part;

/// VPX's `LIGHTSEQGRIDSCALE`: grid cells are 20 table units wide.
const GRID_SCALE: i32 = 20;
/// VPX's `LIGHTSEQQUEUESIZE`.
const QUEUE_SIZE: usize = 100;
/// VPX's `EDITOR_BG_WIDTH`.
const EDITOR_BG_WIDTH: f32 = 1000.0;

// VPX's LightState
const OFF: i32 = 0;
const ON: i32 = 1;
const BLINKING: i32 = 2;

// VPX's SequencerState (vpinball.idl)
const SEQ_UP_ON: i32 = 1;
const SEQ_UP_OFF: i32 = 2;
const SEQ_DOWN_ON: i32 = 3;
const SEQ_DOWN_OFF: i32 = 4;
const SEQ_RIGHT_ON: i32 = 5;
const SEQ_RIGHT_OFF: i32 = 6;
const SEQ_LEFT_ON: i32 = 7;
const SEQ_LEFT_OFF: i32 = 8;
const SEQ_DIAG_UP_RIGHT_ON: i32 = 9;
const SEQ_DIAG_UP_RIGHT_OFF: i32 = 10;
const SEQ_DIAG_UP_LEFT_ON: i32 = 11;
const SEQ_DIAG_UP_LEFT_OFF: i32 = 12;
const SEQ_DIAG_DOWN_RIGHT_ON: i32 = 13;
const SEQ_DIAG_DOWN_RIGHT_OFF: i32 = 14;
const SEQ_DIAG_DOWN_LEFT_ON: i32 = 15;
const SEQ_DIAG_DOWN_LEFT_OFF: i32 = 16;
const SEQ_MIDDLE_OUT_HORIZ_ON: i32 = 17;
const SEQ_MIDDLE_OUT_HORIZ_OFF: i32 = 18;
const SEQ_MIDDLE_IN_HORIZ_ON: i32 = 19;
const SEQ_MIDDLE_IN_HORIZ_OFF: i32 = 20;
const SEQ_MIDDLE_OUT_VERT_ON: i32 = 21;
const SEQ_MIDDLE_OUT_VERT_OFF: i32 = 22;
const SEQ_MIDDLE_IN_VERT_ON: i32 = 23;
const SEQ_MIDDLE_IN_VERT_OFF: i32 = 24;
const SEQ_STRIPE1_HORIZ_ON: i32 = 25;
const SEQ_STRIPE1_HORIZ_OFF: i32 = 26;
const SEQ_STRIPE2_HORIZ_ON: i32 = 27;
const SEQ_STRIPE2_HORIZ_OFF: i32 = 28;
const SEQ_STRIPE1_VERT_ON: i32 = 29;
const SEQ_STRIPE1_VERT_OFF: i32 = 30;
const SEQ_STRIPE2_VERT_ON: i32 = 31;
const SEQ_STRIPE2_VERT_OFF: i32 = 32;
const SEQ_HATCH1_HORIZ_ON: i32 = 33;
const SEQ_HATCH1_HORIZ_OFF: i32 = 34;
const SEQ_HATCH2_HORIZ_ON: i32 = 35;
const SEQ_HATCH2_HORIZ_OFF: i32 = 36;
const SEQ_HATCH1_VERT_ON: i32 = 37;
const SEQ_HATCH1_VERT_OFF: i32 = 38;
const SEQ_HATCH2_VERT_ON: i32 = 39;
const SEQ_HATCH2_VERT_OFF: i32 = 40;
const SEQ_CIRCLE_OUT_ON: i32 = 41;
const SEQ_CIRCLE_OUT_OFF: i32 = 42;
const SEQ_CIRCLE_IN_ON: i32 = 43;
const SEQ_CIRCLE_IN_OFF: i32 = 44;
const SEQ_CLOCK_RIGHT_ON: i32 = 45;
const SEQ_CLOCK_RIGHT_OFF: i32 = 46;
const SEQ_CLOCK_LEFT_ON: i32 = 47;
const SEQ_CLOCK_LEFT_OFF: i32 = 48;
const SEQ_RADAR_RIGHT_ON: i32 = 49;
const SEQ_RADAR_RIGHT_OFF: i32 = 50;
const SEQ_RADAR_LEFT_ON: i32 = 51;
const SEQ_RADAR_LEFT_OFF: i32 = 52;
const SEQ_WIPER_RIGHT_ON: i32 = 53;
const SEQ_WIPER_RIGHT_OFF: i32 = 54;
const SEQ_WIPER_LEFT_ON: i32 = 55;
const SEQ_WIPER_LEFT_OFF: i32 = 56;
const SEQ_FAN_LEFT_UP_ON: i32 = 57;
const SEQ_FAN_LEFT_UP_OFF: i32 = 58;
const SEQ_FAN_LEFT_DOWN_ON: i32 = 59;
const SEQ_FAN_LEFT_DOWN_OFF: i32 = 60;
const SEQ_FAN_RIGHT_UP_ON: i32 = 61;
const SEQ_FAN_RIGHT_UP_OFF: i32 = 62;
const SEQ_FAN_RIGHT_DOWN_ON: i32 = 63;
const SEQ_FAN_RIGHT_DOWN_OFF: i32 = 64;
const SEQ_ARC_BOTTOM_LEFT_UP_ON: i32 = 65;
const SEQ_ARC_BOTTOM_LEFT_UP_OFF: i32 = 66;
const SEQ_ARC_BOTTOM_LEFT_DOWN_ON: i32 = 67;
const SEQ_ARC_BOTTOM_LEFT_DOWN_OFF: i32 = 68;
const SEQ_ARC_BOTTOM_RIGHT_UP_ON: i32 = 69;
const SEQ_ARC_BOTTOM_RIGHT_UP_OFF: i32 = 70;
const SEQ_ARC_BOTTOM_RIGHT_DOWN_ON: i32 = 71;
const SEQ_ARC_BOTTOM_RIGHT_DOWN_OFF: i32 = 72;
const SEQ_ARC_TOP_LEFT_UP_ON: i32 = 73;
const SEQ_ARC_TOP_LEFT_UP_OFF: i32 = 74;
const SEQ_ARC_TOP_LEFT_DOWN_ON: i32 = 75;
const SEQ_ARC_TOP_LEFT_DOWN_OFF: i32 = 76;
const SEQ_ARC_TOP_RIGHT_UP_ON: i32 = 77;
const SEQ_ARC_TOP_RIGHT_UP_OFF: i32 = 78;
const SEQ_ARC_TOP_RIGHT_DOWN_ON: i32 = 79;
const SEQ_ARC_TOP_RIGHT_DOWN_OFF: i32 = 80;
const SEQ_SCREW_RIGHT_ON: i32 = 81;
const SEQ_SCREW_RIGHT_OFF: i32 = 82;
const SEQ_SCREW_LEFT_ON: i32 = 83;
const SEQ_SCREW_LEFT_OFF: i32 = 84;
const SEQ_ALL_OFF: i32 = 1000;
const SEQ_ALL_ON: i32 = 1001;
const SEQ_BLINKING: i32 = 1002;
const SEQ_RANDOM: i32 = 1003;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Kind {
    #[default]
    Null,
    Blink,
    Random,
    Line,
    Circle,
    Radar,
}

/// VPX's `_tracer`.
#[derive(Clone, Copy, Debug, Default)]
struct Tracer {
    kind: Kind,
    delay: i32,
    length: i32,
    frame_count: i32,
    /// Current grid coordinates, the step to the next frame's, and the step
    /// between the points processed this frame.
    x: f32,
    step_x: f32,
    process_step_x: f32,
    y: f32,
    step_y: f32,
    process_step_y: f32,
    radius: f32,
    step_radius: f32,
    process_radius_neg: bool,
    angle: f32,
    step_angle: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Queued {
    animation: i32,
    tail_length: i32,
    repeat: i32,
    pause: i32,
    update_rate: i32,
}

/// A member of the sequencer's collection, as it treats it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Element {
    Light(usize),
    Flasher(usize),
    Primitive(usize),
    Other,
}

/// A light sequencer in play (VPX's `LightSeq`).
#[derive(Clone, Debug)]
pub struct LightSeqRt {
    /// The collection's name, as the script reads it; the one played is the
    /// one found when play started.
    pub collection: String,
    center: [f32; 2],
    update_interval: i32,
    time_next_update: u32,
    grid_x_center: f32,
    grid_y_center: f32,
    grid_width: i32,
    grid_height: i32,
    grid_x_center_adjust: i32,
    grid_y_center_adjust: i32,
    th1: Tracer,
    th2: Tracer,
    tt1: Tracer,
    tt2: Tracer,
    /// The collection's members (VPX's `m_pcollection`), or none when no
    /// collection has the name.
    elements: Option<Vec<Element>>,
    /// Each cell's member, plus 1 (0: none).
    grid: Vec<i16>,
    queue: [Queued; QUEUE_SIZE],
    head: usize,
    tail: usize,
    play_animation: i32,
    tail_length: i32,
    pause_value: i32,
    replay_count: i32,
    update_rate: i32,
    play_in_progress: bool,
    pause_in_progress: bool,
}

impl LightSeqRt {
    /// VPX's `RenderSetup`: finds the collection (by its exact name) and
    /// places its lights, flashers and primitives on the grid.
    /// `collections` are each collection's name and parts, in order.
    pub fn new(seq: &LightSequencer, vpx: &VPX, collections: &[(&str, &[u32])]) -> LightSeqRt {
        let mut s = LightSeqRt {
            collection: seq.collection.clone(),
            center: [seq.pos_x, seq.pos_y],
            update_interval: seq.update_interval as i32,
            time_next_update: 0,
            grid_x_center: 0.0,
            grid_y_center: 0.0,
            grid_width: 0,
            grid_height: 0,
            grid_x_center_adjust: 0,
            grid_y_center_adjust: 0,
            th1: Tracer::default(),
            th2: Tracer::default(),
            tt1: Tracer::default(),
            tt2: Tracer::default(),
            elements: None,
            grid: Vec::new(),
            queue: [Queued::default(); QUEUE_SIZE],
            head: 0,
            tail: 0,
            play_animation: 0,
            tail_length: 0,
            pause_value: 0,
            replay_count: 0,
            update_rate: 0,
            play_in_progress: false,
            pause_in_progress: false,
        };
        let Some(&(_, members)) = collections.iter().find(|(name, _)| *name == seq.collection) else { return s };

        // the grid's size, from the table's
        let g = &vpx.gamedata;
        let (table_width, table_height) = (g.right - g.left, g.bottom - g.top);
        s.grid_width = table_width as i32 / GRID_SCALE;
        s.grid_height = table_height as i32 / GRID_SCALE;

        // the centre point of the grid, for effects which start from the center
        let scale = (1.0 / f64::from(GRID_SCALE)) as f32;
        s.grid_x_center = (s.center[0] * scale).floor();
        s.grid_y_center = (s.center[1] * scale).floor();
        s.grid_x_center_adjust = (s.grid_width / 2 - s.grid_x_center as i32).abs();
        s.grid_y_center_adjust = (s.grid_height / 2 - s.grid_y_center as i32).abs();

        s.grid = vec![0; (s.grid_width.max(0) * s.grid_height.max(0)) as usize];
        let mut elements = Vec::with_capacity(members.len());
        for (i, &p) in members.iter().enumerate() {
            let (element, at) = match &vpx.gameitems[p as usize] {
                // a light on the backdrop has its y scaled up
                GameItemEnum::Light(l) => (Element::Light(p as usize), Some((l.center.x, if l.is_backglass { l.center.y * 2.666 } else { l.center.y }))),
                GameItemEnum::Flasher(f) => (Element::Flasher(p as usize), Some((f.pos_x.unwrap_or(0.0), f.pos_y.unwrap_or(0.0)))),
                GameItemEnum::Primitive(pr) => (Element::Primitive(p as usize), Some((pr.position.x, pr.position.y))),
                _ => (Element::Other, None),
            };
            elements.push(element);
            let Some((x, y)) = at else { continue };
            // scale down to suit the size of the grid; off the table wraps out of range, as in VPX
            let ix = (x * scale) as i32 as u32;
            let iy = (y * scale) as i32 as u32;
            if ix < s.grid_width as u32 && iy < s.grid_height as u32 {
                s.grid[(iy * s.grid_width as u32 + ix) as usize] = (i + 1) as i16;
            }
        }
        s.elements = Some(elements);
        s
    }

    pub fn center_x(&self) -> f32 {
        self.center[0]
    }

    pub fn center_y(&self) -> f32 {
        self.center[1]
    }

    /// `CenterX`: false (VPX's `E_FAIL`) off the backdrop's width.
    pub fn put_center_x(&mut self, x: f32) -> bool {
        if !(0.0..EDITOR_BG_WIDTH).contains(&x) {
            return false;
        }
        self.center[0] = x;
        self.grid_x_center = (x * (1.0 / f64::from(GRID_SCALE)) as f32).floor();
        self.grid_x_center_adjust = (self.grid_width / 2 - self.grid_x_center as i32).abs();
        true
    }

    /// `CenterY`: false (VPX's `E_FAIL`) beyond twice the backdrop's width.
    pub fn put_center_y(&mut self, y: f32) -> bool {
        if !(0.0..2.0 * EDITOR_BG_WIDTH).contains(&y) {
            return false;
        }
        self.center[1] = y;
        self.grid_y_center = (y * (1.0 / f64::from(GRID_SCALE)) as f32).floor();
        self.grid_y_center_adjust = (self.grid_height / 2 - self.grid_y_center as i32).abs();
        true
    }

    pub fn update_interval(&self) -> i32 {
        self.update_interval
    }

    pub fn put_update_interval(&mut self, interval: i32) {
        self.update_interval = interval.max(1);
    }

    /// `Play`: all on and all off happen at once; anything else joins the
    /// queue. False (VPX's `E_FAIL`) when the queue is full.
    pub fn play(&mut self, animation: i32, tail_length: i32, repeat: i32, pause: i32, parts: &mut [Part], now: u32) -> bool {
        // sanity check the parameters
        let tail_length = tail_length.max(0);
        let repeat = if repeat <= 0 { 1 } else { repeat };
        let pause = pause.max(0);
        match animation {
            SEQ_ALL_ON => self.set_all_lights_to_state(ON, parts, now),
            SEQ_ALL_OFF => self.set_all_lights_to_state(OFF, parts, now),
            _ => {
                // the queue is circular
                let new_head = if self.head + 1 >= QUEUE_SIZE { 0 } else { self.head + 1 };
                if new_head == self.tail {
                    return false;
                }
                self.queue[self.head] = Queued { animation, tail_length, repeat, pause, update_rate: self.update_interval };
                self.head = new_head;
            }
        }
        true
    }

    /// `StopPlay`: ends the show, empties the queue and gives the members
    /// back their own state.
    pub fn stop_play(&mut self, parts: &mut [Part], now: u32) {
        self.play_in_progress = false;
        self.pause_in_progress = false;
        self.th1.kind = Kind::Null;
        self.th2.kind = Kind::Null;
        self.tt1.kind = Kind::Null;
        self.tt2.kind = Kind::Null;
        self.head = 0;
        self.tail = 0;
        for &e in self.elements.iter().flatten() {
            match e {
                Element::Light(p) => {
                    if let Some(l) = &mut parts[p].light {
                        let state = l.get_state();
                        l.locked_by_ls = false;
                        l.put_state(state, now);
                    }
                }
                Element::Flasher(p) | Element::Primitive(p) => parts[p].ls_locked = false,
                Element::Other => {}
            }
        }
    }

    /// VPX's `UpdateAnimation`, once a frame: plays the show's steps that
    /// are due. True when the queue ran dry, for `PlayDone`.
    pub fn update(&mut self, now: u32, parts: &mut [Part], rng: &mut Mwc64x) -> bool {
        let mut done = false;
        if self.play_in_progress {
            while now >= self.time_next_update && self.play_in_progress {
                if !self.pause_in_progress {
                    self.time_next_update = self.time_next_update.wrapping_add(self.update_rate as u32);
                    // the head tracers, then the tails
                    let th1 = self.process(Which::Th1, ON, now, parts, rng);
                    let th2 = self.process(Which::Th2, ON, now, parts, rng);
                    let tt1 = self.process(Which::Tt1, OFF, now, parts, rng);
                    let tt2 = self.process(Which::Tt2, OFF, now, parts, rng);
                    if th1 && th2 && tt1 && tt2 {
                        // the sequence has finished: pause for the given time or one interval
                        self.time_next_update = now.wrapping_add(self.update_rate.max(self.pause_value) as u32);
                        self.pause_in_progress = true;
                    }
                } else {
                    self.replay_count -= 1;
                    if self.replay_count != 0 {
                        self.setup_tracers(self.play_animation, self.tail_length, self.replay_count, self.pause_value, now);
                    } else {
                        // on to the next in the queue
                        self.tail += 1;
                        if self.tail >= QUEUE_SIZE {
                            self.tail = 0;
                        }
                        self.play_in_progress = false;
                        // nothing left: the lights get their own state back, and the script hears of it
                        if self.head == self.tail {
                            self.stop_play(parts, now);
                            done = true;
                        }
                    }
                }
            }
        } else if self.head != self.tail {
            let q = self.queue[self.tail];
            self.update_rate = q.update_rate;
            self.setup_tracers(q.animation, q.tail_length, q.repeat, q.pause, now);
        }
        done
    }

    /// VPX's `SetupTracers`: where each tracer starts, how it moves and for
    /// how many steps.
    fn setup_tracers(&mut self, animation: i32, mut tail_length: i32, mut repeat: i32, mut pause: i32, now: u32) {
        let mut inverse = false;

        self.play_in_progress = false;
        self.pause_in_progress = false;
        self.th1.kind = Kind::Null;
        self.th2.kind = Kind::Null;
        self.tt1.kind = Kind::Null;
        self.tt2.kind = Kind::Null;

        // remember the current sequence (before it gets altered)
        self.play_animation = animation;
        self.tail_length = tail_length;
        self.replay_count = repeat;
        self.pause_value = pause;

        let (w, h) = (self.grid_width, self.grid_height);
        let (wf, hf) = (w as f32, h as f32);
        let (xc, yc) = (self.grid_x_center, self.grid_y_center);
        let (xa, ya) = (self.grid_x_center_adjust, self.grid_y_center_adjust);
        let line = |x: f32, step_x: f32, process_step_x: f32, y: f32, step_y: f32, process_step_y: f32, length: i32, frame_count: i32| Tracer {
            kind: Kind::Line,
            x,
            step_x,
            process_step_x,
            y,
            step_y,
            process_step_y,
            length,
            frame_count,
            ..Tracer::default()
        };
        let radar = |angle: f32, step_angle: f32, process_radius_neg: bool, x: f32, y: f32, length: i32, frame_count: i32| Tracer {
            kind: Kind::Radar,
            angle,
            step_angle,
            process_radius_neg,
            x,
            y,
            length,
            frame_count,
            ..Tracer::default()
        };
        // VPX sets only some of a tracer's fields; the rest keep what they had
        let set = |t: &mut Tracer, new: Tracer| {
            let delay = t.delay;
            let kept = *t;
            *t = new;
            t.delay = delay;
            match new.kind {
                Kind::Line => {
                    t.radius = kept.radius;
                    t.step_radius = kept.step_radius;
                    t.process_radius_neg = kept.process_radius_neg;
                    t.angle = kept.angle;
                    t.step_angle = kept.step_angle;
                }
                Kind::Radar => {
                    t.step_x = kept.step_x;
                    t.process_step_x = kept.process_step_x;
                    t.step_y = kept.step_y;
                    t.process_step_y = kept.process_step_y;
                    t.radius = kept.radius;
                    t.step_radius = kept.step_radius;
                }
                _ => {}
            }
        };

        if animation % 2 == 0 && (SEQ_UP_OFF..=SEQ_SCREW_LEFT_OFF).contains(&animation) {
            inverse = true;
        }
        match animation {
            // blink all lights on and off
            SEQ_BLINKING => {
                self.th1.kind = Kind::Blink;
                self.th1.length = tail_length;
                tail_length = 0;
                self.th1.frame_count = 2;
            }
            // randomly turn lights on and off for the pause period
            SEQ_RANDOM => {
                self.th1.kind = Kind::Random;
                self.th1.length = tail_length;
                // VPX divides by the update rate as is
                self.th1.frame_count = pause.checked_div(self.update_rate).unwrap_or(0);
                // no repeat or pause for this effect and certainly no tail
                tail_length = 0;
                repeat = 1;
                pause = 0;
                self.tail_length = tail_length;
                self.replay_count = repeat;
                self.pause_value = pause;
            }
            SEQ_UP_ON | SEQ_UP_OFF => set(&mut self.th1, line(0.0, 0.0, 1.0, hf - 1.0, -1.0, 0.0, w, h)),
            SEQ_DOWN_ON | SEQ_DOWN_OFF => set(&mut self.th1, line(0.0, 0.0, 1.0, 0.0, 1.0, 0.0, w, h)),
            SEQ_RIGHT_ON | SEQ_RIGHT_OFF => {
                set(&mut self.th1, line(0.0, 1.0, 0.0, 0.0, 0.0, 1.0, h, w));
                tail_length /= 2;
            }
            SEQ_LEFT_ON | SEQ_LEFT_OFF => {
                set(&mut self.th1, line(wf - 1.0, -1.0, 0.0, 0.0, 0.0, 1.0, h, w));
                tail_length /= 2;
            }
            SEQ_DIAG_UP_RIGHT_ON | SEQ_DIAG_UP_RIGHT_OFF => set(&mut self.th1, line(0.0, 0.0, 0.5, hf - 1.0, -1.0, 1.0, w * 2, h + w * 2)),
            SEQ_DIAG_UP_LEFT_ON | SEQ_DIAG_UP_LEFT_OFF => set(&mut self.th1, line(wf - 1.0, 0.0, -0.5, hf - 1.0, -1.0, 1.0, w * 2, h + w * 2)),
            SEQ_DIAG_DOWN_RIGHT_ON | SEQ_DIAG_DOWN_RIGHT_OFF => set(&mut self.th1, line(0.0, 0.0, 0.5, 0.0, 1.0, -1.0, w * 2, h + w * 2)),
            SEQ_DIAG_DOWN_LEFT_ON | SEQ_DIAG_DOWN_LEFT_OFF => set(&mut self.th1, line(wf - 1.0, 0.0, -0.5, 0.0, 1.0, -1.0, w * 2, h + w * 2)),
            SEQ_MIDDLE_OUT_HORIZ_ON | SEQ_MIDDLE_OUT_HORIZ_OFF => {
                set(&mut self.th1, line(xc - 1.0, -1.0, 0.0, 0.0, 0.0, 1.0, h, w / 2 + xa));
                set(&mut self.th2, line(xc, 1.0, 0.0, 0.0, 0.0, 1.0, h, w / 2 + xa));
                tail_length /= 4;
            }
            SEQ_MIDDLE_IN_HORIZ_ON | SEQ_MIDDLE_IN_HORIZ_OFF => {
                let effect_length = (w / 2 + xa) as f32;
                set(&mut self.th1, line(xc - effect_length, 1.0, 0.0, 0.0, 0.0, 1.0, h, effect_length as i32 + 1));
                set(&mut self.th2, line(xc + effect_length, -1.0, 0.0, 0.0, 0.0, 1.0, h, effect_length as i32));
                tail_length /= 4;
            }
            SEQ_MIDDLE_OUT_VERT_ON | SEQ_MIDDLE_OUT_VERT_OFF => {
                set(&mut self.th1, line(0.0, 0.0, 1.0, yc - 1.0, -1.0, 0.0, w, h / 2 + ya));
                set(&mut self.th2, line(0.0, 0.0, 1.0, yc, 1.0, 0.0, w, h / 2 + ya));
                tail_length /= 2;
            }
            SEQ_MIDDLE_IN_VERT_ON | SEQ_MIDDLE_IN_VERT_OFF => {
                let effect_length = (h / 2 + ya) as f32;
                set(&mut self.th1, line(0.0, 0.0, 1.0, yc - effect_length, 1.0, 0.0, w, effect_length as i32 + 1));
                set(&mut self.th2, line(0.0, 0.0, 1.0, yc + effect_length, -1.0, 0.0, w, effect_length as i32));
                tail_length /= 2;
            }
            // top half wipes on to the right while the bottom half wipes on to the left
            SEQ_STRIPE1_HORIZ_ON | SEQ_STRIPE1_HORIZ_OFF => {
                set(&mut self.th1, line(0.0, 1.0, 0.0, yc - 1.0, 0.0, -1.0, h / 2 + ya, w));
                // VPX uses the x adjustment here
                set(&mut self.th2, line(wf - 1.0, -1.0, 0.0, yc, 0.0, 1.0, h / 2 + xa, w));
                tail_length /= 2;
            }
            SEQ_STRIPE2_HORIZ_ON | SEQ_STRIPE2_HORIZ_OFF => {
                set(&mut self.th1, line(wf - 1.0, -1.0, 0.0, yc - 1.0, 0.0, -1.0, h / 2 + ya, w));
                set(&mut self.th2, line(0.0, 1.0, 0.0, yc, 0.0, 1.0, h / 2 + ya, w));
                tail_length /= 2;
            }
            // left side wipes on going up while the right side wipes on going down
            SEQ_STRIPE1_VERT_ON | SEQ_STRIPE1_VERT_OFF => {
                set(&mut self.th1, line(xc - 1.0, 0.0, -1.0, hf - 1.0, -1.0, 0.0, w / 2 + xa, h));
                set(&mut self.th2, line(xc, 0.0, 1.0, 0.0, 1.0, 0.0, w / 2 + xa, h));
            }
            SEQ_STRIPE2_VERT_ON | SEQ_STRIPE2_VERT_OFF => {
                set(&mut self.th1, line(xc - 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, w / 2 + xa, h));
                set(&mut self.th2, line(xc, 0.0, 1.0, hf - 1.0, -1.0, 0.0, w / 2 + xa, h));
            }
            // cross-hatch: even lines one way, odd lines the other
            SEQ_HATCH1_HORIZ_ON | SEQ_HATCH1_HORIZ_OFF => {
                set(&mut self.th1, line(0.0, 1.0, 0.0, 0.0, 0.0, 2.0, h / 2, w));
                set(&mut self.th2, line(wf - 1.0, -1.0, 0.0, 1.0, 0.0, 2.0, h / 2, w));
                tail_length /= 2;
            }
            SEQ_HATCH2_HORIZ_ON | SEQ_HATCH2_HORIZ_OFF => {
                set(&mut self.th1, line(wf - 1.0, -1.0, 0.0, 0.0, 0.0, 2.0, h / 2, w));
                set(&mut self.th2, line(0.0, 1.0, 0.0, 1.0, 0.0, 2.0, h / 2, w));
                tail_length /= 2;
            }
            SEQ_HATCH1_VERT_ON | SEQ_HATCH1_VERT_OFF => {
                set(&mut self.th1, line(0.0, 0.0, 2.0, hf - 1.0, -1.0, 0.0, w / 2, h));
                set(&mut self.th2, line(1.0, 0.0, 2.0, 0.0, 1.0, 0.0, w / 2, h));
            }
            SEQ_HATCH2_VERT_ON | SEQ_HATCH2_VERT_OFF => {
                set(&mut self.th1, line(0.0, 0.0, 2.0, 0.0, 1.0, 0.0, w / 2, h));
                set(&mut self.th2, line(1.0, 0.0, 2.0, hf - 1.0, -1.0, 0.0, w / 2, h));
            }
            // circles from the centre out, or from the edges in
            SEQ_CIRCLE_OUT_ON | SEQ_CIRCLE_OUT_OFF => {
                let t = &mut self.th1;
                t.kind = Kind::Circle;
                t.radius = 0.0;
                t.step_radius = 1.0;
                t.x = xc;
                t.y = yc;
                t.frame_count = w / 2 + xa + h / 2 + ya;
            }
            SEQ_CIRCLE_IN_ON | SEQ_CIRCLE_IN_OFF => {
                let t = &mut self.th1;
                t.kind = Kind::Circle;
                t.radius = (w / 2 + xa + h / 2 + ya) as f32;
                t.step_radius = -1.0;
                t.x = xc;
                t.y = yc;
                t.frame_count = w / 2 + xa + h / 2 + ya;
            }
            // sweeps around a point
            SEQ_CLOCK_RIGHT_ON | SEQ_CLOCK_RIGHT_OFF => set(&mut self.th1, radar(0.0, 1.0, false, xc, yc, w / 2 + xa + h / 2 + ya, 360)),
            SEQ_CLOCK_LEFT_ON | SEQ_CLOCK_LEFT_OFF => set(&mut self.th1, radar(0.0, -1.0, false, xc, yc, w / 2 + xa + h / 2 + ya, 360)),
            SEQ_RADAR_RIGHT_ON | SEQ_RADAR_RIGHT_OFF => set(&mut self.th1, radar(270.0, 1.0, false, xc, hf - 1.0, h + w + ya + xa, 180)),
            SEQ_RADAR_LEFT_ON | SEQ_RADAR_LEFT_OFF => set(&mut self.th1, radar(90.0, -1.0, false, xc, hf - 1.0, h + w + ya + xa, 180)),
            SEQ_WIPER_RIGHT_ON | SEQ_WIPER_RIGHT_OFF => set(&mut self.th1, radar(270.0, -1.0, false, xc, 0.0, h + w + ya + xa, 180)),
            SEQ_WIPER_LEFT_ON | SEQ_WIPER_LEFT_OFF => set(&mut self.th1, radar(90.0, 1.0, false, xc, 0.0, h + w + ya + xa, 180)),
            SEQ_FAN_LEFT_UP_ON | SEQ_FAN_LEFT_UP_OFF => set(&mut self.th1, radar(180.0, -1.0, false, 0.0, yc, h + w + ya + xa, 180)),
            SEQ_FAN_LEFT_DOWN_ON | SEQ_FAN_LEFT_DOWN_OFF => set(&mut self.th1, radar(0.0, 1.0, false, 0.0, yc, h + w + ya + xa, 180)),
            SEQ_FAN_RIGHT_UP_ON | SEQ_FAN_RIGHT_UP_OFF => set(&mut self.th1, radar(180.0, -1.0, true, wf - 1.0, yc, h + w + ya + xa, 180)),
            SEQ_FAN_RIGHT_DOWN_ON | SEQ_FAN_RIGHT_DOWN_OFF => set(&mut self.th1, radar(0.0, 1.0, true, wf - 1.0, yc, h + w + ya + xa, 180)),
            SEQ_ARC_BOTTOM_LEFT_UP_ON | SEQ_ARC_BOTTOM_LEFT_UP_OFF => set(&mut self.th1, radar(90.0, -1.0, false, 0.0, hf - 1.0, h + w, 90)),
            SEQ_ARC_BOTTOM_LEFT_DOWN_ON | SEQ_ARC_BOTTOM_LEFT_DOWN_OFF => set(&mut self.th1, radar(0.0, 1.0, false, 0.0, hf - 1.0, h + w, 90)),
            SEQ_ARC_BOTTOM_RIGHT_UP_ON | SEQ_ARC_BOTTOM_RIGHT_UP_OFF => set(&mut self.th1, radar(90.0, 1.0, true, wf - 1.0, hf - 1.0, h + w, 90)),
            SEQ_ARC_BOTTOM_RIGHT_DOWN_ON | SEQ_ARC_BOTTOM_RIGHT_DOWN_OFF => set(&mut self.th1, radar(180.0, -1.0, true, wf - 1.0, hf - 1.0, h + w, 90)),
            SEQ_ARC_TOP_LEFT_UP_ON | SEQ_ARC_TOP_LEFT_UP_OFF => set(&mut self.th1, radar(180.0, -1.0, false, 0.0, 0.0, h + w, 90)),
            SEQ_ARC_TOP_LEFT_DOWN_ON | SEQ_ARC_TOP_LEFT_DOWN_OFF => set(&mut self.th1, radar(90.0, 1.0, false, 0.0, 0.0, h + w, 90)),
            SEQ_ARC_TOP_RIGHT_UP_ON | SEQ_ARC_TOP_RIGHT_UP_OFF => set(&mut self.th1, radar(0.0, 1.0, true, wf - 1.0, 0.0, h + w, 90)),
            SEQ_ARC_TOP_RIGHT_DOWN_ON | SEQ_ARC_TOP_RIGHT_DOWN_OFF => set(&mut self.th1, radar(90.0, -1.0, true, wf - 1.0, 0.0, h + w, 90)),
            // two tracers screwing out from the centre
            SEQ_SCREW_RIGHT_ON | SEQ_SCREW_RIGHT_OFF => {
                set(&mut self.th1, radar(0.0, 1.0, false, xc, yc - 1.0, h / 2 + ya + w / 2 + xa, 180));
                set(&mut self.th2, radar((180 - 1) as f32, 1.0, false, xc, yc, h / 2 + ya + w / 2 + xa, 180));
            }
            SEQ_SCREW_LEFT_ON | SEQ_SCREW_LEFT_OFF => {
                set(&mut self.th1, radar(0.0, -1.0, false, xc, yc - 1.0, h / 2 + ya + w / 2 + xa, 180));
                set(&mut self.th2, radar((180 - 1) as f32, -1.0, false, xc, yc, h / 2 + ya + w / 2 + xa, 180));
            }
            // unknown: expire in one frame and move on to the next animation
            _ => {
                self.th1.length = 0;
                self.th1.frame_count = 1;
                tail_length = 0;
                repeat = 1;
                pause = 0;
            }
        }
        let _ = (repeat, pause);

        // no delays on the effects
        self.th1.delay = 0;
        self.th2.delay = 0;
        self.tt1.delay = 0;
        self.tt2.delay = 0;

        // a tail: a copy of the head, starting that many steps later
        if tail_length != 0 {
            self.tt1 = self.th1;
            self.tt1.delay = tail_length;
            self.tt2 = self.th2;
            self.tt2.delay = tail_length;
        }

        // inverse mode: the tail leads (the effect switches lights off)
        if inverse {
            std::mem::swap(&mut self.th1, &mut self.tt1);
            std::mem::swap(&mut self.th2, &mut self.tt2);
        }

        self.time_next_update = now.wrapping_add(self.update_rate as u32);
        self.play_in_progress = true;
    }

    /// VPX's `ProcessTracer`: one step of a tracer. True when it's done.
    fn process(&mut self, which: Which, state: i32, now: u32, parts: &mut [Part], rng: &mut Mwc64x) -> bool {
        let mut t = match which {
            Which::Th1 => self.th1,
            Which::Th2 => self.th2,
            Which::Tt1 => self.tt1,
            Which::Tt2 => self.tt2,
        };
        let done = self.process_tracer(&mut t, state, now, parts, rng);
        match which {
            Which::Th1 => self.th1 = t,
            Which::Th2 => self.th2 = t,
            Which::Tt1 => self.tt1 = t,
            Which::Tt2 => self.tt2 = t,
        }
        done
    }

    fn process_tracer(&mut self, t: &mut Tracer, state: i32, now: u32, parts: &mut [Part], rng: &mut Mwc64x) -> bool {
        // an unused tracer, or no collection: finished
        if t.kind == Kind::Null || self.elements.is_none() {
            return true;
        }
        if t.delay != 0 {
            t.delay -= 1;
            return false;
        }
        let mut rc = false;
        match t.kind {
            Kind::Blink => {
                let frame = t.frame_count;
                t.frame_count -= 1;
                match frame {
                    2 => {
                        self.set_all_lights_to_state(if t.length <= 0 { ON } else { OFF }, parts, now);
                        self.time_next_update = now.wrapping_add(self.pause_value as u32);
                    }
                    1 => {
                        self.set_all_lights_to_state(if t.length <= 0 { OFF } else { ON }, parts, now);
                        self.time_next_update = now.wrapping_add(self.pause_value as u32);
                    }
                    0 => rc = true,
                    _ => {}
                }
            }
            Kind::Random => {
                let size = self.elements.as_ref().map_or(0, Vec::len) as f32;
                // pick n members and invert their state
                for _ in 0..t.length {
                    let random_light = (size * rng.rand_01()) as i32;
                    let element_state = if self.get_element_state(random_light, parts) == ON { OFF } else { ON };
                    self.set_element_to_state(random_light, element_state, parts, now);
                }
                t.frame_count -= 1;
                if t.frame_count == 0 {
                    t.kind = Kind::Null;
                    rc = true;
                }
            }
            Kind::Line => {
                let (mut x, mut y) = (t.x, t.y);
                for _ in 0..t.length {
                    self.verify_and_set_grid_element(x as i32, y as i32, state, parts, now);
                    x += t.process_step_x;
                    y += t.process_step_y;
                }
                t.frame_count -= 1;
                if t.frame_count == 0 {
                    t.kind = Kind::Null;
                    rc = true;
                } else {
                    t.x += t.step_x;
                    t.y += t.step_y;
                }
            }
            Kind::Circle => {
                for fi in 0..720 {
                    let angle = ((std::f64::consts::PI * 2.0 / 360.0) * (f64::from(fi) / 2.0)) as f32;
                    let (sn, cs) = (angle.sin(), angle.cos());
                    let x = t.x + sn * t.radius;
                    let y = t.y - cs * t.radius;
                    self.verify_and_set_grid_element(x as i32, y as i32, state, parts, now);
                }
                t.frame_count -= 1;
                if t.frame_count == 0 {
                    t.kind = Kind::Null;
                    rc = true;
                } else {
                    t.radius += t.step_radius;
                }
            }
            Kind::Radar => {
                let to_radians = (std::f64::consts::PI * 2.0 / 360.0) as f32;
                let angle = t.angle * to_radians;
                let (sn, cs) = (angle.sin(), angle.cos());
                // an extra three quarter steps, or big radii miss lights
                let angle2 = (t.angle + t.step_angle * 0.25) * to_radians;
                let (sn2, cs2) = (angle2.sin(), angle2.cos());
                let angle3 = (t.angle + t.step_angle * 0.5) * to_radians;
                let (sn3, cs3) = (angle3.sin(), angle3.cos());
                let angle4 = (t.angle + t.step_angle * 0.75) * to_radians;
                let (sn4, cs4) = (angle4.sin(), angle4.cos());
                // VPX reads the first head tracer's direction, whichever tracer this is
                let negative = self.th1.process_radius_neg;
                for i in 0..t.length {
                    let fi = if negative { -(i as f32) } else { i as f32 };
                    for (s, c) in [(sn, cs), (sn2, cs2), (sn3, cs3), (sn4, cs4)] {
                        let x = t.x + s * fi;
                        let y = t.y - c * fi;
                        self.verify_and_set_grid_element(x as i32, y as i32, state, parts, now);
                    }
                }
                t.frame_count -= 1;
                if t.frame_count == 0 {
                    t.kind = Kind::Null;
                    rc = true;
                } else {
                    t.angle += t.step_angle;
                    if t.angle >= 360.0 {
                        t.angle -= 360.0;
                    }
                    if t.angle < 0.0 {
                        t.angle += 360.0;
                    }
                }
            }
            Kind::Null => rc = true,
        }
        rc
    }

    fn set_all_lights_to_state(&mut self, state: i32, parts: &mut [Part], now: u32) {
        let count = self.elements.as_ref().map_or(0, Vec::len);
        for i in 0..count {
            self.set_element_to_state(i as i32, state, parts, now);
        }
    }

    /// VPX's `SetElementToState`: the member takes the sequencer's state
    /// and ignores its own until the show stops.
    fn set_element_to_state(&self, index: i32, state: i32, parts: &mut [Part], now: u32) {
        let Some(&element) = self.elements.as_ref().and_then(|e| e.get(index as usize)) else { return };
        match element {
            Element::Light(p) => {
                if let Some(l) = &mut parts[p].light {
                    l.locked_by_ls = true;
                    l.set_in_play_state(state as f32, now);
                }
            }
            Element::Flasher(p) | Element::Primitive(p) => {
                parts[p].ls_locked = true;
                parts[p].ls_on = state != OFF;
            }
            Element::Other => {}
        }
    }

    fn verify_and_set_grid_element(&self, x: i32, y: i32, state: i32, parts: &mut [Part], now: u32) {
        if (0..self.grid_width).contains(&x) && (0..self.grid_height).contains(&y) {
            let index = self.grid[(y * self.grid_width + x) as usize];
            if index != 0 {
                self.set_element_to_state(i32::from(index) - 1, state, parts, now);
            }
        }
    }

    /// VPX's `GetElementState`.
    fn get_element_state(&self, index: i32, parts: &[Part]) -> i32 {
        let Some(&element) = self.elements.as_ref().and_then(|e| e.get(index as usize)) else { return OFF };
        match element {
            Element::Light(p) => parts[p].light.as_ref().map_or(OFF, |l| {
                if l.in_play_state == 0.0 {
                    OFF
                } else if l.in_play_state == 2.0 {
                    BLINKING
                } else {
                    ON
                }
            }),
            Element::Flasher(p) | Element::Primitive(p) => {
                if parts[p].ls_on {
                    ON
                } else {
                    OFF
                }
            }
            Element::Other => OFF,
        }
    }
}

#[derive(Clone, Copy)]
enum Which {
    Th1,
    Th2,
    Tt1,
    Tt2,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::gameitem::light::Light;
    use vpin::vpx::gameitem::vertex2d::Vertex2D;

    fn part(light: Option<crate::light::LightRt>) -> Part {
        Part {
            name: String::new(),
            class: None,
            obj: std::ptr::null_mut(),
            collider: None,
            timer: None,
            event_collections: Vec::new(),
            single_events: true,
            user_value: None,
            visible: true,
            light,
            reel: None,
            text_box: None,
            light_seq: None,
            ls_locked: false,
            ls_on: true,
            target: None,
            trigger: None,
            bumper: None,
            hit_threshold: 0.0,
            is_dropped: false,
            collidable: true,
            disabled: false,
            look: Default::default(),
            flasher: None,
            primitive: None,
            prim_anim: None,
            ramp_shape: None,
        }
    }

    /// Three lights down the middle of a 1000 × 2000 table: grid rows 5, 50 and 95.
    fn table() -> (VPX, Vec<Part>, Vec<u32>) {
        let mut vpx = VPX::default();
        (vpx.gamedata.left, vpx.gamedata.top, vpx.gamedata.right, vpx.gamedata.bottom) = (0.0, 0.0, 1000.0, 2000.0);
        let mut parts = Vec::new();
        for (i, y) in [100.0, 1000.0, 1900.0].into_iter().enumerate() {
            let l = Light { name: format!("L{i}"), center: Vertex2D::new(500.0, y), state: Some(0.0), ..Light::default() };
            parts.push(part(Some(crate::light::LightRt::new(&l))));
            vpx.gameitems.push(GameItemEnum::Light(l));
        }
        (vpx, parts, vec![0, 1, 2])
    }

    fn sequencer(vpx: &VPX, members: &[u32]) -> LightSeqRt {
        let item = LightSequencer { collection: "c".into(), pos_x: 500.0, pos_y: 1000.0, update_interval: 25, ..LightSequencer::default() };
        LightSeqRt::new(&item, vpx, &[("c", members)])
    }

    #[test]
    fn a_line_sweeps_down_a_row_per_update_then_gives_the_lights_back() {
        let (vpx, mut parts, members) = table();
        let mut seq = sequencer(&vpx, &members);
        let mut rng = Mwc64x::default();
        assert!(seq.play(SEQ_DOWN_ON, 0, 1, 0, &mut parts, 0));
        let mut on_at = [None; 3];
        let mut done_at = None;
        for now in (0..4000).step_by(5) {
            if seq.update(now, &mut parts, &mut rng) {
                done_at = Some(now);
                break;
            }
            for (k, p) in parts.iter().enumerate() {
                let l = p.light.as_ref().unwrap();
                if on_at[k].is_none() && l.in_play_state == 1.0 {
                    assert!(l.locked_by_ls);
                    on_at[k] = Some(now);
                }
            }
        }
        // the queue starts the show at once; row r lights at update r + 1, 25 ms apart
        assert_eq!(on_at, [Some(150), Some(1275), Some(2400)]);
        // 100 rows, then one interval's pause, then the lights are their own again
        assert_eq!(done_at, Some(2525));
        for p in &parts {
            let l = p.light.as_ref().unwrap();
            assert!(!l.locked_by_ls);
            assert_eq!(l.in_play_state, 0.0);
        }
    }

    #[test]
    fn an_off_sweep_with_a_tail_switches_lights_off_then_on_behind_it() {
        let (vpx, mut parts, members) = table();
        let mut seq = sequencer(&vpx, &members);
        let mut rng = Mwc64x::default();
        // inverse: the tail tracer leads (off) and the head follows 10 steps later (on)
        assert!(seq.play(SEQ_DOWN_OFF, 10, 1, 0, &mut parts, 0));
        let mut states = Vec::new();
        for now in (0..=400).step_by(25) {
            seq.update(now, &mut parts, &mut rng);
            states.push(parts[0].light.as_ref().unwrap().in_play_state);
        }
        // row 5: switched off by the leader at 150 ms (already off), on by the follower at 400 ms
        assert_eq!(states[6], 0.0);
        assert!(parts[0].light.as_ref().unwrap().locked_by_ls);
        assert_eq!(states[16], 1.0);
    }

    #[test]
    fn a_circle_grows_from_the_centre() {
        let (vpx, mut parts, members) = table();
        let mut seq = sequencer(&vpx, &members);
        let mut rng = Mwc64x::default();
        assert!(seq.play(SEQ_CIRCLE_OUT_ON, 0, 1, 0, &mut parts, 0));
        let mut on_at = [None; 3];
        let mut done_at = None;
        for now in (0..4000).step_by(5) {
            if seq.update(now, &mut parts, &mut rng) {
                done_at = Some(now);
                break;
            }
            for (k, p) in parts.iter().enumerate() {
                if on_at[k].is_none() && p.light.as_ref().unwrap().in_play_state == 1.0 {
                    on_at[k] = Some(now);
                }
            }
        }
        // the centre light at radius 0, the others 45 cells out, one cell a step
        assert_eq!(on_at[1], Some(25));
        assert!(on_at[0].is_some_and(|t| (1150..=1200).contains(&t)), "{on_at:?}");
        assert!(on_at[2].is_some_and(|t| (1150..=1200).contains(&t)), "{on_at:?}");
        // 75 steps (half the grid's width and height), then one interval's pause
        assert_eq!(done_at, Some(1900));
    }

    #[test]
    fn all_on_happens_at_once_and_stop_play_restores() {
        let (vpx, mut parts, members) = table();
        let mut seq = sequencer(&vpx, &members);
        assert!(seq.play(SEQ_ALL_ON, 0, 1, 0, &mut parts, 0));
        assert!(parts.iter().all(|p| p.light.as_ref().unwrap().in_play_state == 1.0));
        seq.stop_play(&mut parts, 0);
        assert!(parts.iter().all(|p| p.light.as_ref().unwrap().in_play_state == 0.0 && !p.light.as_ref().unwrap().locked_by_ls));
    }

    #[test]
    fn a_missing_collection_plays_nothing_but_still_finishes() {
        let (vpx, mut parts, _) = table();
        let item = LightSequencer { collection: "nope".into(), update_interval: 25, ..LightSequencer::default() };
        let mut seq = LightSeqRt::new(&item, &vpx, &[]);
        let mut rng = Mwc64x::default();
        assert!(seq.play(SEQ_UP_ON, 0, 1, 0, &mut parts, 0));
        let done = (0..200).step_by(5).find(|&now| seq.update(now, &mut parts, &mut rng));
        assert_eq!(done, Some(50));
        assert!(parts.iter().all(|p| p.light.as_ref().unwrap().in_play_state == 0.0));
    }
}
