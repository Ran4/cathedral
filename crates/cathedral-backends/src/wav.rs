//! WAV bytes: what we accept, how long they are, and where they may live.
//!
//! Two independent readers, on purpose:
//!
//! * [`validate_wav_bytes`] is the *sanity gate* for synthesized audio
//!   (`server.py:1927-1948`) — hound refuses anything the game's audio sink
//!   would choke on;
//! * [`wav_duration_seconds`] is a hand-rolled RIFF walk
//!   (`server.py:181-215`), because player recordings are **32-bit float PCM
//!   (format tag 3)** and the strict decoders reject them. The duration only
//!   feeds the latency probe, so a malformed file is `None`, never an error.
//!
//! [`safe_session_path`] is the path confinement of `_safe_basename`
//! (`server.py:152-160`): a WAV name from the game may only ever name a file
//! *directly inside* the session's private runtime directory.
//!
//! [`accept_wav_bytes`] exists because the sanity gate cannot simply *be*
//! hound: OpenAI streams `/v1/audio/speech`, so it writes the 0xFFFFFFFF
//! placeholder into both the RIFF size and the `data` chunk size. Python's
//! `wave` module shrugged (it reported 2 147 483 647 frames and moved on);
//! hound — and therefore rodio, and therefore the game's audio sink — refuses
//! the file outright. The bytes are repaired once, here, and it is the repaired
//! bytes that travel on.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use cathedral_sim::SpeechError;

/// `server.py:1930` — a synthesized utterance larger than this is a bug.
pub const MAX_WAV_BYTES: usize = 16 * 1024 * 1024;
/// One streamed PCM chunk, decoded (`speech-python.md` §3.2 / ARCHITECTURE §1.2).
pub const MAX_PCM_CHUNK_BYTES: usize = 256 * 1024;
/// `protocol.py:11` — the id/basename length ceiling.
pub const MAX_BASENAME_CHARS: usize = 128;

/// Why a WAV (or the path to one) was refused. The text is presentable: it
/// reaches the HUD as a `tts`/`stt` degraded message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavError(pub String);

impl WavError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for WavError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for WavError {}

impl From<WavError> for SpeechError {
    fn from(error: WavError) -> Self {
        SpeechError::new(error.0)
    }
}

/// What a valid WAV turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavInfo {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub frames: u32,
}

/// The synthesis sanity check of `_poll_tts` (`server.py:1927-1948`), byte-exact
/// in its bounds: ≤16 MiB, 1–2 channels, 1–4 byte samples, 8–192 kHz, ≥1 frame.
///
/// A streamed provider header is repaired first ([`accept_wav_bytes`]) — this
/// answers the same yes/no Python's `wave`-based gate did.
pub fn validate_wav_bytes(bytes: &[u8]) -> Result<WavInfo, WavError> {
    accept_wav_bytes(bytes).map(|(_, info)| info)
}

/// The gate, plus the bytes that passed it.
///
/// The returned bytes are the ones to hand on: for a well-formed WAV they are
/// the input, borrowed; for a **streamed** one (OpenAI's `/v1/audio/speech`
/// answers with 0xFFFFFFFF in the RIFF and `data` sizes, because it starts
/// writing before it knows the length) they are an owned copy whose two size
/// fields say what actually arrived. Without that repair every cloud utterance
/// dies in `hound` — first here, and then again in rodio, which is what the
/// game plays WAVs with.
pub fn accept_wav_bytes(bytes: &[u8]) -> Result<(Cow<'_, [u8]>, WavInfo), WavError> {
    if bytes.len() > MAX_WAV_BYTES {
        return Err(WavError::new("generated WAV exceeds 16 MiB"));
    }
    let candidate: Cow<'_, [u8]> = match repair_streamed_sizes(bytes) {
        Some(repaired) => Cow::Owned(repaired),
        None => Cow::Borrowed(bytes),
    };

    let reader = hound::WavReader::new(std::io::Cursor::new(candidate.as_ref()))
        .map_err(|_| WavError::new("generated WAV is invalid"))?;
    let specification = reader.spec();
    let frames = reader.duration();
    let sample_bytes = specification.bits_per_sample.div_ceil(8);
    if !(1..=2).contains(&specification.channels)
        || !(1..=4).contains(&sample_bytes)
        || !(8_000..=192_000).contains(&specification.sample_rate)
        || frames < 1
    {
        return Err(WavError::new("generated WAV has unsupported parameters"));
    }
    Ok((
        candidate,
        WavInfo {
            channels: specification.channels,
            sample_rate: specification.sample_rate,
            bits_per_sample: specification.bits_per_sample,
            frames,
        },
    ))
}

