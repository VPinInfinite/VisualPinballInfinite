//! The link to VPIDB, a free, non-commercial home for virtual pinball, where a player who
//! connects with their Discord account can share the achievements they earn, and their ratings and
//! which tables they have, which VPIDB counts for every table ("in so many libraries", its average
//! rating). Connected the way a TV signs in: a code shown here and entered on VPIDB's site after
//! signing in with Discord there (`/api/link/start`, then `/api/link/poll` every few seconds until
//! it's connected, refused or expired). Its token is kept for this Windows account alone (Windows'
//! data protection, in `Library\vpidb.json`), never shown or put on a command line; this PC is
//! named on VPIDB as the player chooses, never by its computer name; what's shared is the player's
//! choice when connecting, both ticked, and either can be switched off at any time — the library's
//! list then taken off VPIDB. Its calls go through Windows' own curl, as the app's others do.
//! `VPI_VPIDB=<address>` points it at another server, for checks.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// VPIDB's site, where its API is (`/api/...`) and where a player connects.
const SITE: &str = "https://vpidb.org";

fn site() -> String {
    std::env::var("VPI_VPIDB").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| SITE.to_string()).trim_end_matches('/').to_string()
}

/// The name this PC goes by on VPIDB until the player chooses another.
const DEVICE: &str = "Visual Pinball Infinite";

/// What's kept of the link: the player VPIDB connected, the token (protected), whether sharing is on
/// and the name this PC goes by there.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Kept {
    player: Option<Player>,
    /// The token, protected by Windows for this account alone, as hex.
    token: String,
    sharing: bool,
    /// Sharing the library's ratings and which tables it holds (`/api/library`).
    share_library: bool,
    /// What was last sent of the library, as a fingerprint, so it's sent again only when it changes.
    library_sent: String,
    device: String,
    /// When it was connected (`2025-01-02 18-04-05`).
    since: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Player {
    pub id: serde_json::Value,
    pub name: String,
}

fn kept_path() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Library").join("vpidb.json"))
}

fn load() -> Kept {
    kept_path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(k: &Kept) -> Result<(), String> {
    let path = kept_path().ok_or("no APPDATA folder")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_vec_pretty(k).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(windows)]
fn protect(data: &[u8], undo: bool) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let done = if undo {
            CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        } else {
            CryptProtectData(&input, windows::core::w!("VPIDB"), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        };
        done.map_err(|e| e.message().to_string())?;
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as _)));
        Ok(bytes)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len()).step_by(2).map(|i| text.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok())).collect()
}

/// A JSON body or header value written into curl's own settings syntax: quoted, its backslashes and
/// quotes escaped, so nothing secret goes on a command line.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A call to VPIDB's API: its answer's status and body.
fn post(path: &str, body: &serde_json::Value, token: Option<&str>) -> Result<(u16, serde_json::Value), String> {
    use std::io::Write;
    let url = format!("{}/api/{path}", site());
    let version = env!("CARGO_PKG_VERSION");
    let mut config = String::new();
    config.push_str(&format!("header = {}\n", quoted("Content-Type: application/json")));
    if let Some(t) = token {
        config.push_str(&format!("header = {}\n", quoted(&format!("Authorization: Bearer {t}"))));
    }
    config.push_str(&format!("data = {}\n", quoted(&body.to_string())));
    let mut curl = ::library::vps::curl();
    curl.args(["-sS", "--max-time", "20", "-A", &format!("VisualPinballInfinite/{version}"), "-K", "-", "-w", "\n%{http_code}", &url])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = curl.spawn().map_err(|e| format!("VPIDB couldn't be reached: {e}"))?;
    child.stdin.take().ok_or("no input for curl")?.write_all(config.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    let code: u16 = code.trim().parse().unwrap_or(0);
    if code == 0 {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("VPIDB couldn't be reached ({}).", if err.trim().is_empty() { "no answer" } else { err.trim() }));
    }
    Ok((code, serde_json::from_str(body).unwrap_or(serde_json::Value::Null)))
}

/// The link as the screens show it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub connected: bool,
    pub player: Option<Player>,
    pub sharing: bool,
    pub share_library: bool,
    pub device: String,
    pub since: String,
    /// VPIDB's site, for its link.
    pub site: String,
}

