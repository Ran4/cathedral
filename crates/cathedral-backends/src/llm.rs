//! The chat-completions client (`llm_client.py`), as a non-blocking
//! [`Cognition`].
//!
//! `request()` hands the prompt to a tokio task and returns immediately with a
//! [`RequestId`]; the task POSTs, measures the wall time, tallies usage and
//! pushes a [`Completion`] onto the backend channel. Capacity is **one** — the
//! scheduler runs a single turn at a time, and a second submit while busy takes
//! the "cognition worker is busy" branch (`scheduler.py:336-351`).
//!
//! Wire details that are easy to get wrong and are pinned by tests below:
//!
//! * moonshot sends `temperature: 0.6` **and a top-level** `"thinking":
//!   {"type": "disabled"}` — the Python SDK's `extra_body` merges into the body
//!   root, it is not nested (llm-headless.md §1.2, risk 2);
//! * openai sends neither;
//! * `content` is a **parts array** (`[{"type":"text","text":…}]`) so an image
//!   part can be added later without changing the wire type; a config flag falls
//!   back to a plain string if a provider ever rejects it (risk 1).
//!
//! ## Prompt caching, as the two providers actually behave (measured 2026-07-14)
//!
//! `turn.j2` renders its ~1.7k-token instruction block *before* the character
//! sheet so that the block is a prefix shared by every actor's every turn. What
//! each provider then does with that prefix differs, and only one of them pays:
//!
//! * **moonshot** (`kimi-k2.5`) does real prefix caching. A live 8-turn run
//!   reports 63% of input tokens served from cache (`cached_tokens`, which it
//!   sends both top-level and under `prompt_tokens_details`).
//! * **openai** (`gpt-5.6-luna`) does **not**, whatever the docs say about
//!   automatic prefix caching. It caches *whole prompts*: a byte-identical
//!   prompt hits within seconds, but a shared prefix with a different tail never
//!   does. It reports `cache_write_tokens` on every call and `cached_tokens: 0`
//!   forever. The same 8-turn run reports a 0% hit rate.
//!
//! Every client-side lever was tried against `{static}{sheet}` before concluding
//! that, and all nine read zero: the prefix inline or as its own `system`
//! message; at ~2k and at ~8k tokens (so no minimum is unmet); with
//! `prompt_cache_key`; with `prompt_cache_retention: "24h"`; through
//! `/v1/responses` with `instructions`; and with an anthropic-style
//! `cache_control` breakpoint on the message and on the content part — which the
//! API *accepts* only because it silently ignores unknown fields **inside**
//! messages, while strictly rejecting unknown top-level ones. That strictness is
//! the proof there is no explicit-breakpoint parameter to reach for:
//! `prompt_cache_key` and `prompt_cache_retention` are the only cache parameters
//! the endpoint admits, and neither buys a prefix read.
//!
//! The positive control is what makes this conclusive rather than a timing
//! artifact: a byte-identical prompt hits *immediately*, so the cache is live
//! and fast — it simply has no prefix semantics. A game prompt is never
//! byte-identical twice, because the sheet changes every turn.
//!
//! So the reordering is worth ~60% of the input bill on moonshot and nothing at
//! all on openai. [`UsageLedger::prompt_totals`] is what settles the question
//! for any future provider: if the hit rate is 0%, the prefix is not being
//! reused, whatever the docs promise.
//!
//! One thing worth checking against the actual bill: this endpoint reports
//! `cache_write_tokens` ≈ the whole prompt on *every* call. If cache writes
//! carry a premium over plain input (they are said to on gpt-5.6), the game is
//! paying it on every turn and never once recouping it — which would make the
//! openai path *dearer* than having no cache at all. [`PRICING`] charges one
//! flat input rate and cannot see the difference.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use cathedral_sim::{Cognition, CognitionBusy, CognitionError, Completion, RequestId};
use serde::{Deserialize, Serialize};

use crate::{
    config::{LlmConfigError, LlmSettings},
    events::BackendSender,
    runtime::BackendRuntime,
};

/// USD per 1M tokens (input, output) — `llm_client.py:33-38`, as of July 2026.
/// Cache-hit discounts are not modeled, so a run cost is an upper bound.
pub const PRICING: [(&str, (f64, f64)); 2] =
    [("kimi-k2.5", (0.60, 3.00)), ("gpt-5.6-luna", (1.00, 6.00))];

/// The price row for a model, or `None` for one we have no price for.
pub fn pricing_for(model: &str) -> Option<(f64, f64)> {
    PRICING
        .iter()
        .find(|(name, _)| *name == model)
        .map(|(_, price)| *price)
}

