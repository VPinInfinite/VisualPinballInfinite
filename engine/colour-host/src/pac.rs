// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! PIN2DMD's locked colour files (`pin2dmd.pac`), opened with the user's own
//! key as DMD Extensions opens them (GPL-2.0, freezy's
//! `LibDmd/Converter/Vni/VniLoader.cs`, read for the format): `PAC ` and a
//! version byte, then a chunk holding the palette file and, when the
//! colouring has animations, one holding the animation file — each a type (1
//! the `.pal`, 2 the `.vni`, 16 bits) and a length (32 bits), big endian,
//! then that many bytes: the file gzipped, then encrypted with AES-128 in CBC
//! mode, the key its IV too, zero padded. libvni reads only the open files,
//! so the two come out of the `.pac` for it. The key is the user's: never
//! shipped nor made here, and it's not written anywhere.

use std::ffi::c_void;
use std::io::Read;

/// The two files a `.pac` holds, as their own files would hold them.
pub struct Unpacked {
    pub pal: Vec<u8>,
    pub vni: Option<Vec<u8>>,
}

/// A key as DMD Extensions takes it (`vni.key`): 32 hex digits.
pub fn parse_key(hex: &str) -> Result<[u8; 16], String> {
    let hex = hex.trim();
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("the PAC key must be 32 hex digits".into());
    }
    let mut key = [0u8; 16];
    for (i, k) in key.iter_mut().enumerate() {
        *k = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(key)
}

pub fn unpack(file: &[u8], key: &[u8; 16]) -> Result<Unpacked, String> {
    if file.len() < 5 || &file[..4] != b"PAC " {
        return Err("not a PAC file".into());
    }
    let mut at = 5;
    let (mut pal, mut vni) = (None, None);
    while at < file.len() {
        if file.len() - at < 6 {
            return Err("a PAC chunk is cut short".into());
        }
        let kind = u16::from_be_bytes([file[at], file[at + 1]]);
        let len = u32::from_be_bytes([file[at + 2], file[at + 3], file[at + 4], file[at + 5]]) as usize;
        at += 6;
        let data = file.get(at..at + len).ok_or("a PAC chunk runs past the file's end")?;
        at += len;
        let mut plain = Vec::new();
        flate2::read::GzDecoder::new(&decrypt(data, key)?[..]).read_to_end(&mut plain).map_err(|_| "the key doesn't open this PAC file".to_string())?;
        match kind {
            1 => pal = Some(plain),
            2 => vni = Some(plain),
            other => return Err(format!("a PAC chunk of unknown kind {other}")),
        }
    }
    Ok(Unpacked { pal: pal.ok_or("the PAC file holds no palette")?, vni })
}

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptOpenAlgorithmProvider(algorithm: *mut *mut c_void, id: *const u16, implementation: *const u16, flags: u32) -> i32;
    fn BCryptSetProperty(object: *mut c_void, property: *const u16, input: *const u8, input_len: u32, flags: u32) -> i32;
    fn BCryptGenerateSymmetricKey(algorithm: *mut c_void, key: *mut *mut c_void, object: *mut u8, object_len: u32, secret: *const u8, secret_len: u32, flags: u32) -> i32;
    #[allow(clippy::too_many_arguments)]
    fn BCryptDecrypt(key: *mut c_void, input: *const u8, input_len: u32, padding: *const c_void, iv: *mut u8, iv_len: u32, output: *mut u8, output_len: u32, result: *mut u32, flags: u32) -> i32;
    fn BCryptDestroyKey(key: *mut c_void) -> i32;
    fn BCryptCloseAlgorithmProvider(algorithm: *mut c_void, flags: u32) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// AES-128 in CBC mode, the key its IV too, no padding taken off (DMD
/// Extensions' zero padding leaves the gzip stream's end as it is), through
/// Windows' own cryptography (CNG).
fn decrypt(data: &[u8], key: &[u8; 16]) -> Result<Vec<u8>, String> {
    if data.len() % 16 != 0 {
        return Err("a PAC chunk isn't whole AES blocks".into());
    }
    let mut out = vec![0u8; data.len()];
    // SAFETY: CNG's calls with buffers of the lengths given; the key and the algorithm are
    // released on every path
    unsafe {
        let mut algorithm = std::ptr::null_mut();
        if BCryptOpenAlgorithmProvider(&mut algorithm, wide("AES").as_ptr(), std::ptr::null(), 0) != 0 {
            return Err("Windows' AES is unavailable".into());
        }
        let mode = wide("ChainingModeCBC");
        let mut handle = std::ptr::null_mut();
        let mut written = 0u32;
        let mut iv = *key;
        let ok = BCryptSetProperty(algorithm, wide("ChainingMode").as_ptr(), mode.as_ptr().cast(), (mode.len() * 2) as u32, 0) == 0
            && BCryptGenerateSymmetricKey(algorithm, &mut handle, std::ptr::null_mut(), 0, key.as_ptr(), 16, 0) == 0
            && BCryptDecrypt(handle, data.as_ptr(), data.len() as u32, std::ptr::null(), iv.as_mut_ptr(), 16, out.as_mut_ptr(), out.len() as u32, &mut written, 0) == 0;
        if !handle.is_null() {
            BCryptDestroyKey(handle);
        }
        BCryptCloseAlgorithmProvider(algorithm, 0);
        if !ok {
            return Err("AES decryption failed".into());
        }
        out.truncate(written as usize);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_32_hex_digits() {
        assert_eq!(parse_key("000102030405060708090a0b0c0d0e0F").unwrap()[15], 15);
        assert!(parse_key("0001").is_err());
        assert!(parse_key("zz0102030405060708090a0b0c0d0e0f").is_err());
    }

    #[test]
    fn a_file_that_isnt_a_pac_is_refused() {
        assert!(unpack(b"PAL 1", &[0; 16]).is_err());
        // the magic, a version, then nothing: no palette
        assert!(unpack(b"PAC \x01", &[0; 16]).is_err());
    }

    #[test]
    fn decryption_runs_through_windows_aes() {
        // AES-128-CBC of one zero block under a zero key and IV is the well-known 66e94bd4…;
        // decrypting it gives the zero block back
        let cipher = [0x66, 0xe9, 0x4b, 0xd4, 0xef, 0x8a, 0x2c, 0x3b, 0x88, 0x4c, 0xfa, 0x59, 0xca, 0x34, 0x2b, 0x2e];
        assert_eq!(decrypt(&cipher, &[0; 16]).unwrap(), vec![0u8; 16]);
    }
}
