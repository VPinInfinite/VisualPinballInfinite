// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): its copy of Glide (FlexDMD/Glide/Tween.cs,
// Tweener.cs, Ease.cs, MemberLerper.cs), Copyright (c) 2013 Jacob Albano, under the MIT License, as FlexDMD
// carries it (LICENSE-FlexDMD.txt beside this file). Changed in the porting, from C# to Rust, for Visual Pinball
// Infinite, keeping only what FlexDMD uses (no repeats, reflection, rotation or rounding); the port is
// GPL-3.0-or-later.

//! Tweens: a property of an actor moved from where it was to a value over
//! a time, along an easing curve, as FlexDMD's `MoveTo` and UltraDMD's scene
//! animations do it. The tween reads the start when it's made and does
//! nothing on its first update but get ready, so a move lasts one frame
//! longer than its duration, as in FlexDMD.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::actor::{self, Actor, Prop};

/// A tween's easing curve, by FlexDMD's numbers (`Interpolation`).
pub type Ease = fn(f32) -> f32;

/// The curve for FlexDMD's `Interpolation` number; None for linear (0) and
/// for numbers outside the list, which FlexDMD leaves linear too.
pub fn ease_for(n: i32) -> Option<Ease> {
    const CURVES: [Ease; 30] = [
        ease::elastic_in,
        ease::elastic_out,
        ease::elastic_in_out,
        ease::quad_in,
        ease::quad_out,
        ease::quad_in_out,
        ease::cube_in,
        ease::cube_out,
        ease::cube_in_out,
        ease::quart_in,
        ease::quart_out,
        ease::quart_in_out,
        ease::quint_in,
        ease::quint_out,
        ease::quint_in_out,
        ease::sine_in,
        ease::sine_out,
        ease::sine_in_out,
        ease::bounce_in,
        ease::bounce_out,
        ease::bounce_in_out,
        ease::circ_in,
        ease::circ_out,
        ease::circ_in_out,
        ease::expo_in,
        ease::expo_out,
        ease::expo_in_out,
        ease::back_in,
        ease::back_out,
        ease::back_in_out,
    ];
    CURVES.get(usize::try_from(n.checked_sub(1)?).ok()?).copied()
}

/// One property of a tween: Glide's `NumericLerper` and the member it sets.
struct Lerp {
    prop: Prop,
    start: f32,
    end: f32,
    from: f32,
    range: f32,
}

/// One tween (Glide's `Tween`).
pub struct Tween {
    id: u64,
    target: Weak<RefCell<Actor>>,
    duration: f32,
    delay: f32,
    time: f32,
    first_update: bool,
    /// Its properties, in the order given; a cancelled one is None.
    lerps: Vec<Option<Lerp>>,
    ease: Option<Ease>,
}

impl Tween {
    /// Glide's `Update`; true when it completes.
    fn update(&mut self, elapsed: f32, remove: &mut Vec<u64>) -> bool {
        if self.first_update {
            self.first_update = false;
            for l in self.lerps.iter_mut().rev().flatten() {
                l.from = l.start;
                l.range = l.end - l.start;
            }
            return false;
        }
        if self.delay > 0.0 {
            self.delay -= elapsed;
            if self.delay > 0.0 {
                return false;
            }
        }
        self.time += elapsed;
        let mut t = self.time / self.duration;
        let mut complete = false;
        if self.time >= self.duration {
            self.time = self.duration;
            t = 1.0;
            remove.push(self.id);
            complete = true;
        }
        if let Some(ease) = self.ease {
            t = ease(t);
        }
        if let Some(target) = self.target.upgrade() {
            let mut a = target.borrow_mut();
            for l in self.lerps.iter().rev().flatten() {
                actor::set_prop(&mut a, l.prop, l.from + l.range * t);
            }
        }
        complete
    }

    /// Glide's `Cancel(properties)`: forgets those properties; true when that
    /// was as many as the tween has, which cancels it.
    fn cancel_props(&mut self, props: &[Prop]) -> bool {
        let mut cancelled = 0;
        for p in props {
            if let Some(slot) = self.lerps.iter_mut().find(|l| l.as_ref().is_some_and(|l| l.prop == *p)) {
                *slot = None;
                cancelled += 1;
            }
        }
        cancelled == self.lerps.len()
    }
}