/// One retry on a retryable failure, matching the OpenAI SDK's `max_retries=1`
/// (`llm_client.py:81`). The scheduler's own backoff sits on top, so exact
/// parity with the SDK's schedule is not contractual (llm-headless.md risk 3).
const MAX_ATTEMPTS: u32 = 2;
/// Backoff before the retry when the response carries no `Retry-After`.
const RETRY_BACKOFF: Duration = Duration::from_millis(500);
/// A hostile `Retry-After` must not park a turn for an hour.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(20);
/// How much of an error body reaches the log.
const BODY_SNIPPET_CHARS: usize = 200;

// ---------------------------------------------------------------------- errors

/// Every way a completion can fail. The scheduler treats them identically
/// (backoff + percept restore); the split exists for logs (llm-headless.md §1.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    /// Unknown provider, or no API key — Python's `LLMConfigurationError`,
    /// raised lazily inside `complete()` so a long-lived server survives it.
    Config(String),
    /// Connect failure, timeout, TLS, malformed response body.
    Transport(String),
    Http {
        status: u16,
        body_snippet: String,
    },
    /// `choices[0].message.content` was absent or carried no text
    /// (`llm_client.py:98-99`).
    NoTextContent,
}

impl LlmError {
    /// The short kind name the scheduler's diagnostic line prints (the port of
    /// Python's `type(error).__name__`).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Config(_) => "LlmConfigurationError",
            Self::Transport(_) => "LlmTransportError",
            Self::Http { .. } => "LlmHttpError",
            Self::NoTextContent => "LlmNoTextContent",
        }
    }
}

impl fmt::Display for LlmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) => write!(formatter, "{message}"),
            Self::Transport(message) => write!(formatter, "{message}"),
            Self::Http {
                status,
                body_snippet,
            } => write!(formatter, "provider returned {status}: {body_snippet}"),
            Self::NoTextContent => formatter.write_str("LLM returned no text content"),
        }
    }
}

impl std::error::Error for LlmError {}

impl From<LlmConfigError> for LlmError {
    fn from(error: LlmConfigError) -> Self {
        Self::Config(error.0)
    }
}

impl From<&LlmError> for CognitionError {
    /// The kind drives the stderr diagnostic; the detail is what Python's
    /// `repr(error)` put into the prompt archive's `meta.error`. Collapsing both
    /// into the kind would throw away the status code and the provider's
    /// message — the only things that distinguish a bad key from a rate limit.
    fn from(error: &LlmError) -> Self {
        CognitionError::detailed(error.kind(), format!("{}: {error}", error.kind()))
    }
}

// ----------------------------------------------------------------- usage/cost

/// What one model's calls have cost so far, in tokens.
///
/// `cached_prompt_tokens` is the part of `prompt_tokens` the provider served
/// from its prompt cache. It is the only direct evidence that the static prefix
/// of `turn.j2` is being reused instead of re-billed, so it is counted even
/// though [`UsageLedger::run_cost_usd`] does not yet discount it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModelUsage {
    pub prompt_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub completion_tokens: u64,
}

impl ModelUsage {
    /// The share of input tokens the provider did not re-read, 0.0 when it has
    /// billed no input at all.
    pub fn cache_hit_rate(&self) -> f64 {
        if self.prompt_tokens == 0 {
            return 0.0;
        }
        self.cached_prompt_tokens as f64 / self.prompt_tokens as f64
    }
}

/// Per-model token totals (`llm_client.py:41`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageLedger {
    per_model: BTreeMap<String, ModelUsage>,
}

impl UsageLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// A response without usage is silently not counted (`llm_client.py:93-96`).
    pub fn record(
        &mut self,
        model: &str,
        prompt_tokens: u64,
        cached_prompt_tokens: u64,
        completion_tokens: u64,
    ) {
        let entry = self.per_model.entry(model.to_string()).or_default();
        entry.prompt_tokens += prompt_tokens;
        entry.cached_prompt_tokens += cached_prompt_tokens;
        entry.completion_tokens += completion_tokens;
    }

    pub fn is_empty(&self) -> bool {
        self.per_model.is_empty()
    }

    pub fn per_model(&self) -> &BTreeMap<String, ModelUsage> {
        &self.per_model
    }

    /// Input tokens, cached input tokens: the totals across every model.
    pub fn prompt_totals(&self) -> (u64, u64) {
        self.per_model
            .values()
            .fold((0, 0), |(all, cached), usage| {
                (
                    all + usage.prompt_tokens,
                    cached + usage.cached_prompt_tokens,
                )
            })
    }

    /// Total cost of every completion so far (`llm_client.py:126-132`).
    ///
    /// `None` when no call has been made **and** when any model lacks a pricing
    /// row — main.py prints "no pricing entry for this model" in both cases,
    /// which is misleading for the zero-call run but is the current behavior
    /// (llm-headless.md risk 8).
    pub fn run_cost_usd(&self) -> Option<f64> {
        if self.per_model.is_empty() {
            return None;
        }
        let mut total = 0.0;
        for (model, usage) in &self.per_model {
            let (input, output) = pricing_for(model)?;
            total += (usage.prompt_tokens as f64 * input + usage.completion_tokens as f64 * output)
                / 1e6;
        }
        Some(total)
    }
}