fn link_of(k: &Kept) -> Link {
    Link {
        connected: k.player.is_some() && !k.token.is_empty(),
        player: k.player.clone(),
        sharing: k.sharing,
        share_library: k.share_library,
        device: if k.device.trim().is_empty() { DEVICE.to_string() } else { k.device.clone() },
        since: k.since.clone(),
        site: site(),
    }
}

#[tauri::command]
pub fn vpidb_link() -> Link {
    link_of(&load())
}

/// A connection asked for and not yet made: its secret code, how often to ask, and until when.
struct Pending {
    device_code: String,
    interval: Duration,
    next: Instant,
    until: Instant,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// What the player sees to connect: the code, and VPIDB's page that takes it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    pub user_code: String,
    pub link: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// A connection asked for, this PC named as the player chose (`device`).
#[tauri::command]
pub async fn vpidb_connect(device: String) -> Result<Start, String> {
    let device: String = device.chars().filter(|c| !c.is_control()).take(60).collect::<String>().trim().to_string();
    let device = if device.is_empty() { DEVICE.to_string() } else { device };
    let (code, body) = tauri::async_runtime::spawn_blocking({
        let device = device.clone();
        move || post("link/start", &serde_json::json!({ "device": device, "app": env!("CARGO_PKG_VERSION") }), None)
    })
    .await
    .map_err(|e| e.to_string())??;
    if code != 200 {
        return Err(said(&body).unwrap_or_else(|| format!("VPIDB didn't start a connection (answer {code}).")));
    }
    let device_code = body["device_code"].as_str().ok_or("VPIDB's answer had no code.")?.to_string();
    let user_code = body["user_code"].as_str().ok_or("VPIDB's answer had no code.")?.to_string();
    let expires_in = body["expires_in"].as_u64().unwrap_or(600).clamp(30, 3600);
    let interval = body["interval"].as_u64().unwrap_or(5).clamp(2, 60);
    // only a page on VPIDB's own site opens from here
    let link = body["link"].as_str().filter(|l| l.starts_with(&format!("{}/", site()))).map(str::to_string).unwrap_or_else(|| site());
    *PENDING.lock().unwrap() = Some(Pending { device_code, interval: Duration::from_secs(interval), next: Instant::now() + Duration::from_secs(interval), until: Instant::now() + Duration::from_secs(expires_in) });
    let mut k = load();
    k.device = device;
    save(&k)?;
    Ok(Start { user_code, link, expires_in, interval })
}

fn said(body: &serde_json::Value) -> Option<String> {
    body["error"].as_str().map(|e| format!("VPIDB said: {}.", e.trim_end_matches('.')))
}

/// Where a connection asked for stands: `waiting`, `connected` (the link kept), `expired`,
/// `refused`, or `none` when none is asked for. Asked no oftener than VPIDB said.
#[tauri::command]
pub async fn vpidb_poll() -> Result<String, String> {
    let (device_code, wait) = {
        let pending = PENDING.lock().unwrap();
        let Some(p) = pending.as_ref() else { return Ok("none".into()) };
        if Instant::now() > p.until {
            drop(pending);
            *PENDING.lock().unwrap() = None;
            return Ok("expired".into());
        }
        (p.device_code.clone(), p.next.saturating_duration_since(Instant::now()))
    };
    if !wait.is_zero() {
        return Ok("waiting".into());
    }
    let (code, body) = tauri::async_runtime::spawn_blocking(move || post("link/poll", &serde_json::json!({ "device_code": device_code }), None)).await.map_err(|e| e.to_string())??;
    if let Some(p) = PENDING.lock().unwrap().as_mut() {
        p.next = Instant::now() + p.interval;
    }
    if code != 200 {
        return Err(said(&body).unwrap_or_else(|| format!("VPIDB couldn't say how the connection stands (answer {code}).")));
    }
    let state = body["state"].as_str().unwrap_or("waiting").to_string();
    match state.as_str() {
        "connected" => {
            let token = body["token"].as_str().ok_or("VPIDB's answer had no token.")?;
            let player: Player = serde_json::from_value(body["player"].clone()).map_err(|_| "VPIDB's answer named no player.")?;
            #[cfg(windows)]
            let token = hex(&protect(token.as_bytes(), false)?);
            #[cfg(not(windows))]
            let token = token.to_string();
            let mut k = load();
            k.player = Some(player);
            k.token = token;
            k.since = crate::library::stamp();
            save(&k)?;
            *PENDING.lock().unwrap() = None;
        }
        "expired" | "refused" => *PENDING.lock().unwrap() = None,
        _ => {}
    }
    Ok(state)
}

/// A connection asked for, given up.
#[tauri::command]
pub fn vpidb_cancel() {
    *PENDING.lock().unwrap() = None;
}

/// Disconnected: on VPIDB first (`link/disconnect`, which ends the token there and takes off the
/// tables and ratings this PC shared, what was earned staying), then here, whatever VPIDB answers.
/// `there`: whether VPIDB disconnected it too — false where it couldn't be reached or doesn't take
/// the call, the library's list then taken off as before and its own page the way.
#[tauri::command]
pub async fn vpidb_disconnect() -> Result<Disconnected, String> {
    let k = load();
    let there = match token() {
        Some(t) => {
            let library = k.share_library;
            tauri::async_runtime::spawn_blocking(move || match post("link/disconnect", &serde_json::json!({}), Some(&t)) {
                // ended now, or before
                Ok((200 | 401, _)) => true,
                _ => {
                    if library {
                        let _ = post("library/forget", &serde_json::json!({}), Some(&t));
                    }
                    false
                }
            })
            .await
            .unwrap_or(false)
        }
        None => false,
    };
    Ok(Disconnected { link: vpidb_forget()?, there })
}

/// What disconnecting did.
#[derive(Serialize)]
pub struct Disconnected {
    pub link: Link,
    pub there: bool,
}

/// The link forgotten here: the token and the player.
pub fn vpidb_forget() -> Result<Link, String> {
    let mut k = load();
    k.player = None;
    k.token.clear();
    k.sharing = false;
    k.share_library = false;
    k.library_sent.clear();
    k.since.clear();
    save(&k)?;
    Ok(link_of(&k))
}

/// Sharing what's earned switched on or off.
#[tauri::command]
pub fn vpidb_sharing(on: bool) -> Result<Link, String> {
    let mut k = load();
    k.sharing = on && k.player.is_some();
    save(&k)?;
    Ok(link_of(&k))
}

/// Sharing the library's ratings and which tables it holds switched on — the list sent — or off,
/// the list taken off VPIDB.
#[tauri::command]
pub async fn vpidb_share_library(app: tauri::AppHandle, on: bool) -> Result<Link, String> {
    let mut k = load();
    k.share_library = on && k.player.is_some();
    k.library_sent.clear();
    save(&k)?;
    if k.share_library {
        library_changed(&app);
    } else if let Some(t) = token() {
        let (code, body) = tauri::async_runtime::spawn_blocking(move || post("library/forget", &serde_json::json!({}), Some(&t))).await.map_err(|e| e.to_string())??;
        match code {
            200 | 204 => {}
            401 => {
                vpidb_forget()?;
            }
            _ => return Err(said(&body).unwrap_or_else(|| format!("VPIDB didn't take your list off (answer {code}). It's off here: switch it on and off again to retry."))),
        }
    }
    Ok(link_of(&load()))
}

/// When the library's list is next to go to VPIDB: a few seconds after the last change, so a run of
/// changes goes as one.
static DUE: Mutex<Option<Instant>> = Mutex::new(None);

/// How sharing with VPIDB last went, for the top bar's mark: `ok` None before anything was sent
/// this run; a problem's words; when, in seconds since 1970.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sync {
    pub connected: bool,
    pub sharing: bool,
    pub ok: Option<bool>,
    pub problem: Option<String>,
    pub at: Option<u64>,
}

