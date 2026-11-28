"""The settings map: every setting VPX 5436 offers, from its own source, beside what
Visual Pinball Infinite does with it, then the other programs' settings (VPinMAME, B2S Server,
PinUP Player and Popper, VPin Studio, PinVol, table scripts, DOF, DMD Extensions).

    python -I tools/settings-map/settings_map.py [--check]

Reads `vpx-build/vpinball-5436/src/core/Settings_properties.inl` and the `MSGPI_*_SETTING`
declarations of its plugins, and our statuses from `tools/settings-map/status.json`; writes
`docs/private/settings-map.md` and `docs/resources/settings.md`, the public list of every setting
that works in our player: its name in the file, what it does, its values and its default. A VPX
setting no status covers is listed as "not looked at yet", and `--check` fails while any is.

The app's catalog of every setting for its Advanced page, `app/ui/src/lib/settings-catalog.json`,
comes from the VPX the app is made for instead (`VPX_APP`, the same declarations at its build's
commit): each setting with its label, what it does, its kind, range, default and choices as VPX
declares them, by category, with the classic VPinMAME's for every machine and those of ours the app
reads — no word of our player's on any. Move `VPX_APP` with the app's `MADE_FOR`. Ours.
"""

import ast

import fnmatch
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
VPX = ROOT / "vpx-build" / "vpinball-5436"
# the VPX the app is made for (library::vpx::MADE_FOR): its settings make the app's catalog, for the
# Advanced page, while the map and the public list stay those of VPX 5436 beside our player. Its
# source, sparse (src/core and plugins), at the build's commit: `git fetch --depth 1 --filter=blob:none`
VPX_APP = ROOT / "vpx-build" / "vpinball-7001"
VPX_APP_NAME = "VPX 10.9.0 build 7001"
STATUS = pathlib.Path(__file__).resolve().parent / "status.json"
OUT = ROOT / "docs" / "private" / "settings-map.md"
CATALOG = ROOT / "app" / "ui" / "src" / "lib" / "settings-catalog.json"
SETTINGS = ROOT / "docs" / "resources" / "settings.md"

KINDS = {
    "works": "Works",
    "later": "Later",
    "not-ours": "Not ours",
    "own-file": "Its own file",
}


def strip_s(text):
    return text[:-1] if text.endswith("s") else text


def split_args(text):
    """Splits a macro's arguments at top-level commas, keeping strings whole."""
    args, depth, cur, quote = [], 0, "", False
    i = 0
    while i < len(text):
        c = text[i]
        if quote:
            cur += c
            if c == "\\" and i + 1 < len(text):
                cur += text[i + 1]
                i += 1
            elif c == '"':
                quote = False
        elif c == '"':
            quote = True
            cur += c
        elif c in "([{":
            depth += 1
            cur += c
        elif c in ")]}":
            depth -= 1
            cur += c
        elif c == "," and depth == 0:
            args.append(cur.strip())
            cur = ""
        else:
            cur += c
        i += 1
    if cur.strip():
        args.append(cur.strip())
    return args


def literal(arg):
    """A C++ string literal argument (`"..."s`, joined pieces) as text."""
    parts = re.findall(r'"((?:[^"\\]|\\.)*)"', arg)
    return "".join(parts).replace("\\n", " ").replace('\\"', '"')


def section_name(group):
    """A group's section in the settings file, as VPX's `GetBackwardCompatibleSection` names it."""
    if group.startswith("Plugin"):
        return "Plugin." + group[6:]
    if group.startswith("DefaultProps"):
        return "DefaultProps\\" + group[12:]
    return group


# the names VPX's Windows build is made with, for the declarations' `#if`s: the BGFX build, neither
# Android's nor Apple's
WINDOWS_BUILD = {"ENABLE_BGFX": True, "_WIN32": True, "_WIN64": True, "__ANDROID__": False, "__APPLE__": False, "SDL_PLATFORM_ANDROID": False, "__LIBVPINBALL__": False}


def condition(expr):
    """A preprocessor condition, as VPX's Windows build has its names defined (unknown ones not)."""
    known = lambda m: str(WINDOWS_BUILD.get(m.group(1), False))
    e = re.sub(r"defined\s*\(\s*(\w+)\s*\)", known, expr)
    e = re.sub(r"defined\s+(\w+)", known, e)
    e = e.replace("&&", " and ").replace("||", " or ")
    e = re.sub(r"!(?!=)", " not ", e)
    e = re.sub(r"\b(?!True\b|False\b|and\b|or\b|not\b)([A-Za-z_]\w*)\b", known, e)
    try:
        return bool(eval(e, {"__builtins__": {}}))
    except Exception:
        return False


def strip_comment(line):
    """A line's code without its `//` comment, a `//` inside a string kept."""
    quote, i = False, 0
    while i < len(line):
        c = line[i]
        if quote and c == "\\":
            i += 2
            continue
        if c == '"':
            quote = not quote
        elif not quote and line.startswith("//", i):
            return line[:i].rstrip()
        i += 1
    return line


