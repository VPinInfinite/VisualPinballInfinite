"""Makes the public repository's files, from a commit of this one, in a folder of their own: the
program, its sources and what builds it, as public.json lists them, without the private tools,
documents and records.

    python -I tools/release/public.py <folder> [--rev HEAD] [--words <file>]

--words: the list kept outside the repository, scan-words.txt in the folder VPI release in the
user's folder, unless another is named.

Taken from the commit (git archive), so nothing uncommitted goes. Then:
  - README.md and .gitignore come from tools/release/public, which isn't copied itself;
  - CREDITS.md loses its private blocks (from a line <!-- private --> to a line <!-- /private -->)
    and any paragraph about a folder the copy doesn't hold;
  - Cargo.toml's workspace keeps the members the copy holds, and Cargo.lock loses the packages only
    the others used (cargo, offline; no version changes);
  - each patch (git format-patch's) loses its Date line, which says when and in which time zone it
    was made: git am dates it as it's applied;
  - every file is scanned for what scan.py looks for in a release (this PC's user name, user folder
    and name, the repository's folder, the list kept outside the repository, but inside its ok:
    phrases): each find is printed, and the exit code is 1 if there's any; and the copy's own files
    for dates, as code_scan.py
    looks, each printed to be looked at (a test's sample date or a tool's release is fine).
The folder must be new or empty.
"""
import argparse
import io
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tarfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from scan import fine_phrases, within_fine, words_to_find  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
DATE = re.compile(rb'\b20[0-9]{2}-[01][0-9]-[0-3][0-9]\b')
# other projects' sources, kept as they publish them: their dates are theirs
THEIRS = ('vendor/sdl3-src/', 'vendor/ffmpeg/', 'vendor/sdl3-ttf-sys/')


def git(*args, binary=False):
    out = subprocess.run(['git', *args], cwd=ROOT, capture_output=True, check=True).stdout
    return out if binary else out.decode('utf-8')


def credits_public(text, held):
    """CREDITS.md without its private blocks, nor a paragraph about a folder the copy doesn't hold."""
    out, private, dropped = [], False, []
    for line in text.split('\n'):
        if line.strip() == '<!-- private -->':
            private = True
            continue
        if line.strip() == '<!-- /private -->':
            private = False
            continue
        if not private:
            out.append(line)
    paragraphs = '\n'.join(out).split('\n\n')
    kept = []
    for p in paragraphs:
        m = re.match(r'\*\*(?:In|On) [^*]+\*\* \(`([^`]+)`', p)
        if m and not held(m.group(1).rstrip('/')):
            dropped.append(p[:80])
            continue
        kept.append(p)
    return '\n\n'.join(kept), dropped


def members_public(toml, held):
    """Cargo.toml with only the workspace members the copy holds."""
    block = re.search(r'members = \[\n(.*?)\n\]', toml, re.S)
    lines = block.group(1).split('\n')
    keep = [l for l in lines if held(l.strip().strip(',').strip('"'))]
    return toml[:block.start(1)] + '\n'.join(keep) + toml[block.end(1):], len(lines) - len(keep)


