// Ported from VPin Studio (https://github.com/syd711/vpin-studio, its main branch):
// vpin-connector-vps's matcher — TableNameSplitter, TableMatcher, TableVersionMatcher,
// VpsAutomatcher and LegacyTableMatcher — with the cosine and Jaccard distances it takes from
// Apache Commons Text, computed as that library computes them.
// Copyright (c) 2022 syd711. Licensed under the MIT licence (LICENSE-VPinStudio.txt beside this
// crate's Cargo.toml); this port is offered under the same terms.

//! A table matched to VPS's database, as VPin Studio matches one: its file
//! name split into name, maker, year, version and the rest; the closest game
//! by name, maker and year; then the closest of the game's table files by the
//! table's own name, authors and version (its table info, else the file
//! name's), or the day it was made. Rules and thresholds as VPin Studio's.
//!
//! Ours on top (`match_table`), each measured on the test library's tables,
//! whose VPS files are known: a game further than VPin Studio's
//! confirming distance is looked for again by the table info's name, then by
//! the ROM its script names, and left a guess to confirm when neither finds
//! it; the file name's version picks the table file when only one has it
//! (dots aside: 1.13 is 1.1.3); the file name's authors (VPW, Bigus) count
//! beside the table info's, which a mod often keeps from the table it
//! changed; authors match with spaces and digits aside (ClarkKent, Javier15);
//! and VPS's comments are compared with the table's name only for what's left
//! of each once the game's own name, maker and year are taken out.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::vps::{Database, Game, TableFile};

/// The parts of a table's file name, "Name (Maker Year) extra" (VPin Studio's `TableNameParts`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NameParts {
    pub display_name: String,
    pub table_name: Option<String>,
    pub manufacturer: Option<String>,
    /// -1 when the name gives none.
    pub year: i32,
    pub extra: Option<String>,
    pub version: Option<String>,
}

static VP_REFERENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"VPX?[0-9.]+").unwrap());
static FEATURES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[(\+?[A-Z][A-Z&][A-Z]?){1,2}\]").unwrap());
/// "(Maker Year) extra" or "(Year Maker) extra".
static MAKER_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(((([A-Za-z0-9_ ]+)[() ]*([0-9]{4}))|(([0-9]{4})[() ]*)([A-Za-z0-9_ ]+))\)(.*)").unwrap());
/// "Name Year extra".
static NAME_YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.+)([0-9]{4})(.+)").unwrap());
static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)([rv]?[_.]?[0-9]+([_.][0-9]+)+([r-][0-9]+([_.][0-9]+)*)?[abcde]?)(.*)").unwrap());

/// VPin Studio's `TableNameSplitter.parseFilename`.
pub fn parse_filename(file_name: &str) -> NameParts {
    let display = VP_REFERENCE.replace_all(file_name, "");
    let display = FEATURES.replace_all(&display, "").into_owned();
    let mut parts = NameParts { display_name: display.clone(), year: -1, ..Default::default() };
    if let Some(c) = MAKER_YEAR.captures(&display) {
        parts.table_name = Some(display[..c.get(0).map_or(0, |m| m.start())].to_string());
        let maker = c.get(3).or_else(|| c.get(7)).map_or("", |m| m.as_str());
        parts.manufacturer = Some(clean_chars(maker));
        parts.year = c.get(4).or_else(|| c.get(6)).and_then(|m| m.as_str().parse().ok()).unwrap_or(-1);
        parts.extra = Some(c.get(8).map_or("", |m| m.as_str()).trim().to_string());
    } else if let Some(c) = NAME_YEAR.captures(&display) {
        parts.table_name = Some(c[1].trim().to_string());
        parts.year = c[2].parse().unwrap_or(-1);
        parts.extra = Some(c[3].trim().to_string());
    }
    if parts.table_name.is_none() && parts.extra.is_none() {
        parts.table_name = Some(display.clone());
    }
    if let Some(extra) = parts.extra.clone().filter(|e| !e.is_empty()) {
        parts.version = extract_version(&extra);
        if let Some(v) = parts.version.as_deref().filter(|v| !v.is_empty()) {
            parts.extra = Some(remove_ci(&extra, v).trim().to_string());
        }
    }
    if parts.version.is_none() {
        let name = parts.table_name.clone().unwrap_or_default();
        parts.version = extract_version(&name);
        if let Some(v) = parts.version.as_deref().filter(|v| !v.is_empty()) {
            parts.table_name = Some(remove_ci(&name, v));
        }
    }
    parts
}

pub fn extract_version(text: &str) -> Option<String> {
    VERSION.captures(text).map(|c| c[1].to_string())
}

