//! A table's guide, for the pause menu: the pages of Pinball Primer (pinballprimer.github.io), a
//! free site of guides to playing each pinball machine, that VPS lists among a game's tutorials. The
//! menu shows the site's own page as the site draws it, loaded from the site each time it's opened:
//! nothing of the site is kept on the PC, nor goes into the program or the repository. Here, the
//! pages VPS lists for a game, and each page's own title, read once into memory, as VPS sometimes
//! lists another machine of the same name's page beside the game's own and the titles tell them
//! apart by their year. Ours.

use serde::Serialize;

use crate::reading;
use crate::vps::{self, Game};

/// Where the guides are: the menu's view of them goes nowhere else.
pub const SITE: &str = "https://pinballprimer.github.io/";

/// The most read of a page for its title.
const MOST_PAGE: u64 = 4 << 20;

/// A guide page for the menu: its file on the site, the title it gives itself, its address.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Page {
    pub name: String,
    pub title: String,
    pub address: String,
}

/// A name of a page directly on the site, reaching nowhere else.
fn page_name(name: &str) -> bool {
    !name.starts_with('.') && name.len() > ".html".len() && name.ends_with(".html") && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// A page's address on the site, where its name is one of the site's pages.
pub fn address_of(name: &str) -> Option<String> {
    page_name(name).then(|| format!("{SITE}{name}"))
}

/// Whether an address (as a browser writes it out: its scheme and host in small letters) is on the
/// guides' site, by HTTPS.
pub fn on_site(address: &str) -> bool {
    address.starts_with(SITE)
}

/// The guide pages VPS lists for a game, each once, by their files' names on the site.
pub fn pages_of(game: &Game) -> Vec<String> {
    let mut pages: Vec<String> = Vec::new();
    for page in game.tutorial_files.iter().filter_map(|a| a.url.as_deref()).filter_map(|url| url.strip_prefix(SITE)).filter(|name| page_name(name)) {
        if !pages.iter().any(|p| p == page) {
            pages.push(page.to_string());
        }
    }
    pages
}

/// A game's guide pages from those VPS lists for it, each with the title it gives itself, read from
/// the site into memory alone. Where VPS lists several and some name the game's `year`, those alone
/// are kept. A page that can't be had is left out while another can.
pub fn pages(names: &[String], year: Option<i32>) -> Result<Vec<Page>, String> {
    pages_with(names, year, &fetch)
}

fn pages_with(names: &[String], year: Option<i32>, fetch: &dyn Fn(&str) -> Result<String, String>) -> Result<Vec<Page>, String> {
    let mut pages = Vec::new();
    let mut trouble = None;
    for name in names {
        let Some(address) = address_of(name) else {
            trouble = trouble.or_else(|| Some("That isn't a page of the guides' site.".to_string()));
            continue;
        };
        match fetch(name) {
            Ok(html) => pages.push(Page { name: name.clone(), title: title_of(&html).unwrap_or_else(|| name.trim_end_matches(".html").to_string()), address }),
            Err(e) => trouble = trouble.or(Some(e)),
        }
    }
    if pages.is_empty() {
        return Err(trouble.unwrap_or_else(|| "VPS lists no guide for this game.".to_string()));
    }
    if pages.len() > 1
        && let Some(year) = year.map(|y| y.to_string())
        && pages.iter().any(|p| p.title.contains(&year))
    {
        pages.retain(|p| p.title.contains(&year));
    }
    Ok(pages)
}

/// A page's own title: the words of its first top heading (`<h1>`), else of its `<title>`.
pub fn title_of(html: &str) -> Option<String> {
    // ASCII's small letters keep every byte where it was
    let small = html.to_ascii_lowercase();
    ["h1", "title"].into_iter().find_map(|tag| {
        let opening = format!("<{tag}");
        let mut from = 0;
        let start = loop {
            let at = from + small[from..].find(&opening)?;
            // `<h1>` or `<h1 class=…>`, not `<h10>` or `<titles>`
            match small[at + opening.len()..].chars().next() {
                Some(c) if c == '>' || c.is_whitespace() => break at,
                _ => from = at + opening.len(),
            }
        };
        let inside = start + small[start..].find('>')? + 1;
        let end = inside + small[inside..].find(&format!("</{tag}"))?;
        let words = words_of(&html[inside..end]);
        (!words.is_empty()).then_some(words)
    })
}

/// The words of a stretch of HTML: its tags left out, the characters it writes as `&…` put back,
/// its spaces made one.
fn words_of(html: &str) -> String {
    let mut text = String::new();
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        text.push_str(&rest[..at]);
        rest = rest[at..].find('>').map_or("", |end| &rest[at + end + 1..]);
    }
    text.push_str(rest);
    decode(&text).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One of the site's pages read into memory, through Windows' own `curl.exe`, by HTTPS alone.
fn fetch(name: &str) -> Result<String, String> {
    let out = vps::curl()
        .args(["--fail", "--silent", "--location", "--proto", "=https", "--proto-redir", "=https", "--max-redirs", "3", "--max-time", "20", "--max-filesize"])
        .arg(MOST_PAGE.to_string())
        .arg(format!("{SITE}{name}"))
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        // curl's code for an answer of 400 or above from the site
        return Err(if out.status.code() == Some(22) {
            "This guide isn't on Pinball Primer's site: VPS's link to it leads nowhere.".to_string()
        } else {
            "Pinball Primer's site couldn't be reached just now. The guide is shown from the site itself, so it needs the internet.".to_string()
        });
    }
    if out.stdout.len() as u64 > MOST_PAGE {
        return Err("That page of the guides' site is larger than any guide.".to_string());
    }
    Ok(reading::text(&out.stdout, false))
}