static LAST: Mutex<(Option<bool>, Option<String>, Option<u64>)> = Mutex::new((None, None, None));
/// A try again waiting after sharing failed: one at a time.
static RETRYING: Mutex<bool> = Mutex::new(false);

fn sync_state() -> Sync {
    let k = load();
    let last = LAST.lock().unwrap().clone();
    let connected = k.player.is_some() && !k.token.is_empty();
    Sync { connected, sharing: k.sharing || k.share_library, ok: if connected { last.0 } else { None }, problem: if connected { last.1 } else { None }, at: last.2 }
}

/// A try at sharing noted, told to the screens (`vpidb-sync`); a failure tried again every two
/// minutes until VPIDB answers, everything then sent again.
fn note_sync(app: &tauri::AppHandle, result: Result<(), String>) {
    use tauri::Emitter;
    let failed = result.is_err();
    {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        *LAST.lock().unwrap() = (Some(!failed), result.err(), Some(now));
    }
    let _ = app.emit("vpidb-sync", sync_state());
    let connected = sync_state().connected;
    if failed && connected {
        let mut r = RETRYING.lock().unwrap();
        if *r {
            return;
        }
        *r = true;
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(120));
            *RETRYING.lock().unwrap() = false;
            if sync_state().connected {
                note_sync(&app, sync_all(&app));
            }
        });
    }
}