// ------------------------------------------------------------------ wire types

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage; 1],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Top level, not nested: this is what turns moonshot's reasoning pass off.
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: MessageContent,
}

/// Multimodal-ready from day one (llm-headless.md §1.10).
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum MessageContent {
    /// The fallback shape Python sends, behind `LLM_CONTENT_PARTS=0`.
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    Text {
        text: String,
    },
    /// The reason the array exists: an NPC-eye screenshot, later. Unused today.
    #[allow(dead_code)]
    ImageUrl {
        image_url: ImageUrl,
    },
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    /// A `data:image/png;base64,…` URI.
    url: String,
}

#[derive(Debug, Serialize)]
struct Thinking {
    #[serde(rename = "type")]
    mode: &'static str,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    #[serde(default)]
    message: Option<ResponseMessage>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: Option<ResponseContent>,
}

/// Providers answer with a plain string; a parts array is accepted defensively.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ResponseContent {
    Text(String),
    Parts(Vec<ResponsePart>),
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
    /// openai reports the cache hit here; moonshot reports it here *and*
    /// top-level, and omits the whole object on a cold call.
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
    /// moonshot's top-level spelling of the same number.
    #[serde(default)]
    cached_tokens: Option<u64>,
}

impl Usage {
    /// How many input tokens the provider served from its prompt cache.
    ///
    /// Both spellings mean the same thing, so either one answers; a provider
    /// that reports neither reads as an honest zero rather than as a hit.
    fn cached_prompt_tokens(&self) -> u64 {
        self.prompt_tokens_details
            .as_ref()
            .and_then(|details| details.cached_tokens)
            .or(self.cached_tokens)
            .unwrap_or(0)
    }
}

#[derive(Debug, Deserialize)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: Option<u64>,
}

// ------------------------------------------------------------------- the client

/// A stateless chat-completions client: one user message, no provider-side
/// history (`llm_client.py:86-100`).
#[derive(Debug)]
pub struct LlmClient {
    http: reqwest::Client,
    settings: LlmSettings,
    usage: Arc<Mutex<UsageLedger>>,
}

impl LlmClient {
    pub fn new(settings: LlmSettings) -> Result<Self, LlmError> {
        let http = reqwest::Client::builder()
            .build()
            .map_err(|error| LlmError::Transport(error.to_string()))?;
        Ok(Self {
            http,
            settings,
            usage: Arc::new(Mutex::new(UsageLedger::new())),
        })
    }

    pub fn settings(&self) -> &LlmSettings {
        &self.settings
    }

    pub fn usage(&self) -> UsageLedger {
        self.usage
            .lock()
            .expect("usage ledger is not poisoned")
            .clone()
    }

    pub fn run_cost_usd(&self) -> Option<f64> {
        self.usage().run_cost_usd()
    }

    /// One completion, with the single SDK-equivalent retry.
    pub async fn complete(&self, prompt: String) -> Result<String, LlmError> {
        self.complete_with_budget(prompt, None).await
    }