/// A set of tweens updated together (Glide's `Tweener`).
#[derive(Default)]
pub struct Tweener {
    tweens: Vec<Tween>,
    to_add: Vec<Tween>,
    to_remove: Vec<u64>,
    next_id: u64,
}

impl Tweener {
    /// Glide's `Tween(target, values, duration, delay)`: a tween of the
    /// target's properties to the given values, starting from where they
    /// are now, cancelling those properties in the target's other tweens.
    /// Returns its id.
    pub fn tween(&mut self, target: &Rc<RefCell<Actor>>, values: &[(Prop, f32)], duration: f32, delay: f32) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let lerps = {
            let a = target.borrow();
            values.iter().map(|&(prop, end)| Some(Lerp { prop, start: actor::get_prop(&a, prop), end, from: 0.0, range: 0.0 })).collect()
        };
        let weak = Rc::downgrade(target);
        // overwrite: the target's tweens lose these properties
        let props: Vec<Prop> = values.iter().map(|v| v.0).collect();
        for t in self.tweens.iter_mut().filter(|t| t.target.ptr_eq(&weak)) {
            for p in &props {
                if t.cancel_props(std::slice::from_ref(p)) {
                    self.to_remove.push(t.id);
                }
            }
        }
        self.to_add.push(Tween { id, target: weak, duration, delay, time: 0.0, first_update: true, lerps, ease: None });
        self.add_and_remove();
        id
    }

    /// Sets a tween's easing curve (Glide's `Ease`).
    pub fn set_ease(&mut self, id: u64, ease: Option<Ease>) {
        if let Some(t) = self.tweens.iter_mut().chain(self.to_add.iter_mut()).find(|t| t.id == id) {
            t.ease = ease;
        }
    }

    /// Updates every tween; returns the ids of those that completed.
    pub fn update(&mut self, elapsed: f32) -> Vec<u64> {
        let mut done = Vec::new();
        let mut remove = std::mem::take(&mut self.to_remove);
        for t in &mut self.tweens {
            if t.update(elapsed, &mut remove) {
                done.push(t.id);
            }
        }
        self.to_remove = remove;
        self.add_and_remove();
        done
    }

    /// Glide's `Cancel`: every tween goes at the next update.
    pub fn cancel(&mut self) {
        self.to_remove.extend(self.tweens.iter().map(|t| t.id));
    }

    fn add_and_remove(&mut self) {
        self.tweens.append(&mut self.to_add);
        let remove = std::mem::take(&mut self.to_remove);
        self.tweens.retain(|t| !remove.contains(&t.id));
    }
}

/// Glide's easing curves (`Ease`), with its own π and constants, in C#'s
/// mix of single and double precision.
#[allow(clippy::excessive_precision, clippy::approx_constant)]
pub mod ease {
    const PI: f32 = 3.14159;
    const PI2: f32 = PI / 2.0;
    const B1: f32 = 1.0 / 2.75;
    const B2: f32 = 2.0 / 2.75;
    const B3: f32 = 1.5 / 2.75;
    const B4: f32 = 2.5 / 2.75;
    const B5: f32 = 2.25 / 2.75;
    const B6: f32 = 2.625 / 2.75;

    fn d(x: f32) -> f64 {
        f64::from(x)
    }