/// The named characters HTML pages use for words and punctuation.
const NAMED: [(&str, char); 26] = [
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{a0}'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
    ("thinsp", '\u{2009}'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("mdash", '—'),
    ("ndash", '–'),
    ("hellip", '…'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("bull", '•'),
    ("middot", '·'),
    ("times", '×'),
    ("frac12", '½'),
    ("frac14", '¼'),
    ("frac34", '¾'),
    ("deg", '°'),
];

/// Text with the characters HTML writes as `&…` put back; an `&` that begins none stays as it is.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        match entity(rest) {
            Some((c, length)) => {
                out.push(c);
                rest = &rest[length..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The character an `&…` at the start of `text` stands for, and its length (its `;` may be left off,
/// as browsers allow).
fn entity(text: &str) -> Option<(char, usize)> {
    let body = &text[1..];
    let (c, end) = if let Some(number) = body.strip_prefix('#') {
        let (digits, radix, marks) = match number.strip_prefix(['x', 'X']) {
            Some(hex) => (hex, 16, 3),
            None => (number, 10, 2),
        };
        let length = digits.find(|c: char| !c.is_digit(radix)).unwrap_or(digits.len());
        (char::from_u32(u32::from_str_radix(&digits[..length], radix).ok()?)?, marks + length)
    } else {
        let length = body.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(body.len());
        (NAMED.iter().find(|(name, _)| *name == &body[..length])?.1, 1 + length)
    };
    Some((c, end + usize::from(text[end..].starts_with(';'))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vps::Asset;

    fn game(urls: &[&str]) -> Game {
        Game { tutorial_files: urls.iter().map(|u| Asset { url: Some(u.to_string()), ..Asset::default() }).collect(), ..Game::default() }
    }

    /// Only the site's own pages, once each: a video or a misspelt link is no guide.
    #[test]
    fn the_pages_vps_lists() {
        let g = game(&[
            "https://www.youtube.com/watch?v=abc",
            "https://pinballprimer.github.io/one_A.html",
            "https://pinballprimer.github.io/one_A.html",
            "https://pinballprimer.github.io/two_B.hrml",
            "https://pinballprimer.github.io/../secret.html",
            "http://pinballprimer.github.io/three_C.html",
            "https://pinballprimer.github.io/four_D.html",
        ]);
        assert_eq!(pages_of(&g), ["one_A.html", "four_D.html"]);
        assert!(pages_of(&Game::default()).is_empty());
    }

    /// A page's address only for a page of the site, and the view kept to the site.
    #[test]
    fn names_and_addresses_reach_nowhere_else() {
        assert_eq!(address_of("afm_G4do5.html").as_deref(), Some("https://pinballprimer.github.io/afm_G4do5.html"));
        for bad in ["../x.html", "a\\b.html", "a/b.html", ".html", "x.exe", "", "a b.html"] {
            assert!(address_of(bad).is_none(), "{bad}");
        }
        assert!(on_site("https://pinballprimer.github.io/afm_G4do5.html"));
        assert!(on_site("https://pinballprimer.github.io/"));
        for away in ["http://pinballprimer.github.io/x.html", "https://pinballprimer.github.io.example.com/", "https://pinballprimer.github.io@example.com/", "https://example.com/pinballprimer.github.io/"] {
            assert!(!on_site(away), "{away}");
        }
    }

    /// A page's title from its top heading as written, its marks and characters put right; its
    /// `<title>` where it has no heading.
    #[test]
    fn a_pages_own_title() {
        assert_eq!(title_of("<html><head><title>Not this</title></head><body><H1 class=\"x\">Sample  <b>Machine</b>\n(Maker, 1990) &amp; more</h1><h1>Second</h1>").as_deref(), Some("Sample Machine (Maker, 1990) & more"));
        assert_eq!(title_of("<h10>no</h10><title>From &ldquo;the&rdquo; title</title>").as_deref(), Some("From “the” title"));
        assert_eq!(title_of("<h1></h1><p>words</p>"), None);
        assert_eq!(title_of("<p>no title at all</p>"), None);
    }

    /// Where VPS lists another machine's page beside the game's own, the one naming the game's year
    /// is kept; where none or all do, every page is. Pages made up here: nothing is fetched.
    #[test]
    fn the_page_of_the_games_own_year() {
        let site = |name: &str| -> Result<String, String> {
            match name {
                "a.html" | "c.html" => Ok("<h1>Name (Maker, 1991)</h1><p>Words.</p>".to_string()),
                "b.html" => Ok("<h1>Name (Other, 1999)</h1>".to_string()),
                "untitled.html" => Ok("<p>Words.</p>".to_string()),
                _ => Err("not on the made-up site".to_string()),
            }
        };
        let names = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let kept = |list: &[&str], year| pages_with(&names(list), year, &site).unwrap().into_iter().map(|p| p.name).collect::<Vec<_>>();
        assert_eq!(kept(&["a.html", "b.html"], Some(1999)), ["b.html"]);
        assert_eq!(kept(&["a.html", "c.html"], Some(1991)), ["a.html", "c.html"]);
        assert_eq!(kept(&["a.html", "b.html"], Some(1980)), ["a.html", "b.html"]);
        // a page that can't be had left out while another can
        assert_eq!(kept(&["missing.html", "b.html"], None), ["b.html"]);
        let page = &pages_with(&names(&["a.html"]), None, &site).unwrap()[0];
        assert_eq!((page.title.as_str(), page.address.as_str()), ("Name (Maker, 1991)", "https://pinballprimer.github.io/a.html"));
        // a page with no title named by its file
        assert_eq!(pages_with(&names(&["untitled.html"]), None, &site).unwrap()[0].title, "untitled");
        // a name that isn't the site's own refused, nothing asked of the site
        let asked = |_: &str| -> Result<String, String> { panic!("asked the site") };
        assert!(pages_with(&names(&["../a.html"]), None, &asked).is_err());
        assert_eq!(pages_with(&names(&["missing.html"]), None, &site).unwrap_err(), "not on the made-up site");
        assert!(pages_with(&[], None, &site).is_err());
    }

    #[test]
    fn characters_written_out() {
        assert_eq!(decode("a &amp; b &#8217; &#x2019; &copy 2024 &emsp;"), "a & b ’ ’ © 2024 \u{2003}");
        assert_eq!(decode("R&D &unknown; &"), "R&D &unknown; &");
    }

    /// A page really read for its title, and a page the site doesn't have said so:
    /// `VPI_GUIDE_PAGE=<a page of the site> cargo test -p library --lib a_real_guide -- --ignored
    /// --nocapture`. Prints the title's length, none of its words.
    #[test]
    #[ignore = "reads from the guides' site"]
    fn a_real_guide() {
        let page = std::env::var("VPI_GUIDE_PAGE").expect("VPI_GUIDE_PAGE names a page of the site");
        let found = pages(std::slice::from_ref(&page), None).unwrap();
        assert_ne!(found[0].title, page.trim_end_matches(".html"), "the page gave no title");
        println!("a title of {} characters", found[0].title.chars().count());
        let missing = pages(&["no_such_page_at_all.html".to_string()], None).unwrap_err();
        assert!(missing.contains("leads nowhere"), "{missing}");
    }
}
