"""The sample achievement packs as a download of their own beside the installer, never in it: each
pack in tools/achievements/samples zipped as the app takes a pack (its achievements.json, icons/ and
sounds/ at the zip's top), the ten zips in one with a short read-me. Stored, not compressed, so the
release's scans read them as they are; every entry dated alike, so the zip comes out the same each
time.

    python -I tools/release/samples.py <out.zip>
"""
import sys
import zipfile
from io import BytesIO
from pathlib import Path

SAMPLES = Path(__file__).resolve().parents[2] / "tools" / "achievements" / "samples"

README = """Sample achievement packs for Visual Pinball Infinite

Ten achievement packs for ten well-known tables, made as examples to play and to learn from.
Each zip here is one pack, made for one table file: the wiki's Sample achievement packs page
says which.

To add them: drop the packs' zips onto Visual Pinball Infinite (several at once is fine), or
choose one with Add a pack... under Player hub > Achievements.

https://github.com/VPinInfinite/VisualPinballInfinite/wiki/Sample-achievement-packs
"""

WHEN = (2026, 10, 26, 0, 0, 0)


def entry(z, name, data):
    info = zipfile.ZipInfo(name, WHEN)
    info.compress_type = zipfile.ZIP_STORED
    info.external_attr = 0o644 << 16
    z.writestr(info, data)


def pack_zip(folder):
    out = BytesIO()
    with zipfile.ZipFile(out, "w") as z:
        for f in sorted(p for p in folder.rglob("*") if p.is_file()):
            entry(z, f.relative_to(folder).as_posix(), f.read_bytes())
    return out.getvalue()


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    packs = sorted(p for p in SAMPLES.iterdir() if (p / "achievements.json").is_file())
    if not packs:
        sys.exit(f"no packs in {SAMPLES}")
    out = Path(sys.argv[1])
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, "w") as z:
        entry(z, "Read me.txt", README.replace("\n", "\r\n").encode("utf-8"))
        for p in packs:
            entry(z, f"{p.name}.zip", pack_zip(p))
    print(f"{out}: {len(packs)} packs, {out.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
