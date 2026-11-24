//! A RAR archive locked with a password, opened with it. libarchive reads RAR archives but opens
//! none locked ("RAR encryption support unavailable"), and RARLAB's own code, which does, isn't under
//! a free licence; so the archive is decrypted here as it's read and handed to libarchive unlocked
//! (`Unlocked`): its headers as they'd be had it never been locked — each file's encryption record
//! left out, a locked list of files decrypted — and each locked file's data decrypted as libarchive
//! reads it, libarchive decompressing it as it would any other.
//!
//! From RARLAB's description of the RAR 5 format and of how RAR makes its keys, as 7-Zip's own code
//! (LGPL) makes them: RAR 5, AES-256 in CBC mode, its key PBKDF2-HMAC-SHA256 of the password in UTF-8
//! and the salt, 2^n rounds, the password checked against the value 32 rounds on, a file's checksum
//! made a MAC by the key 16 rounds on where the archive says so; RAR 2.9 to 4, AES-128 in CBC mode, key
//! and IV from RAR's own SHA-1 (`Sha1Rar`) of the password in UTF-16 and the salt, 0x40000 rounds. An
//! archive in parts, whose locked files may run from one part to the next, isn't opened. Ours.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use aes::cipher::consts::U16;
use aes::cipher::{Array, BlockCipherDecrypt, KeyInit};
use hmac::{Hmac, Mac};
use sha2::Sha256;

const SIG5: &[u8] = b"Rar!\x1a\x07\x01\x00";
const SIG4: &[u8] = b"Rar!\x1a\x07\x00";
/// The most rounds RAR 5's key takes, as 2's power (RAR's own limit): more is taken for a broken file.
const MOST_ROUNDS: u8 = 24;
/// The longest header read (RAR 5's own limit, 2 MB).
const LONGEST_HEADER: u64 = 2 << 20;

/// Why an archive wasn't unlocked.
#[derive(Debug, PartialEq, Eq)]
pub enum Fail {
    /// None of the passwords opens it, or none was given.
    Locked,
    /// It can't be opened this way, said in words.
    Cannot(String),
}

impl From<std::io::Error> for Fail {
    fn from(e: std::io::Error) -> Fail {
        Fail::Cannot(e.to_string())
    }
}

/// An AES key of either size.
enum Key {
    A128(Box<aes::Aes128>),
    A256(Box<aes::Aes256>),
}

impl Key {
    /// `data` (whole blocks) decrypted in place in CBC mode, `iv` the block before it.
    fn decrypt_cbc(&self, iv: &[u8; 16], data: &mut [u8]) {
        let locked = data.to_vec();
        let (blocks, _) = Array::<u8, U16>::slice_as_chunks_mut(data);
        match self {
            Key::A128(k) => k.decrypt_blocks(blocks),
            Key::A256(k) => k.decrypt_blocks(blocks),
        }
        for (i, block) in data.as_chunks_mut::<16>().0.iter_mut().enumerate() {
            let before = if i == 0 { &iv[..] } else { &locked[(i - 1) * 16..i * 16] };
            for (b, p) in block.iter_mut().zip(before) {
                *b ^= p;
            }
        }
    }
}

/// A piece of the unlocked archive.
enum Piece {
    /// Bytes made here: a header rewritten, or decrypted.
    Bytes(Vec<u8>),
    /// The archive's own bytes, as they are.
    Plain { at: u64, len: u64 },
    /// The archive's bytes decrypted: `len` of them there (whole blocks), the first `keep` served.
    Locked { at: u64, len: u64, keep: u64, key: Arc<Key>, iv: [u8; 16] },
}

impl Piece {
    fn served(&self) -> u64 {
        match self {
            Piece::Bytes(b) => b.len() as u64,
            Piece::Plain { len, .. } => *len,
            Piece::Locked { keep, .. } => *keep,
        }
    }
}

/// A file's checksum RAR 5 keeps made a MAC by its key (its encryption record's flag 2): checked on
/// what libarchive hands out, which can't check it.
#[derive(Clone)]
pub struct Check {
    /// The file, by its number among the archive's files (its list's order).
    pub file: usize,
    mac_key: [u8; 32],
    crc: u32,
}

impl Check {
    /// Whether `crc`, the CRC32 of the file as it came out, is the one the archive keeps.
    pub fn matches(&self, crc: u32) -> bool {
        mac_crc(&self.mac_key, crc) == self.crc
    }
}

/// The archive unlocked: read through as libarchive reads an archive, from start to end, skipping
/// and seeking.
pub struct Unlocked {
    path: PathBuf,
    file: File,
    pieces: Arc<Vec<Piece>>,
    starts: Arc<Vec<u64>>,
    len: u64,
    pos: u64,
    /// The checksums to check on the files' data, which libarchive can't.
    pub checks: Arc<Vec<Check>>,
    /// The first locked file, by its number among the files, where the password couldn't be checked
    /// before its data is read (RAR 2.9 to 4 keep no value to check it against).
    pub unchecked: Option<usize>,
}

impl Unlocked {
    fn new(path: &Path, pieces: Vec<Piece>, checks: Vec<Check>, unchecked: Option<usize>) -> std::io::Result<Unlocked> {
        let pieces: Vec<Piece> = pieces.into_iter().filter(|p| p.served() > 0).collect();
        let mut starts = Vec::with_capacity(pieces.len());
        let mut len = 0;
        for p in &pieces {
            starts.push(len);
            len += p.served();
        }
        Ok(Unlocked { path: path.to_path_buf(), file: File::open(path)?, pieces: Arc::new(pieces), starts: Arc::new(starts), len, pos: 0, checks: Arc::new(checks), unchecked })
    }

