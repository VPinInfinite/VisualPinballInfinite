// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/Actors/Actions.cs. Copyright 2019
// Vincent Bousquet, licensed under the Apache License, Version 2.0 (LICENSE-FlexDMD.txt beside this file). Changed
// in the porting, from C# to Rust, for Visual Pinball Infinite; the port is GPL-3.0-or-later.

//! FlexDMD's actions: what an actor does over time, updated with its actor
//! each frame until they report done. Composite actions (sequences,
//! parallels, repeats, delays) start over once done, so they can run again.
//! An action reaches the actor it was made for without keeping it alive
//! (the actor keeps its actions; a cycle would never be freed); actors it
//! adds are held until then.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::actor::{self, ActorRef, Prop};
use super::animated;
use super::tween::{self, Tweener};

pub type ActionRef = Rc<RefCell<Action>>;

/// An action (FlexDMD's `Action` and what derives from it).
pub enum Action {
    /// Runs another `count` times (forever when 0 or less).
    Repeat { action: ActionRef, count: i32, n: i32 },
    Sequence { actions: Vec<ActionRef>, pos: usize },
    Parallel { actions: Vec<ActionRef>, running: Vec<bool> },
    Show { target: Weak<RefCell<actor::Actor>>, visible: bool },
    Blink { target: Weak<RefCell<actor::Actor>>, show: f32, hide: f32, repeat: i32, n: i32, time: f32 },
    /// Adds the target to a group, or takes it off.
    AddTo { target: Weak<RefCell<actor::Actor>>, parent: ActorRef, add: bool },
    RemoveFromParent { target: Weak<RefCell<actor::Actor>> },
    /// Adds a child to the target group, or takes it off.
    AddChild { target: Weak<RefCell<actor::Actor>>, child: ActorRef, add: bool },
    Seek { target: Weak<RefCell<actor::Actor>>, position: f32 },
    Wait { seconds: f32, time: f32 },
    Delayed { seconds: f32, action: ActionRef, time: f32 },
    MoveTo(MoveTo),
}

/// FlexDMD's `MoveToAction`, a tween action.
pub struct MoveTo {
    pub target: Weak<RefCell<actor::Actor>>,
    pub x: f32,
    pub y: f32,
    pub duration: f32,
    /// FlexDMD's `Interpolation` number.
    pub ease: i32,
    tweener: Option<(Tweener, u64)>,
}

impl MoveTo {
    pub fn new(target: &ActorRef, x: f32, y: f32, duration: f32) -> MoveTo {
        MoveTo { target: Rc::downgrade(target), x, y, duration, ease: 0, tweener: None }
    }
}

pub fn new(a: Action) -> ActionRef {
    Rc::new(RefCell::new(a))
}

/// Adds an action to a sequence or a parallel (FlexDMD's `ICompositeAction.Add`).
pub fn add(composite: &ActionRef, a: ActionRef) {
    match &mut *composite.borrow_mut() {
        Action::Sequence { actions, .. } => actions.push(a),
        Action::Parallel { actions, running } => {
            actions.push(a);
            running.push(true);
        }
        _ => {}
    }
}

