//! The switch to a folder for each game, the app's side (`library::layout_switch`): the plan made
//! for the library as it is — every table, what it holds and the folder its game would get, VPS's
//! name for it — and, on the user's word, carried out and its record kept, each table's own records
//! and pictures carried along as a rename carries them; put back the same way. Off the window's
//! thread; the library read again after.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library::{self, Shared};

/// Where the library's tables are: how many in a folder of their own, how many together in the
/// tables folder, and the switch kept to put back, if one was made here.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutState {
    pub own: usize,
    pub together: usize,
    pub switched: Option<SwitchedView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchedView {
    pub at: String,
    pub tables: usize,
    pub folders: usize,
}

#[tauri::command]
pub fn layout_state(shared: State<'_, Shared>) -> Result<LayoutState, String> {
    let st = shared.0.lock().unwrap();
    let layout = st.layout.as_ref().ok_or("The library isn't read yet.")?;
    let key = |p: &std::path::Path| p.to_string_lossy().to_lowercase();
    let together = st.tables.iter().filter(|t| t.path.parent().is_some_and(|d| key(d) == key(&layout.tables))).count();
    let switched = st.user.switched.as_ref().map(|s| {
        let folders: std::collections::BTreeSet<String> = s.tables.iter().filter_map(|(_, to)| to.parent().map(key)).collect();
        SwitchedView { at: s.at.clone(), tables: s.tables.len(), folders: folders.len() }
    });
    Ok(LayoutState { own: st.tables.len() - together, together, switched })
}

/// What switching would do, nothing moved: the plan, how many tables move, and whether PinUP
/// Popper's list follows them; a front end the switch can't yet follow a table for is a problem.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    #[serde(flatten)]
    pub plan: ::library::layout_switch::Plan,
    pub tables: usize,
    pub popper: bool,
}

#[tauri::command]
pub async fn layout_plan(shared: State<'_, Shared>) -> Result<PlanView, String> {
    let (layout, tables) = {
        let st = shared.0.lock().unwrap();
        (st.layout.clone().ok_or("The library isn't read yet.")?, library::layout_tables(&st))
    };
    tauri::async_runtime::spawn_blocking(move || {
        let list: Vec<::library::layout_switch::Table> = tables.iter().map(|(path, holds, folder)| ::library::layout_switch::Table { path, holds, folder: folder.clone() }).collect();
        let mut plan = ::library::layout_switch::plan(&layout, &list);
        let moving = plan.folders.values().map(Vec::len).sum();
        for f in ::library::frontends::find(&layout) {
            plan.problems.push(format!("{} plays these tables, and following a table into a folder of its own isn't built for it yet", f.front.name()));
        }
        let popper = ::library::popper::database(&layout).is_some();
        PlanView { plan, tables: moving, popper }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Told to the screens as each game's folder is begun.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    of: usize,
    name: String,
}

/// The library moved into a folder for each game, as planned now; refused while a switch is kept
/// to put back.
#[tauri::command]
pub async fn layout_switch(app: AppHandle, shared: State<'_, Shared>) -> Result<(), String> {
    let (layout, tables) = {
        let st = shared.0.lock().unwrap();
        if st.user.switched.is_some() {
            return Err("The library is in folders of its games already: put it back first.".into());
        }
        (st.layout.clone().ok_or("The library isn't read yet.")?, library::layout_tables(&st))
    };
    let kept = library::data_folder().ok_or("no APPDATA folder")?.join("Library").join("Kept");
    let tell = app.clone();
    let switched = tauri::async_runtime::spawn_blocking(move || {
        let list: Vec<::library::layout_switch::Table> = tables.iter().map(|(path, holds, folder)| ::library::layout_switch::Table { path, holds, folder: folder.clone() }).collect();
        let plan = ::library::layout_switch::plan(&layout, &list);
        let progress = |done: usize, of: usize, name: &str| {
            let _ = tell.emit("layout-progress", Progress { done, of, name: name.to_string() });
        };
        ::library::layout_switch::carry_out(&layout, &plan, &kept, &library::stamp(), &progress)
    })
    .await
    .map_err(|e| e.to_string())??;
    {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        let moved = switched.tables.clone();
        library::tables_moved(&mut st, &moved, Some(switched))?;
    }
    // tables added from now on go into their games' folders
    crate::setup::keep_layout(true)?;
    library::start(app);
    Ok(())
}

/// The switch put back, as its record says.
#[tauri::command]
pub async fn layout_put_back(app: AppHandle, shared: State<'_, Shared>) -> Result<(), String> {
    let (layout, switched) = {
        let st = shared.0.lock().unwrap();
        (st.layout.clone().ok_or("The library isn't read yet.")?, st.user.switched.clone().ok_or("The library hasn't been moved into folders of its games.")?)
    };
    let record = switched.clone();
    tauri::async_runtime::spawn_blocking(move || ::library::layout_switch::put_back(&layout, &record, &::library::install::to_recycle_bin)).await.map_err(|e| e.to_string())??;
    {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        let back: Vec<(std::path::PathBuf, std::path::PathBuf)> = switched.tables.iter().map(|(from, to)| (to.clone(), from.clone())).collect();
        library::tables_moved(&mut st, &back, None)?;
    }
    crate::setup::keep_layout(false)?;
    library::start(app);
    Ok(())
}