/// VPin Studio's `cleanChars`: punctuation to spaces, spaces single.
pub fn clean_chars(text: &str) -> String {
    let replaced: String = text.chars().map(|c| if "_.,+-'[]()".contains(c) { ' ' } else { c }).collect();
    replaced.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Every occurrence of `what` taken out, ignoring case.
fn remove_ci(text: &str, what: &str) -> String {
    if what.is_empty() {
        return text.to_string();
    }
    let (lower, find) = (text.to_lowercase(), what.to_lowercase());
    // positions in the lower-cased text map onto the original only where lower-casing keeps lengths
    if lower.len() != text.len() {
        return text.replace(what, "");
    }
    let mut out = String::new();
    let mut at = 0;
    while let Some(p) = lower[at..].find(&find) {
        out.push_str(&text[at..at + p]);
        at += p + find.len();
    }
    out.push_str(&text[at..]);
    out
}

fn contains_ci(text: &str, what: &str) -> bool {
    text.to_lowercase().contains(&what.to_lowercase())
}

/// Apache Commons Text's cosine distance: 1 less the cosine of the two texts' word counts, a
/// word being a run of letters, digits and underscores.
pub fn cosine_distance(a: &str, b: &str) -> f64 {
    let count = |s: &str| {
        let mut m: HashMap<String, f64> = HashMap::new();
        for w in s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).filter(|w| !w.is_empty()) {
            *m.entry(w.to_string()).or_insert(0.0) += 1.0;
        }
        m
    };
    let (l, r) = (count(a), count(b));
    let dot: f64 = l.iter().filter_map(|(k, v)| r.get(k).map(|w| v * w)).sum();
    let (d1, d2): (f64, f64) = (l.values().map(|v| v * v).sum(), r.values().map(|v| v * v).sum());
    let similarity = if d1 <= 0.0 || d2 <= 0.0 { 0.0 } else { dot / (d1.sqrt() * d2.sqrt()) };
    1.0 - similarity
}

/// Apache Commons Text's Jaccard distance: 1 less the share of the two texts' characters, as
/// sets, that both have.
pub fn jaccard_distance(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    if a.is_empty() || b.is_empty() {
        return 1.0;
    }
    let (l, r): (HashSet<char>, HashSet<char>) = (a.chars().collect(), b.chars().collect());
    let union = l.union(&r).count();
    let both = l.len() + r.len() - union;
    1.0 - both as f64 / union as f64
}

/// VPin Studio's `TableMatcher.distance`: 0 for the same name, a part's share for one inside the
/// other, else the product of the cosine and Jaccard distances.
pub fn name_distance(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    if a.is_empty() || b.is_empty() {
        return 1.0;
    }
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    if a == b {
        return 0.0;
    }
    let mut distance = 10000.0f64;
    let (la, lb) = (a.chars().count() as f64, b.chars().count() as f64);
    if a.contains(&b) {
        distance = distance.min(1.0 - lb / la) / 2.0;
    } else if b.contains(&a) {
        distance = distance.min(1.0 - la / lb) / 2.0;
    }
    distance.min(cosine_distance(&a, &b) * jaccard_distance(&a, &b))
}

const THRESHOLD_NOTFOUND: f64 = 10.0;
const THRESHOLD_CONFIRM: f64 = 2.0;

/// VPin Studio's `cleanTable`.
fn clean_table(name: &str) -> String {
    clean_chars(name).replace("the ", "").replace(", the", "")
}

fn alternate_makers(maker: &str) -> Vec<String> {
    let m = maker.to_lowercase();
    let list: &[&str] = match m.as_str() {
        "bally" | "midway" => &["Bally", "Midway"],
        "original" | "tba" => &["Original", "TBA"],
        "mylstar" | "gottlieb" => &["Gottlieb", "Mylstar"],
        _ if m.starts_with("taito") => &["Taito do Brasil", "Taito"],
        _ if m.starts_with("spooky") => &["Spooky Pinball", "Spooky"],
        _ => return vec![maker.to_string()],
    };
    list.iter().map(|s| s.to_string()).collect()
}

fn combined(table: f64, maker: f64, year: f64) -> f64 {
    (1.0 + table * 9.0) * (1.0 + maker * 3.0) * (1.0 + year * 1.5) - 1.0
}

/// A game VPS lists for the formats asked (VPin Studio's `isEmulatorApplicable`).
pub fn applies(formats: &[&str], game: &Game) -> bool {
    if formats.is_empty() {
        return true;
    }
    if game.manufacturer.eq_ignore_ascii_case("Zen Studios") && !formats.iter().any(|f| ["FX", "FX2", "FX3"].contains(f)) {
        return false;
    }
    game.table_files.iter().filter_map(|t| t.table_format.as_deref()).any(|f| formats.contains(&f))
}