    pub async fn complete_with_budget(
        &self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<String, LlmError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.attempt(&prompt, max_output_tokens).await {
                Ok(text) => return Ok(text),
                Err(failure) => {
                    if attempt >= MAX_ATTEMPTS || !failure.retryable {
                        return Err(failure.error);
                    }
                    tokio::time::sleep(failure.retry_after.unwrap_or(RETRY_BACKOFF)).await;
                }
            }
        }
    }

    async fn attempt(
        &self,
        prompt: &str,
        max_output_tokens: Option<u32>,
    ) -> Result<String, Attempt> {
        let spec = self.settings.provider.spec();
        let body = ChatRequest {
            model: &self.settings.model,
            messages: [ChatMessage {
                role: "user",
                content: if self.settings.content_parts {
                    MessageContent::Parts(vec![ContentPart::Text {
                        text: prompt.to_string(),
                    }])
                } else {
                    MessageContent::Text(prompt.to_string())
                },
            }],
            temperature: spec.temperature,
            thinking: spec
                .thinking_disabled
                .then_some(Thinking { mode: "disabled" }),
            max_completion_tokens: max_output_tokens,
        };

        let response = self
            .http
            .post(self.settings.chat_completions_url())
            .bearer_auth(&self.settings.api_key)
            .timeout(Duration::from_secs_f64(
                self.settings.timeout_seconds.max(0.001),
            ))
            .json(&body)
            .send()
            .await
            .map_err(|error| Attempt {
                // Connect errors and timeouts are exactly what a retry is for.
                retryable: true,
                retry_after: None,
                error: LlmError::Transport(error.to_string()),
            })?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = parse_retry_after(
                response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok()),
            );
            let body = response.text().await.unwrap_or_default();
            return Err(Attempt {
                retryable: is_retryable_status(status.as_u16()),
                retry_after,
                error: LlmError::Http {
                    status: status.as_u16(),
                    body_snippet: snippet(&body),
                },
            });
        }

        let payload: ChatResponse = response.json().await.map_err(|error| Attempt {
            retryable: false,
            retry_after: None,
            error: LlmError::Transport(error.to_string()),
        })?;

        if let Some(usage) = payload.usage.as_ref() {
            self.usage
                .lock()
                .expect("usage ledger is not poisoned")
                .record(
                    &self.settings.model,
                    usage.prompt_tokens.unwrap_or(0),
                    usage.cached_prompt_tokens(),
                    usage.completion_tokens.unwrap_or(0),
                );
        }

        // Only `choices[0]` is ever consulted, exactly like Python.
        payload
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message)
            .and_then(|message| message.content)
            .and_then(|content| match content {
                // An empty *string* is a reply, and Python agreed: it only
                // checked `isinstance(content, str)` (`llm_client.py:97-99`).
                ResponseContent::Text(text) => Some(text),
                // A parts array that yields no text at all is not a reply. It
                // has to fail here, or the scheduler resets its failure counter
                // and burns the turn in silence — no backoff, no `system:` line,
                // no degraded status — which is the exact failure mode
                // `NoTextContent` exists to surface.
                ResponseContent::Parts(parts) => {
                    let texts: Vec<String> =
                        parts.into_iter().filter_map(|part| part.text).collect();
                    (!texts.is_empty()).then(|| texts.join(""))
                }
            })
            .ok_or(Attempt {
                retryable: false,
                retry_after: None,
                error: LlmError::NoTextContent,
            })
    }
}

struct Attempt {
    error: LlmError,
    retryable: bool,
    retry_after: Option<Duration>,
}

/// Connect errors, 408, 409, 429 and 5xx — the OpenAI SDK's retryable set.
fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429) || (500..600).contains(&status)
}

fn parse_retry_after(header: Option<&str>) -> Option<Duration> {
    let seconds: f64 = header?.trim().parse().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some(Duration::from_secs_f64(seconds).min(MAX_RETRY_AFTER))
}

fn snippet(body: &str) -> String {
    body.chars().take(BODY_SNIPPET_CHARS).collect()
}

// -------------------------------------------------------------- HttpCognition

/// The provider-backed [`Cognition`]: submit now, complete later.
pub struct HttpCognition {
    runtime: Arc<BackendRuntime>,
    /// `Err` = misconfigured. Requests are still *accepted* and fail on the
    /// channel, so a bad key degrades the same way a dead provider does rather
    /// than taking a different code path (`llm_client.py:44-45`).
    client: Result<Arc<LlmClient>, LlmError>,
    events: BackendSender,
    next_request_id: u64,
    /// Capacity one, like `scheduler._CompletionWorker`.
    busy: Arc<AtomicBool>,
    model: Option<String>,
}

impl HttpCognition {
    pub fn new(
        runtime: Arc<BackendRuntime>,
        settings: Result<LlmSettings, LlmConfigError>,
        events: BackendSender,
    ) -> Self {
        let model = settings.as_ref().ok().map(|s| s.model.clone());
        let client = settings
            .map_err(LlmError::from)
            .and_then(|settings| LlmClient::new(settings).map(Arc::new));
        Self {
            runtime,
            client,
            events,
            next_request_id: 0,
            busy: Arc::new(AtomicBool::new(false)),
            model,
        }
    }

    /// The model `request()` would use, or `None` when misconfigured
    /// (`llm_client.py:103-109`) — the prompt log's `meta.model`.
    pub fn model_name(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn usage(&self) -> UsageLedger {
        self.client
            .as_ref()
            .map(|client| client.usage())
            .unwrap_or_default()
    }

    /// Total USD spent this run, or `None` (see [`UsageLedger::run_cost_usd`]).
    pub fn run_cost_usd(&self) -> Option<f64> {
        self.usage().run_cost_usd()
    }

    fn take_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }

    fn submit(
        &mut self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(CognitionBusy);
        }
        let request_id = self.take_request_id();