    /// The same, read from its start again.
    pub fn again(&self) -> std::io::Result<Unlocked> {
        Ok(Unlocked {
            path: self.path.clone(),
            file: File::open(&self.path)?,
            pieces: self.pieces.clone(),
            starts: self.starts.clone(),
            len: self.len,
            pos: 0,
            checks: self.checks.clone(),
            unchecked: self.unchecked,
        })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn position(&self) -> u64 {
        self.pos
    }

    pub fn seek_to(&mut self, pos: u64) {
        self.pos = pos.min(self.len);
    }

    /// The next bytes into `buf`, as many as fit and the piece they're in holds; 0 at its end.
    pub fn read_into(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let i = match self.starts.binary_search(&self.pos) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let off = self.pos - self.starts[i];
        let n = match &self.pieces[i] {
            Piece::Bytes(b) => {
                let n = buf.len().min(b.len() - off as usize);
                buf[..n].copy_from_slice(&b[off as usize..off as usize + n]);
                n
            }
            Piece::Plain { at, len } => {
                let n = (buf.len() as u64).min(len - off) as usize;
                self.file.seek(SeekFrom::Start(at + off))?;
                self.file.read_exact(&mut buf[..n])?;
                n
            }
            Piece::Locked { at, len, keep, key, iv } => {
                let (block, skip) = (off / 16, (off % 16) as usize);
                let n = (buf.len() as u64).min(keep - off) as usize;
                let wanted = (skip + n).div_ceil(16) as u64 * 16;
                let read = wanted.min(len - block * 16) as usize;
                let mut data = vec![0u8; read];
                let mut before = *iv;
                if block > 0 {
                    self.file.seek(SeekFrom::Start(at + (block - 1) * 16))?;
                    self.file.read_exact(&mut before)?;
                } else {
                    self.file.seek(SeekFrom::Start(*at))?;
                }
                self.file.read_exact(&mut data)?;
                key.decrypt_cbc(&before, &mut data);
                let n = n.min(read - skip);
                buf[..n].copy_from_slice(&data[skip..skip + n]);
                n
            }
        };
        self.pos += n as u64;
        Ok(n)
    }
}

/// What of a RAR archive is locked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    None,
    /// Its files' data, its list of files readable.
    Files,
    /// Its list of files too.
    Everything,
}

/// What of `path`, a RAR archive, is locked: its headers read, its files' data passed over.
pub fn lock_of(path: &Path) -> Result<Lock, Fail> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    let mut sig = [0u8; 8];
    let got = f.read(&mut sig)?;
    if got >= 8 && sig == *SIG5 {
        let mut pos = 8;
        while pos + 7 <= size {
            let (header, next) = plain_header5(&mut f, pos, size)?;
            let h = Header5::parse(&header)?;
            match h.kind {
                4 => return Ok(Lock::Everything),
                2 | 3 if h.encryption.is_some() => return Ok(Lock::Files),
                5 => break,
                _ => {}
            }
            pos = next + h.data_size;
        }
        return Ok(Lock::None);
    }
    if got >= 7 && sig[..7] == *SIG4 {
        let mut pos = 7;
        while pos + 7 <= size {
            let header = read_at(&mut f, pos, 7)?;
            let (kind, flags, len) = (header[2], u16::from_le_bytes([header[3], header[4]]), u16::from_le_bytes([header[5], header[6]]) as u64);
            if len < 7 {
                break;
            }
            if kind == 0x73 && flags & 0x0080 != 0 {
                return Ok(Lock::Everything);
            }
            let header = read_at(&mut f, pos, len)?;
            if matches!(kind, 0x74 | 0x7a) && flags & 0x0004 != 0 {
                return Ok(Lock::Files);
            }
            if kind == 0x7b {
                break;
            }
            pos += len + added4(&header);
        }
        return Ok(Lock::None);
    }
    Err(Fail::Cannot(format!("{}: not a RAR archive", path.display())))
}

/// `path` unlocked with the first of `passwords` that opens each locked part: RAR 5's files each
/// by the value it keeps to check a password against, so files locked with different passwords
/// open where every password is given; RAR 2.9 to 4's locked list by its headers' checksums, their
/// locked files' data by the first (`Unlocked::unchecked`, to check by reading it).
pub fn unlock(path: &Path, passwords: &[String]) -> Result<Unlocked, Fail> {
    if passwords.is_empty() {
        return Err(Fail::Locked);
    }
    let mut f = File::open(path)?;
    let mut sig = [0u8; 8];
    let got = f.read(&mut sig)?;
    if got >= 8 && sig == *SIG5 {
        return unlock5(path, &mut f, passwords);
    }
    if got >= 7 && sig[..7] == *SIG4 {
        return unlock4(path, &mut f, passwords);
    }
    Err(Fail::Cannot(format!("{}: not a RAR archive", path.display())))
}

fn read_at(f: &mut File, at: u64, len: u64) -> std::io::Result<Vec<u8>> {
    let mut b = vec![0u8; len as usize];
    f.seek(SeekFrom::Start(at))?;
    f.read_exact(&mut b)?;
    Ok(b)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(bytes);
    h.finalize()
}

// ---- RAR 5 ----

