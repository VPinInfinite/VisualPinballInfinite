"""Writes a release's licences and notices into <release>/files:

  LICENSE.txt              our programs' licence: the GNU General Public License, version 3
  THIRD-PARTY-NOTICES.txt  everything in the installed files that others made, each with its
                           licence and where it comes from: the libraries beside the programs,
                           what's built into the programs (Rust crates and the C libraries they
                           compile in, code ported from other projects, fonts, pictures, data) and
                           the packages the app's screens are built with
  licenses/                each one's licence texts in full, as its package or source carries them

    python -I tools/release/notices.py --release dist/release/<version>
"""
import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
REPOSITORY = 'https://github.com/VPinInfinite/VisualPinballInfinite'
# the programs the installer ships, by their packages: what's built into them is what they use
SHIPPED = ['player', 'pinmame-host', 'colour-host', 'dmd-host', 'dmddevice-host', 'dof-host', 'headless', 'app']

# where each library beside the programs comes from; each is built from that source, at the
# version Visual Pinball X build 5436 uses, by the build scripts in the repository's tools folder
SOURCES = {
    'PinMAME': 'https://github.com/vpinball/pinmame',
    'libaltsound': 'https://github.com/vpinball/libaltsound',
    'libwinevbs': 'https://github.com/vpinball/libwinevbs',
    'MinGW-w64 winpthreads': 'https://www.mingw-w64.org (as MSYS2 builds it)',
    'FFmpeg': 'https://ffmpeg.org (its LGPL parts only)',
    'zlib': 'https://zlib.net (as MSYS2 builds it)',
    'bzip2': 'https://sourceware.org/bzip2 (as MSYS2 builds it)',
    'XZ Utils': 'https://tukaani.org/xz (as MSYS2 builds it)',
    'GNU libiconv': 'https://www.gnu.org/software/libiconv (as MSYS2 builds it)',
    'libserum': 'https://github.com/PPUC/libserum',
    'libvni': 'https://github.com/PPUC/libvni',
    'libdmdutil': 'https://github.com/vpinball/libdmdutil',
    'libzedmd': 'https://github.com/PPUC/libzedmd',
    'libpupdmd': 'https://github.com/PPUC/libpupdmd',
    'sockpp': 'https://github.com/fpagliughi/sockpp',
    'libserialport': 'https://github.com/sigrokproject/libserialport',
    'libusb': 'https://github.com/libusb/libusb',
    'libdof': 'https://github.com/jsm174/libdof',
    'hidapi': 'https://github.com/libusb/hidapi',
    'libftdi': 'https://github.com/jsm174/libftdi',
    'libarchive': 'https://github.com/libarchive/libarchive',
    'rom2altsound': 'https://github.com/Le-Syl21/rom2altsound (v0.4.0; its PinMAME from https://github.com/Le-Syl21/pinmame, branch bsmt2000-lle)',
    'Microsoft Visual C++ runtime': 'Visual Studio\'s redistributable files',
}

# the GNU LGPL 2.1's text, for a library whose source carries it under another file's name
LGPL21 = ROOT / 'vendor' / 'ffmpeg' / 'COPYING.LGPLv2.1'