def macros_of(root):
    """The values VPX names its defaults by (`#define DEFAULT_TABLE_GRAVITY 0.97f`), from its physics
    header, for the declarations that give a default by name."""
    out = {}
    header = root / "src" / "physics" / "physconst.h"
    if header.is_file():
        for m in re.finditer(r"^\s*#define\s+([A-Z_][A-Z0-9_]*)\s+([^\n/]+)", header.read_text(encoding="utf-8", errors="replace"), re.M):
            out[m.group(1)] = m.group(2).strip()
    return out


def with_macros(arg, macros):
    """An argument with the names of known values in it replaced by those values."""
    for _ in range(4):
        new = re.sub(r"\b([A-Z_][A-Z0-9_]+)\b", lambda m: macros.get(m.group(1), m.group(1)), arg)
        if new == arg:
            break
        arg = new
    return arg


def vpx_core(root=VPX):
    """VPX's core settings, with the heading each falls under: those its Windows build declares
    (the `#if`s' branches it takes), their defaults given by name read as their values."""
    text = (root / "src" / "core" / "Settings_properties.inl").read_text(encoding="utf-8")
    out, heading = [], ""
    macros = macros_of(root)
    # a comment ends a line's code, a `//` inside a string kept
    lines = [strip_comment(l) if not l.strip().startswith("//") else l for l in text.split("\n")]
    i = 0
    # the `#if`s around a line: whether each branch is taken, and whether one was
    stack = []
    # a block of comments (preprocessor lines between them kept in it) heads what follows: its first line
    in_block = False
    while i < len(lines):
        line = lines[i].strip()
        if line.startswith("//"):
            if not in_block:
                heading = line[2:].strip()
            in_block = True
            i += 1
            continue
        if line.startswith("#"):
            d = re.match(r"#\s*(ifdef|ifndef|if|elif|else|endif|define)\b\s*(.*)", line)
            if d:
                kind, rest = d.group(1), d.group(2).strip()
                if kind in ("ifdef", "ifndef"):
                    v = WINDOWS_BUILD.get(rest.split()[0] if rest else "", False) == (kind == "ifdef")
                    stack.append([v, v])
                elif kind == "if":
                    v = condition(rest)
                    stack.append([v, v])
                elif kind == "elif" and stack:
                    v = not stack[-1][1] and condition(rest)
                    stack[-1] = [v, stack[-1][1] or v]
                elif kind == "else" and stack:
                    stack[-1] = [not stack[-1][1], True]
                elif kind == "endif" and stack:
                    stack.pop()
                elif kind == "define" and all(s[0] for s in stack):
                    name, _, value = rest.partition(" ")
                    macros[name] = value.strip()
            i += 1
            continue
        if not line or not all(s[0] for s in stack):
            i += 1
            continue
        in_block = False
        m = re.match(r"(Prop[A-Za-z]+)\(", line)
        if not m:
            i += 1
            continue
        body = line
        # join continuation lines: a property runs until its parentheses close (one VPX leaves
        # without its semicolon)
        def closed(b):
            depth, quote = 0, False
            for c in b:
                if c == '"':
                    quote = not quote
                elif not quote and c == "(":
                    depth += 1
                elif not quote and c == ")":
                    depth -= 1
                    if depth == 0:
                        return True
            return False
        while not closed(body) and i + 1 < len(lines):
            i += 1
            body += " " + lines[i].strip()
        i += 1
        kind = m.group(1)
        if kind == "PropArray":
            continue
        args = split_args(body[len(kind) + 1:body.rstrip().rfind(")")])
        if len(args) < 2:
            continue
        group, name = args[0], args[1]
        label = literal(args[2]) if len(args) > 2 else ""
        desc = literal(args[3]) if len(args) > 3 else ""
        if not name or not re.match(r"^\w+$", name):
            continue
        out.append({"section": section_name(group), "key": name, "label": label, "desc": desc, "heading": heading, "kind": kind, "args": [with_macros(a, macros) for a in args[4:]]})
    # the same property twice where no `#if` tells them apart: keep the first
    seen, unique = set(), []
    for p in out:
        k = (p["section"], p["key"])
        if k not in seen:
            seen.add(k)
            unique.append(p)
    return unique


def vpx_plugins(root=VPX):
    """VPX's plugins' settings (`MSGPI_*_SETTING(var, "Key", "Label", "Description", ...)`)."""
    out = []
    for cfg in sorted((root / "plugins").glob("*/plugin.cfg")):
        m = re.search(r'^\s*id\s*=\s*"([^"]+)"', cfg.read_text(encoding="utf-8"), re.M)
        if not m:
            continue
        plugin = m.group(1)
        seen = set()
        for src in sorted(cfg.parent.rglob("*.cpp")):
            for d in re.finditer(r"MSGPI_[A-Z_]*SETTING[A-Z_]*\(([^;]*)\);", src.read_text(encoding="utf-8", errors="replace")):
                args = split_args(d.group(1))
                if len(args) < 4:
                    continue
                key = literal(args[1])
                if not key or key in seen:
                    continue
                seen.add(key)
                macro = re.match(r"MSGPI_([A-Z]+)_", d.group(0)).group(1)
                values = None
                if macro == "ENUM" and len(args) > 7:
                    # the choices' names, an array the same file declares
                    arr = re.search(re.escape(args[7]) + r"\s*\[\s*\]\s*=\s*\{([^}]*)\}", src.read_text(encoding="utf-8", errors="replace"))
                    values = [literal(a) for a in split_args(arr.group(1))] if arr else None
                out.append({"section": "Plugin." + plugin, "key": key, "label": literal(args[2]), "desc": literal(args[3]), "heading": plugin, "kind": "plugin", "macro": macro, "args": args[5:], "values": values})
    return out


