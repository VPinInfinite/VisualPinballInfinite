//! What the app is doing in the background, for the bar beside Add tables: each
//! task's words and how far it's come, told to the screens (`tasks`) whenever it changes. A task
//! may wait its turn (making media, one table at a time) and may have steps, each said done as it
//! is (hovering shows every task queued and in progress).

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: u64,
    pub label: String,
    /// How many of `of` are done; `of` 0: no telling how far.
    pub done: u32,
    pub of: u32,
    /// Waiting its turn, not begun.
    pub waiting: bool,
    /// What it goes through, in order, each done or not.
    pub steps: Vec<Step>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub label: String,
    pub done: bool,
}

/// The tasks running, and the last ID given.
#[derive(Default)]
pub struct Tasks(Mutex<(u64, Vec<Task>)>);

/// A task running: told to the screens as it goes, and gone when it's dropped.
pub struct Running {
    app: AppHandle,
    id: u64,
}

/// A task begun: `of` steps, or 0 for one with no telling how far.
pub fn begin(app: &AppHandle, label: impl Into<String>, of: u32) -> Running {
    queue(app, label, of, false)
}

/// A task that waits its turn, said as waiting until `Running::start`.
pub fn waiting(app: &AppHandle, label: impl Into<String>) -> Running {
    queue(app, label, 0, true)
}

fn queue(app: &AppHandle, label: impl Into<String>, of: u32, waiting: bool) -> Running {
    let id = {
        let tasks = app.state::<Tasks>();
        let mut t = tasks.0.lock().unwrap();
        t.0 += 1;
        let id = t.0;
        t.1.push(Task { id, label: label.into(), done: 0, of, waiting, steps: Vec::new() });
        id
    };
    tell(app);
    Running { app: app.clone(), id }
}

impl Running {
    fn change(&self, f: impl FnOnce(&mut Task)) {
        {
            let tasks = self.app.state::<Tasks>();
            let mut t = tasks.0.lock().unwrap();
            if let Some(task) = t.1.iter_mut().find(|t| t.id == self.id) {
                f(task);
            }
        }
        tell(&self.app);
    }

    /// How far it's come, and what it's doing now, where that changes.
    pub fn step(&self, done: u32, label: Option<String>) {
        self.change(|task| {
            task.done = done;
            if let Some(l) = label {
                task.label = l;
            }
        });
    }

    /// How many of how many are done, and what it's doing, for a task whose count it learns as it goes.
    pub fn count(&self, done: u32, of: u32, label: String) {
        self.change(|task| {
            task.done = done;
            task.of = of;
            task.label = label;
        });
    }

    /// Its turn come: begun, with what it goes through, in order (none: no telling how far).
    pub fn start(&self, label: impl Into<String>, steps: Vec<String>) {
        let label = label.into();
        self.change(|task| {
            task.waiting = false;
            task.label = label;
            task.done = 0;
            task.of = steps.len() as u32;
            task.steps = steps.into_iter().map(|label| Step { label, done: false }).collect();
        });
    }

    /// Its `n`th step done.
    pub fn done(&self, n: usize) {
        self.change(|task| {
            if let Some(s) = task.steps.get_mut(n) {
                s.done = true;
            }
            task.done = task.steps.iter().filter(|s| s.done).count() as u32;
        });
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.app.state::<Tasks>().0.lock().unwrap().1.retain(|t| t.id != self.id);
        tell(&self.app);
    }
}

fn tell(app: &AppHandle) {
    let list = app.state::<Tasks>().0.lock().unwrap().1.clone();
    let _ = app.emit("tasks", list);
}

/// The tasks running now, for a screen that opens while they are.
pub fn now(app: &AppHandle) -> Vec<Task> {
    app.state::<Tasks>().0.lock().unwrap().1.clone()
}
