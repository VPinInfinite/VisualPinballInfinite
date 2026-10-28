"""The settings map: every setting VPX 5436 offers, from its own source, beside what
Visual Pinball Infinite does with it, then the other programs' settings (VPinMAME, B2S Server,
PinUP Player and Popper, VPin Studio, PinVol, table scripts, DOF, DMD Extensions).

    python -I tools/settings-map/settings_map.py [--check]

Reads `vpx-build/vpinball-5436/src/core/Settings_properties.inl` and the `MSGPI_*_SETTING`
declarations of its plugins, and our statuses from `tools/settings-map/status.json`; writes
`docs/private/settings-map.md`, the app's catalog of every setting for its Advanced page,
`app/ui/src/lib/settings-catalog.json` — each with its label, what it does, its kind, range,
default and choices as VPX 5436 declares them (GPL, as the license audit reads that file), by
category, with VPinMAME's for every machine and ours — and `docs/resources/settings.md`, the public list of
every setting that works in ours: its name in the file, what it does, its values and its default.
A VPX setting no status covers is listed as "not looked at yet", and `--check` fails while any is
(run it after moving to a newer VPX). Ours.
"""

import ast

import fnmatch
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
VPX = ROOT / "vpx-build" / "vpinball-5436"
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
    return "Plugin." + group[6:] if group.startswith("Plugin") else group


def vpx_core():
    """VPX's core settings, with the heading each falls under."""
    text = (VPX / "src" / "core" / "Settings_properties.inl").read_text(encoding="utf-8")
    out, heading = [], ""
    # a comment line ends a line's code (none hold `//` inside a string)
    lines = [re.sub(r'\s*//[^"]*$', "", l) if not l.strip().startswith("//") else l for l in text.split("\n")]
    i = 0
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
        if line.startswith("#") or not line:
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
        out.append({"section": section_name(group), "key": name, "label": label, "desc": desc, "heading": heading, "kind": kind, "args": args[4:]})
    # the same property twice under #ifdef: keep the first
    seen, unique = set(), []
    for p in out:
        k = (p["section"], p["key"])
        if k not in seen:
            seen.add(k)
            unique.append(p)
    return unique


def vpx_plugins():
    """VPX's plugins' settings (`MSGPI_*_SETTING(var, "Key", "Label", "Description", ...)`)."""
    out = []
    for cfg in sorted((VPX / "plugins").glob("*/plugin.cfg")):
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
        elif m == "STRING":
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
    ("vpinmame", "VPinMAME, every machine", "PinMAME's settings for every machine, kept in Machine settings.ini (a game's own, on its table's settings, count over these)."),
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


def catalog(rows, data):
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
    for key, label, kind, lo, hi, default, values in VPINMAME:
        e = {"s": "Machine.default", "k": key, "l": label, "d": "", "c": "vpinmame", "g": "VPinMAME", "t": kind}
        if lo is not None:
            e["min"], e["max"] = lo, hi
        if default is not None:
            e["def"] = default
        if values:
            e["v"], e["v0"] = values, 0
        st = vpinmame_status(data, key)
        if st:
            e["st"], e["why"] = st[0], st[1]
        out.append(e)
    for v in data.get("vpi", []):
        e = {"s": "VPI", "k": v["key"], "l": v["label"], "d": v["what"], "c": "vpi", "g": "Ours", "t": v["kind"], "st": "works"}
        for f in ("def", "min", "max", "opts"):
            if f in v:
                e[f] = v[f]
        out.append(e)
    doc = {
        "about": "Every setting VPX 5436 declares (src/core/Settings_properties.inl, its plugins' MSGPI_*_SETTING declarations), VPinMAME's for every machine and ours, for the app's Advanced page. Generated by tools/settings-map/settings_map.py: edit that and tools/settings-map/status.json, not this file.",
        "categories": [{"id": i, "label": l, "note": n} for i, l, n in CATEGORIES],
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
    entries = catalog(rows, data)
    print(f"settings catalog: {len(entries)} settings -> {CATALOG.relative_to(ROOT)}")
    print(f"settings list: {settings_list(entries)} that work -> {SETTINGS.relative_to(ROOT)}")
    if "--check" in sys.argv and unknown:
        print("not looked at yet: " + ", ".join(unknown[:40]))
        sys.exit(1)


if __name__ == "__main__":
    main()
