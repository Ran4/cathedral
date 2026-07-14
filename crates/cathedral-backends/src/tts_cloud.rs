//! Cloud synthesis — `POST /v1/audio/speech` (`OpenAISpeechBackend.synthesize`,
//! `speech_client.py:180-205`).
//!
//! OpenAI starts returning a WAV before it knows the final RIFF/data sizes. The
//! previous in-process port collected that entire response, repaired its two
//! placeholder sizes, and only then gave it to Bevy. [`CloudTts`] now consumes
//! the HTTP body as it arrives and decodes the provider's PCM16 WAV into the
//! same ordered mono chunks as local Pocket TTS. Honest non-streaming WAVs and
//! unusual formats retain the complete-WAV compatibility path.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use cathedral_sim::SpeechError;
use futures_util::StreamExt;
use serde::Serialize;

use crate::{
    config::SpeechSettings,
    stt_cloud::{NO_KEY_MESSAGE, is_retryable, transport_error},
    tts::{PcmChunk, StreamCompletion, resolve_openai_voice, validate_tts_text},
    wav::{MAX_PCM_CHUNK_BYTES, MAX_WAV_BYTES, accept_wav_bytes},
    worker::truncate,
};

const MAX_ATTEMPTS: u32 = 2;
const RETRY_BACKOFF: Duration = Duration::from_millis(500);
const MIN_STREAM_SAMPLE_RATE: u32 = 8_000;
const MAX_STREAM_SAMPLE_RATE: u32 = 48_000;
const PCM16_BYTES_PER_SAMPLE: usize = 2;
const MAX_STREAM_CHUNK_SAMPLES: usize = MAX_PCM_CHUNK_BYTES / PCM16_BYTES_PER_SAMPLE;

#[derive(Debug, Serialize)]
struct SpeechRequest<'a> {
    model: &'a str,
    voice: &'a str,
    input: &'a str,
    /// WAV, not MP3: the game decodes it with `hound` and plays raw samples.
    response_format: &'static str,
}

