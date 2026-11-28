//! A table's builds: the table files of one table, which the library can show as one tile with
//! the table files as tabs on its page (Settings › Library). Table files are one table's where VPS
//! matches them to the same machine, surely or by the user's word; one VPS doesn't know stays on
//! its own. The user takes one out ("Not this table", `User::builds_apart`), puts one in by hand
//! ("Add a build", `User::builds_with`), and chooses the one the tile shows and plays whichever was
//! played last (`User::main_builds`). Two table files that look like one saved twice (a copy's
//! name, the same ROM) are told on their pages, until the user keeps both (`User::twins_kept`).

use tauri::{AppHandle, Emitter, State as TauriState};

use crate::library::{Shared, State, Table, User, key};

/// The group of the table file `k` among the library's: the group of the table file it was put
/// with by hand, else its own where it's kept apart, else its machine's on VPS (`game`), else its
/// own.
pub fn group_in(user: &User, k: &str, game: &dyn Fn(&str) -> Option<String>) -> String {
    let mut k = k.to_string();
    // put with another, which may itself be put with another (never round in a ring)
    for _ in 0..8 {
        match user.builds_with.get(&k) {
            Some(to) if *to != k => k = to.clone(),
            _ => break,
        }
    }
    if user.builds_apart.contains(&k) {
        return format!("file:{k}");
    }
    match game(&k) {
        Some(g) => format!("vps:{g}"),
        None => format!("file:{k}"),
    }
}

/// A table's machine on VPS, where it's matched surely (a guess, confirmed by none, groups nothing).
fn sure_game(t: &Table) -> Option<String> {
    // a Future Pinball table a build of its own game's Future Pinball tables alone, apart from VPX's
    let fp = if ::library::future_pinball::is_table(&t.path) { ":fp" } else { "" };
    t.on_vps.game.clone().filter(|_| t.on_vps.sure).map(|g| format!("{g}{fp}"))
}

/// The group of the table `t`, as the library holds it.
pub fn group_of(st: &State, t: &Table) -> String {
    let game = |k: &str| st.tables.iter().find(|x| key(&x.path) == k).and_then(sure_game);
    group_in(&st.user, &key(&t.path), &game)
}

/// A table file's name without a copy's mark: ` (1)`, ` - Copy`, ` - Copy (2)`, ` copy`, `Copy of `,
/// in lower case.
pub fn copy_free(stem: &str) -> String {
    let mut s = stem.trim().to_lowercase();
    loop {
        let before = s.clone();
        if let Some(rest) = s.strip_prefix("copy of ") {
            s = rest.trim().to_string();
        }
        // a small number in brackets at its end (a year in brackets is the name's own)
        if s.ends_with(')')
            && let Some(open) = s.rfind('(')
            && open > 0
            && (1..=3).contains(&(s.len() - open - 2))
            && s[open + 1..s.len() - 1].chars().all(|c| c.is_ascii_digit())
        {
            s = s[..open].trim_end().to_string();
        }
        for mark in [" - copy", " copy"] {
            if let Some(rest) = s.strip_suffix(mark) {
                s = rest.trim_end().to_string();
            }
        }
        if s == before {
            return s;
        }
    }
}

/// Another table file of the library that looks like the table file `t` saved twice — its name
/// the same but for a copy's mark, the same ROM (or none) — unless the user kept both as builds:
/// its ID.
pub fn twin_of<'a>(st: &'a State, t: &Table) -> Option<&'a str> {
    if st.user.twins_kept.contains(&key(&t.path)) {
        return None;
    }
    let me = copy_free(&t.stem);
    st.tables
        .iter()
        .find(|o| o.id != t.id && copy_free(&o.stem) == me && o.holds.rom_set == t.holds.rom_set && !st.user.twins_kept.contains(&key(&o.path)))
        .map(|o| o.id.as_str())
}

fn table_key(st: &State, id: &str) -> Result<String, String> {
    st.tables.iter().find(|t| t.id == id).map(|t| key(&t.path)).ok_or_else(|| "That table isn't in the library any more.".to_string())
}