def number(arg):
    """A C++ number argument as a number (casts, suffixes and comments taken off), else None."""
    text = re.sub(r"/\*.*?\*/", "", arg).strip()
    text = re.sub(r"\((?:int|float|double|unsigned int)\)", "", text)
    if text in ("true", "false"):
        return text == "true"
    text = re.sub(r"(?<=[0-9.])[fF]\b", "", text).replace("u", "") if re.fullmatch(r"[-+*/(). 0-9eEfFu]+", text) else text
    try:
        tree = ast.parse(text, mode="eval")
    except SyntaxError:
        return None
    ok = all(isinstance(n, (ast.Expression, ast.BinOp, ast.UnaryOp, ast.Constant, ast.operator, ast.unaryop)) for n in ast.walk(tree))
    if not ok:
        return None
    try:
        v = eval(compile(tree, "<number>", "eval"), {"__builtins__": {}})
    except Exception:
        return None
    if isinstance(v, float):
        if abs(v) > 1e30:
            return None
        v = round(v, 6)
        if v == int(v) and "." not in text and "e" not in text.lower():
            v = int(v)
    return v


def text_default(arg):
    """A C++ string default as text, else None (one made by code)."""
    arg = arg.strip()
    return literal(arg) if arg.startswith('"') else None


def described(p):
    """A VPX property's kind, range, default and choices, from its declaration's arguments."""
    k, a = p["kind"], p.get("args", [])
    out = {}
    if k.startswith("PropBool"):
        out = {"t": "bool", "def": number(a[0]) if a else None}
    elif k == "PropIntUnbounded":
        out = {"t": "int", "def": number(a[0]) if a else None}
    elif k.startswith("PropInt"):
        out = {"t": "int", "min": number(a[0]), "max": number(a[1]), "def": number(a[2]) if len(a) > 2 else None}
    elif k == "PropFloatUnbounded":
        out = {"t": "float", "def": number(a[0]) if a else None}
    elif k.startswith("PropFloatStepped"):
        out = {"t": "float", "min": number(a[0]), "max": number(a[1]), "step": number(a[2]), "def": number(a[3]) if len(a) > 3 else None}
    elif k.startswith("PropFloat"):
        out = {"t": "float", "min": number(a[0]), "max": number(a[1]), "def": number(a[2]) if len(a) > 2 else None}
    elif k.startswith("PropString"):
        out = {"t": "string", "def": text_default(a[0]) if a else None}
    elif k == "PropEnumWithMin":
        out = {"t": "enum", "v0": number(a[1]), "def": number(a[2]), "v": [literal(x) for x in a[3:]]}
    elif k.startswith("PropEnum"):
        out = {"t": "enum", "v0": 1 if k == "PropEnum1" else 0, "def": number(a[1]) if len(a) > 1 else None, "v": [literal(x) for x in a[2:]]}
    elif k == "plugin":
        m = p["macro"]
        if m == "BOOL":
            out = {"t": "bool", "def": number(a[0]) if a else None}
        elif m == "INT":
            out = {"t": "int", "min": number(a[0]), "max": number(a[1]), "def": number(a[2]) if len(a) > 2 else None}
        elif m == "FLOAT":
            out = {"t": "float", "min": number(a[0]), "max": number(a[1]), "step": number(a[2]), "def": number(a[3]) if len(a) > 3 else None}
        elif m == "ENUM":
            out = {"t": "enum", "v0": number(a[0]), "def": number(a[3]) if len(a) > 3 else None, "v": p.get("values") or []}
        elif m in ("STRING", "FOLDER", "FILE"):
            out = {"t": "string", "def": text_default(a[0]) if a else None}
    if out.get("t") == "enum" and not out.get("v"):
        out = {"t": "int", "def": out.get("def")}
    # a range as wide as the numbers go is no range
    for edge in ("min", "max"):
        if edge in out and (out[edge] is None or isinstance(out[edge], bool) or abs(out[edge]) >= 2**31 - 1):
            del out[edge]
    if p.get("kind", "").endswith("Dyn"):
        out["live"] = True
    return {k2: v for k2, v in out.items() if v is not None}