/// Rewrite a `data` chunk that claims more bytes than the file holds — the
/// streaming placeholder, and any truncated download — down to the bytes that
/// are really there, and fix the RIFF size to match. `None` when the header is
/// already honest, which is every WAV a file-writing encoder produces.
fn repair_streamed_sizes(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut offset = 12usize;
    // Bytes per frame across all channels; the repaired `data` length must be a
    // whole number of frames or hound refuses it for a different reason.
    let mut block_align = 1usize;
    loop {
        let header = bytes.get(offset..offset + 8)?;
        let chunk_id = &header[..4];
        let declared = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
        let body = offset + 8;

        if chunk_id == b"fmt " && declared >= 16 {
            let fmt = bytes.get(body..body + 16)?;
            let align = u16::from_le_bytes(fmt[12..14].try_into().ok()?) as usize;
            block_align = align.max(1);
        } else if chunk_id == b"data" {
            let available = bytes.len().checked_sub(body)?;
            if declared <= available {
                return None; // The header tells the truth.
            }
            let repaired_len = available - (available % block_align);
            let mut repaired = bytes.to_vec();
            let data_size = u32::try_from(repaired_len).ok()?;
            repaired[body - 4..body].copy_from_slice(&data_size.to_le_bytes());
            // Everything after "RIFF" and its own 4-byte length field.
            let riff_size = u32::try_from(body + repaired_len - 8).ok()?;
            repaired[4..8].copy_from_slice(&riff_size.to_le_bytes());
            repaired.truncate(body + repaired_len);
            return Some(repaired);
        }
        // RIFF chunks are word-aligned: an odd size carries one pad byte.
        offset = body.checked_add(declared + (declared & 1))?;
    }
}

/// Duration by header math alone (`_wav_duration_seconds`, `server.py:181-215`).
///
/// Walks the RIFF chunks by hand: the player's microphone writes float32 PCM
/// (format tag 3), which `hound` and Python's `wave` both reject, and the
/// latency probe still wants to know how long the utterance was. Any
/// malformation answers `None` — this must never be the thing that breaks an
/// utterance.
pub fn wav_duration_seconds(bytes: &[u8]) -> Option<f64> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut offset = 12usize;
    let mut byte_rate: u64 = 0;
    loop {
        let header = bytes.get(offset..offset + 8)?;
        let chunk_id = &header[..4];
        let chunk_size = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
        // RIFF chunks are word-aligned: an odd size carries one pad byte.
        let padded = chunk_size + (chunk_size & 1);
        let body = offset + 8;

        if chunk_id == b"fmt " && chunk_size >= 16 {
            let fmt = bytes.get(body..body + 16)?;
            let sample_rate = u32::from_le_bytes(fmt[4..8].try_into().ok()?) as u64;
            let declared_rate = u32::from_le_bytes(fmt[8..12].try_into().ok()?) as u64;
            let block_align = u16::from_le_bytes(fmt[12..14].try_into().ok()?) as u64;
            // `block_align` is bytes per frame across all channels.
            byte_rate = if declared_rate > 0 {
                declared_rate
            } else {
                sample_rate * block_align
            };
        } else if chunk_id == b"data" {
            if byte_rate == 0 {
                return None;
            }
            return Some(chunk_size as f64 / byte_rate as f64);
        }
        offset = body.checked_add(padded)?;
    }
}

/// Confine a WAV basename to the session's runtime directory
/// (`_safe_basename`, `server.py:152-160`).
///
/// Rejects the empty name, control characters, anything over 128 chars, `.`,
/// `..`, any path separator, and any name that does not end in `.wav`. The
/// result is `dir/<name>` and nothing else — no `..` can climb out of it.
pub fn safe_session_path(directory: &Path, basename: &str) -> Result<PathBuf, WavError> {
    let invalid_path = || WavError::new("WAV path must be a basename inside the runtime directory");

    if basename.is_empty() || basename.chars().count() > MAX_BASENAME_CHARS {
        return Err(WavError::new(
            "wav_basename must be a non-empty string of at most 128 characters",
        ));
    }
    if basename.chars().any(|character| (character as u32) < 0x20) {
        return Err(WavError::new("wav_basename contains control characters"));
    }
    if basename == "." || basename == ".." {
        return Err(invalid_path());
    }
    if basename.contains('/') || basename.contains('\\') || basename.contains('\0') {
        return Err(invalid_path());
    }
    if Path::new(basename)
        .file_name()
        .and_then(|name| name.to_str())
        != Some(basename)
    {
        return Err(invalid_path());
    }
    if !basename.to_lowercase().ends_with(".wav") {
        return Err(WavError::new("audio basename must end in .wav"));
    }
    Ok(directory.join(basename))
}