# what's built into the programs that isn't a Rust crate: code ported from other projects, fonts,
# pictures and data, each with its licence and the files carrying its notice in this repository
BUILT_IN = [
    {
        'name': 'Visual Pinball X',
        'by': 'the Visual Pinball development team and contributors',
        'licence': 'GPL-3.0-or-later',
        'what': 'Code ported from Visual Pinball X build 5436 (the physics, much of the player and its plugins), and pictures and layouts of its own',
        'source': 'https://github.com/vpinball/vpinball',
        'texts': [],
        'note': 'Under the same licence as our programs: LICENSE.txt.',
    },
    {
        'name': 'FlexDMD',
        'by': 'Vincent Bousquet, with Glide\'s tweens by Jacob Albano and the bitmap-font reader by Cyotek Ltd.',
        'licence': 'Apache-2.0 (FlexDMD), MIT (the tweens, the font reader)',
        'what': 'FlexDMD 1.9.1, ported, for tables that draw their own score displays',
        'source': 'https://github.com/vbousquet/flexdmd',
        'texts': ['engine/player/src/flexdmd/LICENSE-FlexDMD.txt'],
    },
    {
        'name': 'FlexDMD\'s fonts and pictures',
        'by': 'their authors, as listed',
        'licence': 'each font\'s own terms, as listed',
        'what': 'The bitmap fonts and pictures FlexDMD carries inside itself, built into the player as FlexDMD builds them into itself',
        'source': 'https://github.com/vbousquet/flexdmd',
        'table': 'engine/player/assets/flexdmd/README.md',
    },
    {
        'name': 'VPin Studio',
        'by': 'syd711',
        'licence': 'MIT',
        'what': 'Its rules for sorting out what a download holds and its matching of tables to the Virtual Pinball Spreadsheet, ported',
        'source': 'https://github.com/syd711/vpin-studio',
        'texts': ['engine/library/LICENSE-VPinStudio.txt'],
    },
    {
        'name': 'JANPatch',
        'by': 'ARM Ltd.',
        'licence': 'Apache-2.0',
        'what': 'Its reader of JojoDiff patches, ported, for applying table patches',
        'source': 'https://github.com/janjongboom/janpatch',
        'texts': ['engine/library/LICENSE-JANPatch.txt'],
    },
    {
        'name': 'libpupdmd',
        'by': 'the PPUC project',
        'licence': 'GPL-3.0',
        'what': 'Its triggers from pictures of the score display, ported, for PinUP packs',
        'source': 'https://github.com/PPUC/libpupdmd',
        'texts': [],
        'note': 'The GNU GPL 3\'s text: LICENSE.txt.',
    },
    {
        'name': 'PinMAME\'s alphanumeric display drawing',
        'by': 'the PinMAME development team and contributors',
        'licence': 'BSD-3-Clause',
        'what': 'How PinMAME\'s display devices draw a segment game\'s digits as dots, ported',
        'source': 'https://github.com/vpinball/pinmame',
        'texts': [],
        'note': 'The licence\'s text is in PinMAME\'s licence: licenses/PinMAME.txt.',
    },
    {
        'name': 'xBRZ',
        'by': 'Zenju',
        'licence': 'GPL-3.0-only',
        'what': 'The xBRZ picture upscaler, ported from Visual Pinball X\'s DMD upscaler',
        'source': 'https://sourceforge.net/projects/xbrz',
        'texts': [],
        'note': 'The GNU GPL 3\'s text: LICENSE.txt.',
    },
    {
        'name': 'ScaleFX',
        'by': 'Sp00kyFox',
        'licence': 'MIT',
        'what': 'The ScaleFX picture upscaler, ported from Visual Pinball X\'s DMD upscaler',
        'source': 'https://github.com/libretro/slang-shaders',
        'mit': 'Copyright (c) 2016 Sp00kyFox',
    },
    {
        'name': 'Super-xBR',
        'by': 'Hyllian',
        'licence': 'MIT',
        'what': 'The Super-xBR picture upscaler, ported from Visual Pinball X\'s DMD upscaler',
        'source': 'https://github.com/libretro/common-shaders',
        'mit': 'Copyright (c) 2016 Hyllian',
    },
    {
        'name': 'MMPX',
        'by': 'Morgan McGuire and Mara Gagiu',
        'licence': 'MIT',
        'what': 'The MMPX picture upscaler, ported from Visual Pinball X\'s DMD upscaler',
        'source': 'https://casual-effects.com/research/McGuire2021PixelArt',
        'mit': 'Copyright 2020 Morgan McGuire & Mara Gagiu',
    },
    {
        'name': 'SMAA',
        'by': 'Jorge Jimenez, Jose I. Echevarria, Belen Masia, Fernando Navarro and Diego Gutierrez',
        'licence': 'MIT',
        'what': 'SMAA\'s smoothing filter and its lookup pictures, as Visual Pinball X carries them',
        'source': 'https://github.com/iryoku/smaa',
        'texts': ['engine/player/assets/smaa/README.txt'],
    },
    {
        'name': 'Inter',
        'by': 'Rasmus Andersson',
        'licence': 'OFL-1.1',
        'what': 'A font of the app and the player\'s pause menu',
        'source': 'https://github.com/rsms/inter',
        'texts': ['engine/player/assets/fonts/LICENSE-Inter.txt'],
    },
    {
        'name': 'Barlow Condensed',
        'by': 'Jeremy Tribby',
        'licence': 'OFL-1.1',
        'what': 'A font of the app and the player\'s pause menu',
        'source': 'https://github.com/jpt/barlow',
        'texts': ['engine/player/assets/fonts/LICENSE-BarlowCondensed.txt'],
    },
    {
        'name': 'Liberation Sans',
        'by': 'Red Hat, Inc. and Google Corporation',
        'licence': 'OFL-1.1',
        'what': 'The font PinUP packs\' labels fall back to, as Visual Pinball X\'s',
        'source': 'https://github.com/liberationfonts/liberation-fonts',
        'texts': ['engine/player/assets/fonts/LICENSE-LiberationSans.txt'],
    },
    {
        'name': 'Rostock-Laage Airport',
        'by': 'Greg Zaal, Poly Haven',
        'licence': 'CC0-1.0',
        'what': 'The room picture tables are drawn in when they name none of their own, made from Poly Haven\'s picture',
        'source': 'https://polyhaven.com/a/rostock_laage_airport',
        'texts': [],
        'note': 'Dedicated to the public domain: free for any use.',
    },
]