# the catalog's categories, in order: (id, label, what it holds)
CATEGORIES = [
    ("windows", "Windows and screens", "Where each window goes, its screen and size; the cabinet's screens and where the player stands."),
    ("picture", "Picture", "How tables are drawn: quality, effects, the ball, light, smoothing and frame timing."),
    ("displays", "Score displays", "The looks of dot-matrix and segment displays, VPX's profiles."),
    ("sound", "Sound", "The sound outputs, the speaker layout and the volumes."),
    ("input", "Controls, nudging and plunger", "The tilt bob, the nudge and the plunger."),
    ("physics", "Physics", "VPX's physics sets, which a table's own can be swapped for."),
    ("table", "Table overrides and camera", "Settings that count over every table's own, and the camera tables open with."),
    ("plugins", "Plugins", "VPX 5436's plugins' settings, ours where we've built the same."),
    ("vpinmame", "VPinMAME, every machine", "The classic VPinMAME's settings for every machine, kept where it keeps them, in your part of Windows' registry (a game's own, on its table's settings, count over these)."),
    ("vpi", "Visual Pinball Infinite's own", "Ours, in the [VPI] section: what VPX hasn't."),
    ("vr", "VR and 3D", "VPX's VR and stereo 3D settings, kept for when ours has them."),
    ("player", "The player, other", "VPX's other player settings: logs, debugging, its standalone build's, and VPinMAME's older B2S and DOF switches."),
    ("editor", "VPX's editor", "VPX's table editor's settings and the defaults it gives new parts. VPI has no editor: they're kept as VPX keeps them."),
]

PICTURE = {"Graphics settings", "Aliasing & sharpening", "Ball rendering", "Overall scene lighting settings", "Graphics synchronisation and latency reduction"}
WINDOWS = {"Output (windows) settings", "Backglass Window", "ScoreView Window", "Topper Window", "Real world cabinet & player settings"}
INPUT = {"UI & input settings", "Plumb settings", "Nudge settings", "Plunger settings"}
STEREO = {"Stereo settings", "Red Cyan", "Green Magenta", "Blue Amber", "Cyan Red", "Magenta Green", "Amber Blue", "VR settings", "VR Preview Window"}


def category(p):
    sec, head = p["section"], p["heading"]
    if p["kind"] == "plugin" or sec == "Plugin":
        return "plugins"
    if head == "Audio settings":
        return "sound"
    if head in WINDOWS:
        return "windows"
    if head in PICTURE:
        return "picture"
    if sec in ("DMD", "Alpha"):
        return "displays"
    if head in INPUT:
        return "input"
    if head == "Physics override profiles":
        return "physics"
    if sec in ("TableOverride", "DefaultCamera"):
        return "table"
    if sec == "PlayerVR" or head in STEREO:
        return "vr"
    if sec in ("Editor", "CVEdit", "RecentDir", "Version") or sec.startswith("DefaultProps"):
        return "player" if head == "General Application settings" or head.startswith("Debugging") else "editor"
    return "player"


# VPinMAME's settings for every machine (`Machine settings.ini` `[default]`, the app's
# `Machine.default`), from PinMAME's own declarations — `src/windows/config.c`'s `pinmame_opts` with
# VPINMAME defined, `src/win32com/VPinMAMEConfig.cpp`'s `vpinmame_opts`, MAME's sound and cheat
# options — read at VPX 5436's PinMAME commit (BSD-3-Clause); what each does with ours from the
# settings map's VPinMAME table: (key, label, kind, min, max, default, choices)
VPINMAME = [
    ("sound", "Its sound", "bool", None, None, True, None),
    ("volume", "Its volume (dB)", "int", -32, 32, 0, None),
    ("sound_mode", "Its sound comes from", "enum", None, None, None, ["PinMAME's own", "Its AltSound pack", "PinSound", "PinSound with recordings"]),
    ("samples", "Older machines' recorded sounds", "bool", None, None, False, None),
    ("cheat", "Cheat mode (skips some games' start-up tests)", "bool", None, None, False, None),
    ("showpindmd", "A real DMD panel for it", "bool", None, None, False, None),
    ("showwindmd", "Its score window", "bool", None, None, True, None),
    ("dmd_red", "DMD color: red", "int", 0, 255, 255, None),
    ("dmd_green", "DMD color: green", "int", 0, 255, 88, None),
    ("dmd_blue", "DMD color: blue", "int", 0, 255, 32, None),
    ("dmd_perc0", "DMD off brightness (%)", "int", 0, 100, 20, None),
    ("dmd_perc33", "DMD low brightness (%)", "int", 0, 100, 33, None),
    ("dmd_perc66", "DMD medium brightness (%)", "int", 0, 100, 67, None),
    ("dmd_colorize", "DMD shades as colors of their own", "bool", None, None, False, None),
    ("dmd_red66", "Colored DMD: red at 66%", "int", 0, 255, 225, None),
    ("dmd_green66", "Colored DMD: green at 66%", "int", 0, 255, 15, None),
    ("dmd_blue66", "Colored DMD: blue at 66%", "int", 0, 255, 193, None),
    ("dmd_red33", "Colored DMD: red at 33%", "int", 0, 255, 6, None),
    ("dmd_green33", "Colored DMD: green at 33%", "int", 0, 255, 0, None),
    ("dmd_blue33", "Colored DMD: blue at 33%", "int", 0, 255, 214, None),
    ("dmd_red0", "Colored DMD: red at 0%", "int", 0, 255, 0, None),
    ("dmd_green0", "Colored DMD: green at 0%", "int", 0, 255, 0, None),
    ("dmd_blue0", "Colored DMD: blue at 0%", "int", 0, 255, 0, None),
    ("dmd_border", "VPinMAME's window: its border", "bool", None, None, True, None),
    ("dmd_title", "VPinMAME's window: its title", "bool", None, None, True, None),
    ("dmd_opacity", "VPinMAME's window: opacity (%)", "int", 0, 100, 100, None),
    ("dmd_antialias", "VPinMAME's window: smoothing (%)", "int", 0, 100, 50, None),
    ("dmd_only", "VPinMAME's window: the DMD alone", "bool", None, None, False, None),
    ("dmd_compact", "VPinMAME's window: compact", "bool", None, None, False, None),
    ("dmd_pos_x", "VPinMAME's window: left", "int", -10000, 10000, 0, None),
    ("dmd_pos_y", "VPinMAME's window: top", "int", -10000, 10000, 0, None),
    ("dmd_width", "VPinMAME's window: width", "int", 0, 10000, 0, None),
    ("dmd_height", "VPinMAME's window: height", "int", 0, 10000, 0, None),
    ("dmd_doublesize", "VPinMAME's window: size (2 = twice, 3 = three times…)", "int", 0, 10000, 0, None),
    ("ignore_rom_crc", "Run a ROM whose files differ from PinMAME's list", "bool", None, None, False, None),
    ("cabinet_mode", "Cabinet mode: no messages on start", "bool", None, None, False, None),
    ("fastframes", "Unthrottled frames at a game's start", "int", -1, 100000, -1, None),
    ("synclevel", "Frame rate kept in step for other programs", "int", -50, 60, 0, None),
    ("threadpriority", "Its thread's priority", "int", 0, 2, 1, None),
    ("low_latency_throttle", "Its work spread across each frame, for flipper latency", "bool", None, None, True, None),
    ("resampling_quality", "Resampling (0 fast, 1 normal)", "int", 0, 1, 0, None),
    ("force_stereo", "Always stereo out", "bool", None, None, False, None),
    ("vgmwrite", "A recording of its sound chips", "bool", None, None, False, None),
]