/// How far a game is from the parsed name, maker and year (VPin Studio's `getTableDistance`).
fn game_distance(game: &Game, display_name: &str, table_name: Option<&str>, maker: Option<&str>, year: i32) -> f64 {
    let theirs = clean_table(&game.name.to_lowercase());
    let their_year = game.year.unwrap_or(0);
    match table_name {
        Some(ours) => {
            let d_table = name_distance(&theirs, ours);
            let d_maker = match maker {
                Some(m) => alternate_makers(&game.manufacturer).iter().map(|alt| name_distance(m, alt)).fold(10000.0, f64::min),
                None => 0.2,
            };
            let d_year = if year > 1900 { f64::from((year - their_year).abs()).min(5.0) / 5.0 } else { 0.2 };
            combined(d_table, d_maker, d_year)
        }
        None => {
            let mut clean = display_name.to_string();
            let mut d_maker = 0.2;
            for alt in alternate_makers(&game.manufacturer) {
                if contains_ci(&clean, &alt) {
                    clean = remove_ci(&clean, &alt);
                    d_maker = 0.0;
                }
            }
            let mut d_year = 0.2;
            let y = their_year.to_string();
            if clean.contains(&y) {
                clean = remove_ci(&clean, &y);
                d_year = 0.0;
            }
            combined(name_distance(&clean_table(&clean), &theirs), d_maker, d_year)
        }
    }
}

/// The games closest to a parsed name, nearest first (VPin Studio's `findAllClosest`): those
/// within its confirming distance if any, else those within its not-found distance, else the
/// nearest alone.
pub fn closest_games<'a>(formats: &[&str], parts: &NameParts, games: &'a [Game]) -> Vec<(&'a Game, f64)> {
    let clean_name = parts.table_name.as_deref().filter(|n| !n.is_empty()).map(clean_table);
    let mut found: Vec<(&Game, f64)> = Vec::new();
    let mut min_dist = 10000.0f64;
    for game in games.iter().filter(|g| applies(formats, g)) {
        let dist = game_distance(game, &parts.display_name, clean_name.as_deref(), parts.manufacturer.as_deref(), parts.year);
        if dist >= min_dist {
            continue;
        }
        if dist <= THRESHOLD_CONFIRM {
            if THRESHOLD_CONFIRM < min_dist {
                found.clear();
                min_dist = THRESHOLD_CONFIRM;
            }
        } else if dist <= THRESHOLD_NOTFOUND {
            if THRESHOLD_NOTFOUND < min_dist {
                found.clear();
                min_dist = THRESHOLD_NOTFOUND;
            }
        } else {
            found.clear();
            min_dist = dist;
        }
        let at = found.iter().position(|&(_, d)| d >= dist).unwrap_or(found.len());
        found.insert(at, (game, dist));
    }
    found
}

/// VPin Studio's `LegacyTableMatcher`: the games whose names hold the file name's start, the
/// last word dropped until some do.
pub fn legacy_find<'a>(file_name: &str, games: &'a [Game]) -> Vec<&'a Game> {
    let mut term = file_name.replace(['_', '\'', '-', '.'], " ").replace("The ", "").replace(", The", "");
    if let Some(p) = term.find('(') {
        term.truncate(p);
    }
    let mut term = term.to_lowercase().trim().to_string();
    loop {
        let found: Vec<&Game> = games.iter().filter(|g| g.name.to_lowercase().replace(['-', '\''], " ").contains(&term)).collect();
        if !found.is_empty() {
            return found;
        }
        match term.rfind(' ') {
            Some(p) => term.truncate(p),
            None => return found,
        }
    }
}

/// VPin Studio's `cleanVersion`: a leading "VPX…" taken off, lower case.
fn clean_version(version: &str) -> String {
    let mut v = version;
    if v.len() >= 2 && v[..2].eq_ignore_ascii_case("VP") {
        let p = 2 + v[2..].chars().take_while(|c| "XS0123456789.".contains(*c)).count();
        v = &v[p..];
    }
    v.to_lowercase().trim().to_string()
}

/// VPin Studio's `VersionTokenizer`: the numbers of a version, one at a time.
struct VersionTokens<'a> {
    text: &'a [u8],
    at: usize,
    number: i64,
}

impl<'a> VersionTokens<'a> {
    fn new(text: &'a str) -> Self {
        let bytes = text.as_bytes();
        let at = bytes.iter().position(u8::is_ascii_digit).unwrap_or(bytes.len());
        VersionTokens { text: bytes, at, number: 0 }
    }