/// The table file `id` chosen as its table's main build (`on`), or not any more: the table's tile
/// shows and plays it, whichever was played last.
#[tauri::command]
pub fn build_main(app: AppHandle, shared: TauriState<'_, Shared>, id: String, on: bool) -> Result<(), String> {
    {
        let mut st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        let (k, group) = (key(&t.path), group_of(&st, t));
        if on {
            st.user.main_builds.insert(group, k);
        } else if st.user.main_builds.get(&group) == Some(&k) {
            st.user.main_builds.remove(&group);
        }
        st.user.save()?;
    }
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// The table file `id` taken out of its table's builds ("Not this table": a table of its own), or
/// let join them again.
#[tauri::command]
pub fn build_apart(app: AppHandle, shared: TauriState<'_, Shared>, id: String, apart: bool) -> Result<(), String> {
    {
        let mut st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        let (k, group) = (key(&t.path), group_of(&st, t));
        if apart {
            // put with none by hand any more, nor its table's main build
            st.user.builds_with.remove(&k);
            if st.user.main_builds.get(&group) == Some(&k) {
                st.user.main_builds.remove(&group);
            }
            // those put with it by hand stay with its table's other builds: they're put with
            // one of those instead, else left on their own with it
            let others: Vec<String> = st.tables.iter().filter(|x| x.id != id && group_of(&st, x) == group).map(|x| key(&x.path)).collect();
            let joined: Vec<String> = st.user.builds_with.iter().filter(|(_, to)| **to == k).map(|(from, _)| from.clone()).collect();
            for from in joined {
                match others.iter().find(|o| **o != from) {
                    Some(o) => {
                        st.user.builds_with.insert(from, o.clone());
                    }
                    None => {
                        st.user.builds_with.remove(&from);
                    }
                }
            }
            st.user.builds_apart.insert(k);
        } else {
            st.user.builds_apart.remove(&k);
        }
        st.user.save()?;
    }
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// The table file `id` put with the table file `to`'s builds by hand ("Add a build").
#[tauri::command]
pub fn build_add(app: AppHandle, shared: TauriState<'_, Shared>, id: String, to: String) -> Result<(), String> {
    {
        let mut st = shared.0.lock().unwrap();
        let (k, to_key) = (table_key(&st, &id)?, table_key(&st, &to)?);
        if k == to_key {
            return Ok(());
        }
        st.user.builds_apart.remove(&k);
        // not round in a ring: the one it joins put with it is taken off it first
        if st.user.builds_with.get(&to_key) == Some(&k) {
            st.user.builds_with.remove(&to_key);
        }
        st.user.builds_with.insert(k, to_key);
        st.user.save()?;
    }
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// The table file `id` and the one that looks like it saved twice kept as builds of their own:
/// neither asked about again.
#[tauri::command]
pub fn builds_keep_both(app: AppHandle, shared: TauriState<'_, Shared>, id: String) -> Result<(), String> {
    {
        let mut st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        let mut keys = vec![key(&t.path)];
        if let Some(twin) = twin_of(&st, t).and_then(|tw| st.tables.iter().find(|x| x.id == tw)) {
            keys.push(key(&twin.path));
        }
        st.user.twins_kept.extend(keys);
        st.user.save()?;
    }
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// The groups' keys of every table, by table key: for the tests, the rules alone.
#[cfg(test)]
fn groups(user: &User, games: &std::collections::BTreeMap<String, Option<String>>) -> std::collections::BTreeMap<String, String> {
    let game = |k: &str| games.get(k).cloned().flatten();
    games.keys().map(|k| (k.clone(), group_in(user, k, &game))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn a_copys_mark_taken_off() {
        assert_eq!(copy_free("Batman (Data East 1991) VPW v1.1 (1)"), "batman (data east 1991) vpw v1.1");
        assert_eq!(copy_free("The Addams Family (Bally 1992) 2.4.41 - Copy"), "the addams family (bally 1992) 2.4.41");
        assert_eq!(copy_free("Copy of Jaws - Copy (2)"), "jaws");
        // a year or a maker in brackets is the name's own
        assert_eq!(copy_free("Twilight Zone (Bally 1993)"), "twilight zone (bally 1993)");
        assert_eq!(copy_free("Time Lord (1993)"), "time lord (1993)");
        assert_eq!(copy_free("Time Lord 4.2"), "time lord 4.2");
    }

    #[test]
    fn table_files_grouped_by_their_machine_and_the_users_word() {
        let games: BTreeMap<String, Option<String>> = [
            ("tz vpw.vpx", Some("tz")),
            ("tz bigus.vpx", Some("tz")),
            ("tz other.vpx", Some("tz")),
            ("time lord 4.2.vpx", Some("tl")),
            ("mine.vpx", None),
            ("mine 2.vpx", None),
        ]
        .into_iter()
        .map(|(k, g)| (k.to_string(), g.map(str::to_string)))
        .collect();
        let mut user = User::default();
        let g = groups(&user, &games);
        assert_eq!(g["tz vpw.vpx"], "vps:tz");
        assert_eq!(g["tz bigus.vpx"], "vps:tz");
        // one VPS doesn't know stays on its own
        assert_eq!(g["mine.vpx"], "file:mine.vpx");
        assert_ne!(g["mine.vpx"], g["mine 2.vpx"]);

        // "Not this table" takes one out; "Add a build" puts one in, a table VPS doesn't know too
        user.builds_apart.insert("tz other.vpx".into());
        user.builds_with.insert("mine.vpx".into(), "time lord 4.2.vpx".into());
        user.builds_with.insert("mine 2.vpx".into(), "mine.vpx".into());
        let g = groups(&user, &games);
        assert_eq!(g["tz other.vpx"], "file:tz other.vpx");
        assert_eq!(g["tz bigus.vpx"], "vps:tz");
        assert_eq!(g["mine.vpx"], "vps:tl");
        assert_eq!(g["mine 2.vpx"], "vps:tl");

        // put with each other in a ring: each settles, nothing loops
        user.builds_with.insert("time lord 4.2.vpx".into(), "mine 2.vpx".into());
        let g = groups(&user, &games);
        assert!(g.values().all(|v| v.starts_with("vps:") || v.starts_with("file:")));
    }
}