/// Everything shared that's switched on: what's earned, and the library's list where it changed.
fn sync_all(app: &tauri::AppHandle) -> Result<(), String> {
    share_earned()?;
    send_library(app)
}

/// How sharing with VPIDB stands, for the top bar's mark.
#[tauri::command]
pub fn vpidb_status() -> Sync {
    sync_state()
}

/// Everything shared again now (the app starting, or Sync now): what's earned and the library's list.
#[tauri::command]
pub async fn vpidb_sync(app: tauri::AppHandle) -> Result<Sync, String> {
    if !sync_state().connected {
        return Ok(sync_state());
    }
    let a = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || sync_all(&a)).await.map_err(|e| e.to_string())?;
    note_sync(&app, result);
    Ok(sync_state())
}

/// What a play earned, shared as the player ends (the play's thread), noted for the top bar.
pub fn share_after_play(app: &tauri::AppHandle) {
    if !load().sharing {
        return;
    }
    let result = share_earned().map(|n| {
        if n > 0 {
            eprintln!("VPIDB: {n} packs' achievements shared");
        }
    });
    note_sync(app, result);
}

/// The library changed (read again, a table rated): its list sent to VPIDB a few seconds on, where
/// it's shared and has changed since it was last sent.
pub fn library_changed(app: &tauri::AppHandle) {
    let k = load();
    if !k.share_library || k.token.is_empty() {
        return;
    }
    let first = {
        let mut due = DUE.lock().unwrap();
        let first = due.is_none();
        *due = Some(Instant::now() + Duration::from_secs(3));
        first
    };
    if !first {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            let wait = {
                let mut due = DUE.lock().unwrap();
                match *due {
                    Some(t) if t > Instant::now() => t - Instant::now(),
                    _ => {
                        *due = None;
                        break;
                    }
                }
            };
            std::thread::sleep(wait);
        }
        let result = send_library(&app);
        if let Err(e) = &result {
            eprintln!("VPIDB: the library's list wasn't sent: {e}");
        }
        note_sync(&app, result);
    });
}

/// The most tables VPIDB takes in one list (it refuses a longer one).
const MOST: usize = 5_000;

/// The library's list cut to what VPIDB takes, the rated tables kept first.
fn within_limit(mut tables: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    if tables.len() > MOST {
        tables.sort_by_key(|t| t["rating"].is_null());
        tables.truncate(MOST);
    }
    tables
}

/// A fingerprint of what's sent, to tell whether it changed.
fn fingerprint(body: &serde_json::Value) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    body.to_string().hash(&mut h);
    format!("{:016x}", h.finish())
}