    fn next(&mut self) -> bool {
        self.number = 0;
        let mut has = false;
        while self.at < self.text.len() {
            let c = self.text[self.at];
            if c == b'.' {
                self.at += 1;
                break;
            }
            if !c.is_ascii_digit() {
                break;
            }
            self.number = self.number.saturating_mul(10).saturating_add(i64::from(c - b'0'));
            self.at += 1;
            has = true;
        }
        has
    }

    fn suffix(&self) -> String {
        String::from_utf8_lossy(&self.text[self.at..]).into_owned()
    }
}

/// VPin Studio's `versionDistance`: VPS's version against the table's, a table older than VPS's
/// file costing more than a newer one.
pub fn version_distance(theirs: &str, ours: &str) -> f64 {
    if theirs.is_empty() && ours.is_empty() {
        return 0.0;
    }
    let (c1, c2) = (clean_version(theirs), clean_version(ours));
    let (mut t1, mut t2) = (VersionTokens::new(&c1), VersionTokens::new(&c2));
    let mut depth = 0i32;
    while t1.next() {
        if !t2.next() {
            return 0.3 / 10f64.powi(depth);
        }
        let (n1, n2) = (t1.number, t2.number);
        if n1 < n2 {
            return f64::from((3 - depth).max(1));
        } else if n1 > n2 {
            return ((n1 - n2) as f64 / 10f64.powi(depth)).min(0.3);
        }
        depth += 1;
    }
    if t2.next() {
        return 0.3 / 10f64.powi(depth);
    }
    let (s1, s2) = (t1.suffix(), t2.suffix());
    let marked = |s: &str| ["-", "r", "/", ":"].iter().any(|m| s.to_lowercase().starts_with(m));
    if marked(&s1) && marked(&s2) {
        return version_distance(&s1[1..], &s2[1..]) / 4.0;
    }
    if !s1.is_empty() && s2.is_empty() || s1.is_empty() && !s2.is_empty() {
        return 0.2;
    }
    match s1.to_lowercase().cmp(&s2.to_lowercase()) {
        std::cmp::Ordering::Less => 1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 0.1,
    }
}

/// VPin Studio's `authorDistance`: each of the table's authors found among VPS's (taken out once
/// found), costing how far down VPS's list it is, or the whole list when not there. Ours: an
/// author is also found by their letters alone (`same_author`).
fn author_distance(ours: &[String], i: usize, theirs: &mut Vec<String>, max: usize) -> f64 {
    if i >= ours.len() {
        return 0.0;
    }
    for j in 0..theirs.len() {
        if same_author(&ours[i], &theirs[j]) {
            theirs.remove(j);
            return j as f64 + author_distance(ours, i + 1, theirs, max);
        }
    }
    max as f64 + author_distance(ours, i + 1, theirs, max)
}

/// VPin Studio's test, close by the cosine distance; ours besides, the same letters with spaces,
/// digits and marks aside ("Clark Kent" and "ClarkKent", "Javier" and "Javier15").
fn same_author(ours: &str, theirs: &str) -> bool {
    let letters = |s: &str| s.chars().filter(char::is_ascii_alphabetic).collect::<String>().to_lowercase();
    let (a, b) = (letters(ours), letters(theirs));
    cosine_distance(ours, &theirs.to_lowercase()) < 0.3 || a.len() >= 3 && a == b
}

/// The table's authors, as VPin Studio splits them.
fn split_authors(authors: &str) -> Vec<String> {
    authors.to_lowercase().split([',', '/', '-', '&']).map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect()
}

/// A version reduced for comparing (ours): lower case, a leading "v" or "r" off, trailing ".0"s off,
/// and the dots out, so "v1.1.3" is "1.13" and "1.0.2" is "1.02".
pub fn plain_version(version: &str) -> String {
    let v = clean_version(version).replace('_', ".");
    let v = v.trim_start_matches(['v', 'r', '.']);
    let mut parts: Vec<&str> = v.split('.').collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.chars().all(|c| c == '0')) {
        parts.pop();
    }
    parts.concat()
}

const VERSION_NOTFOUND: f64 = 3.5;

/// What the game's table files are weighed against.
#[derive(Clone, Debug, Default)]
pub struct FileQuery {
    /// The table's name, for VPS's comments: the table info's, else the file name's extra.
    pub name: String,
    /// The table info's authors.
    pub info_authors: Option<String>,
    /// The file name's extra words, which often name the authors (VPW, Bigus).
    pub name_authors: String,
    /// The file name's version, else the table info's, as VPin Studio cleans it.
    pub version: Option<String>,
    /// The version is the file name's.
    pub version_from_name: bool,
    /// The day the table file was made, in milliseconds since 1970.
    pub made: Option<i64>,
}