def vpinmame_status(data, key):
    """The settings map's word on one of VPinMAME's settings, from its VPinMAME table."""
    table = next((o for o in data["others"] if o["title"].startswith("VPinMAME")), None)
    for names, what, kind, ours in (table or {}).get("settings", []):
        listed = re.findall(r"`([a-z0-9_]+)`", names)
        # the range written `dmd_red66` … `dmd_blue0` holds the colored DMD's nine
        colored = "dmd_red66" in listed and re.fullmatch(r"dmd_(red|green|blue)(66|33|0)", key)
        if key in listed or colored:
            return [kind, ours or what]
    return None


def entries_with_status(rows, data):
    """Every setting with our player's word on it, for the public list: VPX 5436's, VPinMAME's, ours."""
    out = []
    for p in rows:
        st = p.get("status") or []
        e = {"s": p["section"], "k": p["key"], "l": p["label"] or p["key"], "d": p["desc"] if p["desc"] != p["label"] else "", "c": category(p), "g": p["heading"]}
        e.update(described(p))
        if st:
            e["st"] = st[0]
            if len(st) > 1 and st[1]:
                e["why"] = st[1]
        out.append(e)
    out.extend(vpinmame_entries(data, with_status=True))
    for v in data.get("vpi", []):
        e = {"s": "VPI", "k": v["key"], "l": v["label"], "d": v["what"], "c": "vpi", "g": "Ours", "t": v["kind"], "st": "works"}
        for f in ("def", "min", "max", "opts"):
            if f in v:
                e[f] = v[f]
        out.append(e)
    return out


def vpinmame_entries(data, with_status):
    out = []
    for key, label, kind, lo, hi, default, values in VPINMAME:
        e = {"s": "Machine.default", "k": key, "l": label, "d": "", "c": "vpinmame", "g": "VPinMAME", "t": kind}
        if lo is not None:
            e["min"], e["max"] = lo, hi
        if default is not None:
            e["def"] = default
        if values:
            e["v"], e["v0"] = values, 0
        st = vpinmame_status(data, key) if with_status else None
        if st:
            e["st"], e["why"] = st[0], st[1]
        out.append(e)
    return out


def read_by_the_app(key):
    """Whether the app, its library or our VPX add-on reads one of our own settings (`[VPI]` `key`):
    those our player alone reads stay off the app's page. The shared settings crate counts too, its
    tests not."""
    for folder in ("app/src", "engine/library/src", "engine/vpx-addon/src", "engine/settings/src"):
        for f in (ROOT / folder).rglob("*.rs"):
            code = f.read_text(encoding="utf-8", errors="replace").split("#[cfg(test)]")[0]
            if f'"{key}"' in code:
                return True
    return False


