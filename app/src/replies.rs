//! The team's replies to the problem reports this PC sent, the user's own replies to them, and the
//! notices to everyone. Each report goes with a key of its own — 32 random hex characters made here
//! — and is kept in `Library\reports.json` with its note, its table and the conversation since. The
//! reports service keeps only the key's SHA-256 beside the report's post on Discord, where the team
//! answers it; the app asks with its keys for their replies as it starts and every hour while it
//! runs, and passes the user's own replies on into the same post. A notice to everyone is what every
//! app shows once as it starts. Only a build that sends reports asks for any of it.

use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// A report sent to the team, and what's been said of it since.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    /// Its own key: what reads the team's replies and passes the user's on. Known only here and to
    /// the reports service, which keeps its SHA-256 alone.
    pub key: String,
    /// When it was sent, in milliseconds since 1970 by the service's clock.
    pub at: i64,
    /// What happened, in the user's words.
    #[serde(default)]
    pub note: String,
    /// The table it was about, by its name, if it was about one.
    #[serde(default)]
    pub table: Option<String>,
    /// The app's version that sent it.
    #[serde(default)]
    pub version: String,
    /// The report's zip on this PC.
    #[serde(default)]
    pub file: Option<String>,
    /// Whether the team can reply in the app: the service kept it beside its post.
    #[serde(default)]
    pub answerable: bool,
    /// The team's replies and the user's own, oldest first.
    #[serde(default)]
    pub messages: Vec<Message>,
    /// How many of the team's replies the user has opened.
    #[serde(default)]
    pub read: usize,
    /// The team closed its post: nothing more goes in.
    #[serde(default)]
    pub closed: bool,
    /// The team's replies not opened yet, for the screens (worked out as the list is read).
    #[serde(default, skip_deserializing)]
    pub unread: usize,
}

/// One reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub from: Who,
    pub text: String,
    /// Milliseconds since 1970, by the service's clock.
    pub at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Who {
    Team,
    You,
}

impl Sent {
    /// The team's replies the user hasn't opened.
    pub fn unread(&self) -> usize {
        self.team().saturating_sub(self.read)
    }

    fn team(&self) -> usize {
        self.messages.iter().filter(|m| m.from == Who::Team).count()
    }

    /// The team's replies as the service has them now, beside the user's own as kept here: the
    /// service's whole list in place of the one kept before.
    fn take_team(&mut self, team: Vec<(String, i64)>) {
        self.messages.retain(|m| m.from == Who::You);
        self.messages.extend(team.into_iter().map(|(text, at)| Message { from: Who::Team, text, at }));
        self.messages.sort_by_key(|m| m.at);
        self.read = self.read.min(self.team());
    }
}

/// A notice to everyone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub at: i64,
}

/// The most of the user's own reply, as the service takes it.
pub const MOST: usize = 1500;

/// How long the service keeps a report: the app stops asking after it.
const KEPT_FOR_MS: i64 = 365 * 24 * 60 * 60 * 1000;

/// The notices seen, the newest kept.
const SEEN_KEPT: usize = 50;

/// One change to the kept reports at a time.
static FILE: Mutex<()> = Mutex::new(());

