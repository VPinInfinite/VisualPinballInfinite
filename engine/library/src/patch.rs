//! A VPU patch applied: VPURemix shares a table mod as a JojoDiff patch (`.dif`) that,
//! applied to the untouched original table, makes the modded one. Ported from JANPatch's reader of
//! JojoDiff's patches (`janpatch.h` at c8ad3ee, Copyright 2017 ARM Ltd., Apache-2.0,
//! `LICENSE-JANPatch.txt`): the patch is a run of operations, each `ESC` and an opcode — bytes that
//! replace the original's (MOD, the default when no `ESC` comes), bytes inserted (INS), the
//! original's bytes kept (EQL), skipped (DEL) or gone back over (BKT), these three with a length —
//! an `ESC` in the data written twice. Read from the original as the patch asks, written out as it
//! goes; a patch carries no check of the table it was made from, so what it makes is read as a table
//! before it's kept (`drop`, `install`). Ours on top: lengths past 32 bits (JojoDiff writes them;
//! JANPatch stops), and a patch that reads past the original's end refused, as it was made from
//! another table.

use std::io::{Read, Seek, SeekFrom, Write};

const ESC: u8 = 0xa7;
const MOD: u8 = 0xa6;
const INS: u8 = 0xa5;
const DEL: u8 = 0xa4;
const EQL: u8 = 0xa3;
const BKT: u8 = 0xa2;

/// The patch, read byte by byte.
struct Ops<'a> {
    data: &'a [u8],
    at: usize,
}

impl Ops<'_> {
    fn next(&mut self) -> Option<u8> {
        let b = self.data.get(self.at).copied();
        if b.is_some() {
            self.at += 1;
        }
        b
    }

    fn back(&mut self, n: usize) {
        self.at = self.at.saturating_sub(n);
    }

    /// An EQL's, DEL's or BKT's length: 1 to 252 in one byte (less one); 253 to 508 in two; else
    /// 2, 4 or 8 bytes after a marker, the high byte first (JojoDiff's `ufPutLen`).
    fn length(&mut self) -> Result<u64, String> {
        let mut bytes = |n: usize| -> Result<u64, String> {
            let mut v = 0u64;
            for _ in 0..n {
                v = (v << 8) | u64::from(self.next().ok_or("the patch ends inside a length")?);
            }
            Ok(v)
        };
        match bytes(1)? {
            l @ 0..=251 => Ok(l + 1),
            252 => Ok(253 + bytes(1)?),
            253 => bytes(2),
            254 => bytes(4),
            _ => bytes(8),
        }
    }
}

/// The original, read where the patch asks.
struct Original<'a, R: Read + Seek> {
    file: &'a mut R,
    len: u64,
    /// Where the patch is in it.
    pos: i64,
    /// Where the file's own position is, to seek only when they differ.
    at: Option<u64>,
}

impl<R: Read + Seek> Original<'_, R> {
    /// `n` of its bytes, from where the patch is, written out.
    fn copy(&mut self, n: u64, out: &mut impl Write) -> Result<(), String> {
        let from = u64::try_from(self.pos).map_err(|_| "the patch goes back past the table's start: it isn't for this table".to_string())?;
        if from.checked_add(n).is_none_or(|end| end > self.len) {
            return Err("the patch reads past the table's end: it isn't for this table".into());
        }
        if self.at != Some(from) {
            self.file.seek(SeekFrom::Start(from)).map_err(|e| e.to_string())?;
        }
        let mut left = n;
        let mut buf = vec![0u8; 1 << 20];
        while left > 0 {
            let take = left.min(buf.len() as u64) as usize;
            self.file.read_exact(&mut buf[..take]).map_err(|e| e.to_string())?;
            out.write_all(&buf[..take]).map_err(|e| e.to_string())?;
            left -= take as u64;
        }
        self.pos += n as i64;
        self.at = Some(from + n);
        Ok(())
    }
}

/// Bytes the patch gives, written out up to its next operation; the original passed over as many
/// for a MOD (`over`), not for an INS. An `ESC` twice is one `ESC`; an `ESC` and a byte that isn't
/// an operation are both data.
fn data<R: Read + Seek>(ops: &mut Ops, original: &mut Original<R>, over: bool, out: &mut impl Write) -> Result<(), String> {
    let mut put = |bytes: &[u8], original: &mut Original<R>| -> Result<(), String> {
        out.write_all(bytes).map_err(|e| e.to_string())?;
        if over {
            original.pos += bytes.len() as i64;
        }
        Ok(())
    };
    loop {
        let Some(m) = ops.next() else { return Ok(()) };
        if m != ESC {
            put(&[m], original)?;
            continue;
        }
        let Some(m) = ops.next() else { return Ok(()) };
        match m {
            ESC => put(&[ESC], original)?,
            BKT..=MOD => {
                ops.back(2);
                return Ok(());
            }
            _ => put(&[ESC, m], original)?,
        }
    }
}

