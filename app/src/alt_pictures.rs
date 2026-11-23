//! A copy of a table with the player's own pictures in place of some of its own, made from its
//! page: the pictures kept for it (`<table> pictures`, each named as the picture it stands in for)
//! written into a copy of the table file beside it (`library::alt_pictures`), as VPX reads no
//! folder of pictures and draws a picture a script loads plain grey. The copy is put with the
//! table as one of its builds (`User::builds_with`), its VPS match the table's where the user
//! chose that one; made again, the copy before goes to the Recycle Bin first.

use tauri::{AppHandle, Manager};

use crate::library::{Shared, key};

/// The copy of table `id` with its own pictures made, in the background, the tasks bar told: the
/// copy's ID once it's in the library, and the kept pictures no picture of the table is named as.
#[tauri::command]
pub async fn alt_pictures_copy(app: AppHandle, id: String) -> Result<serde_json::Value, String> {
    let (table, pictures, short) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        if t.holds.pictures.is_empty() {
            return Err("No pictures are kept for it: drop pictures named as its own onto its page first.".into());
        }
        (t.path.clone(), t.holds.pictures.clone(), crate::library::view(&st, t).short)
    };
    let stem = ::library::alt_pictures::copy_stem(&table);
    let copy = table.with_file_name(format!("{stem}.vpx"));
    let handle = app.clone();
    let source = table.clone();
    let made = tauri::async_runtime::spawn_blocking(move || {
        let task = crate::tasks::begin(&handle, format!("Making {short} with your pictures"), 1);
        // the copy made before, and its own files, to the Recycle Bin first
        if copy.exists() {
            for ext in ["vpx", "directb2s", "ini", "pov", "vbs", "vpp"] {
                let old = copy.with_extension(ext);
                if old.exists() {
                    ::library::install::to_recycle_bin(&old)?;
                }
            }
        }
        let made = ::library::alt_pictures::make_copy(&source, &pictures, &stem);
        drop(task);
        made
    })
    .await
    .map_err(|e| e.to_string())??;
    // one of the table's builds, matched on VPS as the table is where the user chose its match
    {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        let (from, to) = (key(&table), key(&made.table));
        st.user.builds_with.insert(to.clone(), from.clone());
        st.user.builds_apart.remove(&to);
        if let Some(choice) = st.user.vps.get(&from).cloned() {
            st.user.vps.insert(to, choice);
        }
        st.user.save()?;
    }
    crate::library::start(app.clone());
    Ok(serde_json::json!({ "id": crate::library::id_of(&made.table), "unknown": made.unknown, "swapped": made.swapped }))
}

/// Whether table `id` has a copy with its own pictures beside it: the copy's ID.
pub fn copy_of(st: &crate::library::State, t: &crate::library::Table) -> Option<String> {
    let copy = t.path.with_file_name(format!("{}.vpx", ::library::alt_pictures::copy_stem(&t.path)));
    st.tables.iter().find(|x| key(&x.path) == key(&copy)).map(|x| x.id.clone())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_copy_sits_beside_the_table() {
        let t = std::path::Path::new("C:/Tables/Leprechaun King (Orbital 2020).vpx");
        assert_eq!(t.with_file_name(format!("{}.vpx", ::library::alt_pictures::copy_stem(t))), std::path::Path::new("C:/Tables/Leprechaun King (Orbital 2020) (alt pictures).vpx"));
    }
}