/// A RAR 5 variable-length number at `*i`, moving past it.
fn vint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..70).step_by(7) {
        let c = *b.get(*i)?;
        *i += 1;
        v |= u64::from(c & 0x7f).checked_shl(shift)?;
        if c & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

fn put_vint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let c = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(c);
            return;
        }
        out.push(c | 0x80);
    }
}

/// A RAR 5 header not locked: its bytes from its CRC to its end, and where its data begins.
fn plain_header5(f: &mut File, pos: u64, size: u64) -> Result<(Vec<u8>, u64), Fail> {
    let head = read_at(f, pos, 16.min(size - pos))?;
    let mut i = 4;
    let len = vint(&head, &mut i).ok_or_else(|| Fail::Cannot("a header broken".into()))?;
    if len > LONGEST_HEADER {
        return Err(Fail::Cannot("a header too long".into()));
    }
    let total = i as u64 + len;
    if pos + total > size {
        return Err(Fail::Cannot("a header cut short".into()));
    }
    Ok((read_at(f, pos, total)?, pos + total))
}

/// A RAR 5 header, its parts found.
struct Header5 {
    kind: u64,
    flags: u64,
    data_size: u64,
    /// Where its own fields begin, after its data size.
    fields: usize,
    /// Where its extra area begins (its end where it has none).
    extra: usize,
    /// Its encryption record, in its extra area: where it is, and its bytes after its type.
    encryption: Option<(std::ops::Range<usize>, Vec<u8>)>,
}

impl Header5 {
    /// `h`, a header from its CRC on.
    fn parse(h: &[u8]) -> Result<Header5, Fail> {
        let broken = || Fail::Cannot("a header broken".into());
        let mut i = 4;
        let len = vint(h, &mut i).ok_or_else(broken)? as usize;
        let end = i + len;
        if end != h.len() {
            return Err(broken());
        }
        let kind = vint(h, &mut i).ok_or_else(broken)?;
        let flags = vint(h, &mut i).ok_or_else(broken)?;
        let extra_size = if flags & 1 != 0 { vint(h, &mut i).ok_or_else(broken)? as usize } else { 0 };
        let data_size = if flags & 2 != 0 { vint(h, &mut i).ok_or_else(broken)? } else { 0 };
        let fields = i;
        let extra = end.checked_sub(extra_size).filter(|e| *e >= fields).ok_or_else(broken)?;
        // a file's or a service's extra record 1 is its encryption (the archive's own, its locator)
        let mut encryption = None;
        let mut j = extra;
        while j < end {
            let at = j;
            let size = vint(h, &mut j).ok_or_else(broken)? as usize;
            let record_end = j.checked_add(size).filter(|e| *e <= end).ok_or_else(broken)?;
            let mut k = j;
            let record = vint(h, &mut k).ok_or_else(broken)?;
            if record == 1 && matches!(kind, 2 | 3) {
                encryption = Some((at..record_end, h[k..record_end].to_vec()));
            }
            j = record_end;
        }
        Ok(Header5 { kind, flags, data_size, fields, extra, encryption })
    }
}

/// RAR 5's keys from a password and a salt: the AES key, the key its checksums are made MACs with,
/// and the value a password is checked against.
#[derive(Clone)]
struct Keys5 {
    key: Arc<Key>,
    mac: [u8; 32],
    check: [u8; 8],
}

fn keys5(password: &str, salt: &[u8], rounds_log2: u8) -> Keys5 {
    let prf = <Hmac<Sha256> as KeyInit>::new_from_slice(password.as_bytes()).expect("HMAC takes any key");
    let mut first = prf.clone();
    first.update(salt);
    first.update(&[0, 0, 0, 1]);
    let mut u: [u8; 32] = first.finalize().into_bytes().into();
    let mut f = u;
    let count = 1u64 << rounds_log2;
    let mut out = [[0u8; 32]; 3];
    let mut done = 1u64;
    for (slot, until) in [(0, count), (1, count + 16), (2, count + 32)] {
        while done < until {
            let mut next = prf.clone();
            next.update(&u);
            u = next.finalize().into_bytes().into();
            for (a, b) in f.iter_mut().zip(u) {
                *a ^= b;
            }
            done += 1;
        }
        out[slot] = f;
    }
    let mut check = [0u8; 8];
    for (i, b) in out[2].iter().enumerate() {
        check[i % 8] ^= b;
    }
    Keys5 { key: Arc::new(Key::A256(Box::new(aes::Aes256::new_from_slice(&out[0]).expect("a 32-byte key")))), mac: out[1], check }
}

/// RAR 5's CRC32 made a MAC with `mac_key`, as it keeps a locked file's (its encryption record's
/// flag 2).
fn mac_crc(mac_key: &[u8; 32], crc: u32) -> u32 {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(mac_key).expect("HMAC takes any key");
    mac.update(&crc.to_le_bytes());
    let digest = mac.finalize().into_bytes();
    digest.iter().enumerate().fold(0u32, |v, (i, b)| v ^ (u32::from(*b) << ((i & 3) * 8)))
}

/// An encryption record's or header's parts: its rounds, salt, IV (a record's) and check value.
struct Lock5 {
    flags: u64,
    rounds: u8,
    salt: [u8; 16],
    iv: [u8; 16],
    check: Option<[u8; 8]>,
}

