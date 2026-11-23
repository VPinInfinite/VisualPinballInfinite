//! A read-me or a manual that comes with a table, made readable in the app: which files are read
//! there (PDFs as they are; text files as text), and a text file's words whatever encoding it was
//! written in — UTF-8 (with or without its mark, or UTF-16 with its mark), else Windows' Western
//! code page, or, for an `.nfo`, the DOS code page its box drawing was made in.

use std::path::Path;

/// How a file is read in the app: a PDF in the window's own viewer, or text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reads {
    Pdf,
    Text,
}

/// How `path` is read in the app, by its kind; None for a file the app doesn't read (a Word file,
/// a web page, a picture: those open where Windows opens them).
pub fn reads(path: &Path) -> Option<Reads> {
    match path.extension()?.to_string_lossy().to_ascii_lowercase().as_str() {
        "pdf" => Some(Reads::Pdf),
        "txt" | "md" | "nfo" | "diz" | "log" | "1st" | "me" => Some(Reads::Text),
        _ => None,
    }
}

/// The DOS code page 437's upper half, as Unicode: what an `.nfo`'s box drawing was made in.
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// A text file's words: UTF-8 (its mark taken off) or UTF-16 with its mark as written, else the DOS
/// code page for an `.nfo` (`dos`), else Windows' Western code page.
pub fn text(bytes: &[u8], dos: bool) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some((enc, _)) = encoding_rs::Encoding::for_bom(bytes)
        && enc != encoding_rs::UTF_8
    {
        return enc.decode(bytes).0.into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    if dos {
        return bytes.iter().map(|&b| if b < 0x80 { char::from(b) } else { CP437_HIGH[usize::from(b - 0x80)] }).collect();
    }
    encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn what_reads_in_the_app_and_text_whatever_its_encoding() {
        assert_eq!(reads(&PathBuf::from("Manual.PDF")), Some(Reads::Pdf));
        assert_eq!(reads(&PathBuf::from("read me.txt")), Some(Reads::Text));
        assert_eq!(reads(&PathBuf::from("release.nfo")), Some(Reads::Text));
        assert_eq!(reads(&PathBuf::from("rules.docx")), None);
        assert_eq!(reads(&PathBuf::from("page.html")), None);
        assert_eq!(text("plain".as_bytes(), false), "plain");
        assert_eq!(text(&[0xEF, 0xBB, 0xBF, b'h', b'i'], false), "hi");
        assert_eq!(text(&[0xFF, 0xFE, b'h', 0, b'i', 0], false), "hi");
        // Windows' Western code page: café, its é one byte
        assert_eq!(text(&[b'c', b'a', b'f', 0xE9], false), "café");
        // an .nfo's box drawing in the DOS code page
        assert_eq!(text(&[0xC9, 0xCD, 0xBB, b' ', 0xB0], true), "╔═╗ ░");
    }
}