def packages(lock):
    return set(re.findall(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"', lock))


def undated(patch):
    """A patch as git format-patch writes it, without the Date line of its header (which ends at its
    first blank line); every other byte as it was. Whether it had one."""
    lines = patch.split(b'\n')
    for i, line in enumerate(lines):
        if not line.rstrip(b'\r'):
            break
        if line.startswith(b'Date: '):
            del lines[i]
            return b'\n'.join(lines), True
    return patch, False


def main():
    sys.stdout.reconfigure(encoding='utf-8')
    ap = argparse.ArgumentParser()
    ap.add_argument('folder')
    ap.add_argument('--rev', default='HEAD')
    # the list kept outside the repository, where the release's scripts find it too
    ap.add_argument('--words', default=str(pathlib.Path.home() / 'VPI release' / 'scan-words.txt'))
    a = ap.parse_args()
    out = pathlib.Path(a.folder)
    if out.exists() and any(out.iterdir()):
        sys.exit(f'{out} isn\'t empty')
    out.mkdir(parents=True, exist_ok=True)
    rules = json.loads(git('show', f'{a.rev}:tools/release/public.json'))

    # the kept paths, as the commit has them
    tar = git('archive', '--format=tar', a.rev, '--', *[k['path'].rstrip('/') for k in rules['keep']], binary=True)
    with tarfile.open(fileobj=io.BytesIO(tar)) as t:
        t.extractall(out, filter='data')
    for d in rules['drop']:
        target = out / d['path']
        if target.is_dir():
            shutil.rmtree(target)
        elif target.exists():
            target.unlink()
    for t in rules['top']:
        (out / t['path']).write_bytes(git('show', f'{a.rev}:{t["from"]}', binary=True))
    held = lambda p: (out / p).exists()

    credits, dropped = credits_public((out / 'CREDITS.md').read_text(encoding='utf-8'), held)
    (out / 'CREDITS.md').write_text(credits, encoding='utf-8', newline='')
    for d in dropped:
        print(f'CREDITS.md: left out the paragraph "{d}..."')

    toml, gone = members_public((out / 'Cargo.toml').read_text(encoding='utf-8'), held)
    (out / 'Cargo.toml').write_text(toml, encoding='utf-8', newline='')
    before = packages((out / 'Cargo.lock').read_text(encoding='utf-8'))
    # for Windows, which the programs are built for: asked about every system, cargo wants the
    # sources of other systems' packages, which a Windows build never fetched (the lock file it
    # writes still holds every system's)
    subprocess.run(['cargo', 'metadata', '--format-version', '1', '--offline', '--filter-platform', 'x86_64-pc-windows-msvc',
                    '--manifest-path', str(out / 'Cargo.toml')], capture_output=True, check=True)
    after = packages((out / 'Cargo.lock').read_text(encoding='utf-8'))
    if not after <= before:
        sys.exit(f'Cargo.lock changed versions: {sorted(after - before)[:10]}')
    print(f'Cargo.toml: {gone} workspace members left out; Cargo.lock: {len(before) - len(after)} packages only they used left out, no versions changed')

    # the patches without their Date lines: when and in which time zone each was made
    undone = 0
    for p in sorted(out.rglob('*.patch')):
        data, had = undated(p.read_bytes())
        if had:
            p.write_bytes(data)
            undone += 1
    print(f'patches: {undone} without their Date lines')

    # what mustn't go public
    words = [w for w in words_to_find(a.words) if w != 'vpx-build']
    pats = [(w, re.compile(w[3:].encode('utf-8')) if w.startswith('re:') else re.compile(re.escape(w.encode('utf-8')), re.I)) for w in words]
    pats += [(w + ' (UTF-16)', re.compile(re.escape(w.encode('utf-16-le')), re.I)) for w in words if not w.startswith('re:')]
    fine = fine_phrases(a.words)
    files = sorted(p for p in out.rglob('*') if p.is_file())
    finds, dates = 0, 0

    def shown(data, m):
        lo, hi = max(0, m.start() - 50), m.end() + 50
        return data[lo:hi].decode('utf-8', errors='replace').replace('\n', ' ').replace('\r', '')

    for p in files:
        rel = p.relative_to(out).as_posix()
        data = p.read_bytes()
        for name, pat in pats:
            for m in pat.finditer(data):
                if within_fine(data, m.start(), m.end(), fine):
                    continue
                finds += 1
                print(f'MUSTN\'T GO: {rel}: {name}: ...{shown(data, m)}...')
        if not rel.startswith(THEIRS):
            for m in DATE.finditer(data):
                dates += 1
                print(f'a date, to look at: {rel}: ...{shown(data, m)}...')
    size = sum(p.stat().st_size for p in files)
    print(f'{len(files)} files, {size / 1e6:.1f} MB, in {out}; {finds} finds that mustn\'t go, {dates} dates to look at')
    return 1 if finds else 0


if __name__ == '__main__':
    sys.exit(main())