/// `b`, an encryption record's bytes after its type, or an archive encryption header's fields.
fn lock5(b: &[u8], with_iv: bool) -> Result<Lock5, Fail> {
    let broken = || Fail::Cannot("its lock broken".into());
    let mut i = 0;
    let version = vint(b, &mut i).ok_or_else(broken)?;
    if version != 0 {
        return Err(Fail::Cannot("a lock newer than RAR 5's".into()));
    }
    let flags = vint(b, &mut i).ok_or_else(broken)?;
    let rounds = *b.get(i).ok_or_else(broken)?;
    i += 1;
    if rounds > MOST_ROUNDS {
        return Err(broken());
    }
    let take = |i: &mut usize, n: usize| -> Result<Vec<u8>, Fail> {
        let s = b.get(*i..*i + n).ok_or_else(broken)?.to_vec();
        *i += n;
        Ok(s)
    };
    let salt: [u8; 16] = take(&mut i, 16)?.try_into().map_err(|_| broken())?;
    let iv: [u8; 16] = if with_iv { take(&mut i, 16)?.try_into().map_err(|_| broken())? } else { [0; 16] };
    let check = if flags & 1 != 0 { Some(take(&mut i, 8)?.try_into().map_err(|_| broken())?) } else { None };
    Ok(Lock5 { flags, rounds, salt, iv, check })
}

/// The most keys kept while the program runs, made slowly on purpose (RAR's rounds): beyond, they're
/// made again.
const KEPT_KEYS: usize = 4096;

/// RAR 5's keys made so far, by password, salt and rounds: an archive's files share one salt, and it's
/// read more than once (listed, a file read out, its files added).
type Keys5Kept = HashMap<(String, [u8; 16], u8), Keys5>;
static KEYS5: Mutex<Option<Keys5Kept>> = Mutex::new(None);

fn keys5_kept(password: &str, salt: &[u8; 16], rounds: u8) -> Keys5 {
    let key = (password.to_string(), *salt, rounds);
    if let Some(k) = KEYS5.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)) {
        return k.clone();
    }
    let made = keys5(password, salt, rounds);
    let mut kept = KEYS5.lock().unwrap_or_else(|e| e.into_inner());
    let map = kept.get_or_insert_with(HashMap::new);
    if map.len() >= KEPT_KEYS {
        map.clear();
    }
    map.insert(key, made.clone());
    made
}

/// The keys of the first password opening a lock: by its check value where it keeps one, else the
/// first password's.
fn opening5(passwords: &[String], lock: &Lock5) -> Option<Keys5> {
    for p in passwords {
        let keys = keys5_kept(p, &lock.salt, lock.rounds);
        match lock.check {
            Some(check) if check != keys.check => continue,
            _ => return Some(keys),
        }
    }
    None
}

/// A file or service header made again without its encryption record (and, where its checksums are
/// MACs, without them): its type, flags, data size and own fields as given, then its other extra
/// records; its CRC worked out anew.
fn rewrite5(h: &[u8], parsed: &Header5, data_size: u64, drop_checksums: bool) -> Result<Vec<u8>, Fail> {
    let broken = || Fail::Cannot("a header broken".into());
    let end = h.len();
    // its own fields: file flags, unpacked size, attributes, [mtime], [CRC32], then the rest
    let mut i = parsed.fields;
    let file_flags = vint(h, &mut i).ok_or_else(broken)?;
    let after_flags = i;
    let _unpacked = vint(h, &mut i).ok_or_else(broken)?;
    let _attributes = vint(h, &mut i).ok_or_else(broken)?;
    if file_flags & 2 != 0 {
        i += 4;
    }
    let crc_at = (file_flags & 4 != 0).then_some(i);
    let mut fields = Vec::new();
    let new_flags = if drop_checksums { file_flags & !4 } else { file_flags };
    put_vint(&mut fields, new_flags);
    match crc_at {
        Some(at) if drop_checksums => {
            fields.extend_from_slice(h.get(after_flags..at).ok_or_else(broken)?);
            fields.extend_from_slice(h.get(at + 4..parsed.extra).ok_or_else(broken)?);
        }
        _ => fields.extend_from_slice(h.get(after_flags..parsed.extra).ok_or_else(broken)?),
    }
    // the extra records kept
    let mut extra = Vec::new();
    let mut j = parsed.extra;
    while j < end {
        let at = j;
        let size = vint(h, &mut j).ok_or_else(broken)? as usize;
        let record_end = j + size;
        let mut k = j;
        let kind = vint(h, &mut k).ok_or_else(broken)?;
        if kind != 1 && !(drop_checksums && kind == 2) {
            extra.extend_from_slice(&h[at..record_end]);
        }
        j = record_end;
    }
    let mut flags = parsed.flags & !(1 | 2);
    if !extra.is_empty() {
        flags |= 1;
    }
    if parsed.flags & 2 != 0 {
        flags |= 2;
    }
    let mut body = Vec::new();
    put_vint(&mut body, parsed.kind);
    put_vint(&mut body, flags);
    if !extra.is_empty() {
        put_vint(&mut body, extra.len() as u64);
    }
    if flags & 2 != 0 {
        put_vint(&mut body, data_size);
    }
    body.extend_from_slice(&fields);
    body.extend_from_slice(&extra);
    let mut sized = Vec::new();
    put_vint(&mut sized, body.len() as u64);
    sized.extend_from_slice(&body);
    let mut out = crc32(&sized).to_le_bytes().to_vec();
    out.extend_from_slice(&sized);
    Ok(out)
}