fn table_files_of<'a>(game: &'a Game, formats: &[&str]) -> Vec<&'a TableFile> {
    game.table_files.iter().filter(|t| formats.is_empty() || t.table_format.as_deref().is_some_and(|f| formats.contains(&f))).collect()
}

/// The game's table files of the file name's version (ours): the one to take when it's alone.
pub fn of_named_version<'a>(game: &'a Game, formats: &[&str], q: &FileQuery) -> Vec<&'a TableFile> {
    let Some(v) = q.version.as_deref().filter(|_| q.version_from_name).map(plain_version).filter(|v| !v.is_empty()) else {
        return Vec::new();
    };
    table_files_of(game, formats).into_iter().filter(|t| t.version.as_deref().is_some_and(|tv| plain_version(tv) == v)).collect()
}

/// The game's table file closest to the table (VPin Studio's `TableVersionMatcher.findVersion`):
/// the only one there is, else by the table's name against VPS's comment, its authors, its
/// version, or the day its file was made against VPS's. Ours first: the only one of the file
/// name's version, or the closest of several.
pub fn closest_table_file<'a>(game: &'a Game, formats: &[&str], q: &FileQuery) -> Option<&'a TableFile> {
    let files = table_files_of(game, formats);
    if files.len() == 1 {
        return Some(files[0]);
    }
    let named = of_named_version(game, formats, q);
    if named.len() == 1 {
        return Some(named[0]);
    }
    if q.name.is_empty() && q.version.as_deref().is_none_or(|v| clean_version(v).is_empty()) && q.info_authors.is_none() && q.name_authors.is_empty() && q.made.is_none() {
        return None;
    }
    table_file_scores(game, formats, q)
        .into_iter()
        .filter(|s| named.is_empty() || named.iter().any(|t| std::ptr::eq(*t, s.file)))
        .fold(None, |best: Option<(&TableFile, f64)>, s| if best.is_none_or(|(_, b)| s.total < b) { Some((s.file, s.total)) } else { best })
        .filter(|&(_, d)| d < VERSION_NOTFOUND)
        .map(|(t, _)| t)
}

/// One table file's distances from the table, as `closest_table_file` weighs them.
#[derive(Clone, Debug)]
pub struct FileScore<'a> {
    pub file: &'a TableFile,
    pub name: f64,
    pub authors: f64,
    pub version: f64,
    pub total: f64,
}

/// What's left of a name once the game's own name, maker and year are taken out (ours), in words.
fn residue(text: &str, game: &Game) -> String {
    let words = |s: &str| -> Vec<String> { s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect() };
    let mut known: HashSet<String> = words(&game.name).into_iter().collect();
    known.extend(words(&game.manufacturer));
    if let Some(y) = game.year {
        known.insert(y.to_string());
    }
    words(text).into_iter().filter(|w| !known.contains(w)).collect::<Vec<_>>().join(" ")
}

/// Every one of the game's table files scored against the table (VPin Studio's
/// `TableVersionMatcher.findVersion`, each file's distances kept; ours where `closest_table_file`
/// says).
pub fn table_file_scores<'a>(game: &'a Game, formats: &[&str], q: &FileQuery) -> Vec<FileScore<'a>> {
    let version = clean_version(q.version.as_deref().unwrap_or(""));
    let name = residue(&q.name, game);
    let from_info = q.info_authors.as_deref().map(split_authors).filter(|a| !a.is_empty());
    let mut from_name = q.name_authors.to_lowercase();
    for w in EXCLUDED_WORDS {
        from_name = from_name.split(' ').filter(|x| *x != w).collect::<Vec<_>>().join(" ");
    }
    let from_name = Some(split_authors(&clean_chars(&from_name))).filter(|a| !a.is_empty());
    let mut scores = Vec::new();
    for t in table_files_of(game, formats) {
        let comment = t.comment.as_deref().unwrap_or("");
        let comment = match comment.to_lowercase().find("reupload") {
            Some(p) => &comment[..p],
            None => comment,
        };
        let comment = residue(comment, game);
        let d_name = if !comment.is_empty() && !name.is_empty() { cosine_distance(&comment, &name) } else { 1.0 };
        let d_made = q.made.map_or(10000.0, |m| (t.created_at.unwrap_or(0) - m).abs() as f64 / 1000.0 / 60.0 / 60.0 / 24.0);
        let (mut d_version, mut d_author) = (1.0, 0.5);
        // VPin Studio takes a close name as settling it; ours weighs it beside the authors and version
        if d_made < 1.0 {
            (d_version, d_author) = (0.0, 0.0);
        } else {
            if let Some(v) = t.version.as_deref().filter(|v| !v.is_empty())
                && !version.is_empty()
            {
                d_version = if plain_version(v) == plain_version(&version) { 0.0 } else { version_distance(v, &version) };
            }
            if !t.authors.is_empty() {
                // the table info's authors as VPin Studio weighs them, the file name's at half (they may not
                // be authors at all), the nearer counting
                let weigh = |ours: &[String]| {
                    let whole = (ours.len() * t.authors.len()) as f64;
                    let mut theirs = t.authors.clone();
                    author_distance(ours, 0, &mut theirs, t.authors.len()) / whole
                };
                let d = [from_info.as_deref().map(weigh), from_name.as_deref().map(|a| weigh(a) / 2.0)].into_iter().flatten().fold(None, |m: Option<f64>, d| Some(m.map_or(d, |m| m.min(d))));
                if let Some(d) = d {
                    d_author = d;
                    if d_author == 0.0 {
                        d_version /= 3.0;
                    }
                }
            }
        }
        let d = (1.0 + d_name) * (1.0 + d_author) * (1.0 + d_version / 3.0) - 1.0;
        scores.push(FileScore { file: t, name: d_name, authors: d_author, version: d_version, total: d });
    }
    scores
}