/// The `.wav` suffix check both cloud and local transcription do *before* they
/// touch the network or the worker (`speech_client.py:166-169, 735-738`).
pub fn require_existing_wav(path: &Path) -> Result<(), WavError> {
    let is_wav = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"));
    if !is_wav || !path.is_file() {
        return Err(WavError::new(
            "transcription input must be an existing WAV file",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn wav(channels: u16, sample_rate: u32, bits: u16, frames: usize) -> Vec<u8> {
        let specification = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: bits,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut buffer, specification).expect("header");
            for _ in 0..frames * channels as usize {
                writer.write_sample(0i16).expect("sample");
            }
            writer.finalize().expect("finalize");
        }
        buffer.into_inner()
    }

    /// A WAV exactly as OpenAI's streaming `/v1/audio/speech` sends it: the RIFF
    /// size and the `data` size are both the 0xFFFFFFFF placeholder, because the
    /// provider starts writing before it knows how long the answer is.
    fn streamed_wav(frames: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&24_000u32.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes()); // byte rate
        bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
        bytes.extend_from_slice(&16u16.to_le_bytes()); // bits
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0u8, frames * 2));
        bytes
    }

    /// A float32 recording exactly as the game's microphone writes it: format
    /// tag 3, which no strict decoder will open.
    fn float32_recording(sample_rate: u32, frames: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        let data_bytes = (frames * 4) as u32;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 4).to_le_bytes()); // byte rate
        bytes.extend_from_slice(&4u16.to_le_bytes()); // block align
        bytes.extend_from_slice(&32u16.to_le_bytes()); // bits
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_bytes.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0u8, data_bytes as usize));
        bytes
    }

    #[test]
    fn a_plain_mono_wav_passes_the_synthesis_gate() {
        let info = validate_wav_bytes(&wav(1, 24_000, 16, 480)).expect("valid");
        assert_eq!(
            info,
            WavInfo {
                channels: 1,
                sample_rate: 24_000,
                bits_per_sample: 16,
                frames: 480,
            }
        );
    }

    #[test]
    fn the_synthesis_gate_refuses_what_the_audio_sink_cannot_play() {
        assert_eq!(
            validate_wav_bytes(b"not a wav at all"),
            Err(WavError("generated WAV is invalid".to_string()))
        );
        assert_eq!(
            validate_wav_bytes(&wav(1, 24_000, 16, 0)),
            Err(WavError(
                "generated WAV has unsupported parameters".to_string()
            )),
            "zero frames is silence nobody asked for"
        );
        assert_eq!(
            validate_wav_bytes(&wav(1, 4_000, 16, 10)),
            Err(WavError(
                "generated WAV has unsupported parameters".to_string()
            )),
            "4 kHz is below the 8 kHz floor"
        );
        assert_eq!(
            validate_wav_bytes(&vec![0u8; MAX_WAV_BYTES + 1]),
            Err(WavError("generated WAV exceeds 16 MiB".to_string()))
        );
        // Stereo is allowed; three channels are not.
        assert!(validate_wav_bytes(&wav(2, 24_000, 16, 10)).is_ok());
    }

    /// The whole cloud cast was silent because of this: the provider streams, so
    /// its header lies about its length, and hound (the gate here, and rodio in
    /// the game) refuses the file. Python's `wave` accepted the same bytes.
    #[test]
    fn the_streamed_provider_header_is_repaired_instead_of_refused() {
        let streamed = streamed_wav(480);
        assert!(
            hound::WavReader::new(Cursor::new(streamed.clone())).is_err(),
            "the raw provider bytes are exactly what hound will not open",
        );

        let (playable, info) = accept_wav_bytes(&streamed).expect("the gate accepts the provider");
        assert_eq!(
            info,
            WavInfo {
                channels: 1,
                sample_rate: 24_000,
                bits_per_sample: 16,
                frames: 480,
            }
        );
        // What leaves the gate must decode where it is going: rodio is hound.
        let reader = hound::WavReader::new(Cursor::new(playable.as_ref())).expect("rodio's reader");
        assert_eq!(reader.duration(), 480);
        assert_eq!(
            u32::from_le_bytes(playable[4..8].try_into().unwrap()) as usize,
            playable.len() - 8,
            "the RIFF size is repaired too, not just the data chunk",
        );

        // An honest header is passed through untouched, byte for byte.
        let honest = wav(1, 24_000, 16, 480);
        let (unchanged, _) = accept_wav_bytes(&honest).expect("valid");
        assert!(matches!(unchanged, std::borrow::Cow::Borrowed(_)));
        assert_eq!(unchanged.as_ref(), honest.as_slice());

        // A header-only stream has no audio in it at all, and is still refused.
        assert_eq!(
            validate_wav_bytes(&streamed_wav(0)),
            Err(WavError(
                "generated WAV has unsupported parameters".to_string()
            ))
        );
    }

    #[test]
    fn the_duration_walk_reads_the_float32_recordings_hound_rejects() {
        let recording = float32_recording(24_000, 12_000);
        assert!(
            hound::WavReader::new(Cursor::new(recording.clone())).is_err()
                || validate_wav_bytes(&recording).is_ok(),
            "the strict readers do not have to like format tag 3",
        );
        let duration = wav_duration_seconds(&recording).expect("a duration");
        assert!((duration - 0.5).abs() < 1e-9, "{duration}");

        // 16-bit ints work too — the same walk, a different tag.
        let duration = wav_duration_seconds(&wav(1, 16_000, 16, 8_000)).expect("a duration");
        assert!((duration - 0.5).abs() < 1e-9, "{duration}");
    }

    #[test]
    fn a_malformed_recording_has_no_duration_and_no_error() {
        assert_eq!(wav_duration_seconds(b""), None);
        assert_eq!(wav_duration_seconds(b"RIFF____NOPE"), None);
        // A truncated chunk table walks off the end rather than panicking.
        assert_eq!(
            wav_duration_seconds(&float32_recording(24_000, 10)[..20]),
            None
        );
        // fmt-less: data with no rate to divide by.
        let mut headerless = Vec::new();
        headerless.extend_from_slice(b"RIFF");
        headerless.extend_from_slice(&4u32.to_le_bytes());
        headerless.extend_from_slice(b"WAVE");
        headerless.extend_from_slice(b"data");
        headerless.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(wav_duration_seconds(&headerless), None);
    }

    #[test]
    fn a_basename_may_only_ever_name_a_file_inside_the_runtime_directory() {
        let directory = Path::new("/tmp/cathedral-smart-actors-1");
        assert_eq!(
            safe_session_path(directory, "speech-3.wav").expect("a session path"),
            PathBuf::from("/tmp/cathedral-smart-actors-1/speech-3.wav")
        );
        // Case is honored on the way in and irrelevant to the suffix check.
        assert!(safe_session_path(directory, "Recording-1.WAV").is_ok());

        for hostile in [
            "",
            ".",
            "..",
            "../../etc/passwd.wav",
            "/etc/passwd.wav",
            "sub/dir.wav",
            "back\\slash.wav",
            "speech-3.mp3",
            "no-suffix",
            "nul\0.wav",
        ] {
            assert!(
                safe_session_path(directory, hostile).is_err(),
                "{hostile:?} must never resolve"
            );
        }
        assert!(
            safe_session_path(directory, "\u{1}.wav").is_err(),
            "control char"
        );
        assert!(
            safe_session_path(directory, &format!("{}.wav", "x".repeat(130))).is_err(),
            "over 128 characters"
        );
    }

    #[test]
    fn transcription_input_must_be_an_existing_wav() {
        let directory = std::env::temp_dir().join(format!(
            "cathedral-wav-{}",
            crate::session_dir::SessionDir::new_session_id()
        ));
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("input.wav");
        assert!(require_existing_wav(&path).is_err(), "missing file");
        std::fs::write(&path, wav(1, 24_000, 16, 10)).expect("write");
        assert!(require_existing_wav(&path).is_ok());
        assert!(
            require_existing_wav(&directory.join("input.mp3")).is_err(),
            "suffix first, existence second"
        );
        std::fs::remove_dir_all(&directory).ok();
    }
}