/// A file or service header's unpacked size, CRC32 and whether it's stored rather than compressed.
fn file_facts5(h: &[u8], fields: usize) -> Option<(Option<u64>, Option<u32>, bool)> {
    let mut i = fields;
    let flags = vint(h, &mut i)?;
    let unpacked = vint(h, &mut i)?;
    let _attributes = vint(h, &mut i)?;
    if flags & 2 != 0 {
        i += 4;
    }
    let crc = if flags & 4 != 0 {
        let c = u32::from_le_bytes(h.get(i..i + 4)?.try_into().ok()?);
        i += 4;
        Some(c)
    } else {
        None
    };
    let compression = vint(h, &mut i)?;
    Some(((flags & 8 == 0).then_some(unpacked), crc, (compression >> 7) & 7 == 0))
}

/// `n` bytes at `off` of locked data at `at` (`len` bytes, whole blocks), decrypted: the blocks
/// holding them alone, the one before them their IV.
fn decrypt_at(f: &mut File, at: u64, len: u64, key: &Key, iv: &[u8; 16], off: u64, n: u64) -> std::io::Result<Vec<u8>> {
    let (first, last) = (off / 16, (off + n).div_ceil(16).min(len / 16));
    let mut before = *iv;
    if first > 0 {
        before = read_at(f, at + (first - 1) * 16, 16)?.try_into().unwrap_or(before);
    }
    let mut data = read_at(f, at + first * 16, (last - first) * 16)?;
    key.decrypt_cbc(&before, &mut data);
    let from = (off - first * 16) as usize;
    Ok(data[from..(from + n as usize).min(data.len())].to_vec())
}

/// How long a locked file's compressed data is, without the padding its encryption adds — which
/// libarchive would go on reading as another block: its blocks' headers walked (a block's flags, last
/// block among them, its checksum and its size in 1 to 3 bytes, as libarchive reads them), each
/// checked, so a wrong password's data fails here.
fn compressed_len5(f: &mut File, at: u64, len: u64, key: &Key, iv: &[u8; 16]) -> Result<u64, Fail> {
    let mut off = 0u64;
    while off + 3 <= len {
        let head = decrypt_at(f, at, len, key, iv, off, 5.min(len - off))?;
        let flags = head[0];
        let count = usize::from((flags >> 3) & 7);
        if count > 2 || head.len() < 3 + count {
            return Err(Fail::Locked);
        }
        let size = head[2..3 + count].iter().rev().fold(0u64, |v, b| (v << 8) | u64::from(*b));
        let check = 0x5A ^ flags ^ size as u8 ^ (size >> 8) as u8 ^ (size >> 16) as u8;
        if check != head[1] {
            return Err(Fail::Locked);
        }
        off += 3 + count as u64 + size;
        if flags & 0x40 != 0 {
            return if off <= len { Ok(off) } else { Err(Fail::Locked) };
        }
    }
    Err(Fail::Locked)
}

fn unlock5(path: &Path, f: &mut File, passwords: &[String]) -> Result<Unlocked, Fail> {
    let size = f.metadata()?.len();
    let mut pieces = vec![Piece::Bytes(SIG5.to_vec())];
    let mut checks = Vec::new();
    let mut headers_key: Option<Arc<Key>> = None;
    let mut files = 0usize;
    let mut pos = 8u64;
    while pos + 7 <= size {
        let (header, next) = match &headers_key {
            None => plain_header5(f, pos, size)?,
            Some(key) => {
                // each header after the archive's lock: its IV, then itself in whole blocks
                if pos + 32 > size {
                    break;
                }
                let iv: [u8; 16] = read_at(f, pos, 16)?.try_into().map_err(|_| Fail::Locked)?;
                let mut first = read_at(f, pos + 16, 16)?;
                key.decrypt_cbc(&iv, &mut first);
                let mut i = 4;
                let len = vint(&first, &mut i).ok_or(Fail::Locked)?;
                if len > LONGEST_HEADER {
                    return Err(Fail::Locked);
                }
                let total = i as u64 + len;
                let padded = total.div_ceil(16) * 16;
                if pos + 16 + padded > size {
                    return Err(Fail::Locked);
                }
                let mut whole = read_at(f, pos + 16, padded)?;
                key.decrypt_cbc(&iv, &mut whole);
                whole.truncate(total as usize);
                // a wrong password's header fails its checksum
                if crc32(&whole[4..]) != u32::from_le_bytes([whole[0], whole[1], whole[2], whole[3]]) {
                    return Err(Fail::Locked);
                }
                (whole, pos + 16 + padded)
            }
        };
        let h = Header5::parse(&header)?;
        if next + h.data_size > size && h.kind != 5 {
            return Err(Fail::Cannot("cut short".into()));
        }
        match h.kind {
            4 => {
                // the archive's lock: every header after it locked
                let lock = lock5(&header[h.fields..h.extra], false)?;
                let keys = opening5(passwords, &lock).ok_or(Fail::Locked)?;
                headers_key = Some(keys.key);
            }
            2 | 3 => {
                if h.kind == 2 {
                    files += 1;
                }
                match &h.encryption {
                    Some((_, record)) => {
                        let lock = lock5(record, true)?;
                        let keys = opening5(passwords, &lock).ok_or(Fail::Locked)?;
                        if h.flags & (0x08 | 0x10) != 0 {
                            return Err(Fail::Cannot("a locked file runs on into another part of the archive, which ours can't open: unpack it first with the program it was made for".into()));
                        }
                        let (unpacked, crc, stored) = file_facts5(&header, h.fields).ok_or_else(|| Fail::Cannot("a header broken".into()))?;
                        if !h.data_size.is_multiple_of(16) {
                            return Err(Fail::Cannot("a locked file's data broken".into()));
                        }
                        // its data without the padding: a stored file's own size, a compressed
                        // file's blocks' length
                        let keep = match (stored, unpacked) {
                            _ if h.data_size == 0 => 0,
                            (true, Some(u)) if u <= h.data_size => u,
                            (true, _) => h.data_size,
                            (false, _) => compressed_len5(f, next, h.data_size, &keys.key, &lock.iv)?,
                        };
                        let tweaked = lock.flags & 2 != 0;
                        if tweaked && let Some(crc) = crc.filter(|_| h.kind == 2) {
                            checks.push(Check { file: files - 1, mac_key: keys.mac, crc });
                        }
                        pieces.push(Piece::Bytes(rewrite5(&header, &h, keep, tweaked)?));
                        if h.data_size > 0 {
                            pieces.push(Piece::Locked { at: next, len: h.data_size, keep, key: keys.key, iv: lock.iv });
                        }
                    }
                    None => {
                        pieces.push(Piece::Bytes(header.clone()));
                        pieces.push(Piece::Plain { at: next, len: h.data_size });
                    }
                }
            }
            5 => {
                pieces.push(Piece::Bytes(header.clone()));
                break;
            }
            _ => {
                pieces.push(Piece::Bytes(header.clone()));
                pieces.push(Piece::Plain { at: next, len: h.data_size });
            }
        }
        pos = next + h.data_size;
    }
    Ok(Unlocked::new(path, pieces, checks, None)?)
}

