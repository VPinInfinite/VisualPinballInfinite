"""Lists, file by file, what in the repository's own files mustn't go public: the words scan.py looks
for (this PC's user name, user folder and name, the repository's folder, and the list kept outside
the repository) but the libraries' build folder, which the source names where it builds them, and
dates written into comments. Reads the files git tracks, or those listed in --files (one path per
line); prints each file with its count of each, the most first, and a total.

    python -I tools/release/code_scan.py [--words <file>] [--files <list>] [--show]

--show prints every line found as well.
"""
import argparse
import pathlib
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from scan import words_to_find  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
DATE = re.compile(r'\b20[0-9]{2}-[01][0-9]-[0-3][0-9]\b')


def main():
    sys.stdout.reconfigure(encoding='utf-8')
    ap = argparse.ArgumentParser()
    ap.add_argument('--words')
    ap.add_argument('--files')
    ap.add_argument('--show', action='store_true')
    a = ap.parse_args()
    if a.files:
        paths = [l.strip() for l in pathlib.Path(a.files).read_text(encoding='utf-8').splitlines() if l.strip()]
    else:
        paths = subprocess.run(['git', 'ls-files'], cwd=ROOT, capture_output=True, text=True, check=True).stdout.splitlines()
    finds = []
    for w in words_to_find(a.words):
        if w == 'vpx-build':
            continue
        if w.startswith('re:'):
            finds.append((w[3:], re.compile(w[3:])))
        else:
            finds.append((w, re.compile(re.escape(w), re.IGNORECASE)))
    finds.append(('a date', DATE))
    total = 0
    per_file = []
    for rel in paths:
        path = ROOT / rel
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding='utf-8')
        except UnicodeDecodeError:
            continue
        counts = {}
        shown = []
        for n, line in enumerate(text.splitlines(), 1):
            for name, pat in finds:
                k = len(pat.findall(line))
                if k:
                    counts[name] = counts.get(name, 0) + k
                    shown.append(f'  {n}: {line.strip()[:200]}')
        if counts:
            per_file.append((sum(counts.values()), rel, counts, shown))
            total += sum(counts.values())
    for k, rel, counts, shown in sorted(per_file, reverse=True):
        print(f'{k:5}  {rel}  ' + ', '.join(f'{n} {c}' for n, c in sorted(counts.items(), key=lambda x: -x[1])))
        if a.show:
            print('\n'.join(dict.fromkeys(shown)))
    print(f'{len(per_file)} files, {total} finds')
    return 0


if __name__ == '__main__':
    sys.exit(main())