# the packages the app's screens are built with whose code or fonts end up in them: Svelte's
# runtime, with clsx, which it calls (both from a build package, so listed by hand), and every
# package the screens depend on, with what those depend on in turn (checked against the lock file)
SCREEN_PACKAGES = [
    'svelte', 'clsx', '@tauri-apps/api', '@fontsource/inter', '@fontsource/barlow-condensed',
    '@codemirror/state', '@codemirror/view', '@codemirror/commands', '@codemirror/search',
    '@codemirror/language', '@codemirror/legacy-modes', '@codemirror/streamparser',
    '@lezer/common', '@lezer/highlight', '@lezer/lr', '@marijn/find-cluster-break', 'crelt',
    'style-mod', 'w3c-keyname',
]


def screen_dependencies():
    """Every package the screens depend on, as their lock file has it, and theirs in turn."""
    lock = json.loads((ROOT / 'app' / 'ui' / 'package-lock.json').read_text(encoding='utf-8'))
    packages = lock['packages']
    found, waiting = set(), list(packages['']['dependencies'])
    while waiting:
        name = waiting.pop()
        if name not in found:
            found.add(name)
            waiting += list(packages.get(f'node_modules/{name}', {}).get('dependencies', {}))
    return found

LICENCE_FILE = re.compile(r'^(licen[cs]e|copying|notice|unlicense|copyright|ftl)([-._ ].*)?$', re.I)
SKIP_DIRS = {'test', 'tests', 'testdata', 'test-data', 'fuzz', 'examples', 'example', 'benches', 'doc', 'docs', '.github', 'target'}

MIT_TEXT = """Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
"""


def safe(name):
    return re.sub(r'[^A-Za-z0-9._+-]+', '-', name).strip('-')


def licence_files(folder, deep=True):
    """A package's licence files: at its top, and (for one carrying other projects' sources) inside it."""
    found = [p for p in sorted(folder.iterdir()) if p.is_file() and LICENCE_FILE.match(p.name)]
    if deep:
        for p in sorted(folder.rglob('*')):
            rel = p.relative_to(folder)
            if p.is_file() and p.parent != folder and LICENCE_FILE.match(p.name) and len(rel.parts) <= 6 and not any(part.lower() in SKIP_DIRS for part in rel.parts[:-1]):
                found.append(p)
    return found


def read(path):
    data = path.read_bytes()
    for enc in ('utf-8', 'cp1252'):
        try:
            return data.decode(enc)
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1')


def write_texts(out, header, paths, base):
    parts = [header, '']
    for p in paths:
        # by its path in the source; one taken from the libraries' build folder, which isn't a part of
        # the source, by its name alone
        in_source = base and p.is_relative_to(base) and not p.is_relative_to(ROOT / 'vpx-build')
        rel = p.relative_to(base) if in_source else p.name
        parts += ['=' * 78, str(rel).replace('\\', '/'), '=' * 78, read(p).rstrip(), '']
    out.write_text('\n'.join(parts) + '\n', encoding='utf-8')