/// A cloud provider normally takes the streaming branch. The buffered variant
/// preserves compatibility with an honest-length WAV or a provider format the
/// incremental PCM16 decoder deliberately does not guess at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudSynthesis {
    Streamed(StreamCompletion),
    Buffered(Arc<[u8]>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StreamFormat {
    data_offset: usize,
    channels: u16,
    sample_rate: u32,
    block_align: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderDecision {
    NeedMore,
    Stream(StreamFormat),
    Buffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodeMode {
    Inspecting,
    Streaming {
        format: StreamFormat,
        emitted_raw_bytes: usize,
    },
    Buffered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DecodedCloudAudio {
    Streamed { chunk_count: u32 },
    Buffered(Arc<[u8]>),
}

/// Incremental decoder for the actual provider shape: PCM16 WAV with unknown
/// RIFF/data lengths (`0xFFFFFFFF`). It keeps the bounded original bytes too so
/// EOF still passes through the existing hound-based sanity gate.
#[derive(Debug)]
struct CloudWavDecoder {
    bytes: Vec<u8>,
    mode: DecodeMode,
    next_seq: u32,
}

impl CloudWavDecoder {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            mode: DecodeMode::Inspecting,
            next_seq: 0,
        }
    }

    fn push(&mut self, incoming: &[u8]) -> Result<Vec<PcmChunk>, SpeechError> {
        let new_len = self
            .bytes
            .len()
            .checked_add(incoming.len())
            .filter(|length| *length <= MAX_WAV_BYTES)
            .ok_or_else(|| SpeechError::new("generated WAV exceeds 16 MiB"))?;
        self.bytes.reserve(new_len - self.bytes.len());
        self.bytes.extend_from_slice(incoming);

        if self.mode == DecodeMode::Inspecting {
            self.mode = match inspect_streaming_header(&self.bytes) {
                HeaderDecision::NeedMore => DecodeMode::Inspecting,
                HeaderDecision::Stream(format) => DecodeMode::Streaming {
                    format,
                    emitted_raw_bytes: 0,
                },
                HeaderDecision::Buffer => DecodeMode::Buffered,
            };
        }

        let DecodeMode::Streaming {
            format,
            emitted_raw_bytes,
        } = self.mode
        else {
            return Ok(Vec::new());
        };

        let available = self.bytes.len().saturating_sub(format.data_offset);
        let complete_raw_bytes = available - available % format.block_align;
        if complete_raw_bytes <= emitted_raw_bytes {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        let mut raw_offset = emitted_raw_bytes;
        while raw_offset < complete_raw_bytes {
            let available_frames = (complete_raw_bytes - raw_offset) / format.block_align;
            let frames = available_frames.min(MAX_STREAM_CHUNK_SAMPLES);
            let raw_len = frames * format.block_align;
            let start = format.data_offset + raw_offset;
            let end = start + raw_len;
            let samples = decode_pcm16_mono(&self.bytes[start..end], format.channels);
            chunks.push(PcmChunk {
                seq: self.next_seq,
                sample_rate: format.sample_rate,
                samples: Arc::from(samples),
            });
            self.next_seq = self.next_seq.saturating_add(1);
            raw_offset += raw_len;
        }

        self.mode = DecodeMode::Streaming {
            format,
            emitted_raw_bytes: complete_raw_bytes,
        };
        Ok(chunks)
    }

    fn finish(self) -> Result<DecodedCloudAudio, SpeechError> {
        let (playable, info) = accept_wav_bytes(&self.bytes)?;
        match self.mode {
            DecodeMode::Streaming {
                format,
                emitted_raw_bytes,
            } => {
                let expected_raw_bytes = (info.frames as usize)
                    .checked_mul(format.block_align)
                    .ok_or_else(|| SpeechError::new("generated WAV is invalid"))?;
                if self.next_seq == 0
                    || info.channels != format.channels
                    || info.sample_rate != format.sample_rate
                    || info.bits_per_sample != 16
                    || emitted_raw_bytes != expected_raw_bytes
                {
                    return Err(SpeechError::new("generated WAV is invalid"));
                }
                Ok(DecodedCloudAudio::Streamed {
                    chunk_count: self.next_seq,
                })
            }
            DecodeMode::Inspecting | DecodeMode::Buffered => {
                Ok(DecodedCloudAudio::Buffered(Arc::from(playable.as_ref())))
            }
        }
    }
}

fn inspect_streaming_header(bytes: &[u8]) -> HeaderDecision {
    if bytes.len() < 12 {
        return HeaderDecision::NeedMore;
    }
    if &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return HeaderDecision::Buffer;
    }

    let mut offset = 12usize;
    let mut stream_format = None;
    loop {
        let Some(header_end) = offset.checked_add(8) else {
            return HeaderDecision::Buffer;
        };
        let Some(header) = bytes.get(offset..header_end) else {
            return HeaderDecision::NeedMore;
        };
        let chunk_id = &header[..4];
        let declared = u32::from_le_bytes(header[4..8].try_into().expect("four bytes")) as usize;
        let body = header_end;

        if chunk_id == b"fmt " {
            if declared < 16 {
                return HeaderDecision::Buffer;
            }
            let Some(format) = bytes.get(body..body + 16) else {
                return HeaderDecision::NeedMore;
            };
            let format_tag = u16::from_le_bytes(format[..2].try_into().expect("two bytes"));
            let channels = u16::from_le_bytes(format[2..4].try_into().expect("two bytes"));
            let sample_rate = u32::from_le_bytes(format[4..8].try_into().expect("four bytes"));
            let block_align =
                u16::from_le_bytes(format[12..14].try_into().expect("two bytes")) as usize;
            let bits_per_sample = u16::from_le_bytes(format[14..16].try_into().expect("two bytes"));
            let expected_align = usize::from(channels) * PCM16_BYTES_PER_SAMPLE;
            if format_tag == 1
                && (1..=2).contains(&channels)
                && (MIN_STREAM_SAMPLE_RATE..=MAX_STREAM_SAMPLE_RATE).contains(&sample_rate)
                && bits_per_sample == 16
                && block_align == expected_align
            {
                stream_format = Some((channels, sample_rate, block_align));
            } else {
                return HeaderDecision::Buffer;
            }
        } else if chunk_id == b"data" {
            let Some((channels, sample_rate, block_align)) = stream_format else {
                return HeaderDecision::Buffer;
            };
            // This placeholder is the provider's promise that bytes following
            // the header are an open-ended PCM stream. Honest finite WAVs use
            // the complete-file compatibility path.
            if declared != u32::MAX as usize {
                return HeaderDecision::Buffer;
            }
            return HeaderDecision::Stream(StreamFormat {
                data_offset: body,
                channels,
                sample_rate,
                block_align,
            });
        }

        let Some(padded) = declared.checked_add(declared & 1) else {
            return HeaderDecision::Buffer;
        };
        let Some(next) = body.checked_add(padded) else {
            return HeaderDecision::Buffer;
        };
        if next > MAX_WAV_BYTES {
            return HeaderDecision::Buffer;
        }
        if bytes.len() < next {
            return HeaderDecision::NeedMore;
        }
        offset = next;
    }
}

fn decode_pcm16_mono(raw: &[u8], channels: u16) -> Vec<i16> {
    match channels {
        1 => raw
            .chunks_exact(2)
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
            .collect(),
        2 => raw
            .chunks_exact(4)
            .map(|frame| {
                let left = i16::from_le_bytes([frame[0], frame[1]]);
                let right = i16::from_le_bytes([frame[2], frame[3]]);
                ((i32::from(left) + i32::from(right)) / 2) as i16
            })
            .collect(),
        _ => unreachable!("the streaming header accepts mono or stereo only"),
    }
}

/// The cloud TTS client.
#[derive(Debug, Clone)]
pub struct CloudTts {
    http: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
    timeout: Duration,
    settings: SpeechSettings,
}

impl CloudTts {
    pub fn new(settings: &SpeechSettings) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: settings.api_key.clone(),
            base_url: settings.base_url.trim_end_matches('/').to_string(),
            model: settings.tts_model.clone(),
            timeout: Duration::from_secs_f64(settings.timeout_seconds.max(0.001)),
            settings: settings.clone(),
        }
    }

    /// `speech_client.py:150-152` — the same key as cloud STT, probed the same
    /// way (not at all).
    pub fn available(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// One utterance, delivering PCM chunks as the provider's body arrives.
    ///
    /// Text and voice are validated **locally first** (`speech_client.py:180-183`):
    /// an empty line, a control character or a hostile voice override must never
    /// become a provider call.
    pub async fn synthesize_stream(
        &self,
        text: &str,
        voice_key: &str,
        mut on_chunk: impl FnMut(PcmChunk),
    ) -> Result<CloudSynthesis, SpeechError> {
        validate_tts_text(text)?;
        let voice = resolve_openai_voice(&self.settings, voice_key)?;
        let Some(api_key) = self.api_key.clone() else {
            return Err(SpeechError::new(NO_KEY_MESSAGE));
        };

        let url = format!("{}/audio/speech", self.base_url);
        let body = SpeechRequest {
            model: &self.model,
            voice: &voice,
            input: text,
            response_format: "wav",
        };

        let started = Instant::now();
        let mut attempt = 0;
        let response = loop {
            attempt += 1;
            let response = self
                .http
                .post(&url)
                .bearer_auth(&api_key)
                .timeout(self.timeout)
                .json(&body)
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(RETRY_BACKOFF).await;
                        continue;
                    }
                    return Err(transport_error("synthesis", &error));
                }
            };

            let status = response.status();
            if !status.is_success() {
                if attempt < MAX_ATTEMPTS && is_retryable(status.as_u16()) {
                    tokio::time::sleep(RETRY_BACKOFF).await;
                    continue;
                }
                return Err(SpeechError::new(truncate(
                    &format!("cloud speech provider failed (HTTP {})", status.as_u16()),
                    160,
                )));
            }

            break response;
        };

        let mut decoder = CloudWavDecoder::new();
        let mut first_chunk_ms = None;
        let mut body = response.bytes_stream();
        while let Some(next) = body.next().await {
            let bytes = next.map_err(|error| transport_error("synthesis", &error))?;
            for chunk in decoder.push(&bytes)? {
                first_chunk_ms.get_or_insert_with(|| {
                    started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32
                });
                on_chunk(chunk);
            }
        }

        match decoder.finish()? {
            DecodedCloudAudio::Streamed { chunk_count } => {
                Ok(CloudSynthesis::Streamed(StreamCompletion {
                    chunk_count,
                    first_chunk_ms: first_chunk_ms
                        .ok_or_else(|| SpeechError::new("generated WAV is invalid"))?,
                }))
            }
            DecodedCloudAudio::Buffered(wav) => Ok(CloudSynthesis::Buffered(wav)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{BackendsConfig, BackendsOptions, Environment},
        runtime::BackendRuntime,
        testing::MockServer,
    };
    use serde_json::Value;
    use std::{collections::BTreeMap, io::Cursor, path::PathBuf};

    fn settings(pairs: &[(&str, &str)]) -> SpeechSettings {
        let vars: BTreeMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        BackendsConfig::resolve(
            &Environment::from_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir: PathBuf::from("/nonexistent"),
                uv_binary: "uv".to_string(),
                fake_mode: false,
            },
        )
        .speech
    }

    /// A real, playable WAV — the mock provider has to answer with one, because
    /// the client refuses anything the audio sink could not play.
    fn wav_body() -> Vec<u8> {
        let specification = hound::WavSpec {
            channels: 1,
            sample_rate: 24_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut buffer, specification).expect("header");
            for _ in 0..240 {
                writer.write_sample(0i16).expect("sample");
            }
            writer.finalize().expect("finalize");
        }
        buffer.into_inner()
    }

    fn synthesize(
        runtime: &BackendRuntime,
        client: &CloudTts,
        text: &str,
        voice: &str,
    ) -> Result<CloudSynthesis, SpeechError> {
        runtime.block_on(client.synthesize_stream(text, voice, |_| {}))
    }

    fn streamed_wav_with_channels(channels: u16, samples: &[i16]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&24_000u32.to_le_bytes());
        bytes.extend_from_slice(&(24_000 * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    fn streamed_wav(samples: &[i16]) -> Vec<u8> {
        streamed_wav_with_channels(1, samples)
    }

    /// speech-python.md test 10: model `tts-1`, Ilse's default voice `nova`,
    /// `response_format: "wav"`.
    #[test]
    fn synthesis_asks_for_wav_in_the_voice_the_character_was_cast_with() {
        let server = MockServer::start(vec![MockServer::ok_bytes(&wav_body())]);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk-speech"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert!(client.available());
        assert_eq!(client.model(), "tts-1");

        let CloudSynthesis::Buffered(wav) =
            synthesize(&runtime, &client, "Greetings", "ilse").expect("synthesized")
        else {
            panic!("an honest finite WAV keeps the compatibility path");
        };
        assert_eq!(&wav[..4], b"RIFF");

        let request = server.request(0);
        assert_eq!(request.path, "/v1/audio/speech");
        let body: Value = request.json();
        assert_eq!(body["model"], "tts-1");
        assert_eq!(body["voice"], "nova");
        assert_eq!(body["input"], "Greetings");
        assert_eq!(body["response_format"], "wav");
    }

    /// speech-python.md test 11: the legacy `TTS_VOICE_*` override still wins
    /// for the cloud provider.
    #[test]
    fn a_configured_voice_overrides_the_default() {
        let server = MockServer::start(vec![MockServer::ok_bytes(&wav_body())]);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
            ("TTS_VOICE_SVEN", "alloy"),
        ]));
        synthesize(&runtime, &client, "Hello", "sven").expect("synthesized");
        assert_eq!(server.request(0).json()["voice"], "alloy");

        // The provider-qualified variable outranks the legacy one.
        let server = MockServer::start(vec![MockServer::ok_bytes(&wav_body())]);
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
            ("TTS_VOICE_SVEN", "alloy"),
            ("TTS_OPENAI_VOICE_SVEN", "onyx"),
        ]));
        synthesize(&runtime, &client, "Hello", "sven").expect("synthesized");
        assert_eq!(server.request(0).json()["voice"], "onyx");
    }

    /// speech-python.md test 14: empty text, control characters and a
    /// path-traversal voice are all refused **before** the provider is called.
    #[test]
    fn hostile_text_and_voices_never_reach_the_provider() {
        let server = MockServer::start(vec![MockServer::ok_bytes(&wav_body())]);
        let runtime = BackendRuntime::new().expect("runtime");
        let base = server.base_url();

        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &base),
        ]));
        assert!(synthesize(&runtime, &client, "", "ilse").is_err());
        assert!(synthesize(&runtime, &client, "   ", "ilse").is_err());
        assert!(synthesize(&runtime, &client, "bad\0speech", "ilse").is_err());
        assert!(synthesize(&runtime, &client, "bad\rspeech", "ilse").is_err());
        assert!(
            synthesize(&runtime, &client, &"x".repeat(501), "ilse").is_err(),
            "the 500-character limit"
        );
        assert!(
            synthesize(&runtime, &client, "Hello", "gandalf").is_err(),
            "only the three cast voices exist"
        );

        let hostile = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &base),
            ("TTS_VOICE_ILSE", "../../bad"),
        ]));
        assert!(synthesize(&runtime, &hostile, "Hello", "ilse").is_err());

        assert_eq!(server.request_count(), 0, "not one provider call");
    }

    #[test]
    fn a_missing_key_is_unavailable_and_a_provider_error_degrades() {
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[]));
        assert!(!client.available());
        assert_eq!(
            synthesize(&runtime, &client, "Hello", "ilse")
                .expect_err("no key")
                .presentable,
            NO_KEY_MESSAGE
        );

        let server = MockServer::start(vec![
            MockServer::status(503, "down"),
            MockServer::status(503, "still down"),
        ]);
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert_eq!(
            synthesize(&runtime, &client, "Hello", "ilse")
                .expect_err("the provider is down")
                .presentable,
            "cloud speech provider failed (HTTP 503)"
        );
        assert_eq!(server.request_count(), 2, "one retry");
    }

    /// The decoder emits the first samples before EOF/final validation. This is
    /// the regression for the latency bug: buffering until `finish` would make
    /// both `push` calls return no chunks.
    #[test]
    fn provider_pcm_is_decoded_before_the_complete_wav_arrives() {
        let wav = streamed_wav(&[100, -200, 300, -400]);
        let split = wav.len() - 4;
        let mut decoder = CloudWavDecoder::new();

        let first = decoder.push(&wav[..split]).expect("first body fragment");
        assert_eq!(first.len(), 1, "audio is available before response EOF");
        assert_eq!(first[0].seq, 0);
        assert_eq!(first[0].sample_rate, 24_000);
        assert_eq!(first[0].samples.as_ref(), &[100, -200]);

        let second = decoder.push(&wav[split..]).expect("last body fragment");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].seq, 1);
        assert_eq!(second[0].samples.as_ref(), &[300, -400]);
        assert_eq!(
            decoder.finish().expect("valid at EOF"),
            DecodedCloudAudio::Streamed { chunk_count: 2 }
        );
    }

    #[test]
    fn provider_stereo_is_downmixed_before_it_reaches_the_mono_game_source() {
        let wav = streamed_wav_with_channels(2, &[100, 300, -300, 100]);
        let mut decoder = CloudWavDecoder::new();
        let chunks = decoder.push(&wav).expect("stereo body");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].samples.as_ref(), &[200, -100]);
        assert_eq!(
            decoder.finish().expect("valid stereo at EOF"),
            DecodedCloudAudio::Streamed { chunk_count: 1 }
        );
    }

    /// The actual provider shape takes the streaming channel end to end, not
    /// the complete-WAV compatibility branch.
    #[test]
    fn the_streaming_wav_the_provider_actually_sends_becomes_pcm_chunks() {
        let streamed = streamed_wav(&[100, -200, 300, -400]);

        let server = MockServer::start(vec![MockServer::ok_bytes(&streamed)]);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));

        let mut chunks = Vec::new();
        let result = runtime
            .block_on(client.synthesize_stream("Greetings", "ilse", |chunk| chunks.push(chunk)))
            .expect("the provider's own bytes are accepted");
        let CloudSynthesis::Streamed(completion) = result else {
            panic!("the provider placeholder sizes select streaming");
        };
        assert_eq!(completion.chunk_count, 1);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].samples.as_ref(), &[100, -200, 300, -400]);
    }

    /// The provider answering with something unplayable is a degrade, not a
    /// crash in the audio sink three frames later.
    #[test]
    fn audio_the_game_could_not_play_is_refused_here() {
        let server = MockServer::start(vec![MockServer::ok("this is not a WAV")]);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert_eq!(
            synthesize(&runtime, &client, "Hello", "ilse")
                .expect_err("not a WAV")
                .presentable,
            "generated WAV is invalid"
        );
    }
}