/// What a table file says about itself, for matching: its file name, and what its table info
/// gives (name, authors, version), the day its file was made, and the game name its script gives
/// (a PinMAME table's ROM).
#[derive(Clone, Debug, Default)]
pub struct Clues {
    pub file_name: String,
    pub info_name: Option<String>,
    pub info_authors: Option<String>,
    pub info_version: Option<String>,
    pub made: Option<i64>,
    pub rom: Option<String>,
}

/// How a game was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    /// By the file name, within VPin Studio's confirming distance.
    ByFileName,
    /// By the table info's name, the file name finding nothing as near (ours).
    ByTableInfo,
    /// By the ROM the script names, which VPS lists for the game (ours).
    ByRom,
    /// The nearest game, but further than VPin Studio's confirming distance: a guess to confirm.
    Guess,
    /// By VPin Studio's older, looser matcher, the closer one finding nothing.
    Loosely,
}

impl Found {
    pub fn describe(self) -> &'static str {
        match self {
            Found::ByFileName => "by its file name",
            Found::ByTableInfo => "by the name in its table info",
            Found::ByRom => "by its ROM",
            Found::Guess => "a guess, to confirm",
            Found::Loosely => "by the looser matcher, to confirm",
        }
    }

    /// Sure enough to take without asking.
    pub fn sure(self) -> bool {
        matches!(self, Found::ByFileName | Found::ByTableInfo | Found::ByRom)
    }
}

/// A table's match in VPS's database.
#[derive(Clone, Debug)]
pub struct Match<'a> {
    pub game: Option<&'a Game>,
    pub found: Found,
    /// The game's distance, as VPin Studio measures it (0 the same name, maker and year).
    pub distance: f64,
    pub table_file: Option<&'a TableFile>,
    /// The version found in the name or the table info, cleaned.
    pub version: Option<String>,
    pub parts: NameParts,
    /// What the game's table files were weighed against.
    pub asked: Option<FileQuery>,
}

const EXCLUDED_WORDS: [&str; 14] = ["mod", "vpx", "vr", "dt", "fss", "fs", "4k", "alt", "alt2", "(1)", "(2)", "edition", "version", "vrroom"];

/// Table info names that say nothing of the table (ours): VPX's example table's, kept by tables
/// made from it.
const PLACEHOLDER_NAMES: [&str; 2] = ["visual pinball demo table", "table1"];

/// The file name says something of the game (ours), letters only: it holds the game name's first
/// word, or its initials ("TOTAN" for Tales of the Arabian Nights; two letters only at its start).
pub fn relates(file_name: &str, game: &Game) -> bool {
    let letters = |s: &str| s.chars().filter(char::is_ascii_alphabetic).collect::<String>().to_lowercase();
    let file = letters(file_name);
    let words: Vec<String> = game.name.split(|c: char| !c.is_alphanumeric()).map(letters).filter(|w| !w.is_empty()).collect();
    let first = words.iter().find(|w| w.len() >= 3 && !["the", "and"].contains(&w.as_str()));
    let initials: String = words.iter().filter_map(|w| w.chars().next()).collect();
    first.is_some_and(|w| file.contains(w.as_str())) || initials.len() >= 3 && file.contains(&initials) || initials.len() == 2 && file.starts_with(&initials)
}

