"""Every comment in the code that goes public, in one text file to read through: each file the public
repository keeps (tools/release/public.json) that holds code, its comments with their line numbers.
Other projects' files kept whole (vendor/) give only the lines of ours inside them, and a patch only
its own description and the comments it adds.

    python -I tools/release/every_comment.py <out.txt>
"""
import json
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
C_LIKE = {".rs", ".js", ".ts", ".mjs", ".c", ".cc", ".cpp", ".h", ".hpp", ".hlsl", ".hlsli", ".css", ".jsonc", ".m", ".java", ".kt", ".swift", ".cg", ".glsl", ".frag", ".vert"}
HASH = {".py", ".sh", ".ps1", ".psm1", ".toml", ".yml", ".yaml", ".cmake", ".pl", ".am", ".in", ".gitattributes", ".gitignore", ".cfg", ".conf", ".ini"}
HASH_NAMES = {"CMakeLists.txt", ".gitattributes", ".gitignore", "Makefile-manual", "Makefile"}
BATCH = {".bat", ".cmd"}
OURS = re.compile(r"Visual Pinball Infinite|VPI\b|\bvpi[-_]", re.IGNORECASE)


def public_files():
    spec = json.loads((ROOT / "tools" / "release" / "public.json").read_text(encoding="utf-8"))
    keep = [e["path"] if isinstance(e, dict) else e for e in spec["keep"]]
    drop = [e["path"] if isinstance(e, dict) else e for e in spec["drop"]]
    listed = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard"], cwd=ROOT, capture_output=True, text=True, encoding="utf-8").stdout.splitlines()
    kept = [f for f in listed if any(f == k or (k.endswith("/") and f.startswith(k)) for k in keep) and not any(f.startswith(x) for x in drop)]
    return [f for f in kept if (ROOT / f).is_file()]


def c_like(text, rust=False, template=False):
    """The comments of C-like code: (line, text) for each // and /* */, strings and characters skipped."""
    out, i, n, line = [], 0, len(text), 1
    while i < n:
        ch = text[i]
        if ch == "\n":
            line += 1
            i += 1
        elif text.startswith("//", i):
            end = text.find("\n", i)
            end = n if end < 0 else end
            out.append((line, text[i:end]))
            i = end
        elif text.startswith("/*", i):
            end = text.find("*/", i + 2)
            end = n if end < 0 else end + 2
            for k, part in enumerate(text[i:end].split("\n")):
                out.append((line + k, part))
            line += text.count("\n", i, end)
            i = end
        elif rust and text.startswith(("r\"", "r#", "br\"", "br#"), i) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            j = i + (2 if text[i] == "b" else 1)
            hashes = 0
            while j < n and text[j] == "#":
                hashes += 1
                j += 1
            if j < n and text[j] == '"':
                close = '"' + "#" * hashes
                end = text.find(close, j + 1)
                end = n if end < 0 else end + len(close)
                line += text.count("\n", i, end)
                i = end
            else:
                i += 1
        elif ch == '"' or (ch == "'" and not rust) or (ch == "`" and template):
            j = i + 1
            while j < n and text[j] != ch:
                if text[j] == "\\":
                    j += 1
                elif text[j] == "\n" and ch != "`":
                    break
                j += 1
            line += text.count("\n", i, min(j + 1, n))
            i = j + 1
        elif ch == "'" and rust:
            # a character ('a', '\n') or a lifetime ('a)
            if i + 1 < n and text[i + 1] == "\\":
                end = text.find("'", i + 2)
                i = n if end < 0 else end + 1
            elif i + 2 < n and text[i + 2] == "'":
                i += 3
            else:
                i += 1
        else:
            i += 1
    return out


def hash_comments(text, powershell=False):
    """The comments of #-commented code: each # outside quotes, and PowerShell's <# #> blocks."""
    out = []
    in_block = False
    for number, raw in enumerate(text.split("\n"), 1):
        if powershell:
            if in_block:
                out.append((number, raw))
                if "#>" in raw:
                    in_block = False
                continue
            if raw.lstrip().startswith("<#"):
                out.append((number, raw))
                in_block = "#>" not in raw
                continue
        quote = None
        for k, ch in enumerate(raw):
            if quote:
                if ch == quote:
                    quote = None
            elif ch in "\"'":
                quote = ch
            elif ch == "#":
                out.append((number, raw[k:]))
                break
    return out


