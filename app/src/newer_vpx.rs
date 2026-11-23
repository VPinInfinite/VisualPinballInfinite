//! Tables chosen to play with VPX 10.8.0 because 10.8.1 stopped them, tried again in each newer
//! 10.8.1 build the library gets: quietly, unseen and unheard, one at a time in the background,
//! never while a game plays. Where one plays now, its page asks whether to play it with 10.8.1
//! again (`User::newer_plays`); where it still doesn't, the build is noted and the next one tries
//! it (`User::older_tried`).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::library::Shared;

static RUNNING: AtomicBool = AtomicBool::new(false);
static AGAIN: AtomicBool = AtomicBool::new(false);

/// The tables to try in the library's newest 10.8.1 build `newest`: those chosen to play with
/// another VPX line (`play_with`), not waiting on the user's word already, last found not to play
/// in an older build than `newest` (or never tried).
pub fn to_try(play_with: &BTreeMap<String, String>, older_tried: &BTreeMap<String, u32>, newer_plays: &BTreeMap<String, u32>, newest: u32) -> Vec<String> {
    play_with
        .keys()
        .filter(|k| !newer_plays.contains_key(*k))
        .filter(|k| older_tried.get(*k).is_none_or(|b| *b < newest))
        .cloned()
        .collect()
}

/// The library's newest VPX 10.8.1, the one that takes add-ons, and its build.
pub fn newest_10_8_1(layout: &::library::plan::Layout) -> Option<(::library::vpx::Vpx, u32)> {
    ::library::vpx::all_for(layout).into_iter().find(::library::vpx::Vpx::has_addons).map(|v| {
        let build = u32::from(v.version[3]);
        (v, build)
    })
}

/// The tables tried, on a thread of their own; asked again while it runs, it runs once more after.
pub fn start(app: &AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        AGAIN.store(true, Ordering::SeqCst);
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("newer vpx".into()).spawn(move || {
        loop {
            AGAIN.store(false, Ordering::SeqCst);
            run(&app);
            if !AGAIN.load(Ordering::SeqCst) {
                break;
            }
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
    if spawned.is_err() {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

fn run(app: &AppHandle) {
    // the test mode tries nothing
    if app.state::<Option<crate::shot::Shot>>().is_some() {
        return;
    }
    let (vpx, build, todo) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let Some((vpx, build)) = st.layout.as_ref().and_then(newest_10_8_1) else { return };
        let keys = to_try(&st.user.play_with, &st.user.older_tried, &st.user.newer_plays, build);
        let todo: Vec<(String, std::path::PathBuf, String)> = keys
            .iter()
            .filter_map(|k| st.tables.iter().find(|t| crate::library::key(&t.path) == *k).map(|t| (k.clone(), t.path.clone(), t.stem.clone())))
            .collect();
        (vpx, build, todo)
    };
    if todo.is_empty() {
        return;
    }
    let task = crate::tasks::begin(app, format!("Trying tables in VPX 10.8.1 build {build}"), todo.len() as u32);
    for (n, (key, path, name)) in todo.iter().enumerate() {
        // never beside a game being played
        while app.state::<Shared>().0.lock().unwrap().playing.is_some() {
            std::thread::sleep(Duration::from_secs(5));
        }
        task.step(n as u32, Some(format!("Trying {name} in VPX 10.8.1 build {build}")));
        let report = ::library::vpx_test::test(&vpx, path, &vpi_settings::vpx_settings_text());
        let plays = report.look != "bad";
        {
            let shared = app.state::<Shared>();
            let mut st = shared.0.lock().unwrap();
            // the user's choice changed meanwhile: nothing to note
            if st.user.play_with.contains_key(key) {
                if plays {
                    st.user.newer_plays.insert(key.clone(), build);
                } else {
                    st.user.older_tried.insert(key.clone(), build);
                }
                let _ = st.user.save();
            }
        }
        let _ = app.emit("library-changed", ());
    }
    // the task said over as it goes
    drop(task);
}

/// The user's word on a table found to play in a newer 10.8.1 build: play it with 10.8.1 again
/// (its own choice of VPX taken away), or keep the older VPX, not asked again until a newer build.
#[tauri::command]
pub fn newer_vpx_answer(app: AppHandle, shared: tauri::State<'_, Shared>, id: String, use_newer: bool) -> Result<(), String> {
    {
        let mut st = shared.0.lock().unwrap();
        let key = crate::library::key(&st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?.path);
        let build = st.user.newer_plays.remove(&key);
        if use_newer {
            st.user.play_with.remove(&key);
            st.user.older_tried.remove(&key);
        } else if let Some(b) = build {
            st.user.older_tried.insert(key, b);
        }
        st.user.save()?;
    }
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_tried_in_a_newer_build_once_each() {
        let map = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<BTreeMap<_, _>>();
        let builds = |pairs: &[(&str, u32)]| pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>();
        let play_with = map(&[("a.vpx", "10.8.0"), ("b.vpx", "10.8.0"), ("c.vpx", "10.8.0"), ("d.vpx", "10.8.0")]);
        // a tried in 6681 already, b never, c waiting on the user's word, d tried in 6625
        let older = builds(&[("a.vpx", 6681), ("c.vpx", 6625), ("d.vpx", 6625)]);
        let newer = builds(&[("c.vpx", 6681)]);
        assert_eq!(to_try(&play_with, &older, &newer, 6681), ["b.vpx", "d.vpx"]);
        assert_eq!(to_try(&play_with, &older, &newer, 6700), ["a.vpx", "b.vpx", "d.vpx"]);
        assert!(to_try(&BTreeMap::new(), &older, &newer, 6700).is_empty());
    }
}