/// The game nearest a file's name alone, with its distance (VPin Studio's `findClosest`), for a
/// table's companion files: a backglass, its views, its settings.
pub fn game_by_file_name<'a>(db: &'a Database, file_name: &str) -> Option<(&'a Game, f64)> {
    closest_games(&["VPX"], &parse_filename(file_name), &db.games).first().copied()
}

/// How far a file's name is from a game, as `closest_games` measures it.
pub fn file_name_distance(game: &Game, file_name: &str) -> f64 {
    let parts = parse_filename(file_name);
    let clean_name = parts.table_name.as_deref().filter(|n| !n.is_empty()).map(clean_table);
    game_distance(game, &parts.display_name, clean_name.as_deref(), parts.manufacturer.as_deref(), parts.year)
}

/// VPin Studio's confirming distance: a game this near a name is taken without asking.
pub const CONFIRMING: f64 = THRESHOLD_CONFIRM;

/// A ROM's name as VPS lists it, for comparing.
fn rom_key(rom: &str) -> String {
    let r = rom.trim().to_lowercase();
    r.strip_suffix(".zip").unwrap_or(&r).to_string()
}

/// A VPX table matched to VPS: VPin Studio's `VpsAutomatcher.autoMatch` — the game, then its
/// table file — with ours where the module's notes say.
pub fn match_table<'a>(db: &'a Database, clues: &Clues) -> Match<'a> {
    const FORMATS: [&str; 1] = ["VPX"];
    let mut parts = parse_filename(&clues.file_name);
    let by_name = closest_games(&FORMATS, &parts, &db.games).first().copied();
    let (game, found, distance) = match by_name {
        Some((g, d)) if d <= THRESHOLD_CONFIRM => (Some(g), Found::ByFileName, d),
        _ => {
            // the table info's name, with the file name's maker and year when it gives none; a game
            // the file name says nothing of is left out (a table built on another's keeps its name)
            let by_info = clues.info_name.as_deref().filter(|n| !PLACEHOLDER_NAMES.contains(&n.trim().to_lowercase().as_str())).and_then(|n| {
                let mut p = parse_filename(n);
                if p.manufacturer.is_none() && p.year < 0 {
                    (p.manufacturer, p.year) = (parts.manufacturer.clone(), parts.year);
                }
                closest_games(&FORMATS, &p, &db.games).first().copied().filter(|(g, _)| relates(&clues.file_name, g))
            });
            // the games VPS lists the script's ROM for that the file name speaks of, the nearest by name
            let by_rom = clues.rom.as_deref().map(rom_key).and_then(|rom| {
                let clean_name = parts.table_name.as_deref().filter(|n| !n.is_empty()).map(clean_table);
                db.games
                    .iter()
                    .filter(|g| applies(&FORMATS, g) && g.rom_files.iter().any(|f| f.version.as_deref().is_some_and(|v| rom_key(v) == rom)))
                    .filter(|g| relates(&clues.file_name, g))
                    .map(|g| (g, game_distance(g, &parts.display_name, clean_name.as_deref(), parts.manufacturer.as_deref(), parts.year)))
                    .min_by(|a, b| a.1.total_cmp(&b.1))
            });
            match (by_name, by_info, by_rom) {
                (_, Some((g, d)), _) if d <= THRESHOLD_CONFIRM => (Some(g), Found::ByTableInfo, d),
                (_, _, Some((g, d))) => (Some(g), Found::ByRom, d),
                (Some((_, d)), Some((g, di)), _) if di < d => (Some(g), Found::Guess, di),
                (Some((g, d)), _, _) => (Some(g), Found::Guess, d),
                (None, Some((g, d)), _) => (Some(g), Found::Guess, d),
                (None, None, None) => (legacy_find(&clues.file_name, &db.games).first().copied(), Found::Loosely, f64::INFINITY),
            }
        }
    };
    let mut version = parts.version.clone();
    let version_from_name = version.is_some();
    let mut table_file = None;
    let mut asked = None;
    if let Some(game) = game {
        let extra = match parts.extra.take() {
            None => remove_ci(parts.table_name.as_deref().unwrap_or(""), &game.name).trim().to_string(),
            Some(e) => clean_chars(&e),
        };
        parts.extra = Some(extra.clone());
        let name = clues.info_name.clone().unwrap_or_else(|| extra.clone());
        if version.is_none()
            && let Some(v) = clues.info_version.as_deref().filter(|v| !v.is_empty() && !contains_ci(v, "VP"))
        {
            version = extract_version(v);
        }
        if let Some(v) = version.take() {
            let mut v = v.replace('_', ".").to_lowercase().trim().to_string();
            for start in ["v", "r", "."] {
                if let Some(rest) = v.strip_prefix(start) {
                    v = rest.to_string();
                }
            }
            version = Some(v);
        }
        let q = FileQuery {
            name,
            info_authors: clues.info_authors.clone().filter(|a| !a.trim().is_empty()),
            name_authors: extra,
            version: version.clone(),
            version_from_name,
            made: clues.made,
        };
        table_file = closest_table_file(game, &FORMATS, &q);
        asked = Some(q);
    }
    Match { game, found, distance, table_file, version, parts, asked }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_names_as_vpin_studio() {
        let p = parse_filename("Attack from Mars (Bally 1995) JPSalas 6.0.0");
        assert_eq!(p.table_name.as_deref(), Some("Attack from Mars "));
        assert_eq!((p.manufacturer.as_deref(), p.year), (Some("Bally"), 1995));
        assert_eq!((p.extra.as_deref(), p.version.as_deref()), (Some("JPSalas"), Some("6.0.0")));
        let q = parse_filename("Tron Legacy (Stern 2011) VPW Mod v1.1");
        assert_eq!(q.version.as_deref(), Some("v1.1"));
        assert_eq!(q.extra.as_deref(), Some("VPW Mod"));
        // a reference to VPX's version and features in brackets are taken out
        let r = parse_filename("Twister (Sega 1996) VPX10.7 [FSS] v2.0");
        assert_eq!(r.version.as_deref(), Some("v2.0"));
        // no maker: the name and the year
        let s = parse_filename("Bad Cats 1989 v3");
        assert_eq!((s.table_name.as_deref(), s.year), (Some("Bad Cats"), 1989));
    }

    #[test]
    fn distances_as_commons_text() {
        // in floating point, as Java's: the same words, a hair from 0
        assert!(cosine_distance("attack from mars", "attack from mars").abs() < 1e-12);
        assert!((cosine_distance("a b", "a c") - 0.5).abs() < 1e-12);
        assert_eq!(jaccard_distance("abc", "abc"), 0.0);
        assert!((jaccard_distance("ab", "bc") - (1.0 - 1.0 / 3.0)).abs() < 1e-12);
        assert_eq!(name_distance("Attack from Mars", "attack from mars"), 0.0);
        // one inside the other: at most half the share left over, less when the words say so
        let d = name_distance("terminator 2 judgment day", "terminator 2");
        let (cd, jd) = (1.0 - 2.0 / (2.0 * 2f64.sqrt()), 1.0 - 10.0 / 15.0);
        assert!((d - ((1.0 - 12.0 / 25.0) / 2.0f64).min(cd * jd)).abs() < 1e-12);
    }

    #[test]
    fn versions_as_vpin_studio() {
        assert_eq!(version_distance("1.2", "1.2"), 0.0);
        // the table's older than VPS's: costs more
        assert_eq!(version_distance("1.3", "1.2"), 0.1);
        assert_eq!(version_distance("1.2", "1.3"), 2.0);
        assert_eq!(version_distance("2.0", "2.0b"), 0.2);
        assert_eq!(clean_version("VPX10.8 1.2"), "1.2");
    }

    fn game(name: &str, maker: &str, year: i32) -> Game {
        Game { name: name.into(), manufacturer: maker.into(), year: Some(year), ..Default::default() }
    }

    #[test]
    fn ours_beside_vpin_studio() {
        // versions with the dots aside, and trailing zeros
        assert_eq!(plain_version("v1.1.3"), plain_version("1.13"));
        assert_eq!(plain_version("1.0.2"), plain_version("1.02"));
        assert_eq!(plain_version("4.0.0"), plain_version("4.0"));
        assert_ne!(plain_version("1.1.0"), plain_version("1.10"));
        // authors by their letters
        assert!(same_author("clark kent", "ClarkKent"));
        assert!(same_author("javier", "Javier15"));
        assert!(!same_author("tom", "Tomate"));
        // what's left of a name once the game's name, maker and year are out
        let taxi = game("Taxi", "Williams", 1988);
        assert_eq!(residue("Taxi (Williams 1988)", &taxi), "");
        assert_eq!(residue("Mod of Taxi (Williams 1988) by VPW", &taxi), "mod of by vpw");
        // a file name that speaks of the game: its first word, or its initials
        assert!(relates("TOTAN4k 1.5", &game("Tales of the Arabian Nights", "Williams", 1996)));
        assert!(relates("TimeLord-4.1a", &game("Time Lord", "Original", 2022)));
        assert!(relates("tz_94h", &game("Twilight Zone", "Bally", 1993)));
        assert!(!relates("JP's Arcade Physics v3.01 Rubber objects", &game("Count-Down", "Gottlieb", 1979)));
    }
}