// ---- RAR 2.9 to 4 ----

/// SHA-1 as RAR 2.9 to 4 make their keys with it: the standard's, but for a block of 64 bytes taken
/// straight from what it's handed after the first, which RAR's writes back over those bytes as its
/// working words (`update_rar29`) — which changes the key only for passwords of 28 characters or more.
#[derive(Clone)]
struct Sha1Rar {
    state: [u32; 5],
    buf: [u8; 64],
    count: u64,
}

impl Sha1Rar {
    fn new() -> Sha1Rar {
        Sha1Rar { state: [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0], buf: [0; 64], count: 0 }
    }

    /// One block into `state`: the working words after the last round handed back.
    fn transform(state: &mut [u32; 5], block: &[u8]) -> [u32; 16] {
        let mut w = [0u32; 16];
        for (t, word) in w.iter_mut().enumerate() {
            *word = u32::from_be_bytes([block[t * 4], block[t * 4 + 1], block[t * 4 + 2], block[t * 4 + 3]]);
        }
        let [mut a, mut b, mut c, mut d, mut e] = *state;
        for t in 0..80 {
            let wt = if t < 16 {
                w[t]
            } else {
                let x = (w[(t + 13) & 15] ^ w[(t + 8) & 15] ^ w[(t + 2) & 15] ^ w[t & 15]).rotate_left(1);
                w[t & 15] = x;
                x
            };
            let (f, k) = match t {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(wt);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        for (s, v) in state.iter_mut().zip([a, b, c, d, e]) {
            *s = s.wrapping_add(v);
        }
        w
    }

    fn update(&mut self, data: &[u8]) {
        let mut j = (self.count & 63) as usize;
        self.count += data.len() as u64;
        let mut i = 0;
        if j + data.len() > 63 {
            i = 64 - j;
            self.buf[j..].copy_from_slice(&data[..i]);
            Self::transform(&mut self.state, &self.buf);
            while i + 63 < data.len() {
                Self::transform(&mut self.state, &data[i..i + 64]);
                i += 64;
            }
            j = 0;
        }
        self.buf[j..j + data.len() - i].copy_from_slice(&data[i..]);
    }

    fn update_rar29(&mut self, data: &mut [u8]) {
        let mut j = (self.count & 63) as usize;
        self.count += data.len() as u64;
        let mut i = 0;
        if j + data.len() > 63 {
            i = 64 - j;
            self.buf[j..].copy_from_slice(&data[..i]);
            Self::transform(&mut self.state, &self.buf);
            while i + 63 < data.len() {
                let w = Self::transform(&mut self.state, &data[i..i + 64]);
                for (k, word) in w.iter().enumerate() {
                    data[i + k * 4..i + k * 4 + 4].copy_from_slice(&word.to_le_bytes());
                }
                i += 64;
            }
            j = 0;
        }
        self.buf[j..j + data.len() - i].copy_from_slice(&data[i..]);
    }

    fn finish(mut self) -> [u32; 5] {
        let bits = self.count.wrapping_mul(8);
        self.update(&[0x80]);
        while self.count & 63 != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        self.state
    }
}

/// RAR 2.9 to 4's AES-128 key and IV from a password and a salt.
fn keys3(password: &str, salt: &[u8; 8]) -> ([u8; 16], [u8; 16]) {
    const ROUNDS: u32 = 0x40000;
    let mut raw: Vec<u8> = password.encode_utf16().take(127).flat_map(u16::to_le_bytes).collect();
    raw.extend_from_slice(salt);
    let mut sha = Sha1Rar::new();
    let mut iv = [0u8; 16];
    for i in 0..ROUNDS {
        sha.update_rar29(&mut raw);
        sha.update(&[i as u8, (i >> 8) as u8, (i >> 16) as u8]);
        if i % (ROUNDS / 16) == 0 {
            iv[(i / (ROUNDS / 16)) as usize] = sha.clone().finish()[4] as u8;
        }
    }
    let digest = sha.finish();
    let mut key = [0u8; 16];
    for (i, word) in digest.iter().take(4).enumerate() {
        key[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    (key, iv)
}

/// RAR 2.9 to 4's keys and IVs made so far, by password and salt.
type Keys3 = HashMap<(String, [u8; 8]), (Arc<Key>, [u8; 16])>;
static KEYS3: Mutex<Option<Keys3>> = Mutex::new(None);

/// The keys of a password and a salt, made once while the program runs.
fn keys3_kept(password: &str, salt: [u8; 8]) -> (Arc<Key>, [u8; 16]) {
    let key = (password.to_string(), salt);
    if let Some(k) = KEYS3.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)) {
        return k.clone();
    }
    let (aes_key, iv) = keys3(password, &salt);
    let made = (Arc::new(Key::A128(Box::new(aes::Aes128::new_from_slice(&aes_key).expect("a 16-byte key")))), iv);
    let mut kept = KEYS3.lock().unwrap_or_else(|e| e.into_inner());
    let map = kept.get_or_insert_with(HashMap::new);
    if map.len() >= KEPT_KEYS {
        map.clear();
    }
    map.insert(key, made.clone());
    made
}

/// A RAR 2.9 to 4 block's added size: a file's packed size (its high half too, for a large one), or
/// the size another block says follows it.
fn added4(h: &[u8]) -> u64 {
    let (kind, flags) = (h[2], u16::from_le_bytes([h[3], h[4]]));
    if h.len() < 11 || (flags & 0x8000 == 0 && !matches!(kind, 0x74 | 0x7a)) {
        return 0;
    }
    let low = u64::from(u32::from_le_bytes([h[7], h[8], h[9], h[10]]));
    let high = if matches!(kind, 0x74 | 0x7a) && flags & 0x0100 != 0 && h.len() >= 36 { u64::from(u32::from_le_bytes([h[32], h[33], h[34], h[35]])) } else { 0 };
    low | (high << 32)
}

fn crc16_of(h: &[u8]) -> u16 {
    (crc32(&h[2..]) & 0xffff) as u16
}

fn with_crc4(mut h: Vec<u8>) -> Vec<u8> {
    let crc = crc16_of(&h);
    h[..2].copy_from_slice(&crc.to_le_bytes());
    h
}

fn unlock4(path: &Path, f: &mut File, passwords: &[String]) -> Result<Unlocked, Fail> {
    let size = f.metadata()?.len();
    let broken = || Fail::Cannot("a header broken".into());
    let mut pieces = vec![Piece::Bytes(SIG4.to_vec())];
    let mut pos = 7u64;
    // the archive's header: whether its list of files is locked
    let head = read_at(f, pos, 7)?;
    let main_len = u64::from(u16::from_le_bytes([head[5], head[6]]));
    if head[2] != 0x73 || main_len < 13 {
        return Err(broken());
    }
    let mut main = read_at(f, pos, main_len)?;
    let main_flags = u16::from_le_bytes([main[3], main[4]]);
    let headers_locked = main_flags & 0x0080 != 0;
    if main_flags & 0x0200 != 0 && main.len() > 13 {
        // its encryption version, a byte after its reserved fields
        main.remove(13);
    }
    let new_flags = main_flags & !(0x0080 | 0x0200);
    main[3..5].copy_from_slice(&new_flags.to_le_bytes());
    let new_len = main.len() as u16;
    main[5..7].copy_from_slice(&new_len.to_le_bytes());
    pieces.push(Piece::Bytes(with_crc4(main)));
    pos += main_len;
    // the password that opens a locked list of files: the first whose header passes its checksum
    let mut headers_password: Option<usize> = None;
    let mut files = 0usize;
    let mut unchecked = None;
    while pos + 7 <= size {
        let (header, next) = if headers_locked {
            if pos + 8 + 16 > size {
                break;
            }
            let salt: [u8; 8] = read_at(f, pos, 8)?.try_into().map_err(|_| broken())?;
            let first = read_at(f, pos + 8, 16)?;
            let candidates: Vec<usize> = match headers_password {
                Some(n) => vec![n],
                None => (0..passwords.len()).collect(),
            };
            let mut found = None;
            for n in candidates {
                let (key, iv) = keys3_kept(&passwords[n], salt);
                let mut block = first.clone();
                key.decrypt_cbc(&iv, &mut block);
                let len = u64::from(u16::from_le_bytes([block[5], block[6]]));
                if !(7..=LONGEST_HEADER).contains(&len) {
                    continue;
                }
                let padded = len.div_ceil(16) * 16;
                if pos + 8 + padded > size {
                    continue;
                }
                let mut whole = read_at(f, pos + 8, padded)?;
                key.decrypt_cbc(&iv, &mut whole);
                whole.truncate(len as usize);
                if crc16_of(&whole) == u16::from_le_bytes([whole[0], whole[1]]) {
                    found = Some((n, whole, pos + 8 + padded));
                    break;
                }
            }
            let (n, whole, next) = found.ok_or(Fail::Locked)?;
            headers_password = Some(n);
            (whole, next)
        } else {
            let head = read_at(f, pos, 7)?;
            let len = u64::from(u16::from_le_bytes([head[5], head[6]]));
            if len < 7 || pos + len > size {
                break;
            }
            (read_at(f, pos, len)?, pos + len)
        };
        let (kind, flags) = (header[2], u16::from_le_bytes([header[3], header[4]]));
        let added = added4(&header);
        if next + added > size && kind != 0x7b {
            return Err(Fail::Cannot("cut short".into()));
        }
        match kind {
            0x74 | 0x7a => {
                if kind == 0x74 {
                    files += 1;
                }
                if header.len() < 32 {
                    return Err(broken());
                }
                let large = flags & 0x0100 != 0;
                let name_at = if large { 40 } else { 32 };
                let name_len = usize::from(u16::from_le_bytes([header[26], header[27]]));
                let name_end = name_at + name_len;
                if flags & 0x0004 == 0 {
                    pieces.push(Piece::Bytes(header.clone()));
                    pieces.push(Piece::Plain { at: next, len: added });
                } else {
                    if flags & 0x0003 != 0 {
                        return Err(Fail::Cannot("a locked file runs on into another part of the archive, which ours can't open: unpack it first with the program it was made for".into()));
                    }
                    if flags & 0x0400 == 0 || header.get(24).is_some_and(|v| *v < 29) {
                        return Err(Fail::Cannot("it's locked the way RAR 2.0 locked archives, which ours can't open: unpack it first with the program it was made for".into()));
                    }
                    let salt: [u8; 8] = header.get(name_end..name_end + 8).ok_or_else(broken)?.try_into().map_err(|_| broken())?;
                    // RAR 2.9 to 4 keep nothing to check a file's password by: a locked list's, or the
                    // first password, the first locked file read to check it
                    let n = headers_password.unwrap_or(0);
                    if headers_password.is_none() && unchecked.is_none() && kind == 0x74 {
                        unchecked = Some(files - 1);
                    }
                    let (key, iv) = keys3_kept(&passwords[n], salt);
                    let unpacked = u64::from(u32::from_le_bytes([header[11], header[12], header[13], header[14]]))
                        | if large && header.len() >= 40 { u64::from(u32::from_le_bytes([header[36], header[37], header[38], header[39]])) << 32 } else { 0 };
                    let stored = header[25] == 0x30;
                    let keep = if stored && unpacked <= added { unpacked } else { added };
                    let mut h = Vec::with_capacity(header.len() - 8);
                    h.extend_from_slice(&header[..name_end]);
                    h.extend_from_slice(&header[name_end + 8..]);
                    let flags = flags & !(0x0004 | 0x0400);
                    h[3..5].copy_from_slice(&flags.to_le_bytes());
                    let len = h.len() as u16;
                    h[5..7].copy_from_slice(&len.to_le_bytes());
                    h[7..11].copy_from_slice(&(keep as u32).to_le_bytes());
                    if large {
                        h[32..36].copy_from_slice(&((keep >> 32) as u32).to_le_bytes());
                    }
                    pieces.push(Piece::Bytes(with_crc4(h)));
                    if added > 0 {
                        if !added.is_multiple_of(16) {
                            return Err(Fail::Cannot("a locked file's data broken".into()));
                        }
                        pieces.push(Piece::Locked { at: next, len: added, keep, key, iv });
                    }
                }
            }
            0x7b => {
                pieces.push(Piece::Bytes(header.clone()));
                break;
            }
            _ => {
                pieces.push(Piece::Bytes(header.clone()));
                pieces.push(Piece::Plain { at: next, len: added });
            }
        }
        pos = next + added;
    }
    Ok(Unlocked::new(path, pieces, Vec::new(), unchecked)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rars_sha1_is_the_standard_for_what_it_hands_over_whole() {
        let mut s = Sha1Rar::new();
        s.update(b"abc");
        let d = s.finish();
        assert_eq!(d, [0xa999_3e36, 0x4706_816a, 0xba3e_2571, 0x7850_c26c, 0x9cd0_d89d]);
        // a block taken straight from what it's handed is written back over, the hash the same
        let mut long = vec![b'a'; 200];
        let mut s = Sha1Rar::new();
        s.update_rar29(&mut long);
        let mut plain = Sha1Rar::new();
        plain.update(&[b'a'; 200]);
        assert_eq!(s.finish(), plain.finish());
        assert_ne!(long, vec![b'a'; 200]);
    }

    /// `VPI_UNLOCK_IN=<a locked RAR> VPI_UNLOCK_PASSWORD=<its password> VPI_UNLOCK_OUT=<a file>
    /// cargo test -p library --lib an_archive_unlocked_written_out -- --ignored`: the archive as
    /// libarchive is handed it, written out, for another program to test.
    #[test]
    #[ignore]
    fn an_archive_unlocked_written_out() {
        let var = |v: &str| std::env::var(v).unwrap_or_else(|_| panic!("{v}"));
        let mut u = unlock(Path::new(&var("VPI_UNLOCK_IN")), &[var("VPI_UNLOCK_PASSWORD")]).unwrap();
        let mut out = Vec::new();
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let n = u.read_into(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        std::fs::write(var("VPI_UNLOCK_OUT"), out).unwrap();
    }

    #[test]
    fn numbers_written_as_rar_5_writes_them() {
        for v in [0u64, 1, 127, 128, 300, 1 << 35] {
            let mut b = Vec::new();
            put_vint(&mut b, v);
            let mut i = 0;
            assert_eq!(vint(&b, &mut i), Some(v));
            assert_eq!(i, b.len());
        }
    }
}