/// `patch` applied to `original` (its whole length `len`), the patched file written to `out`.
pub fn apply<R: Read + Seek>(original: &mut R, len: u64, patch: &[u8], out: &mut impl Write) -> Result<(), String> {
    let mut ops = Ops { data: patch, at: 0 };
    let mut original = Original { file: original, len, pos: 0, at: None };
    while let Some(c) = ops.next() {
        // since JojoDiff 0.8.5, bytes with no operation before them are a MOD's
        let op = if c == ESC {
            match ops.next() {
                Some(op) => op,
                None => break,
            }
        } else {
            ops.back(1);
            MOD
        };
        match op {
            EQL => {
                let n = ops.length()?;
                original.copy(n, out)?;
            }
            MOD => data(&mut ops, &mut original, true, out)?,
            INS => data(&mut ops, &mut original, false, out)?,
            BKT => original.pos -= ops.length()? as i64,
            DEL => original.pos += ops.length()? as i64,
            // an `ESC` that starts no operation: data, from the `ESC` on
            _ => {
                ops.back(2);
                data(&mut ops, &mut original, true, out)?;
            }
        }
    }
    Ok(())
}

/// The table at `original` patched into `out`, through a passing file beside it renamed in once
/// the patched table reads as a table; nothing left behind on failure.
pub fn make(original: &std::path::Path, patch: &[u8], out: &std::path::Path) -> Result<(), String> {
    let mut file = std::fs::File::open(original).map_err(|e| format!("{}: {e}", original.display()))?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    let mut name = out.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".vpi-part");
    let passing = out.with_file_name(name);
    let made = (|| -> Result<(), String> {
        let mut w = std::io::BufWriter::new(std::fs::File::create(&passing).map_err(|e| format!("{}: {e}", passing.display()))?);
        apply(&mut std::io::BufReader::new(&mut file), len, patch, &mut w)?;
        w.flush().map_err(|e| e.to_string())?;
        drop(w);
        // a patch made from another table makes no table at all
        crate::table::read(&passing).map_err(|_| "what the patch made isn't a table VPX could open: the patch isn't for this table (or this version of it)".to_string())?;
        std::fs::rename(&passing, out).map_err(|e| format!("{}: {e}", out.display()))
    })();
    if made.is_err() {
        let _ = std::fs::remove_file(&passing);
    }
    made
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn patched(original: &[u8], patch: &[u8]) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        apply(&mut Cursor::new(original), original.len() as u64, patch, &mut out)?;
        Ok(out)
    }

    #[test]
    fn each_operation() {
        let original = b"abcdefghij";
        // keep 3, replace 2 (bare bytes are a MOD's), insert 1, skip 1, keep the rest
        let patch = [&[ESC, EQL, 2][..], b"XY", &[ESC, INS], b"+", &[ESC, DEL, 0, ESC, EQL, 3]].concat();
        assert_eq!(patched(original, &patch).unwrap(), b"abcXY+ghij");
        // go back over two and keep them again
        let patch = [ESC, EQL, 2, ESC, BKT, 1, ESC, EQL, 1];
        assert_eq!(patched(original, &patch).unwrap(), b"abcbc");
        // an ESC in the data written twice; an ESC and a byte that's no operation both data
        let patch = [ESC, MOD, ESC, ESC, b'q', ESC, 0x41];
        assert_eq!(patched(original, &patch).unwrap(), [ESC, b'q', ESC, 0x41]);
    }

    #[test]
    fn lengths_in_every_size() {
        let original = vec![7u8; 70_000];
        for n in [1u64, 252, 253, 508, 509, 65_535, 70_000] {
            let mut patch = vec![ESC, EQL];
            match n {
                1..=252 => patch.push((n - 1) as u8),
                253..=508 => patch.extend([252, (n - 253) as u8]),
                509..=0xffff => patch.extend([253, (n >> 8) as u8, n as u8]),
                _ => patch.extend([254, (n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8]),
            }
            assert_eq!(patched(&original, &patch).unwrap().len() as u64, n, "{n}");
        }
    }

    /// JojoDiff's own patches, both ways, as JANPatch's demo has them: `VPI_JOJODIFF_DEMO` names the
    /// folder holding `blinky-k64f-old.bin`, `-new.bin`, `.patch` and `-reverse.patch` (not kept in
    /// the repository); without it, nothing to check.
    #[test]
    fn jojodiffs_own_patches() {
        let Some(dir) = std::env::var_os("VPI_JOJODIFF_DEMO").map(std::path::PathBuf::from) else { return };
        let read = |n: &str| std::fs::read(dir.join(n)).unwrap();
        let (old, new) = (read("blinky-k64f-old.bin"), read("blinky-k64f-new.bin"));
        assert!(patched(&old, &read("blinky-k64f.patch")).unwrap() == new);
        assert!(patched(&new, &read("blinky-k64f-reverse.patch")).unwrap() == old);
    }

    #[test]
    fn a_patch_for_another_table_is_refused() {
        assert!(patched(b"short", &[ESC, EQL, 9]).unwrap_err().contains("isn't for this table"));
        assert!(patched(b"short", &[ESC, BKT, 3, ESC, EQL, 0]).unwrap_err().contains("isn't for this table"));
    }
}