/// FlexDMD's `Update`: true when the action is done. An action can't run
/// inside itself (FlexDMD would never return): it waits then.
pub fn update(a: &ActionRef, dt: f32) -> bool {
    let Ok(mut act) = a.try_borrow_mut() else { return false };
    match &mut *act {
        Action::Repeat { action, count, n } => {
            if update(action, dt) {
                *n += 1;
                if *n == *count {
                    *n = 0;
                    return true;
                }
            }
            false
        }
        Action::Sequence { actions, pos } => {
            if *pos >= actions.len() {
                *pos = 0;
                return true;
            }
            loop {
                let current = actions[*pos].clone();
                if !update(&current, dt) {
                    return false;
                }
                *pos += 1;
                if *pos >= actions.len() {
                    *pos = 0;
                    return true;
                }
            }
        }
        Action::Parallel { actions, running } => {
            let mut alive = false;
            for (act, run) in actions.clone().iter().zip(running.iter_mut()) {
                if *run {
                    if update(act, dt) {
                        *run = false;
                    } else {
                        alive = true;
                    }
                }
            }
            if !alive {
                running.iter_mut().for_each(|r| *r = true);
                return true;
            }
            false
        }
        Action::Show { target, visible } => {
            let (target, visible) = (target.upgrade(), *visible);
            drop(act);
            if let Some(t) = target {
                actor::set_visible(&t, visible);
            }
            true
        }
        Action::Blink { target, show, hide, repeat, n, time } => {
            let Some(t) = target.upgrade() else { return true };
            *time += dt;
            let visible = t.borrow().visible;
            if visible && *time > *show {
                *time -= *show;
                *n += 1;
                let done = *repeat >= 0 && *n > *repeat;
                drop(act);
                actor::set_visible(&t, false);
                return done;
            } else if !visible && *time > *hide {
                *time -= *hide;
                drop(act);
                actor::set_visible(&t, true);
            }
            false
        }
        Action::AddTo { target, parent, add } => {
            let (target, parent, add) = (target.upgrade(), parent.clone(), *add);
            drop(act);
            if let Some(t) = target {
                if add { actor::add_actor(&parent, &t, None) } else { actor::remove_actor(&parent, &t) }
            }
            true
        }
        Action::RemoveFromParent { target } => {
            let target = target.upgrade();
            drop(act);
            if let Some(t) = target {
                actor::remove(&t);
            }
            true
        }
        Action::AddChild { target, child, add } => {
            let (target, child, add) = (target.upgrade(), child.clone(), *add);
            drop(act);
            if let Some(t) = target {
                if add { actor::add_actor(&t, &child, None) } else { actor::remove_actor(&t, &child) }
            }
            true
        }
        Action::Seek { target, position } => {
            let (target, position) = (target.upgrade(), *position);
            drop(act);
            if let Some(t) = target {
                animated::seek(&t, position);
            }
            true
        }
        Action::Wait { seconds, time } => {
            *time += dt;
            if *time >= *seconds {
                *time = 0.0;
                return true;
            }
            false
        }
        Action::Delayed { seconds, action, time } => {
            *time += dt;
            if *time >= *seconds && update(action, dt) {
                *time = 0.0;
                return true;
            }
            false
        }
        Action::MoveTo(m) => {
            let Some(target) = m.target.upgrade() else { return true };
            if m.tweener.is_none() {
                let mut tw = Tweener::default();
                let id = tw.tween(&target, &[(Prop::X, m.x), (Prop::Y, m.y)], m.duration, 0.0);
                tw.set_ease(id, tween::ease_for(m.ease));
                m.tweener = Some((tw, id));
            }
            let done = m.tweener.as_mut().is_some_and(|(tw, id)| tw.update(dt).contains(id));
            if done {
                m.tweener = None;
            }
            done
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flexdmd::actor::Actor;

    fn staged() -> (ActorRef, ActorRef) {
        let rt = Rc::new(std::cell::Cell::new(1008));
        let stage = Actor::new_group("Stage", &rt);
        stage.borrow_mut().on_stage = true;
        let f = Actor::new_frame("F");
        actor::add_actor(&stage, &f, None);
        (stage, f)
    }

    #[test]
    fn a_sequence_runs_its_actions_in_turn_then_starts_over() {
        let (_stage, f) = staged();
        let w = Rc::downgrade(&f);
        let seq = new(Action::Sequence { actions: Vec::new(), pos: 0 });
        add(&seq, new(Action::Show { target: w.clone(), visible: false }));
        add(&seq, new(Action::Wait { seconds: 0.5, time: 0.0 }));
        add(&seq, new(Action::Show { target: w.clone(), visible: true }));
        assert!(!update(&seq, 0.25));
        assert!(!f.borrow().visible);
        assert!(update(&seq, 0.25));
        assert!(f.borrow().visible);
        // again from the start
        assert!(!update(&seq, 0.1));
        assert!(!f.borrow().visible);
    }

    #[test]
    fn a_repeat_of_zero_or_less_never_ends() {
        let wait = new(Action::Wait { seconds: 0.0, time: 0.0 });
        let rep = new(Action::Repeat { action: wait.clone(), count: -1, n: 0 });
        for _ in 0..100 {
            assert!(!update(&rep, 0.1));
        }
        let rep = new(Action::Repeat { action: wait, count: 3, n: 0 });
        assert!(!update(&rep, 0.1) && !update(&rep, 0.1) && update(&rep, 0.1));
    }

    #[test]
    fn a_move_takes_its_duration_and_a_frame() {
        let (_stage, f) = staged();
        let mv = new(Action::MoveTo(MoveTo::new(&f, 10.0, 0.0, 1.0)));
        assert!(!update(&mv, 0.5));
        assert_eq!(f.borrow().x, 0.0);
        assert!(!update(&mv, 0.5));
        assert_eq!(f.borrow().x, 5.0);
        assert!(update(&mv, 0.5));
        assert_eq!(f.borrow().x, 10.0);
    }

    #[test]
    fn actors_skip_the_action_after_a_finished_one_for_a_frame() {
        let (_stage, f) = staged();
        let w = Rc::downgrade(&f);
        // two shows: the first finishes and is removed, the second waits a frame
        f.borrow_mut().actions.push(new(Action::Show { target: w.clone(), visible: false }));
        f.borrow_mut().actions.push(new(Action::Show { target: w.clone(), visible: true }));
        actor::update(&f, 0.016);
        assert!(!f.borrow().visible);
        assert_eq!(f.borrow().actions.len(), 1);
        actor::update(&f, 0.016);
        assert!(f.borrow().visible);
        assert!(f.borrow().actions.is_empty());
    }

    #[test]
    fn an_action_inside_itself_waits() {
        let seq = new(Action::Sequence { actions: Vec::new(), pos: 0 });
        add(&seq, seq.clone());
        assert!(!update(&seq, 0.1));
        // the cycle, broken so the test frees it
        if let Action::Sequence { actions, .. } = &mut *seq.borrow_mut() {
            actions.clear();
        }
    }
}
