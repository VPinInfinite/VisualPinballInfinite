"""The player's fonts for its pause menu, the app's own: Inter and Barlow
Condensed, both SIL Open Font License 1.1 with no reserved font names, taken from the WOFF files of
the app's font packages (`app/ui/node_modules/@fontsource/...`, their Latin subsets) and written as
the TrueType files they hold, which SDL_ttf reads. WOFF 1.0 is the font's own tables, each
compressed with zlib or stored: nothing in the font changes but its wrapping.

    python -I tools/menu-fonts/woff_to_ttf.py <file.woff> <file.ttf>
"""

import struct
import sys
import zlib


def checksum(data: bytes) -> int:
    data += b"\0" * (-len(data) % 4)
    return sum(struct.unpack(f">{len(data) // 4}I", data)) & 0xFFFFFFFF


def woff_to_sfnt(woff: bytes) -> bytes:
    signature, flavor, _length, count, _reserved, _total = struct.unpack(">4sIIHHI", woff[:20])
    if signature != b"wOFF":
        raise ValueError("not a WOFF 1.0 file")
    tables = []
    for k in range(count):
        tag, offset, comp_length, orig_length, _orig_checksum = struct.unpack(">4sIIII", woff[44 + 20 * k : 64 + 20 * k])
        data = woff[offset : offset + comp_length]
        if comp_length < orig_length:
            data = zlib.decompress(data)
        if len(data) != orig_length:
            raise ValueError(f"table {tag!r}: {len(data)} bytes, {orig_length} expected")
        tables.append((tag, data))
    tables.sort(key=lambda t: t[0])
    # the offset table's search fields, as the TrueType format defines them
    power = 1
    while power * 2 <= count:
        power *= 2
    entry_selector = power.bit_length() - 1
    search_range = power * 16
    head = struct.pack(">IHHHH", flavor, count, search_range, entry_selector, count * 16 - search_range)
    offset = 12 + 16 * count
    records, body = b"", b""
    head_at = None
    for tag, data in tables:
        if tag == b"head":
            # the whole font's checksum is counted with checkSumAdjustment zero
            data = data[:8] + b"\0\0\0\0" + data[12:]
            head_at = offset + len(body)
        records += struct.pack(">4sIII", tag, checksum(data), offset + len(body), len(data))
        body += data + b"\0" * (-len(data) % 4)
    font = bytearray(head + records + body)
    if head_at is not None:
        struct.pack_into(">I", font, head_at + 8, (0xB1B0AFBA - checksum(bytes(font))) & 0xFFFFFFFF)
    return bytes(font)


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    with open(sys.argv[1], "rb") as f:
        woff = f.read()
    with open(sys.argv[2], "wb") as f:
        f.write(woff_to_sfnt(woff))


if __name__ == "__main__":
    main()