/// A new report's key: 16 random bytes from Windows, in hex.
pub fn new_key() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("No key could be made for the report: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Where the reports sent are kept: `Library\reports.json`, or the file `VPI_REPORTS_FILE` names.
fn reports_file() -> Option<PathBuf> {
    if let Some(f) = std::env::var_os("VPI_REPORTS_FILE") {
        return Some(PathBuf::from(f));
    }
    Some(crate::library::data_folder()?.join("Library").join("reports.json"))
}

fn seen_file() -> Option<PathBuf> {
    Some(reports_file()?.with_file_name("notices-seen.json"))
}

fn load() -> Vec<Sent> {
    reports_file().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Written beside its place first, then moved there, so a failed write leaves the old copy.
fn save(all: &[Sent]) -> Result<(), String> {
    let path = reports_file().ok_or("no APPDATA folder")?;
    write_json(&path, &all)
}

fn write_json<T: Serialize + ?Sized>(path: &std::path::Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let part = path.with_extension("json.part");
    std::fs::write(&part, text).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The reports sent from here, the newest first.
pub fn list() -> Vec<Sent> {
    let _held = FILE.lock().unwrap_or_else(|e| e.into_inner());
    newest_first(load())
}

fn newest_first(mut all: Vec<Sent>) -> Vec<Sent> {
    all.sort_by_key(|s| std::cmp::Reverse(s.at));
    for s in &mut all {
        s.unread = s.unread();
    }
    all
}

/// A report just sent, kept.
pub fn remember(sent: Sent) -> Result<(), String> {
    let _held = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = load();
    all.retain(|s| s.key != sent.key);
    all.push(sent);
    save(&all)
}

/// A report's replies all opened.
pub fn mark_read(key: &str) -> Result<Vec<Sent>, String> {
    let _held = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = load();
    if let Some(s) = all.iter_mut().find(|s| s.key == key) {
        s.read = s.team();
        save(&all)?;
    }
    Ok(newest_first(all))
}

/// Where the service answers: the reports' address without its `/report`, in a build that sends
/// reports.
fn service() -> Option<String> {
    crate::report::address().map(|a| a.trim_end_matches('/').trim_end_matches("/report").to_string())
}

/// A call to the service — `body` posted as JSON, through curl's input so no key shows on its
/// command line, else a GET — and its answer: the HTTP code and the JSON it sent, if any.
fn call(base: &str, path: &str, body: Option<&serde_json::Value>) -> Result<(u16, serde_json::Value), String> {
    let mut curl = ::library::vps::curl();
    curl.args(["-sS", "--max-time", "30", "-A", &format!("VisualPinballInfinite/{}", env!("CARGO_PKG_VERSION"))]);
    if body.is_some() {
        curl.args(["-H", "content-type: application/json", "--data-binary", "@-"]);
    }
    curl.arg("-w").arg("\n%{http_code}").arg(format!("{base}{path}"));
    curl.stdin(if body.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = curl.spawn().map_err(|e| format!("The reports service couldn't be asked: {e}"))?;
    if let (Some(body), Some(mut input)) = (body, child.stdin.take()) {
        input.write_all(body.to_string().as_bytes()).map_err(|e| format!("The reports service couldn't be asked: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("The reports service couldn't be asked: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (said, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    let code: u16 = code.trim().parse().unwrap_or(0);
    if code == 0 {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("The reports service couldn't be reached{}", if err.trim().is_empty() { String::new() } else { format!(": {}", err.trim()) }));
    }
    Ok((code, serde_json::from_str(said).unwrap_or(serde_json::Value::Null)))
}

/// The service's own words why not, else a line saying the code.
fn why(code: u16, said: &serde_json::Value, what: &str) -> String {
    said["error"].as_str().map(str::to_string).unwrap_or_else(|| format!("{what} (the reports service answered {code})."))
}

/// The team's replies asked for, for every report still kept by the service and not closed, and
/// taken in; the reports, the newest first.
pub fn check() -> Result<Vec<Sent>, String> {
    let base = service().ok_or("This copy of Visual Pinball Infinite doesn't send reports.")?;
    check_with(&base, now_ms())
}

fn check_with(base: &str, now: i64) -> Result<Vec<Sent>, String> {
    let keys: Vec<String> = {
        let _held = FILE.lock().unwrap_or_else(|e| e.into_inner());
        load().into_iter().filter(|s| asked_for(s, now)).map(|s| s.key).collect()
    };
    if keys.is_empty() {
        return Ok(list());
    }
    let mut found = std::collections::HashMap::new();
    // fifty at a time, as the service takes them
    for some in keys.chunks(50) {
        let (code, said) = call(base, "/replies", Some(&serde_json::json!({ "keys": some })))?;
        if code != 200 {
            return Err(why(code, &said, "The team's replies couldn't be read"));
        }
        found.extend(team_replies(&said));
    }
    let _held = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = load();
    let mut changed = false;
    for s in &mut all {
        if let Some(team) = found.remove(&s.key) {
            let before = s.messages.clone();
            s.take_team(team);
            changed |= s.messages != before;
        }
    }
    if changed {
        save(&all)?;
    }
    Ok(newest_first(all))
}

/// Whether a report's replies are asked for: one the team can reply to, not closed, still kept.
fn asked_for(s: &Sent, now: i64) -> bool {
    s.answerable && !s.closed && now - s.at < KEPT_FOR_MS
}

/// The service's answer to /replies: each key's replies, as text and time.
fn team_replies(said: &serde_json::Value) -> std::collections::HashMap<String, Vec<(String, i64)>> {
    let mut out = std::collections::HashMap::new();
    let Some(replies) = said["replies"].as_object() else { return out };
    for (key, list) in replies {
        let team: Vec<(String, i64)> = list
            .as_array()
            .map(|l| l.iter().filter_map(|m| Some((m["text"].as_str()?.to_string(), m["at"].as_i64()?))).collect())
            .unwrap_or_default();
        out.insert(key.clone(), team);
    }
    out
}

/// The user's own reply passed on into the report's post, and kept; the report as it stands.
pub fn reply(key: &str, text: &str) -> Result<Sent, String> {
    let base = service().ok_or("This copy of Visual Pinball Infinite doesn't send reports.")?;
    reply_with(&base, key, text)
}

fn reply_with(base: &str, key: &str, text: &str) -> Result<Sent, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Write your reply first.".into());
    }
    if text.chars().count() > MOST {
        return Err(format!("That's longer than a reply takes ({MOST} letters): make it shorter, or send a new report."));
    }
    if !list().iter().any(|s| s.key == key) {
        return Err("That report isn't kept here any more.".into());
    }
    let (code, said) = call(base, "/reply", Some(&serde_json::json!({ "key": key, "text": text })))?;
    let _held = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = load();
    let at = all.iter().position(|s| s.key == key).ok_or("That report isn't kept here any more.")?;
    match code {
        200 => {
            let when = said["at"].as_i64().unwrap_or_else(now_ms);
            all[at].messages.push(Message { from: Who::You, text: text.to_string(), at: when });
            save(&all)?;
            let mut s = all[at].clone();
            s.unread = s.unread();
            Ok(s)
        }
        // the post is gone or closed on Discord
        410 => {
            all[at].closed = true;
            save(&all)?;
            Err(why(code, &said, "The team has closed this report"))
        }
        _ => Err(why(code, &said, "Your reply couldn't be sent")),
    }
}

/// The notices to everyone this app hasn't shown yet, each kept as shown once it's given.
pub fn new_notices() -> Result<Vec<Notice>, String> {
    let base = service().ok_or("This copy of Visual Pinball Infinite doesn't send reports.")?;
    new_notices_with(&base)
}

fn new_notices_with(base: &str) -> Result<Vec<Notice>, String> {
    let (code, said) = call(base, "/notices", None)?;
    if code != 200 {
        return Err(why(code, &said, "The notices couldn't be read"));
    }
    let notices = notices_in(&said);
    let file = seen_file().ok_or("no APPDATA folder")?;
    let _held = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut seen: Vec<String> = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let new: Vec<Notice> = notices.into_iter().filter(|n| !seen.contains(&n.id)).collect();
    if !new.is_empty() {
        seen.extend(new.iter().map(|n| n.id.clone()));
        let from = seen.len().saturating_sub(SEEN_KEPT);
        write_json(&file, &seen[from..])?;
    }
    Ok(new)
}

/// The service's answer to /notices: each notice with its ID and words.
fn notices_in(said: &serde_json::Value) -> Vec<Notice> {
    said["notices"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|n| {
                    let id = n["id"].as_str()?.trim();
                    let text = n["text"].as_str()?.trim();
                    (!id.is_empty() && !text.is_empty()).then(|| Notice { id: id.to_string(), text: text.to_string(), at: n["at"].as_i64().unwrap_or(0) })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent(key: &str, at: i64) -> Sent {
        Sent { key: key.into(), at, note: "The backglass stayed dark".into(), table: Some("Attack from Mars".into()), version: "0.2.1".into(), file: None, answerable: true, messages: vec![], read: 0, closed: false, unread: 0 }
    }

    #[test]
    fn a_reports_key_is_32_random_hex_characters() {
        let (a, b) = (new_key().unwrap(), new_key().unwrap());
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{a}");
        assert_ne!(a, b);
    }

    /// The team's replies as the service has them take the place of those kept, the user's own
    /// staying beside them in time order; only replies not yet opened count as new.
    #[test]
    fn the_teams_replies_join_the_users_own_in_order() {
        let mut s = sent("k", 1000);
        s.messages.push(Message { from: Who::You, text: "Mine".into(), at: 3000 });
        s.take_team(vec![("First".into(), 2000)]);
        assert_eq!(s.unread(), 1);
        s.read = 1;
        s.take_team(vec![("First".into(), 2000), ("Second".into(), 4000)]);
        let order: Vec<&str> = s.messages.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(order, ["First", "Mine", "Second"]);
        assert_eq!(s.unread(), 1);
        // fewer than were opened: nothing new, nothing below none
        s.read = 2;
        s.take_team(vec![("First".into(), 2000)]);
        assert_eq!((s.read, s.unread()), (1, 0));
    }

    #[test]
    fn only_reports_the_team_can_still_answer_are_asked_for() {
        let now = 10 * KEPT_FOR_MS;
        let open = sent("a", now - 1000);
        let closed = Sent { closed: true, ..sent("b", now - 1000) };
        let unanswerable = Sent { answerable: false, ..sent("c", now - 1000) };
        let old = sent("d", now - KEPT_FOR_MS - 1);
        assert!(asked_for(&open, now));
        assert!(!asked_for(&closed, now) && !asked_for(&unanswerable, now) && !asked_for(&old, now));
    }

    #[test]
    fn the_services_answers_are_read_as_it_gives_them() {
        let said = serde_json::json!({ "ok": true, "replies": { "abc": [{ "text": "Fixed in 0.2.1", "at": 5 }, { "text": 7 }], "def": [] } });
        let found = team_replies(&said);
        assert_eq!(found["abc"], vec![("Fixed in 0.2.1".to_string(), 5)]);
        assert!(found["def"].is_empty());
        let notices = notices_in(&serde_json::json!({ "notices": [{ "id": "17", "text": " Hello \n", "at": 3 }, { "id": "", "text": "x" }, { "text": "no id" }] }));
        assert_eq!(notices.len(), 1);
        assert_eq!((notices[0].id.as_str(), notices[0].text.as_str()), ("17", "Hello"));
    }

    /// The reports kept in a file of their own, read back newest first; a report's replies opened.
    #[test]
    fn reports_sent_are_kept_and_read_back() {
        let dir = std::env::temp_dir().join(format!("vpi-replies-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("reports.json");
        // SAFETY: the tests that read this variable are this one alone
        unsafe { std::env::set_var("VPI_REPORTS_FILE", &file) };
        remember(sent("old", 1000)).unwrap();
        let mut new = sent("new", 2000);
        new.take_team(vec![("Thanks".into(), 2500)]);
        remember(new).unwrap();
        let all = list();
        assert_eq!(all.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(), ["new", "old"]);
        assert_eq!(all[0].unread, 1);
        let all = mark_read("new").unwrap();
        assert_eq!((all[0].unread, all[1].unread), (0, 0));
        assert_eq!(list()[0].unread, 0);
        unsafe { std::env::remove_var("VPI_REPORTS_FILE") };
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A build given no reports address — one made from the public source alone — asks no service
    /// anything: no replies, no reply sent, no notices (checked with `VPI_REPORTS_ADDRESS=none`).
    #[test]
    fn a_build_without_the_address_asks_nothing() {
        if crate::report::can_send() {
            return;
        }
        assert!(service().is_none());
        for err in [check().unwrap_err(), reply(&"0".repeat(32), "hello").unwrap_err(), new_notices().unwrap_err()] {
            assert!(err.contains("doesn't send reports"), "{err}");
        }
    }

    /// Replies and notices through a reports service running on this PC (`wrangler dev`), its
    /// address in `VPI_REPORTS_LOCAL` and a report already sent there with the key in
    /// `VPI_REPORTS_LOCAL_KEY`, the team's reply and a notice set (the service's own local check
    /// does that): `cargo test -p app replies_through_a_local_service -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn replies_through_a_local_service() {
        let base = std::env::var("VPI_REPORTS_LOCAL").expect("VPI_REPORTS_LOCAL");
        let key = std::env::var("VPI_REPORTS_LOCAL_KEY").expect("VPI_REPORTS_LOCAL_KEY");
        let dir = std::env::temp_dir().join(format!("vpi-replies-local-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: an ignored test, run alone
        unsafe { std::env::set_var("VPI_REPORTS_FILE", dir.join("reports.json")) };
        remember(sent(&key, now_ms() - 1000)).unwrap();
        let all = check_with(&base, now_ms()).unwrap();
        println!("replies: {:?}", all[0].messages);
        assert!(all[0].unread() >= 1, "no reply from the team");
        let s = reply_with(&base, &key, "Updated, and it works now.").unwrap();
        assert_eq!(s.messages.last().map(|m| (m.from, m.text.as_str())), Some((Who::You, "Updated, and it works now.")));
        let notices = new_notices_with(&base).unwrap();
        println!("notices: {notices:?}");
        assert_eq!(notices.len(), 1);
        assert!(new_notices_with(&base).unwrap().is_empty(), "a notice shown twice");
        unsafe { std::env::remove_var("VPI_REPORTS_FILE") };
        std::fs::remove_dir_all(&dir).ok();
    }
}
