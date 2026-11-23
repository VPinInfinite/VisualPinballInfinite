"""Scans every file in a folder - programs and libraries too - for words that mustn't ship: this PC's
user name, user folder and name, the folder this repository is in (as Windows, other tools and
MSYS2 write it, by its long name and its short one) and the folder the libraries are built in
(vpx-build), and the words in a list kept outside the repository (one per line, blank lines and
lines starting with # left out; a line starting with re: is a regular expression, matched as
written, in plain text; a line starting with ok: a phrase a find is fine inside, such as an
author's name in the credits that shares a word on the list). Words are looked for as they're
stored in programs as well as in text files: plain (ASCII and UTF-8) and UTF-16, in any case.
Prints each find with the text around it and exits 1 when there's any, else 0.

    python -I tools/release/scan.py <folder> [--words <file>] [--allow <word>]...

--allow takes a word out of the list again (for one the release needs, such as a web address).
"""
import argparse
import os
import pathlib
import re
import sys


def short_name(path):
    """A folder's 8.3 short name (C:\\VISUAL~1), as build tools given it write it; the path itself
    when it has none."""
    import ctypes
    buffer = ctypes.create_unicode_buffer(1024)
    n = ctypes.windll.kernel32.GetShortPathNameW(str(path), buffer, len(buffer))
    return buffer.value if 0 < n < len(buffer) else str(path)


def folder_forms(path):
    """A folder as it's written: C:\\x, C:/x and MSYS2's /c/x."""
    path = str(path).rstrip('\\/')
    forward = path.replace('\\', '/')
    forms = [path, forward]
    if len(path) > 2 and path[1] == ':':
        forms.append('/' + path[0].lower() + forward[2:])
    return forms


def words_to_find(words_file):
    found = []
    user = os.environ.get('USERNAME', '')
    profile = os.environ.get('USERPROFILE', '')
    computer = os.environ.get('COMPUTERNAME', '')
    repo = pathlib.Path(__file__).resolve().parents[2]
    for w in (user, *folder_forms(profile), computer, *folder_forms(repo), *folder_forms(short_name(repo)), 'vpx-build'):
        if w and len(w) >= 3:
            found.append(w)
    if words_file and pathlib.Path(words_file).is_file():
        for line in pathlib.Path(words_file).read_text(encoding='utf-8').splitlines():
            line = line.strip()
            if line and not line.startswith(('#', 'ok:')):
                found.append(line)
    # longest first, so a folder is reported before the name inside it
    return sorted(set(found), key=len, reverse=True)


def fine_phrases(words_file):
    """The phrases of the list a find is fine inside (its ok: lines), as patterns for plain text and
    UTF-16, in any case."""
    out = []
    if words_file and pathlib.Path(words_file).is_file():
        for line in pathlib.Path(words_file).read_text(encoding='utf-8').splitlines():
            line = line.strip()
            if line.startswith('ok:') and line[3:].strip():
                phrase = line[3:].strip()
                out += [re.compile(re.escape(phrase.encode(enc)), re.IGNORECASE) for enc in ('utf-8', 'utf-16-le')]
    return out


def within_fine(data, start, end, fine):
    """Whether the find from start to end lies inside one of the fine phrases where data holds it."""
    for pat in fine:
        for m in pat.finditer(data, max(0, start - 400), min(len(data), end + 400)):
            if m.start() <= start and end <= m.end():
                return True
    return False


def patterns(words):
    out = []
    for w in words:
        if w.startswith('re:'):
            out.append((w, 'text', re.compile(w[3:].encode('utf-8'))))
            continue
        for name, data in (('text', w.encode('utf-8')), ('UTF-16', w.encode('utf-16-le'))):
            out.append((w, name, re.compile(re.escape(data), re.IGNORECASE)))
    return out


def context(data, start, end, utf16):
    lo, hi = max(0, start - 40), min(len(data), end + 40)
    piece = data[lo:hi]
    if utf16:
        piece = piece[(lo % 2):]
        text = piece.decode('utf-16-le', errors='replace')
    else:
        text = piece.decode('utf-8', errors='replace')
    return ''.join(c if c.isprintable() else '.' for c in text)


def main():
    sys.stdout.reconfigure(encoding='utf-8')
    ap = argparse.ArgumentParser()
    ap.add_argument('folder')
    ap.add_argument('--words')
    ap.add_argument('--allow', action='append', default=[])
    a = ap.parse_args()
    allowed = {w.lower() for w in a.allow}
    words = [w for w in words_to_find(a.words) if w.lower() not in allowed]
    pats = patterns(words)
    fine = fine_phrases(a.words)
    finds = 0
    files = sorted(p for p in pathlib.Path(a.folder).rglob('*') if p.is_file())
    for path in files:
        data = path.read_bytes()
        reported = set()
        for word, kind, pat in pats:
            for m in pat.finditer(data):
                key = (m.start(), m.end())
                if any(s <= key[0] < e for s, e in reported) or within_fine(data, m.start(), m.end(), fine):
                    continue
                reported.add(key)
                finds += 1
                if finds <= 200:
                    rel = path.relative_to(a.folder)
                    print(f'{rel}: "{word}" ({kind}) at {m.start()}: ...{context(data, m.start(), m.end(), kind == "UTF-16")}...')
    print(f'scanned {len(files)} files for {len(words)} words: {finds} found')
    return 1 if finds else 0


if __name__ == '__main__':
    sys.exit(main())
