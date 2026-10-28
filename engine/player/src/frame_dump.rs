//! A game's score-display frames written down as they come, for making a colouring of it: every
//! new frame the machine shows before any colouring (PinMAME's identify frames, as a colouring's
//! program is handed them) in the frame dump the colouring editors open — a line with the frame's
//! time in milliseconds as `0x` and eight hex digits, then the frame a row a line, one hex digit a
//! dot, its shade, then a blank line (ColorizingDMD 3.17.3 finds the frames' size by the first
//! one's ending there) — into the file `VPI_FRAME_DUMP` names, gzipped where its name ends `.gz`.
//! The times are the game's own clock, so a run in test mode writes the same dump each time. How
//! many frames it holds is written beside it when it's finished (`<dump>.frames`), for the app.
//! Ours.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::pinmame::Machine;

/// The dump being written.
pub struct FrameDump {
    out: Option<Box<dyn Write + Send>>,
    /// The identify frame last written, by its number on PinMAME's link.
    last: Option<u32>,
    shades: Vec<u8>,
    /// Frames written.
    pub frames: u64,
    path: PathBuf,
}

impl FrameDump {
    /// A dump into `path`, made anew; gzipped where its name ends `.gz`.
    pub fn create(path: &Path) -> std::io::Result<FrameDump> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let file = BufWriter::new(File::create(path)?);
        let gz = path.extension().is_some_and(|x| x.eq_ignore_ascii_case("gz"));
        let out: Box<dyn Write + Send> = if gz { Box::new(flate2::write::GzEncoder::new(file, flate2::Compression::default())) } else { Box::new(file) };
        Ok(FrameDump { out: Some(out), last: None, shades: Vec::new(), frames: 0, path: path.to_path_buf() })
    }

    /// The dump `VPI_FRAME_DUMP` asks for, if it does.
    pub fn from_env() -> Option<FrameDump> {
        let path = std::env::var_os("VPI_FRAME_DUMP").filter(|p| !p.is_empty())?;
        match FrameDump::create(Path::new(&path)) {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("frame dump: {}: {e}", Path::new(&path).display());
                None
            }
        }
    }

    /// The machine's newest frame written, when it has a new one, at `now_ms` on the game's clock.
    pub fn poll(&mut self, machine: &Machine, now_ms: u64) {
        let Some(count) = machine.dmd_identify_count() else { return };
        if self.last == Some(count) || self.out.is_none() {
            return;
        }
        let Some((width, height, _bits, number)) = machine.dmd_identify(&mut self.shades) else { return };
        self.last = Some(count);
        let (w, h) = (width.max(0) as usize, height.max(0) as usize);
        if w == 0 || h == 0 || self.shades.len() < w * h {
            return;
        }
        let text = frame_text(&self.shades, w, h, now_ms);
        let failed = self.out.as_mut().is_some_and(|o| o.write_all(text.as_bytes()).is_err());
        if failed {
            eprintln!("frame dump: writing frame {number} failed; no more are written");
            self.out = None;
        } else {
            self.frames += 1;
        }
    }

    /// The dump finished: a gzipped one's end written, the file closed.
    pub fn finish(&mut self) {
        if let Some(mut out) = self.out.take() {
            // a gzip stream ends as its writer is dropped; the file's buffer is flushed after it
            let _ = out.flush();
            drop(out);
            let _ = std::fs::write(format!("{}.frames", self.path.display()), self.frames.to_string());
            println!("frame dump: {} frames", self.frames);
        }
    }
}

impl Drop for FrameDump {
    fn drop(&mut self) {
        self.finish();
    }
}

/// One frame as the dump has it: its time line, then a line a row, a hex digit a dot (its shade,
/// 0 to 15), then a blank line.
pub fn frame_text(shades: &[u8], width: usize, height: usize, now_ms: u64) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut s = String::with_capacity(14 + height * (width + 2));
    s.push_str(&format!("0x{:08X}\r\n", now_ms as u32));
    for row in shades.chunks(width).take(height) {
        s.extend(row.iter().map(|&v| HEX[usize::from(v.min(15))] as char));
        s.push_str("\r\n");
    }
    s.push_str("\r\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_written_as_the_colouring_editor_reads_a_dump() {
        let mut shades = vec![0u8; 8 * 2];
        shades[0] = 3;
        shades[15] = 15;
        assert_eq!(frame_text(&shades, 8, 2, 0x29D8_9DCF), "0x29D89DCF\r\n30000000\r\n0000000F\r\n\r\n");
    }
}
