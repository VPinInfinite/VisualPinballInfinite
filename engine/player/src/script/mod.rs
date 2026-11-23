//! Table scripts: existing tables' VBScript, run by libwinevbs, with
//! our script objects in place of VPX's, inside the fence.
//!
//! Everything here runs on the game thread. While the script runs, the
//! objects it calls reach the game's state through a context set by
//! [`enter`]; whoever calls into the script sets it, and holds no other
//! reference to that state until the call returns.

pub mod api;
pub mod com;
pub mod controller;
pub mod fence;
pub mod files;
pub mod flexdmd;
pub mod host;
pub mod idl;
pub mod idl_types;
pub mod log;
pub mod object;
pub mod pup;
pub mod site;
pub mod value;
pub mod winevbs;
pub mod wmp;

use std::cell::Cell;

use physics_compat::engine::PhysicsEngine;

use crate::world::World;
use host::ScriptHost;
use value::Ret;

/// The game's state while the script runs.
#[derive(Clone, Copy)]
pub struct Ctx {
    pub engine: *mut PhysicsEngine,
    pub world: *mut World,
    pub host: *const ScriptHost,
}

thread_local! {
    static CTX: Cell<Option<Ctx>> = const { Cell::new(None) };
}

/// Makes the game's state reachable from script callbacks while `f` runs.
/// Calls nest: an event fired from inside a script call keeps the context.
pub fn enter<R>(engine: &mut PhysicsEngine, world: &mut World, host: Option<&ScriptHost>, f: impl FnOnce() -> R) -> R {
    let ctx = Ctx { engine, world, host: host.map_or(std::ptr::null(), |h| h) };
    let previous = CTX.replace(Some(ctx));
    let r = f();
    CTX.set(previous);
    r
}

/// The game's state, from inside a script callback. The borrow lasts only
/// for `f`, which must not call into the script.
pub fn with<R>(f: impl FnOnce(&mut PhysicsEngine, &mut World) -> R) -> Option<R> {
    let ctx = CTX.get()?;
    // SAFETY: `enter` made these pointers from live exclusive references
    // whose owners don't touch them until `enter` returns; this borrow ends
    // with `f`, and `f` doesn't reach the script, so no two borrows overlap.
    Some(unsafe { f(&mut *ctx.engine, &mut *ctx.world) })
}

/// The running script, if there is one.
fn host() -> Option<&'static ScriptHost> {
    let ctx = CTX.get()?;
    // SAFETY: the host outlives every `enter` it is passed to
    (!ctx.host.is_null()).then(|| unsafe { &*ctx.host })
}

/// Calls a script routine by name, if the script has one.
pub fn call(name: &str, args: Vec<Ret>) -> bool {
    host().is_some_and(|h| h.call(name, args))
}

/// VPX's `FireVoidEvent`: the part's own event only (`Bumper1_Init`).
pub fn fire_part(part: u32, event: &str, args: Vec<Ret>) {
    let Some(name) = with(|_, w| w.parts.get(part as usize).map(|p| p.name.clone())).flatten() else { return };
    call(&format!("{name}_{event}"), args);
}

/// VPX's `FireGroupEvent`: first each collection that fires events for
/// the part (`Bumpers_Hit(index)`), then the part's own event, unless a
/// collection stops it.
pub fn fire_group(part: u32, event: &str) {
    let Some((name, collections, single)) = with(|_, w| {
        let p = w.parts.get(part as usize)?;
        let collections: Vec<(String, i32)> =
            p.event_collections.iter().map(|&(c, i)| (w.collections[c as usize].name.clone(), i)).collect();
        Some((p.name.clone(), collections, p.single_events))
    })
    .flatten() else {
        return;
    };
    for (collection, index) in collections {
        call(&format!("{collection}_{event}"), vec![Ret::Int(index)]);
    }
    if single {
        call(&format!("{name}_{event}"), Vec::new());
    }
}

/// An event of the table itself (`Table1_KeyDown`).
pub fn fire_table(event: &str, args: Vec<Ret>) {
    let Some(name) = with(|_, w| w.table_name.clone()) else { return };
    call(&format!("{name}_{event}"), args);
}
