//! Compressed sound files — MP3, Ogg Vorbis, FLAC, and WAV in any of its
//! encodings — decoded with Symphonia (MPL-2.0). VPX build 5436 decodes the
//! same formats with miniaudio: a table's sounds stored in them, and
//! the music files `PlayMusic` plays.

use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

/// Decoded sound: interleaved samples, -1 to 1.
pub struct Pcm {
    pub rate: u32,
    pub channels: usize,
    pub samples: Vec<f32>,
}

/// Decodes a whole file held in memory; `ext` (`"mp3"`, `"ogg"`) helps
/// find the format, which is otherwise told by the data itself.
pub fn decode_all(bytes: Vec<u8>, ext: Option<&str>) -> Result<Pcm, String> {
    let mut pcm = Pcm { rate: 0, channels: 0, samples: Vec::new() };
    stream(Box::new(std::io::Cursor::new(bytes)), ext, |rate, channels, chunk| {
        (pcm.rate, pcm.channels) = (rate, channels);
        pcm.samples.extend_from_slice(chunk);
        true
    })?;
    if pcm.rate == 0 || pcm.channels == 0 {
        return Err("no sound in it".into());
    }
    Ok(pcm)
}

/// Decodes a file packet by packet, handing each packet's interleaved
/// samples to `sink` with the rate and channel count; stops early when
/// `sink` returns false. A damaged packet is skipped, as players do.
pub fn stream(source: Box<dyn MediaSource>, ext: Option<&str>, mut sink: impl FnMut(u32, usize, &[f32]) -> bool) -> Result<(), String> {
    let mut hint = Hint::new();
    if let Some(ext) = ext {
        hint.with_extension(ext);
    }
    let mss = MediaSourceStream::new(source, MediaSourceStreamOptions::default());
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| format!("not a sound file we can read: {e}"))?;
    let track = format.default_track(TrackType::Audio).ok_or("no sound in it")?;
    let track_id = track.id;
    let Some(CodecParameters::Audio(params)) = track.codec_params.clone() else { return Err("no sound in it".into()) };
    let mut decoder =
        symphonia::default::get_codecs().make_audio_decoder(&params, &AudioDecoderOptions::default()).map_err(|e| format!("can't decode it: {e}"))?;
    let mut samples: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(()),
            // some formats end with an unexpected end of the data
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => return Err(format!("reading it: {e}")),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buffer) => {
                let (rate, channels) = (buffer.spec().rate(), buffer.spec().channels().count());
                buffer.copy_to_vec_interleaved(&mut samples);
                if channels > 0 && !samples.is_empty() && !sink(rate, channels, &samples) {
                    return Ok(());
                }
            }
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decoding it: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A short WAV file in memory: 16-bit stereo, a rising ramp.
    fn wav(rate: u32, frames: u32) -> Vec<u8> {
        let data_len = frames * 4;
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&2u16.to_le_bytes()); // stereo
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 4).to_le_bytes());
        w.extend_from_slice(&4u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            let v = (i as i16).wrapping_mul(8);
            w.extend_from_slice(&v.to_le_bytes());
            w.extend_from_slice(&(-v).to_le_bytes());
        }
        w
    }

    #[test]
    fn decodes_a_wav_file() {
        let pcm = decode_all(wav(22_050, 1000), Some("wav")).expect("decodes");
        assert_eq!((pcm.rate, pcm.channels, pcm.samples.len()), (22_050, 2, 2000));
        assert_eq!(pcm.samples[2], 8.0 / 32768.0);
        assert_eq!(pcm.samples[3], -8.0 / 32768.0);
    }

    #[test]
    fn refuses_what_is_not_sound() {
        assert!(decode_all(b"not a sound file at all".to_vec(), None).is_err());
    }
}