    pub fn elastic_in(t: f32) -> f32 {
        (d(13.0 * PI2 * t).sin() * 2f64.powf(d(10.0 * (t - 1.0)))) as f32
    }
    pub fn elastic_out(t: f32) -> f32 {
        if t == 1.0 {
            return 1.0;
        }
        (d(-13.0 * PI2 * (t + 1.0)).sin() * 2f64.powf(d(-10.0 * t)) + 1.0) as f32
    }
    pub fn elastic_in_out(t: f32) -> f32 {
        if d(t) < 0.5 {
            return (0.5 * d(13.0 * PI2 * (2.0 * t)).sin() * 2f64.powf(d(10.0 * ((2.0 * t) - 1.0)))) as f32;
        }
        (0.5 * (d(-13.0 * PI2 * ((2.0 * t - 1.0) + 1.0)).sin() * 2f64.powf(d(-10.0 * (2.0 * t - 1.0))) + 2.0)) as f32
    }
    pub fn quad_in(t: f32) -> f32 {
        t * t
    }
    pub fn quad_out(t: f32) -> f32 {
        -t * (t - 2.0)
    }
    pub fn quad_in_out(t: f32) -> f32 {
        if d(t) <= 0.5 {
            t * t * 2.0
        } else {
            let t = t - 1.0;
            1.0 - t * t * 2.0
        }
    }
    pub fn cube_in(t: f32) -> f32 {
        t * t * t
    }
    pub fn cube_out(t: f32) -> f32 {
        let t = t - 1.0;
        1.0 + t * t * t
    }
    pub fn cube_in_out(t: f32) -> f32 {
        if d(t) <= 0.5 {
            t * t * t * 4.0
        } else {
            let t = t - 1.0;
            1.0 + t * t * t * 4.0
        }
    }
    pub fn quart_in(t: f32) -> f32 {
        t * t * t * t
    }
    pub fn quart_out(t: f32) -> f32 {
        let t = t - 1.0;
        1.0 - t * t * t * t
    }
    pub fn quart_in_out(t: f32) -> f32 {
        if d(t) <= 0.5 {
            t * t * t * t * 8.0
        } else {
            let t = t * 2.0 - 2.0;
            (d((1.0 - t * t * t * t) / 2.0) + 0.5) as f32
        }
    }
    pub fn quint_in(t: f32) -> f32 {
        t * t * t * t * t
    }
    pub fn quint_out(t: f32) -> f32 {
        let t = t - 1.0;
        t * t * t * t * t + 1.0
    }
    pub fn quint_in_out(t: f32) -> f32 {
        let t = t * 2.0;
        if t < 1.0 {
            (t * t * t * t * t) / 2.0
        } else {
            let t = t - 2.0;
            (t * t * t * t * t + 2.0) / 2.0
        }
    }
    pub fn sine_in(t: f32) -> f32 {
        if t == 1.0 {
            return 1.0;
        }
        (-d(PI2 * t).cos() + 1.0) as f32
    }
    pub fn sine_out(t: f32) -> f32 {
        d(PI2 * t).sin() as f32
    }
    pub fn sine_in_out(t: f32) -> f32 {
        (-d(PI * t).cos() / 2.0 + 0.5) as f32
    }
    pub fn bounce_in(t: f32) -> f32 {
        let t = 1.0 - t;
        let tt = d(t);
        if t < B1 {
            return (1.0 - 7.5625 * tt * tt) as f32;
        }
        if t < B2 {
            return (1.0 - (7.5625 * d(t - B3) * d(t - B3) + 0.75)) as f32;
        }
        if t < B4 {
            return (1.0 - (7.5625 * d(t - B5) * d(t - B5) + 0.9375)) as f32;
        }
        (1.0 - (7.5625 * d(t - B6) * d(t - B6) + 0.984375)) as f32
    }
    pub fn bounce_out(t: f32) -> f32 {
        let tt = d(t);
        if t < B1 {
            return (7.5625 * tt * tt) as f32;
        }
        if t < B2 {
            return (7.5625 * d(t - B3) * d(t - B3) + 0.75) as f32;
        }
        if t < B4 {
            return (7.5625 * d(t - B5) * d(t - B5) + 0.9375) as f32;
        }
        (7.5625 * d(t - B6) * d(t - B6) + 0.984375) as f32
    }
    pub fn bounce_in_out(t: f32) -> f32 {
        if d(t) < 0.5 {
            let t = 1.0 - t * 2.0;
            let tt = d(t);
            if t < B1 {
                return ((1.0 - 7.5625 * tt * tt) / 2.0) as f32;
            }
            if t < B2 {
                return ((1.0 - (7.5625 * d(t - B3) * d(t - B3) + 0.75)) / 2.0) as f32;
            }
            if t < B4 {
                return ((1.0 - (7.5625 * d(t - B5) * d(t - B5) + 0.9375)) / 2.0) as f32;
            }
            return ((1.0 - (7.5625 * d(t - B6) * d(t - B6) + 0.984375)) / 2.0) as f32;
        }
        let t = t * 2.0 - 1.0;
        let tt = d(t);
        if t < B1 {
            return ((7.5625 * tt * tt) / 2.0 + 0.5) as f32;
        }
        if t < B2 {
            return ((7.5625 * d(t - B3) * d(t - B3) + 0.75) / 2.0 + 0.5) as f32;
        }
        if t < B4 {
            return ((7.5625 * d(t - B5) * d(t - B5) + 0.9375) / 2.0 + 0.5) as f32;
        }
        ((7.5625 * d(t - B6) * d(t - B6) + 0.984375) / 2.0 + 0.5) as f32
    }
    pub fn circ_in(t: f32) -> f32 {
        (-(d(1.0 - t * t).sqrt() - 1.0)) as f32
    }
    pub fn circ_out(t: f32) -> f32 {
        d(1.0 - (t - 1.0) * (t - 1.0)).sqrt() as f32
    }
    pub fn circ_in_out(t: f32) -> f32 {
        if d(t) <= 0.5 {
            ((d(1.0 - t * t * 4.0).sqrt() - 1.0) / -2.0) as f32
        } else {
            ((d(1.0 - (t * 2.0 - 2.0) * (t * 2.0 - 2.0)).sqrt() + 1.0) / 2.0) as f32
        }
    }
    pub fn expo_in(t: f32) -> f32 {
        2f64.powf(d(10.0 * (t - 1.0))) as f32
    }
    pub fn expo_out(t: f32) -> f32 {
        if t == 1.0 {
            return 1.0;
        }
        (-2f64.powf(d(-10.0 * t)) + 1.0) as f32
    }
    pub fn expo_in_out(t: f32) -> f32 {
        if t == 1.0 {
            return 1.0;
        }
        if d(t) < 0.5 {
            (2f64.powf(d(10.0 * (t * 2.0 - 1.0))) / 2.0) as f32
        } else {
            ((-2f64.powf(d(-10.0 * (t * 2.0 - 1.0))) + 2.0) / 2.0) as f32
        }
    }
    pub fn back_in(t: f32) -> f32 {
        (d(t * t) * (2.70158 * d(t) - 1.70158)) as f32
    }
    pub fn back_out(t: f32) -> f32 {
        let t = t - 1.0;
        (1.0 - d(t * t) * (-2.70158 * d(t) - 1.70158)) as f32
    }
    pub fn back_in_out(t: f32) -> f32 {
        let t = t * 2.0;
        if t < 1.0 {
            return (d(t * t) * (2.70158 * d(t) - 1.70158) / 2.0) as f32;
        }
        // C#'s `t--` and then `--t`: two less
        let t = t - 2.0;
        ((1.0 - d(t * t) * (-2.70158 * d(t) - 1.70158)) / 2.0 + 0.5) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flexdmd::actor::Actor;

    #[test]
    fn a_tween_waits_a_frame_then_moves_to_its_end() {
        let a = Actor::new_plain();
        a.borrow_mut().x = 10.0;
        let mut tw = Tweener::default();
        let id = tw.tween(&a, &[(Prop::X, 20.0)], 1.0, 0.0);
        // the first update only gets ready
        assert!(tw.update(0.25).is_empty());
        assert_eq!(a.borrow().x, 10.0);
        tw.update(0.25);
        assert_eq!(a.borrow().x, 12.5);
        tw.update(0.5);
        assert_eq!(a.borrow().x, 17.5);
        assert_eq!(tw.update(0.5), vec![id]);
        assert_eq!(a.borrow().x, 20.0);
        // done: it's gone
        a.borrow_mut().x = 0.0;
        tw.update(0.5);
        assert_eq!(a.borrow().x, 0.0);
    }

    #[test]
    fn a_new_tween_takes_its_properties_from_older_ones() {
        let a = Actor::new_plain();
        let mut tw = Tweener::default();
        tw.tween(&a, &[(Prop::X, 100.0), (Prop::Y, 100.0)], 1.0, 0.0);
        tw.update(0.0);
        tw.tween(&a, &[(Prop::X, -100.0)], 1.0, 0.0);
        tw.update(0.0);
        tw.update(0.5);
        // X follows the new tween, Y the old one
        assert_eq!(a.borrow().x, -50.0);
        assert_eq!(a.borrow().y, 50.0);
    }

    #[test]
    #[allow(clippy::approx_constant)] // Glide's own π
    fn easing_curves_end_where_they_should() {
        for n in 1..=30 {
            let e = ease_for(n).unwrap();
            assert!((e(1.0) - 1.0).abs() < 1e-3, "curve {n} ends at {}", e(1.0));
            assert!(e(0.0).abs() < 2e-3, "curve {n} starts at {}", e(0.0));
        }
        assert!(ease_for(0).is_none() && ease_for(31).is_none() && ease_for(-1).is_none());
        // Glide's π is 3.14159, so sine out at the end falls just short of 1
        assert_eq!(ease::sine_out(1.0), (f64::from(3.14159f32 / 2.0)).sin() as f32);
    }
}