        let client = match &self.client {
            Ok(client) => Arc::clone(client),
            Err(error) => {
                self.busy.store(false, Ordering::SeqCst);
                self.events.send(Completion {
                    request_id,
                    result: Err(CognitionError::from(error)),
                    duration_seconds: 0.0,
                });
                return Ok(request_id);
            }
        };

        let events = self.events.clone();
        let busy = Arc::clone(&self.busy);
        self.runtime.spawn(async move {
            let started = Instant::now();
            let result = client.complete_with_budget(prompt, max_output_tokens).await;
            let duration_seconds = started.elapsed().as_secs_f64();
            busy.store(false, Ordering::SeqCst);
            events.send(Completion {
                request_id,
                result: result.map_err(|error| CognitionError::from(&error)),
                duration_seconds,
            });
        });
        Ok(request_id)
    }
}

impl Cognition for HttpCognition {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.submit(prompt, None)
    }

    fn request_with_budget(
        &mut self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.submit(prompt, max_output_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{PROVIDERS, Provider},
        events::{BackendEvent, backend_channel},
        testing::MockServer,
    };
    use crossbeam_channel::Receiver;
    use serde_json::Value;

    fn settings(provider: Provider, base_url: &str) -> LlmSettings {
        LlmSettings {
            provider,
            model: provider.spec().default_model.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            timeout_seconds: 5.0,
            content_parts: true,
        }
    }

    /// Submit one prompt and wait for its completion off the channel.
    fn complete_once(
        settings: LlmSettings,
        events: (crate::events::BackendSender, Receiver<BackendEvent>),
    ) -> Completion {
        let runtime = BackendRuntime::new().expect("runtime");
        let mut cognition = HttpCognition::new(runtime, Ok(settings), events.0);
        cognition
            .request("the prompt".to_string())
            .expect("accepted");
        let event = events
            .1
            .recv_timeout(Duration::from_secs(10))
            .expect("a completion arrives");
        let BackendEvent::LlmCompletion(completion) = event else {
            panic!("expected a completion");
        };
        completion
    }

    #[test]
    fn the_moonshot_body_carries_temperature_and_a_top_level_thinking_flag() {
        let server = MockServer::start(vec![MockServer::ok(
            r#"{
            "choices": [{"message": {"content": "wait {}"}}]
        }"#,
        )]);
        let completion = complete_once(
            settings(Provider::Moonshot, &server.base_url()),
            backend_channel(),
        );
        assert_eq!(completion.result, Ok("wait {}".to_string()));

        let request = server.request(0);
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/chat/completions");
        assert_eq!(
            request.header("authorization").as_deref(),
            Some("Bearer sk-test")
        );
        let body: Value = request.json();
        assert_eq!(body["model"], "kimi-k2.5");
        assert_eq!(body["temperature"], 0.6);
        assert_eq!(
            body["thinking"],
            serde_json::json!({"type": "disabled"}),
            "the flag must sit at the body root, not nested"
        );
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(
            body["messages"][0]["content"],
            serde_json::json!([{"type": "text", "text": "the prompt"}]),
            "content is a multimodal-ready parts array"
        );
    }

    #[test]
    fn the_openai_body_sends_no_extras() {
        let server = MockServer::start(vec![MockServer::ok(
            r#"{
            "choices": [{"message": {"content": "hi"}}]
        }"#,
        )]);
        let completion = complete_once(
            settings(Provider::Openai, &server.base_url()),
            backend_channel(),
        );
        assert_eq!(completion.result, Ok("hi".to_string()));

        let body: Value = server.request(0).json();
        assert_eq!(body["model"], "gpt-5.6-luna");
        assert!(body.get("temperature").is_none(), "{body}");
        assert!(body.get("thinking").is_none(), "{body}");
        assert_eq!(
            body.as_object().expect("object").len(),
            2,
            "only model and messages: {body}"
        );
    }

    #[test]
    fn a_significance_budget_is_sent_as_max_completion_tokens() {
        let server = MockServer::start(vec![MockServer::ok(
            r#"{"choices": [{"message": {"content": "wait {}"}}]}"#,
        )]);
        let events = backend_channel();
        let runtime = BackendRuntime::new().expect("runtime");
        let mut cognition = HttpCognition::new(
            Arc::clone(&runtime),
            Ok(settings(Provider::Openai, &server.base_url())),
            events.0,
        );
        cognition
            .request_with_budget("the prompt".to_string(), Some(350))
            .expect("accepted");
        let event = events
            .1
            .recv_timeout(Duration::from_secs(10))
            .expect("a completion arrives");
        assert!(matches!(event, BackendEvent::LlmCompletion(_)));

        let body: Value = server.request(0).json();
        assert_eq!(body["max_completion_tokens"], 350);
    }

    #[test]
    fn the_fallback_flag_sends_plain_string_content() {
        let server = MockServer::start(vec![MockServer::ok(
            r#"{
            "choices": [{"message": {"content": "ok"}}]
        }"#,
        )]);
        let mut settings = settings(Provider::Moonshot, &server.base_url());
        settings.content_parts = false;
        assert_eq!(
            complete_once(settings, backend_channel()).result,
            Ok("ok".to_string())
        );

        let body: Value = server.request(0).json();
        assert_eq!(body["messages"][0]["content"], "the prompt");
    }

    #[test]
    fn a_429_is_retried_once_and_honors_retry_after() {
        let server = MockServer::start(vec![
            MockServer::status(429, r#"{"error": "slow down"}"#).retry_after("0"),
            MockServer::ok(r#"{"choices": [{"message": {"content": "second try"}}]}"#),
        ]);
        let completion = complete_once(
            settings(Provider::Moonshot, &server.base_url()),
            backend_channel(),
        );
        assert_eq!(completion.result, Ok("second try".to_string()));
        assert_eq!(server.request_count(), 2, "exactly one retry");
    }

    #[test]
    fn a_500_is_retried_once_and_then_gives_up() {
        let server = MockServer::start(vec![
            MockServer::status(500, "upstream exploded"),
            MockServer::status(503, "still down"),
            MockServer::ok(r#"{"choices": [{"message": {"content": "never reached"}}]}"#),
        ]);
        let completion = complete_once(
            settings(Provider::Moonshot, &server.base_url()),
            backend_channel(),
        );
        let error = completion
            .result
            .expect_err("the scheduler backs off on any error");
        assert_eq!(error.kind(), "LlmHttpError");
        assert_eq!(server.request_count(), 2, "one attempt plus one retry");
    }

    /// The archive has to be able to tell a bad key from a rate limit from an
    /// outage. Python kept `repr(error)` — the status and the provider's body —
    /// in `meta.error`; the kind alone (`LlmHttpError`) says nothing.
    #[test]
    fn an_http_failure_keeps_its_status_and_body_for_the_archive() {
        let server = MockServer::start(vec![MockServer::status(
            401,
            r#"{"error": {"message": "Invalid Authentication"}}"#,
        )]);
        let completion = complete_once(
            settings(Provider::Moonshot, &server.base_url()),
            backend_channel(),
        );
        let error = completion.result.expect_err("a 401 is a failure");
        // The diagnostic line prints the kind, exactly as Python printed
        // `type(error).__name__`.
        assert_eq!(error.kind(), "LlmHttpError");
        assert_eq!(error.to_string(), "LlmHttpError");
        // The prompt archive gets everything.
        assert!(error.detail().contains("401"), "{}", error.detail());
        assert!(
            error.detail().contains("Invalid Authentication"),
            "{}",
            error.detail()
        );
    }

    #[test]
    fn a_400_is_not_retried() {
        let server = MockServer::start(vec![
            MockServer::status(400, "bad request"),
            MockServer::ok(r#"{"choices": [{"message": {"content": "never reached"}}]}"#),
        ]);
        let completion = complete_once(
            settings(Provider::Moonshot, &server.base_url()),
            backend_channel(),
        );
        assert!(completion.result.is_err());
        assert_eq!(server.request_count(), 1);
    }

    #[test]
    fn a_reply_without_text_content_is_an_error() {
        let server = MockServer::start(vec![MockServer::ok(
            r#"{"choices": [{"message": {"content": null}}]}"#,
        )]);
        let completion = complete_once(
            settings(Provider::Moonshot, &server.base_url()),
            backend_channel(),
        );
        assert_eq!(
            completion
                .result
                .expect_err("no content is a failure")
                .kind(),
            "LlmNoTextContent"
        );
    }

    /// A parts array with no text in it is not a reply. Returning `Ok("")` would
    /// reset the scheduler's failure counter and burn the turn silently — the
    /// whole cast would go quiet at the normal cadence with no backoff and no
    /// diagnostic. Python raised here too (a list is not a `str`).
    #[test]
    fn a_parts_array_with_no_text_is_not_an_empty_reply() {
        for body in [
            r#"{"choices": [{"message": {"content": []}}]}"#,
            r#"{"choices": [{"message": {"content": [{"type": "image_url", "image_url": {"url": "x"}}]}}]}"#,
        ] {
            let server = MockServer::start(vec![MockServer::ok(body)]);
            let completion = complete_once(
                settings(Provider::Moonshot, &server.base_url()),
                backend_channel(),
            );
            assert_eq!(
                completion
                    .result
                    .expect_err("a text-less reply is a failure")
                    .kind(),
                "LlmNoTextContent",
                "{body}"
            );
        }

        // An empty *string*, though, is a reply — `isinstance("", str)` is true.
        let server = MockServer::start(vec![MockServer::ok(
            r#"{"choices": [{"message": {"content": ""}}]}"#,
        )]);
        let completion = complete_once(
            settings(Provider::Moonshot, &server.base_url()),
            backend_channel(),
        );
        assert_eq!(completion.result, Ok(String::new()));
    }

    #[test]
    fn a_parts_array_reply_is_joined_into_text() {
        let server = MockServer::start(vec![MockServer::ok(
            r#"{"choices": [{"message": {"content": [
                {"type": "text", "text": "wait "},
                {"type": "text", "text": "{}"}
            ]}}]}"#,
        )]);
        let completion = complete_once(
            settings(Provider::Moonshot, &server.base_url()),
            backend_channel(),
        );
        assert_eq!(completion.result, Ok("wait {}".to_string()));
    }

    #[test]
    fn usage_is_tallied_across_calls_and_priced() {
        let server = MockServer::start(vec![
            MockServer::ok(
                r#"{"choices": [{"message": {"content": "a"}}],
                    "usage": {"prompt_tokens": 1000, "completion_tokens": 100}}"#,
            ),
            MockServer::ok(
                r#"{"choices": [{"message": {"content": "b"}}],
                    "usage": {"prompt_tokens": 1000, "completion_tokens": 100}}"#,
            ),
            // A reply without usage is silently not counted.
            MockServer::ok(r#"{"choices": [{"message": {"content": "c"}}]}"#),
        ]);
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, receiver) = backend_channel();
        let mut cognition = HttpCognition::new(
            runtime,
            Ok(settings(Provider::Moonshot, &server.base_url())),
            sender,
        );
        assert_eq!(cognition.model_name(), Some("kimi-k2.5"));
        assert_eq!(
            cognition.run_cost_usd(),
            None,
            "zero calls: no cost, not 0.0"
        );

        for _ in 0..3 {
            cognition.request("p".to_string()).expect("accepted");
            receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("completion");
        }

        let usage = cognition.usage();
        assert_eq!(
            usage.per_model()["kimi-k2.5"],
            ModelUsage {
                prompt_tokens: 2000,
                cached_prompt_tokens: 0,
                completion_tokens: 200,
            }
        );
        // (2000 * 0.60 + 200 * 3.00) / 1e6
        let cost = cognition.run_cost_usd().expect("priced");
        assert!((cost - 0.0018).abs() < 1e-12, "{cost}");
    }

    /// The evidence that `turn.j2`'s static prefix is being reused. moonshot
    /// spells it both ways and omits the details object entirely on a cold
    /// call; openai spells it only the nested way. All three must read.
    #[test]
    fn cached_input_tokens_are_tallied_however_the_provider_spells_them() {
        let server = MockServer::start(vec![
            // Cold: no details object at all (moonshot's first call).
            MockServer::ok(
                r#"{"choices": [{"message": {"content": "a"}}],
                    "usage": {"prompt_tokens": 2000, "completion_tokens": 10}}"#,
            ),
            // Warm, nested (openai, and moonshot's other spelling).
            MockServer::ok(
                r#"{"choices": [{"message": {"content": "b"}}],
                    "usage": {"prompt_tokens": 2000, "completion_tokens": 10,
                              "prompt_tokens_details": {"cached_tokens": 1792}}}"#,
            ),
            // Warm, top-level only (moonshot).
            MockServer::ok(
                r#"{"choices": [{"message": {"content": "c"}}],
                    "usage": {"prompt_tokens": 2000, "completion_tokens": 10,
                              "cached_tokens": 1792}}"#,
            ),
        ]);
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, receiver) = backend_channel();
        let mut cognition = HttpCognition::new(
            runtime,
            Ok(settings(Provider::Moonshot, &server.base_url())),
            sender,
        );

        for _ in 0..3 {
            cognition.request("p".to_string()).expect("accepted");
            receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("completion");
        }

        let usage = cognition.usage();
        let model = usage.per_model()["kimi-k2.5"];
        assert_eq!(model.prompt_tokens, 6000);
        assert_eq!(
            model.cached_prompt_tokens, 3584,
            "the cold call cached none"
        );
        assert_eq!(usage.prompt_totals(), (6000, 3584));
        // Two of the three calls reused the prefix.
        assert!((model.cache_hit_rate() - 3584.0 / 6000.0).abs() < 1e-12);
    }

    #[test]
    fn a_model_that_has_billed_no_input_has_no_cache_hit_rate() {
        assert_eq!(ModelUsage::default().cache_hit_rate(), 0.0);
    }

    #[test]
    fn an_unpriced_model_makes_the_run_cost_unknown() {
        let mut ledger = UsageLedger::new();
        assert_eq!(ledger.run_cost_usd(), None, "no calls at all");

        ledger.record("gpt-5.6-luna", 1_000_000, 0, 0);
        assert_eq!(ledger.run_cost_usd(), Some(1.0));

        ledger.record("some-unpriced-model", 10, 0, 10);
        assert_eq!(ledger.run_cost_usd(), None, "one unknown model poisons it");
    }

    #[test]
    fn a_second_request_while_one_is_in_flight_is_refused() {
        // The server never answers, so the first request stays in flight.
        let server = MockServer::start(vec![MockServer::hang()]);
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, _receiver) = backend_channel();
        let mut cognition = HttpCognition::new(
            runtime,
            Ok(settings(Provider::Moonshot, &server.base_url())),
            sender,
        );
        assert!(cognition.request("first".to_string()).is_ok());
        assert_eq!(cognition.request("second".to_string()), Err(CognitionBusy));
    }

    #[test]
    fn a_misconfigured_client_fails_on_the_channel_instead_of_at_startup() {
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, receiver) = backend_channel();
        let mut cognition = HttpCognition::new(
            runtime,
            Err(LlmConfigError("MOONSHOT_API_KEY not set".to_string())),
            sender,
        );
        assert_eq!(cognition.model_name(), None);

        let request_id = cognition.request("p".to_string()).expect("accepted anyway");
        let BackendEvent::LlmCompletion(completion) = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("event")
        else {
            panic!("expected a completion");
        };
        assert_eq!(completion.request_id, request_id);
        let error = completion.result.expect_err("a missing key is a failure");
        assert_eq!(error.kind(), "LlmConfigurationError");
        assert!(
            error.detail().contains("MOONSHOT_API_KEY not set"),
            "{}",
            error.detail()
        );
        // The slot is free again: the scheduler retries after its backoff.
        assert!(cognition.request("p".to_string()).is_ok());
    }

    #[test]
    fn request_ids_increase_and_match_their_completion() {
        let server = MockServer::start(vec![
            MockServer::ok(r#"{"choices": [{"message": {"content": "one"}}]}"#),
            MockServer::ok(r#"{"choices": [{"message": {"content": "two"}}]}"#),
        ]);
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, receiver) = backend_channel();
        let mut cognition = HttpCognition::new(
            runtime,
            Ok(settings(Provider::Moonshot, &server.base_url())),
            sender,
        );

        let mut seen = Vec::new();
        for _ in 0..2 {
            let request_id = cognition.request("p".to_string()).expect("accepted");
            let BackendEvent::LlmCompletion(completion) = receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("completion")
            else {
                panic!("expected a completion");
            };
            assert_eq!(completion.request_id, request_id);
            assert!(completion.duration_seconds >= 0.0);
            seen.push(request_id);
        }
        assert_eq!(seen, vec![RequestId(0), RequestId(1)]);
    }

    #[test]
    fn the_provider_table_matches_llm_client_py() {
        assert_eq!(PROVIDERS.len(), 2);
        let moonshot = Provider::Moonshot.spec();
        assert_eq!(moonshot.base_url, "https://api.moonshot.ai/v1");
        assert_eq!(moonshot.key_env, "MOONSHOT_API_KEY");
        assert_eq!(moonshot.default_model, "kimi-k2.5");
        assert_eq!(moonshot.temperature, Some(0.6));
        assert!(moonshot.thinking_disabled);

        let openai = Provider::Openai.spec();
        assert_eq!(openai.key_env, "OPENAI_API_KEY");
        assert_eq!(openai.default_model, "gpt-5.6-luna");
        assert_eq!(openai.temperature, None);
        assert!(!openai.thinking_disabled);

        assert_eq!(pricing_for("kimi-k2.5"), Some((0.60, 3.00)));
        assert_eq!(pricing_for("gpt-5.6-luna"), Some((1.00, 6.00)));
        assert_eq!(pricing_for("kimi-k9"), None);
    }

    #[test]
    fn retry_after_parsing_is_clamped_and_forgiving() {
        assert_eq!(parse_retry_after(Some("2")), Some(Duration::from_secs(2)));
        assert_eq!(
            parse_retry_after(Some(" 0.5 ")),
            Some(Duration::from_millis(500))
        );
        assert_eq!(parse_retry_after(Some("99999")), Some(MAX_RETRY_AFTER));
        assert_eq!(parse_retry_after(Some("-1")), None);
        assert_eq!(
            parse_retry_after(Some("Wed, 21 Oct 2026 07:28:00 GMT")),
            None
        );
        assert_eq!(parse_retry_after(None), None);
    }
}