/// The library's list sent (`/api/library`): its tables as VPIDB counts them, each with its rating,
/// replacing what this PC sent before; nothing when it's the same as last sent.
fn send_library(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let k = load();
    if !k.share_library {
        return Ok(());
    }
    let tables = {
        let shared = app.state::<crate::library::Shared>();
        let st = shared.0.lock().unwrap();
        // only once the library's been read: an empty list would take every table off
        if st.status.phase != "ready" {
            return Ok(());
        }
        within_limit(crate::library::vpidb_library(&st))
    };
    let body = serde_json::json!({ "tables": tables, "app": env!("CARGO_PKG_VERSION") });
    let print = fingerprint(&body);
    if print == k.library_sent {
        return Ok(());
    }
    let token = token().ok_or("Not connected to VPIDB.")?;
    let (code, answer) = post("library", &body, Some(&token))?;
    match code {
        200 | 204 => {
            let mut k = load();
            k.library_sent = print;
            save(&k)
        }
        401 => {
            vpidb_forget()?;
            Err("VPIDB disconnected this PC: connect again to share.".into())
        }
        _ => Err(said(&answer).unwrap_or_else(|| format!("VPIDB didn't take it (answer {code})."))),
    }
}

/// A file of VPIDB's site fetched (`GET`), within `seconds`: its bytes, or why not.
fn get(url: &str, seconds: u32, to: Option<&std::path::Path>) -> Result<Vec<u8>, String> {
    let version = env!("CARGO_PKG_VERSION");
    let mut curl = ::library::vps::curl();
    curl.args(["-sS", "-L", "--fail", "--max-time", &seconds.to_string(), "-A", &format!("VisualPinballInfinite/{version}")]);
    if let Some(file) = to {
        curl.arg("-o").arg(file);
    }
    curl.arg(url).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let out = curl.output().map_err(|e| format!("VPIDB couldn't be reached: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("VPIDB couldn't be reached ({}).", if err.trim().is_empty() { "no answer" } else { err.trim() }));
    }
    Ok(out.stdout)
}

/// VPIDB's list of achievement packs (`/achievements.json`), kept an hour in the data folder
/// (`VPIDB\achievements.json`) and fetched again after; the kept one where VPIDB doesn't answer.
fn packs_list() -> Result<serde_json::Value, String> {
    let kept = crate::library::data_folder().map(|d| d.join("VPIDB").join("achievements.json"));
    let fresh = kept.as_ref().and_then(|f| std::fs::metadata(f).ok()).and_then(|m| m.modified().ok()).is_some_and(|t| t.elapsed().is_ok_and(|e| e < Duration::from_secs(3_600)));
    let read_kept = || kept.as_ref().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok());
    if fresh && let Some(v) = read_kept() {
        return Ok(v);
    }
    match get(&format!("{}/achievements.json", site()), 20, None).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| format!("VPIDB's list couldn't be read: {e}"))) {
        Ok(v) => {
            if let Some(f) = &kept {
                let _ = std::fs::create_dir_all(f.parent().unwrap_or(f));
                let _ = std::fs::write(f, v.to_string());
            }
            Ok(v)
        }
        Err(e) => read_kept().ok_or(e),
    }
}

/// One of VPIDB's achievement packs, its newest version, as the screens list it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlinePack {
    pub id: String,
    pub name: String,
    pub official: bool,
    /// How VPIDB tags it: `official`, `community`, `sample` or `mod`.
    pub kind: String,
    pub by: String,
    pub version: String,
    pub count: u64,
    pub points: u64,
    pub about: Option<String>,
    /// How many players mastered it.
    pub players: u64,
    /// The version of it kept here, if one is.
    pub installed: Option<String>,
    /// Its page on VPIDB.
    pub page: String,
}

