# Locked archives for the library's tests

Archives locked with a password, for the tests of opening them (`archive.rs`, `rar_unlock.rs`).

- `rar4_*.rar`, `rar5_*.rar` — libarchive's own test archives (`libarchive/test/test_read_format_rar*_encrypted*.rar.uu`, decoded), from libarchive 3.8.11, under libarchive's licence (BSD-2-Clause; see CREDITS.md). Each holds `a.txt`, `b.txt`, `c.txt` and `d.txt`, each saying `This is from <its name>`. In the solid ones and the ones whose list of files is locked, all four are locked with the password `password`; in the others `a.txt` and `c.txt` aren't locked, `b.txt` is locked with `password` and `d.txt` with `password2`.
- `zipcrypto.zip`, `aes.zip`, `data.7z`, `headers.7z` — ours, made with 7-Zip 25.01: `a.txt` and `b.txt`, saying `This is from a.txt` and `This is from b.txt`, locked with `password` (the zips' with ZipCrypto and AES-256; `data.7z` its files alone, `headers.7z` its list of files too).