def python_comments(text):
    """# comments and the strings that open a module, a class or a function (their docstrings)."""
    out = hash_comments(text)
    for m in re.finditer(r'(?m)^[ \t]*[rRbB]?("""|\'\'\')(.*?)\1', text, re.S):
        start = text.count("\n", 0, m.start()) + 1
        for k, part in enumerate(m.group(0).split("\n")):
            out.append((start + k, part))
    return sorted(set(out))


def batch_comments(text):
    return [(n, l) for n, l in enumerate(text.split("\n"), 1) if re.match(r"\s*@?(?:rem\b|::)", l, re.IGNORECASE)]


def markup(text):
    """HTML's <!-- --> comments, and in <script> and <style> their own."""
    out = []
    for m in re.finditer(r"<!--.*?-->", text, re.S):
        start = text.count("\n", 0, m.start()) + 1
        out += [(start + k, part) for k, part in enumerate(m.group(0).split("\n"))]
    for tag in ("script", "style"):
        for m in re.finditer(rf"(<{tag}\b[^>]*>)(.*?)</{tag}>", text, re.S | re.IGNORECASE):
            offset = text.count("\n", 0, m.start(2))
            out += [(offset + n, l) for n, l in c_like(m.group(2), template=(tag == "script"))]
    return sorted(set(out))


def patch_comments(text):
    """A patch's own description (before its first diff) and the comment lines it adds."""
    out = []
    head = text.split("\ndiff --git", 1)[0]
    out += [(n, l) for n, l in enumerate(head.split("\n"), 1) if l.strip()]
    base = head.count("\n") + 1
    for n, l in enumerate(text.split("\n")[base:], base + 1):
        if l.startswith("+") and not l.startswith("+++") and re.match(r"\+\s*(//|/\*|\*|#|<!--|--)", l):
            out.append((n, l))
    return out


def comments_of(path, text):
    ext = path.suffix.lower()
    if ext == ".patch":
        return patch_comments(text)
    if ext == ".rs":
        return c_like(text, rust=True)
    if ext in (".js", ".ts", ".mjs"):
        return c_like(text, template=True)
    if ext in C_LIKE:
        return c_like(text)
    if ext in (".svelte", ".html", ".htm", ".vue"):
        return markup(text)
    if ext == ".py":
        return python_comments(text)
    if ext in (".ps1", ".psm1"):
        return hash_comments(text, powershell=True)
    if ext in HASH or path.name in HASH_NAMES:
        return hash_comments(text)
    if ext in BATCH:
        return batch_comments(text)
    return None


def main():
    out_path = pathlib.Path(sys.argv[1])
    files = public_files()
    blocks, total_lines, counted = [], 0, 0
    for rel in sorted(files, key=str.lower):
        path = ROOT / rel
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        found = comments_of(path, text)
        if found is None:
            continue
        lines = text.split("\n")
        if rel.startswith("vendor/") and rel != "vendor/README.md":
            # another project's file: only the lines of ours, and the comments next to them
            ours = {n for n, l in enumerate(lines, 1) if OURS.search(l)}
            found = [(n, l) for n, l in found if any(abs(n - o) <= 3 for o in ours)]
        found = [(n, l.rstrip()) for n, l in sorted(set(found)) if l.strip() not in ("", "//", "#", "*", "/*", "*/", "/**", "///", "//!")]
        if not found:
            continue
        counted += 1
        total_lines += len(found)
        block = [f"==== {rel} ===="]
        last = None
        for n, l in found:
            if last is not None and n > last + 1:
                block.append("")
            block.append(f"{n:6}  {l.strip()}")
            last = n
        blocks.append("\n".join(block))
    head = (
        "Every comment in the code that goes public, file by file, each with its line number.\n"
        f"{counted} files, {total_lines} lines of comments. Other projects' files kept whole (vendor/) give only\n"
        "the lines of ours inside them; a patch gives its own description and the comments it adds.\n"
    )
    out_path.write_text(head + "\n" + "\n\n".join(blocks) + "\n", encoding="utf-8")
    print(f"{counted} files, {total_lines} comment lines -> {out_path} ({out_path.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