/// VPIDB's achievement packs by the VPS game each is made for, with the version kept here of each.
#[tauri::command]
pub async fn vpidb_packs() -> Result<std::collections::HashMap<String, Vec<OnlinePack>>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let list = packs_list()?;
        let here: std::collections::HashMap<String, String> = crate::achievements::installed().into_iter().map(|(_, p)| (p.pack.id, p.pack.version)).collect();
        let mut out: std::collections::HashMap<String, Vec<OnlinePack>> = std::collections::HashMap::new();
        for p in list["packs"].as_array().into_iter().flatten() {
            let (Some(id), Some(game)) = (p["id"].as_str(), p["game"].as_str()) else { continue };
            let Some(newest) = p["versions"].as_array().and_then(|v| v.first()) else { continue };
            let text = |v: &serde_json::Value| v.as_str().map(str::to_string);
            out.entry(game.to_string()).or_default().push(OnlinePack {
                id: id.to_string(),
                name: text(&p["name"]).unwrap_or_else(|| id.to_string()),
                official: p["official"].as_bool().unwrap_or(false),
                kind: text(&p["kind"]).map(|k| k.to_lowercase()).unwrap_or_else(|| if p["official"].as_bool().unwrap_or(false) { "official".into() } else { "community".into() }),
                by: text(&p["uploaderName"]).unwrap_or_default(),
                version: text(&newest["version"]).unwrap_or_default(),
                count: newest["count"].as_u64().unwrap_or(0),
                points: newest["points"].as_u64().unwrap_or(0),
                about: text(&newest["about"]),
                players: p["players"].as_u64().unwrap_or(0),
                installed: here.get(id).cloned(),
                page: format!("{}/#/achievements", site()),
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// One of VPIDB's packs added: its newest version downloaded, checked against the checksum VPIDB
/// lists, then kept as a pack the user adds is (`achievements::add_from`). Its name.
#[tauri::command]
pub async fn vpidb_pack_add(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let name = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        use sha2::Digest;
        let list = packs_list()?;
        let p = list["packs"].as_array().into_iter().flatten().find(|p| p["id"].as_str() == Some(id.as_str())).ok_or("VPIDB doesn't list that pack any more.")?;
        let newest = p["versions"].as_array().and_then(|v| v.first()).ok_or("VPIDB lists no version of it.")?;
        let url = newest["url"].as_str().ok_or("VPIDB gives no file for it.")?;
        let want = newest["sha256"].as_str().unwrap_or_default().to_lowercase();
        let file = std::env::temp_dir().join(format!("vpi-vpidb-pack-{}.zip", std::process::id()));
        get(url, 60, Some(&file))?;
        let got = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let sum: String = sha2::Sha256::digest(&got).iter().map(|b| format!("{b:02x}")).collect();
        if !want.is_empty() && sum != want {
            let _ = std::fs::remove_file(&file);
            return Err("The pack downloaded isn't the one VPIDB lists (its checksum differs): nothing was added.".into());
        }
        let added = crate::achievements::add_from(&file);
        let _ = std::fs::remove_file(&file);
        added
    })
    .await
    .map_err(|e| e.to_string())??;
    crate::library::start(app);
    Ok(name)
}

/// The token, unprotected, for a call on the player's behalf; None while not connected.
fn token() -> Option<String> {
    let k = load();
    if k.token.is_empty() {
        return None;
    }
    #[cfg(windows)]
    {
        protect(&unhex(&k.token)?, true).ok().and_then(|b| String::from_utf8(b).ok())
    }
    #[cfg(not(windows))]
    {
        Some(k.token)
    }
}

/// Everything earned, each pack's records sent while sharing is on (sending one again is harmless:
/// VPIDB keeps the earliest); how many packs were. None sent while it's off or not connected.
pub fn share_earned() -> Result<usize, String> {
    let k = load();
    if !k.sharing || k.token.is_empty() {
        return Ok(0);
    }
    let Some(file) = crate::achievements::earned_file() else { return Ok(0) };
    let earned = vpi_achievements::earned::Earned::load(&file);
    let packs = crate::achievements::installed();
    let mut sent = 0;
    for (id, mine) in &earned.packs {
        let Some((_, pack)) = packs.iter().find(|(_, p)| &p.pack.id == id) else { continue };
        let records: Vec<serde_json::Value> = mine
            .achievements
            .iter()
            .flat_map(|(a, rs)| rs.iter().map(move |r| serde_json::json!({ "id": a, "at": r.at, "mode": r.mode, "packVersion": r.pack_version })))
            .collect();
        let mastered: Vec<serde_json::Value> = mine.mastered.iter().map(|m| serde_json::json!({ "packVersion": m.pack_version, "at": m.at })).collect();
        let record = serde_json::json!({
            "pack": id,
            "packVersion": pack.pack.version,
            "table": {},
            "earned": records,
            "mastered": mastered,
            "app": env!("CARGO_PKG_VERSION"),
        });
        match send_earned(&record) {
            Ok(_) => sent += 1,
            // a pack VPIDB doesn't hold — the player's own, or one not uploaded — has nothing to
            // be shared with: the others still go
            Err(e) if e == NOT_ON_VPIDB => {}
            Err(e) => return Err(e),
        }
    }
    Ok(sent)
}

/// What `send_earned` says of a pack VPIDB doesn't hold.
const NOT_ON_VPIDB: &str = "That pack isn't on VPIDB.";

/// What a player earned for one pack, sent while sharing is on (`/api/achievements/earned`); a
/// token VPIDB no longer takes (the player disconnected this PC there) forgets the link here.
pub fn send_earned(record: &serde_json::Value) -> Result<serde_json::Value, String> {
    let k = load();
    if !k.sharing {
        return Err("Sharing is off.".into());
    }
    let token = token().ok_or("Not connected to VPIDB.")?;
    let (code, body) = post("achievements/earned", record, Some(&token))?;
    match code {
        200 => Ok(body),
        401 => {
            vpidb_forget()?;
            Err("VPIDB disconnected this PC: connect again to share.".into())
        }
        _ if body["error"].as_str().is_some_and(|e| e.to_lowercase().contains("unknown pack")) => Err(NOT_ON_VPIDB.into()),
        _ => Err(said(&body).unwrap_or_else(|| format!("VPIDB didn't take it (answer {code})."))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_too_long_for_vpidb_keeps_its_rated_tables() {
        let mut tables: Vec<serde_json::Value> = (0..MOST + 10).map(|i| serde_json::json!({ "game": format!("g{i}"), "file": null, "rating": null })).collect();
        tables[MOST + 5]["rating"] = 4.into();
        let sent = within_limit(tables);
        assert_eq!(sent.len(), MOST);
        assert_eq!(sent[0]["game"], format!("g{}", MOST + 5));
        assert_eq!(sent[1]["game"], "g0");
        assert_eq!(within_limit(vec![serde_json::json!({ "game": "a", "rating": null })]).len(), 1);
    }

    #[test]
    fn a_body_quoted_for_curls_settings() {
        assert_eq!(quoted(r#"{"a":"b\c"}"#), r#""{\"a\":\"b\\c\"}""#);
        assert_eq!(unhex(&hex(&[0, 15, 255])), Some(vec![0, 15, 255]));
    }

    #[cfg(windows)]
    #[test]
    fn a_token_protected_for_this_account_comes_back() {
        let sealed = protect(b"a token", false).unwrap();
        assert_ne!(sealed, b"a token");
        assert_eq!(protect(&sealed, true).unwrap(), b"a token");
    }

    /// VPIDB itself, running on this PC for a check (`VPI_VPIDB_CHECK=<its address>`, a scratch
    /// `APPDATA`): the code asked for and its link written to `link.txt` in that folder, then
    /// polled for up to three minutes while someone enters it on the site; then an earned record
    /// for a pack VPIDB hasn't published, which it refuses as unknown — so the token was taken.
    #[test]
    #[ignore]
    fn connected_to_vpidb_running_here() {
        let Ok(site) = std::env::var("VPI_VPIDB_CHECK") else { return };
        unsafe {
            std::env::set_var("VPI_VPIDB", &site);
        }
        let data = crate::library::data_folder().unwrap();
        std::fs::create_dir_all(&data).unwrap();
        let start = tauri::async_runtime::block_on(vpidb_connect("VPI check".into())).unwrap();
        std::fs::write(data.join("link.txt"), format!("{}\n{}\n", start.user_code, start.link)).unwrap();
        let until = Instant::now() + Duration::from_secs(180);
        let mut state = String::new();
        while Instant::now() < until {
            state = tauri::async_runtime::block_on(vpidb_poll()).unwrap();
            if state != "waiting" {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        assert_eq!(state, "connected");
        let link = vpidb_link();
        println!("connected as {:?}", link.player.as_ref().map(|p| &p.name));
        vpidb_sharing(true).unwrap();
        let record = serde_json::json!({ "pack": "00000000-0000-4000-8000-000000000000", "packVersion": "1.0", "table": {}, "earned": [{ "id": "x", "at": "2026-10-26T12:00:00Z", "mode": "practice", "packVersion": "1.0" }], "mastered": [], "app": env!("CARGO_PKG_VERSION") });
        let said = send_earned(&record).unwrap_err();
        println!("an unpublished pack: {said}");
        assert_eq!(said, NOT_ON_VPIDB, "VPIDB's unknown pack, told apart");
        assert!(vpidb_link().connected, "the token was taken, so the link stays");
    }

    /// A pretend VPIDB on this PC, answering as VPIDB's API does: a code, waiting once, then
    /// connected; earned records taken only with the token. Run alone, as it sets `APPDATA` and
    /// `VPI_VPIDB` for the process: `cargo test -p app --release vpidb -- --ignored`.
    #[test]
    #[ignore]
    fn connected_through_a_code_and_earned_records_sent_with_the_token() {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(Mutex::new(Vec::<(String, String, String)>::new()));
        let heard = seen.clone();
        std::thread::spawn(move || {
            let mut polls = 0;
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let (mut length, mut auth) = (0usize, String::new());
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    let low = h.to_lowercase();
                    if let Some(v) = low.strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    if low.starts_with("authorization:") {
                        auth = h["authorization:".len()..].trim().to_string();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let body = String::from_utf8(body).unwrap();
                heard.lock().unwrap().push((path.clone(), auth.clone(), body));
                let (code, answer) = match path.as_str() {
                    "/api/link/start" => (200, format!(r#"{{"device_code":"secret-dc","user_code":"ABCD-2345","link":"http://127.0.0.1:{port}/#/link?code=ABCD-2345","expires_in":600,"interval":2}}"#)),
                    "/api/link/poll" => {
                        polls += 1;
                        (200, if polls == 1 { r#"{"state":"waiting"}"#.to_string() } else { r#"{"state":"connected","token":"tok-123","player":{"id":7,"name":"Tester"}}"#.to_string() })
                    }
                    "/api/achievements/earned" if auth == "Bearer tok-123" => (200, r#"{"stored":1,"mastered":[],"player":"x"}"#.to_string()),
                    "/api/achievements/earned" => (401, r#"{"error":"disconnected"}"#.to_string()),
                    _ => (404, r#"{"error":"no"}"#.to_string()),
                };
                let mut s = stream;
                let _ = write!(s, "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len());
            }
        });
        let data = std::env::temp_dir().join(format!("vpi-vpidb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        unsafe {
            std::env::set_var("APPDATA", &data);
            std::env::set_var("VPI_VPIDB", format!("http://127.0.0.1:{port}"));
        }
        let start = tauri::async_runtime::block_on(vpidb_connect("My cab".into())).unwrap();
        assert_eq!(start.user_code, "ABCD-2345");
        assert!(start.link.starts_with(&format!("http://127.0.0.1:{port}/")));
        // asked no sooner than VPIDB said: waiting, then connected
        assert_eq!(tauri::async_runtime::block_on(vpidb_poll()).unwrap(), "waiting");
        std::thread::sleep(Duration::from_millis(2100));
        assert_eq!(tauri::async_runtime::block_on(vpidb_poll()).unwrap(), "waiting");
        std::thread::sleep(Duration::from_millis(2100));
        assert_eq!(tauri::async_runtime::block_on(vpidb_poll()).unwrap(), "connected");
        let link = vpidb_link();
        assert!(link.connected && !link.sharing);
        assert_eq!((link.device.as_str(), link.player.as_ref().map(|p| p.name.as_str())), ("My cab", Some("Tester")));
        // the token kept protected, never as it came
        let kept = std::fs::read_to_string(kept_path().unwrap()).unwrap();
        assert!(!kept.contains("tok-123"));
        // nothing sent while sharing is off; sent with the token once it's on
        let record = serde_json::json!({ "pack": "p", "packVersion": "1.0", "earned": [] });
        assert!(send_earned(&record).is_err());
        vpidb_sharing(true).unwrap();
        assert_eq!(send_earned(&record).unwrap()["stored"], 1);
        let calls = seen.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&data);
        assert_eq!(calls.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["/api/link/start", "/api/link/poll", "/api/link/poll", "/api/achievements/earned"]);
        assert!(calls[0].2.contains(r#""device":"My cab""#) && calls[0].1.is_empty());
        assert!(calls[1].2.contains("secret-dc"));
        assert_eq!(calls[3].1, "Bearer tok-123");
    }
}