def rust_crates(workspace=ROOT, shipped=SHIPPED):
    meta = json.loads(subprocess.run(
        ['cargo', 'metadata', '--format-version', '1', '--locked', '--offline', '--filter-platform', 'x86_64-pc-windows-msvc'],
        cwd=workspace, capture_output=True, check=True).stdout)
    packages = {p['id']: p for p in meta['packages']}
    members = set(meta['workspace_members'])
    nodes = {n['id']: n for n in meta['resolve']['nodes']}
    todo = [i for i in members if packages[i]['name'] in shipped]
    seen = set()
    while todo:
        i = todo.pop()
        if i in seen:
            continue
        seen.add(i)
        for d in nodes[i]['deps']:
            # what's compiled into the programs: their dependencies, not the tools that build them
            if any(k.get('kind') is None for k in d['dep_kinds']):
                todo.append(d['pkg'])
    return sorted((packages[i] for i in seen if i not in members), key=lambda p: (p['name'].lower(), p['version']))


def crate_lines(crates, lic, folder_name):
    """Each crate's licence files written into licenses/<folder_name>, and its line for the notices."""
    lines = []
    for p in crates:
        folder = pathlib.Path(p['manifest_path']).parent
        texts = licence_files(folder)
        target = f"{folder_name}/{safe(p['name'])}-{safe(p['version'])}.txt"
        authors = ', '.join(p.get('authors') or []) or 'its authors'
        header = f"{p['name']} {p['version']}, by {authors}, under {p.get('license') or 'its own licence'}."
        if texts:
            write_texts(lic / target, header, texts, folder)
        else:
            # a package with no licence file of its own: the licence's own words, with its authors
            body = header + '\n\nIts package carries no licence file. '
            if (p.get('license') or '').startswith('MIT'):
                body += f'The MIT licence, with its authors as its copyright holders:\n\nCopyright (c) {authors}\n\n{MIT_TEXT}'
            else:
                body += 'Its licence\'s standard text applies.\n'
            (lic / target).write_text(body, encoding='utf-8')
        lines.append(f"{p['name']} {p['version']}  {p.get('license') or ''}  {p.get('repository') or ''}  (licenses/{target})")
    return lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--release', required=True)
    a = ap.parse_args()
    unlisted = sorted(screen_dependencies() - set(SCREEN_PACKAGES))
    if unlisted:
        print(f'notices: the screens use packages SCREEN_PACKAGES leaves out: {", ".join(unlisted)}', file=sys.stderr)
        return 1
    release = pathlib.Path(a.release)
    files = release / 'files'
    lic = files / 'licenses'
    shutil.rmtree(lic, ignore_errors=True)
    (lic / 'rust').mkdir(parents=True)
    (lic / 'screens').mkdir(parents=True)
    shutil.copyfile(ROOT / 'LICENSE', files / 'LICENSE.txt')
    listing = json.loads((ROOT / 'tools' / 'release' / 'files.json').read_text(encoding='utf-8'))
    third = ROOT / 'vpx-build' / 'thirdparty' / 'out' / 'windows-x64'
    out = [
        'Visual Pinball Infinite: third-party notices',
        '',
        'Visual Pinball Infinite is free software under the GNU General Public License, version 3 or',
        '(at your option) any later version: LICENSE.txt. It is built on the work of many others,',
        'listed here with their licences; each licence\'s text is in the licenses folder.',
        '',
        f'The source of everything here is published: ours, with the build scripts and the changes we',
        f'made to the libraries we build, at {REPOSITORY}; the others\' at the addresses given.',
        '',
    ]

    # the libraries beside the programs
    out += ['LIBRARIES BESIDE THE PROGRAMS', '=' * 29, '']
    by_name = {}
    for l in listing['libraries']:
        by_name.setdefault(l['name'], []).append(l)
    for name, libs in by_name.items():
        first = libs[0]
        texts = [ROOT / f for f in first.get('licenceFiles', [])]
        if first.get('licenceKey'):
            texts += sorted(third.glob(f"LICENSE-{first['licenceKey']}*.txt"))
        if name == 'libwinevbs':
            texts.append(LGPL21)
        target = f'{safe(name)}.txt'
        if texts:
            write_texts(lic / target, f'{name}: its licence, as its source carries it.', texts, ROOT)
        out += [name, f"  files:    {', '.join(l['file'] for l in libs)}", f"  licence:  {first['licence']}", f"  source:   {SOURCES.get(name, '')}"]
        out += [f'  text:     licenses/{target}' if texts else '  text:     redistributed as Microsoft\'s Visual Studio licence allows for its distributable code', '']

    # folders of other projects' files installed as they are
    for d in listing.get('folders', []):
        target = f"{safe(d['name'])}.txt"
        write_texts(lic / target, f"{d['name']}: their licence.", [ROOT / f for f in d['licenceFiles']], ROOT)
        out += [d['name'], f"  files:    {d['folder']}\\ ({d['pattern']})", f"  licence:  {d['licence']}", '  source:   https://github.com/vpinball/vpinball (the scripts of Visual Pinball X build 5436)', f'  text:     licenses/{target}', '']

    # Rust crates built into the programs
    crates = rust_crates()
    out += ['BUILT INTO THE PROGRAMS: RUST CRATES', '=' * 36, '', f'{len(crates)} crates, each under the licence its package states (an OR is a choice; we take', 'either). Their own licence files, and the licences of other projects\' code they carry inside', 'them, are in licenses/rust.', '']
    out += crate_lines(crates, lic, 'rust')
    out.append('')

    # rom2altsound's own Rust crates, from its own workspace
    r2a = ROOT / 'vpx-build' / 'rom2altsound'
    if any(l['name'] == 'rom2altsound' for l in listing['libraries']) and r2a.is_dir():
        crates = rust_crates(r2a, ['rom2altsound'])
        (lic / 'rust-rom2altsound').mkdir(parents=True, exist_ok=True)
        out += ['BUILT INTO ROM2ALTSOUND: RUST CRATES', '=' * 36, '', f'{len(crates)} crates, each under the licence its package states; their licence files are in', 'licenses/rust-rom2altsound.', '']
        out += crate_lines(crates, lic, 'rust-rom2altsound')
        out.append('')

    # other projects' code, fonts, pictures and data built into the programs
    out += ['BUILT INTO THE PROGRAMS: OTHER PROJECTS\' CODE, FONTS, PICTURES AND DATA', '=' * 71, '']
    for b in BUILT_IN:
        target = f"{safe(b['name'])}.txt"
        line = [b['name'], f"  by:       {b['by']}", f"  what:     {b['what']}", f"  licence:  {b['licence']}", f"  source:   {b['source']}"]
        if b.get('texts'):
            write_texts(lic / target, f"{b['name']}: its licence and notices.", [ROOT / t for t in b['texts']], ROOT)
            line.append(f'  text:     licenses/{target}')
        elif b.get('mit'):
            (lic / target).write_text(f"{b['name']}, by {b['by']}, under the MIT licence:\n\n{b['mit']}\n\n{MIT_TEXT}", encoding='utf-8')
            line.append(f'  text:     licenses/{target}')
        elif b.get('table'):
            rows = [r for r in read(ROOT / b['table']).splitlines() if r.startswith('|')]
            (lic / target).write_text(f"{b['name']}: each one's author and terms.\n\n" + '\n'.join(rows) + '\n', encoding='utf-8')
            line.append(f'  text:     licenses/{target}')
        if b.get('note'):
            line.append(f"  note:     {b['note']}")
        out += line + ['']

    # the packages the app's screens are built with
    out += ['IN THE APP\'S SCREENS', '=' * 20, '']
    modules = ROOT / 'app' / 'ui' / 'node_modules'
    for name in SCREEN_PACKAGES:
        folder = modules / pathlib.Path(name)
        info = json.loads((folder / 'package.json').read_text(encoding='utf-8'))
        target = f"screens/{safe(name)}-{safe(info.get('version', ''))}.txt"
        texts = licence_files(folder, deep=False)
        licence = info.get('license') or ''
        if texts:
            write_texts(lic / target, f"{name} {info.get('version', '')}, under {licence}.", texts, folder)
        repo = info.get('repository')
        repo = repo.get('url') if isinstance(repo, dict) else (repo or '')
        out.append(f"{name} {info.get('version', '')}  {licence}  {repo}  (licenses/{target})")
    out.append('')
    (files / 'THIRD-PARTY-NOTICES.txt').write_text('\n'.join(out), encoding='utf-8')
    print(f'notices: {len(by_name)} libraries, {len(crates)} crates, {len(BUILT_IN)} other works, {len(SCREEN_PACKAGES)} screen packages')
    return 0


if __name__ == '__main__':
    sys.exit(main())