# the add-ons that send VPX sound (their `AudioSource` lanes), by their IDs, as VPX's sound settings
# name them: VPX's own and ours
AUDIO_SOURCES = [("PinMAME", "PinMAME"), ("AltSound", "AltSound"), ("PUP", "PinUP"), ("WMP", "Windows Media Player"), ("B2S", "B2S"), ("VPIAchievements", "Achievements")]


# the app's categories, as the catalog gives them: those VPX 5436's notes spoke of for our player
# said of VPX itself
APP_NOTES = {
    "plugins": "The settings of VPX's own add-ons: its PinMAME, backglass, PinUP, DMD, colour, sound and toy add-ons.",
    "vpi": "Ours, in the [VPI] section: what VPX hasn't.",
    "vr": "VPX's VR and stereo 3D settings.",
    "editor": "VPX's table editor's settings and the defaults it gives new parts.",
    "player": "VPX's other settings: logs, debugging, its profiles, and VPinMAME's older backglass and toy switches.",
}


def app_catalog(data):
    """The app's catalog for its Advanced page: every setting the VPX it's made for declares, the
    classic VPinMAME's for every machine and ours the app reads, as each is declared, with no word of
    our player's on them."""
    rows = vpx_core(VPX_APP) + vpx_plugins(VPX_APP)
    out = []
    for p in rows:
        e = {"s": p["section"], "k": p["key"], "l": p["label"] or p["key"], "d": p["desc"] if p["desc"] != p["label"] else "", "c": category(p), "g": p["heading"]}
        e.update(described(p))
        out.append(e)
    # each add-on's sound level, which VPX registers as the add-on's sound first plays rather than
    # declaring it (`Player::OnAudioSrcChanged`: `AudioSource.<add-on>.Gain`, 0 to 2, 1 its
    # default, for every table or one): those of VPX's own add-ons that send it sound, and ours
    for source, shown in AUDIO_SOURCES:
        out.append({"s": "Player", "k": f"AudioSource.{source}.Gain", "l": f"{shown} Gain", "d": f"Volume gain applied to audio from {shown}: 0 silent, 2 twice as loud.", "c": "sound", "g": "Audio source levels", "t": "float", "min": 0, "max": 2, "step": 0.05, "def": 1})
    out.extend(vpinmame_entries(data, with_status=False))
    for v in data.get("vpi", []):
        if not read_by_the_app(v["key"]):
            continue
        e = {"s": "VPI", "k": v["key"], "l": v["label"], "d": v["what"], "c": "vpi", "g": "Ours", "t": v["kind"]}
        for f in ("def", "min", "max", "opts"):
            if f in v:
                e[f] = v[f]
        out.append(e)
    doc = {
        "about": f"Every setting {VPX_APP_NAME} declares (src/core/Settings_properties.inl, its add-ons' MSGPI_*_SETTING declarations), the classic VPinMAME's for every machine and the app's own, for the app's Advanced page. Generated by tools/settings-map/settings_map.py: edit that and tools/settings-map/status.json, not this file.",
        "vpx": VPX_APP_NAME.removeprefix("VPX "),
        "categories": [{"id": i, "label": l, "note": APP_NOTES.get(i, n)} for i, l, n in CATEGORIES],
        "settings": out,
    }
    CATALOG.write_text(json.dumps(doc, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8", newline="\n")
    return out


# the public list's categories, in order, with what each holds
LIST_CATEGORIES = [
    ("windows", "Windows and screens", "Where each of the player's windows goes, on which screen and how big; the cabinet's screens and where the player stands."),
    ("picture", "Picture", "How tables are drawn: quality, effects, the ball, light, smoothing and frame timing."),
    ("displays", "Score displays", "How dot-matrix and segment displays look."),
    ("sound", "Sound", "The sound outputs, the speaker layout and the volumes."),
    ("input", "Controls, nudging and plunger", "The buttons for each action, the tilt bob, the nudge and the plunger."),
    ("physics", "Physics", "Eight saved physics sets, 0 to 7, each of a flipper's and the table's physics values: a table whose file names one plays with it in place of its own physics."),
    ("table", "Table overrides and camera", "Settings that count over every table's own, and the camera tables open with."),
    ("plugins", "Machines, packs, displays and toys", "ROM machines, PinUP packs, backglasses, color files, real DMD panels, the DMD upscaler and cabinet toys."),
    ("vpinmame", "Every ROM machine", "PinMAME's settings for every machine, in `Machine settings.ini`'s `[default]` section; a game's own section there counts over them for that game."),
    ("vpi", "Visual Pinball Infinite's own", "In the `[VPI]` section."),
    ("vr", "VR and 3D", "VR and stereo 3D."),
    ("player", "The player, other", "The player's other settings."),
    ("editor", "Editor", "The table editor's."),
]


# group names, as VPX heads them, said more plainly in the public list
LIST_HEADINGS = {
    "Controller: legacy VPinMAME B2S/DOF settings, exposed in VPX UI, then provided through Windows registry (replaced by plugin settings)": "Older tables' sound and toy hints (their controller.vbs reads these)",
    "Default core plugins enable state": "What's switched on",
    "Misc player settings": "Other",
}


def number_text(n):
    return str(int(n)) if isinstance(n, float) and n.is_integer() else str(n)


def values_text(e):
    """The values a setting takes, in words."""
    t = e.get("t")
    if e.get("opts"):
        return " · ".join(f"`{v}` {label}" if v else f"empty: {label}" for v, label in e["opts"])
    if t == "enum":
        base = e.get("v0", 0)
        return " · ".join(f"{base + i} {name}" for i, name in enumerate(e["v"]))
    if t == "bool":
        return "1 on, 0 off"
    if t in ("int", "float"):
        if "min" in e and "max" in e:
            text = f"{number_text(e['min'])} to {number_text(e['max'])}"
            return text + (f", in steps of {number_text(e['step'])}" if e.get("step") else "")
        return "a whole number" if t == "int" else "a number"
    return "text"


def default_text(e):
    """A setting's default, in words."""
    if "def" not in e:
        return ""
    d, t = e["def"], e.get("t")
    if e.get("opts"):
        label = dict(e["opts"]).get(d, "")
        return f"`{d}` ({label})" if d else f"empty ({label})"
    if t == "enum" and isinstance(d, (int, float)) and not isinstance(d, bool):
        i = int(d) - e.get("v0", 0)
        return f"{int(d)} ({e['v'][i]})" if 0 <= i < len(e["v"]) else number_text(d)
    if t == "bool":
        return "1 (on)" if d in (True, 1) else "0 (off)"
    if isinstance(d, str):
        return f"`{d}`" if d else "empty"
    return number_text(d)


def settings_list(entries):
    """docs/resources/settings.md: every setting that works in ours, by category and group, each with its
    name in the file, what it does, the values it takes and its default — every one laid out the
    same way (a table sizes its columns to what it holds, so the page's tables came out at
    different widths)."""
    works = [e for e in entries if e.get("st") == "works"]
    lines = [
        "# Settings",
        "",
        "Every setting Visual Pinball Infinite has today: its name in the settings file, what it does, the values it takes and its default. The app's Settings pages set the everyday ones, and its Advanced page lists every one.",
        "",
        "They're kept in `%APPDATA%\\Visual Pinball Infinite\\Settings.ini`, each in its `[section]`; a table's own settings, in a `<table>.ini` beside the table, count over them for that table. The ROM machines' are in `Machine settings.ini` beside it: its `[default]` section for every machine, a game's own section for one.",
        "",
        f"{len(works)} settings. This page is made from the program's own list of its settings (`tools/settings-map`), so it grows as settings arrive.",
        "",
    ]
    for cid, label, note in LIST_CATEGORIES:
        items = [e for e in works if e["c"] == cid]
        if not items:
            continue
        lines += [f"## {label}", "", note, ""]
        groups = {}
        for e in items:
            groups.setdefault(e["g"], []).append(e)
        for heading, group in groups.items():
            if len(groups) > 1:
                lines += [f"### {cell(LIST_HEADINGS.get(heading, heading))}", ""]
            for e in group:
                name = f"`[default]` `{e['k']}` in `Machine settings.ini`" if e["s"] == "Machine.default" else f"`[{e['s']}]` `{e['k']}`"
                what = cell(e.get("d", ""))
                if what and what[-1] not in ".!?:":
                    what += "."
                default = default_text(e)
                block = [f"- **{cell(e['l'])}** · {name}"] + ([f"  {what}"] if what else []) + [f"  Values: {values_text(e)}" + (f" · Default: {default}" if default else "")]
                # each line but the last ends in a backslash, Markdown's line break
                lines += [line + "\\" for line in block[:-1]] + [block[-1], ""]
    SETTINGS.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    return len(works)


def status_for(statuses, section, key):
    full = f"{section}.{key}"
    if full in statuses:
        return statuses[full]
    best = None
    for pattern, value in statuses.items():
        if "*" in pattern and fnmatch.fnmatchcase(full, pattern):
            # the longest pattern is the most precise
            if best is None or len(pattern) > len(best[0]):
                best = (pattern, value)
    return best[1] if best else None


def cell(text):
    return text.replace("|", "\\|").replace("\n", " ").strip()


def main():
    data = json.loads(STATUS.read_text(encoding="utf-8"))
    statuses = data["vpx"]
    core, plugins = vpx_core(), vpx_plugins()
    rows = core + plugins
    unknown = []
    counts = {}
    for p in rows:
        s = status_for(statuses, p["section"], p["key"])
        p["status"] = s
        kind = s[0] if s else "unknown"
        counts[kind] = counts.get(kind, 0) + 1
        if not s:
            unknown.append(f'{p["section"]}.{p["key"]}')

    lines = [
        "# Settings map",
        "",
        "Every setting VPX 5436 offers, read from its own source, beside what Visual Pinball Infinite does with it; then the settings of the other programs a VPX setup uses. **Works**: read from our settings file under VPX's name and acting as in VPX (or as the other program does). **Later**: it belongs to something we haven't built yet, named. **Not ours**: something VPX or the other program does that has no place in ours, with the reason. **Its own file**: the program that reads it is one we run, and it reads its own settings file itself. (generated by `tools/settings-map/settings_map.py` from `tools/settings-map/status.json` — edit those, not this page.)",
        "",
        "Where the settings live: `%APPDATA%\\Visual Pinball Infinite\\Settings.ini`, under VPX 5436's sections and names, a table's own `<table>.ini` beside it counting over it; VPinMAME's settings for each game in `Machine settings.ini` beside it, under VPinMAME's own names; add-ons' in `Add-on settings.ini`. A setting the file doesn't give keeps our default, which is VPX's unless a decision says otherwise; the player's command-line options count over the file.",
        "",
        f"VPX 5436: {len(core)} core settings and {len(plugins)} plugin settings — " + ", ".join(f"{KINDS.get(k, 'not looked at yet')} {n}" for k, n in sorted(counts.items(), key=lambda kv: -kv[1])) + ".",
        "",
    ]

    def table(title, items, note=None):
        lines.append(f"## {title}")
        lines.append("")
        if note:
            lines.append(note)
            lines.append("")
        groups = []
        for p in items:
            if not groups or groups[-1][0] != p["heading"]:
                groups.append((p["heading"], []))
            groups[-1][1].append(p)
        for heading, group in groups:
            # a group whose settings share one status, and are many, in one line
            shared = {json.dumps(p["status"]) for p in group}
            if len(group) > 6 and len(shared) == 1 and group[0]["status"]:
                s = group[0]["status"]
                names = ", ".join(f'`{p["key"]}`' for p in group[:4]) + f", … ({len(group)} in all)"
                lines.append(f"**{cell(heading)}** (`[{group[0]['section']}]`: {names}) — **{KINDS[s[0]]}**: {cell(s[1])}")
                lines.append("")
                continue
            lines.append(f"**{cell(heading)}**")
            lines.append("")
            lines.append("| Setting | What it does | Ours |")
            lines.append("|---|---|---|")
            # runs of more than four settings sharing a status in one row
            runs = []
            for p in group:
                if runs and runs[-1][0]["status"] == p["status"] and p["status"]:
                    runs[-1].append(p)
                else:
                    runs.append([p])
            for run in runs:
                p = run[0]
                s = p["status"]
                ours = f"**{KINDS[s[0]]}**" + (f" — {s[1]}" if len(s) > 1 and s[1] else "") if s else "**Not looked at yet**"
                if len(run) > 4:
                    names = f"`{run[0]['key']}`, `{run[1]['key']}` … `{run[-1]['key']}` ({len(run)} settings)"
                    lines.append(f"| `[{p['section']}]` {names} | {cell(p['label'])} and the rest | {cell(ours)} |")
                    continue
                for p in run:
                    what = p["label"] if not p["desc"] or p["desc"] == p["label"] else f'{p["label"]}: {p["desc"]}'
                    if len(what) > 220:
                        what = what[:217] + "…"
                    lines.append(f"| `[{p['section']}]` `{p['key']}` | {cell(what)} | {cell(ours)} |")
            lines.append("")

    table("VPX 5436 — the player and its settings", core, data.get("vpx_note"))
    table("VPX 5436's plugins", plugins, data.get("plugins_note"))

    if data.get("vpi"):
        lines.append("## Visual Pinball Infinite's own")
        lines.append("")
        lines.append("Ours, in the settings file's `[VPI]` section, for what VPX hasn't; the app's pages write them, and its Advanced page lists them with VPX's.")
        lines.append("")
        lines.append("| Setting | What it does |")
        lines.append("|---|---|")
        for v in data["vpi"]:
            lines.append(f"| `[VPI]` `{v['key']}` | {cell(v['label'])}: {cell(v['what'])} |")
        lines.append("")

    for other in data["others"]:
        lines.append(f"## {other['title']}")
        lines.append("")
        if other.get("note"):
            lines.append(other["note"])
            lines.append("")
        lines.append("| Setting | What it does | Ours |")
        lines.append("|---|---|---|")
        for name, what, kind, ours in other["settings"]:
            lines.append(f"| {cell(name)} | {cell(what)} | **{KINDS[kind]}**" + (f" — {cell(ours)}" if ours else "") + " |")
        lines.append("")

    if unknown:
        lines.append("## Not looked at yet")
        lines.append("")
        lines.append(", ".join(f"`{u}`" for u in unknown))
        lines.append("")

    OUT.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    print(f"settings map: {len(rows)} VPX settings, {len(unknown)} not looked at yet -> {OUT.relative_to(ROOT)}")
    entries = entries_with_status(rows, data)
    app = app_catalog(data)
    print(f"settings catalog: {len(app)} settings of {VPX_APP_NAME}'s, VPinMAME's and ours -> {CATALOG.relative_to(ROOT)}")
    print(f"settings list: {settings_list(entries)} that work -> {SETTINGS.relative_to(ROOT)}")
    if "--check" in sys.argv and unknown:
        print("not looked at yet: " + ", ".join(unknown[:40]))
        sys.exit(1)


if __name__ == "__main__":
    main()
