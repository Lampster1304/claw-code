use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ApiError;
use crate::http_client::build_http_client_or_default;
use crate::prompt_cache::{PromptCache, PromptCacheRecord, PromptCacheStats};
use crate::types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStartEvent, ContentBlockStopEvent,
    InputContentBlock, InputMessage, MessageDelta, MessageDeltaEvent, MessageRequest,
    MessageResponse, MessageStartEvent, MessageStopEvent, OutputContentBlock, StreamEvent,
    ToolDefinition, Usage,
};

use super::{preflight_message_request, Provider, ProviderFuture};

pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
const LOCAL_PROVIDER_ENV: &str = "AGCLI_LOCAL_PROVIDER";
const LOCAL_BASE_URL_ENV: &str = "AGCLI_LOCAL_BASE_URL";
const OLLAMA_HOST_ENV: &str = "OLLAMA_HOST";
const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";

#[derive(Debug, Clone)]
pub struct OllamaClient {
    http: reqwest::Client,
    model: String,
    base_url: String,
    prompt_cache: Option<PromptCache>,
    last_prompt_cache_record: Arc<Mutex<Option<PromptCacheRecord>>>,
}

impl OllamaClient {
    #[must_use]
    pub fn from_model(model: &str) -> Self {
        let trimmed = model
            .strip_prefix("ollama/")
            .or_else(|| model.strip_prefix("local/"))
            .unwrap_or(model)
            .trim()
            .to_string();
        Self {
            http: build_http_client_or_default(),
            model: if trimmed.is_empty() {
                DEFAULT_OLLAMA_MODEL.to_string()
            } else {
                trimmed
            },
            base_url: read_base_url(),
            prompt_cache: None,
            last_prompt_cache_record: Arc::new(Mutex::new(None)),
        }
    }

    #[must_use]
    pub fn with_prompt_cache(mut self, prompt_cache: PromptCache) -> Self {
        self.prompt_cache = Some(prompt_cache);
        self
    }

    #[must_use]
    pub fn prompt_cache_stats(&self) -> Option<PromptCacheStats> {
        self.prompt_cache.as_ref().map(PromptCache::stats)
    }

    #[must_use]
    pub fn take_last_prompt_cache_record(&self) -> Option<PromptCacheRecord> {
        self.last_prompt_cache_record
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        let request = MessageRequest {
            stream: false,
            ..request.clone()
        };
        preflight_message_request(&request)?;
        let response = self
            .http
            .post(chat_endpoint(&self.base_url))
            .json(&build_ollama_request(&request, &self.model, false))
            .send()
            .await
            .map_err(ApiError::from)?;
        let status = response.status();
        let body = response.text().await.map_err(ApiError::from)?;
        if !status.is_success() {
            return Err(build_ollama_api_error(status, &body));
        }
        if let Some(err) = check_ollama_body_error(&body) {
            return Err(err);
        }
        let payload = serde_json::from_str::<OllamaChatResponse>(&body)
            .map_err(|error| ApiError::json_deserialize("Ollama", &request.model, &body, error))?;
        let normalized = normalize_response(payload, request.model.clone());
        if let Some(prompt_cache) = &self.prompt_cache {
            let record = prompt_cache.record_response(&request, &normalized);
            *self
                .last_prompt_cache_record
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(record);
        }
        Ok(normalized)
    }

    pub async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageStream, ApiError> {
        preflight_message_request(request)?;
        let response = self
            .http
            .post(chat_endpoint(&self.base_url))
            .json(&build_ollama_request(request, &self.model, true))
            .send()
            .await
            .map_err(ApiError::from)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.map_err(ApiError::from)?;
            return Err(build_ollama_api_error(status, &body));
        }
        Ok(MessageStream {
            lines: Box::pin(response.bytes_stream()),
            pending: VecDeque::new(),
            buffer: String::new(),
            started: false,
            model: request.model.clone(),
            finished: false,
            tool_blocks_emitted: 0,
        })
    }
}

impl Provider for OllamaClient {
    type Stream = MessageStream;

    fn send_message<'a>(
        &'a self,
        request: &'a MessageRequest,
    ) -> ProviderFuture<'a, MessageResponse> {
        Box::pin(async move { self.send_message(request).await })
    }

    fn stream_message<'a>(
        &'a self,
        request: &'a MessageRequest,
    ) -> ProviderFuture<'a, Self::Stream> {
        Box::pin(async move { self.stream_message(request).await })
    }
}

fn chat_endpoint(base_url: &str) -> String {
    format!("{}/api/chat", base_url.trim_end_matches('/'))
}

pub struct MessageStream {
    lines: impl_futures_stream::BoxByteStream,
    pending: VecDeque<StreamEvent>,
    buffer: String,
    started: bool,
    model: String,
    finished: bool,
    tool_blocks_emitted: u32,
}

mod impl_futures_stream {
    pub type BoxByteStream = std::pin::Pin<
        Box<dyn futures_core::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>,
    >;
}

// Manual Debug impl: `Pin<Box<dyn Stream>>` does not implement Debug, so we
// cannot `#[derive(Debug)]`. The parent `api::client::MessageStream` enum
// derives Debug and requires every variant (including this one) to be Debug.
impl std::fmt::Debug for MessageStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OllamaMessageStream")
            .field("model", &self.model)
            .field("started", &self.started)
            .field("finished", &self.finished)
            .field("pending_len", &self.pending.len())
            .field("buffer_len", &self.buffer.len())
            .field("tool_blocks_emitted", &self.tool_blocks_emitted)
            .finish()
    }
}

impl MessageStream {
    #[must_use]
    pub const fn request_id(&self) -> Option<&str> {
        None
    }

    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(event));
        }
        if self.finished {
            return Ok(None);
        }

        use futures_util::StreamExt;

        while let Some(chunk) = self.lines.next().await {
            let chunk = chunk.map_err(ApiError::from)?;
            self.buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline) = self.buffer.find('\n') {
                let line = self.buffer[..newline].trim().to_string();
                self.buffer.drain(..=newline);
                if line.is_empty() {
                    continue;
                }
                if let Some(err) = check_ollama_body_error(&line) {
                    return Err(err);
                }
                let payload =
                    serde_json::from_str::<OllamaChatResponse>(&line).map_err(|error| {
                        ApiError::json_deserialize("Ollama", &self.model, &line, error)
                    })?;
                self.enqueue_payload(payload);
                if let Some(event) = self.pending.pop_front() {
                    return Ok(Some(event));
                }
            }
        }

        self.finished = true;
        Ok(self.pending.pop_front())
    }

    fn enqueue_payload(&mut self, payload: OllamaChatResponse) {
        if !self.started {
            self.pending
                .push_back(StreamEvent::MessageStart(MessageStartEvent {
                    message: MessageResponse {
                        id: "ollama-stream".to_string(),
                        kind: "message".to_string(),
                        role: "assistant".to_string(),
                        content: Vec::new(),
                        model: self.model.clone(),
                        stop_reason: None,
                        stop_sequence: None,
                        usage: Usage::default(),
                        request_id: None,
                    },
                }));
            // Always open a text block at index 0 so the runtime has a
            // stable anchor even for tool-only responses.
            self.pending
                .push_back(StreamEvent::ContentBlockStart(ContentBlockStartEvent {
                    index: 0,
                    content_block: OutputContentBlock::Text {
                        text: String::new(),
                    },
                }));
            self.started = true;
        }

        let text = payload.message.content;
        if !text.is_empty() {
            self.pending
                .push_back(StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
                    index: 0,
                    delta: ContentBlockDelta::TextDelta { text },
                }));
        }

        for call in payload.message.tool_calls {
            self.tool_blocks_emitted += 1;
            let idx = self.tool_blocks_emitted;
            let arguments = call.function.arguments;
            let partial_json = if arguments.is_null() {
                "{}".to_string()
            } else {
                arguments.to_string()
            };
            self.pending
                .push_back(StreamEvent::ContentBlockStart(ContentBlockStartEvent {
                    index: idx,
                    content_block: OutputContentBlock::ToolUse {
                        id: format!("ollama-call-{idx}"),
                        name: call.function.name,
                        input: Value::Object(serde_json::Map::new()),
                    },
                }));
            self.pending
                .push_back(StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
                    index: idx,
                    delta: ContentBlockDelta::InputJsonDelta { partial_json },
                }));
            self.pending
                .push_back(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                    index: idx,
                }));
        }

        if payload.done {
            // Close the always-open text block at index 0.
            self.pending
                .push_back(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                    index: 0,
                }));
            let has_tool_calls = self.tool_blocks_emitted > 0;
            self.pending
                .push_back(StreamEvent::MessageDelta(MessageDeltaEvent {
                    delta: MessageDelta {
                        stop_reason: if has_tool_calls {
                            Some("tool_use".to_string())
                        } else {
                            payload.done_reason.or(Some("end_turn".to_string()))
                        },
                        stop_sequence: None,
                    },
                    usage: Usage {
                        input_tokens: payload.prompt_eval_count.unwrap_or(0),
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                        output_tokens: payload.eval_count.unwrap_or(0),
                    },
                }));
            self.pending
                .push_back(StreamEvent::MessageStop(MessageStopEvent {}));
            self.finished = true;
        }
    }
}

// ---------- wire types: request ----------

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<OllamaChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
}

#[derive(Debug, Serialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<OllamaOutgoingToolCall>,
}

#[derive(Debug, Serialize)]
struct OllamaOutgoingToolCall {
    function: OllamaOutgoingToolCallFunction,
}

#[derive(Debug, Serialize)]
struct OllamaOutgoingToolCallFunction {
    name: String,
    arguments: Value,
}

#[derive(Debug, Default, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OllamaToolFunction,
}

#[derive(Debug, Serialize)]
struct OllamaToolFunction {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: Value,
}

// ---------- wire types: response ----------

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    #[serde(default)]
    message: OllamaMessage,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct OllamaMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaIncomingToolCall>,
}

#[derive(Debug, Deserialize)]
struct OllamaIncomingToolCall {
    function: OllamaIncomingToolCallFunction,
}

#[derive(Debug, Deserialize)]
struct OllamaIncomingToolCallFunction {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
struct OllamaErrorResponse {
    error: String,
}

// ---------- request builders ----------

fn build_ollama_request(request: &MessageRequest, model: &str, stream: bool) -> OllamaChatRequest {
    let mut messages: Vec<OllamaChatMessage> = Vec::new();
    if let Some(system) = request
        .system
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        messages.push(OllamaChatMessage {
            role: "system".to_string(),
            content: system.to_string(),
            tool_calls: Vec::new(),
        });
    }
    for message in &request.messages {
        convert_single_message(message, &mut messages);
    }

    let options = build_ollama_options(request);
    let tools = request
        .tools
        .as_ref()
        .map(|list| list.iter().map(build_ollama_tool).collect::<Vec<_>>());

    OllamaChatRequest {
        model: model.to_string(),
        stream,
        messages,
        options,
        tools,
    }
}

fn build_ollama_tool(def: &ToolDefinition) -> OllamaTool {
    OllamaTool {
        kind: "function",
        function: OllamaToolFunction {
            name: def.name.clone(),
            description: def.description.clone(),
            parameters: def.input_schema.clone(),
        },
    }
}

fn build_ollama_options(request: &MessageRequest) -> Option<OllamaOptions> {
    let stop = request.stop.clone().unwrap_or_default();
    let has_max_tokens = request.max_tokens > 0;
    if request.temperature.is_none()
        && request.top_p.is_none()
        && !has_max_tokens
        && stop.is_empty()
    {
        return None;
    }
    let num_predict = if has_max_tokens {
        Some(i32::try_from(request.max_tokens).unwrap_or(i32::MAX))
    } else {
        None
    };
    Some(OllamaOptions {
        temperature: request.temperature,
        top_p: request.top_p,
        num_predict,
        stop,
    })
}

/// Convert a single `InputMessage` into zero or more `OllamaChatMessage`
/// entries. A message that contains tool-result blocks is split into
/// individual `role: "tool"` messages, and any remaining text / tool-use
/// blocks are grouped into a single role-preserving message.
fn convert_single_message(message: &InputMessage, out: &mut Vec<OllamaChatMessage>) {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<OllamaOutgoingToolCall> = Vec::new();

    for block in &message.content {
        match block {
            InputContentBlock::Text { text } => {
                text_parts.push(text.clone());
            }
            InputContentBlock::ToolUse { id: _, name, input } => {
                tool_calls.push(OllamaOutgoingToolCall {
                    function: OllamaOutgoingToolCallFunction {
                        name: name.clone(),
                        arguments: input.clone(),
                    },
                });
            }
            InputContentBlock::ToolResult {
                tool_use_id: _,
                content,
                is_error: _,
            } => {
                out.push(OllamaChatMessage {
                    role: "tool".to_string(),
                    content: flatten_tool_result_content(content),
                    tool_calls: Vec::new(),
                });
            }
        }
    }

    if !text_parts.is_empty() || !tool_calls.is_empty() {
        out.push(OllamaChatMessage {
            role: message.role.clone(),
            content: text_parts.join("\n\n"),
            tool_calls,
        });
    }
}

fn flatten_tool_result_content(content: &[crate::types::ToolResultContentBlock]) -> String {
    content
        .iter()
        .map(|block| match block {
            crate::types::ToolResultContentBlock::Text { text } => text.clone(),
            crate::types::ToolResultContentBlock::Json { value } => {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------- response normalizer ----------

fn normalize_response(payload: OllamaChatResponse, model: String) -> MessageResponse {
    let mut content: Vec<OutputContentBlock> = Vec::new();
    if !payload.message.content.is_empty() {
        content.push(OutputContentBlock::Text {
            text: payload.message.content,
        });
    }
    for (idx, call) in payload.message.tool_calls.into_iter().enumerate() {
        content.push(OutputContentBlock::ToolUse {
            id: format!("ollama-call-{idx}"),
            name: call.function.name,
            input: call.function.arguments,
        });
    }
    let has_tool_calls = content
        .iter()
        .any(|block| matches!(block, OutputContentBlock::ToolUse { .. }));
    let stop_reason = if has_tool_calls {
        Some("tool_use".to_string())
    } else {
        payload.done_reason.or(Some("end_turn".to_string()))
    };
    MessageResponse {
        id: "ollama-chat".to_string(),
        kind: "message".to_string(),
        role: "assistant".to_string(),
        content,
        model,
        stop_reason,
        stop_sequence: None,
        usage: Usage {
            input_tokens: payload.prompt_eval_count.unwrap_or(0),
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            output_tokens: payload.eval_count.unwrap_or(0),
        },
        request_id: None,
    }
}

// ---------- error detection ----------

fn build_ollama_api_error(status: StatusCode, body: &str) -> ApiError {
    let message = serde_json::from_str::<OllamaErrorResponse>(body)
        .ok()
        .map(|err| err.error);
    ApiError::Api {
        status,
        error_type: Some("ollama_error".to_string()),
        message,
        request_id: None,
        body: body.to_string(),
        retryable: status.is_server_error(),
    }
}

/// Detect an `{"error": "..."}` body returned by Ollama with a 2xx status
/// (the server sometimes does this on unknown-model or unsupported-request
/// conditions). Returns `None` for normal chat responses, which do not
/// contain an `error` field.
fn check_ollama_body_error(body: &str) -> Option<ApiError> {
    let err = serde_json::from_str::<OllamaErrorResponse>(body).ok()?;
    Some(ApiError::Api {
        status: StatusCode::BAD_GATEWAY,
        error_type: Some("ollama_error".to_string()),
        message: Some(err.error),
        request_id: None,
        body: body.to_string(),
        retryable: false,
    })
}

// ---------- env / config ----------

#[must_use]
pub fn read_base_url() -> String {
    std::env::var(LOCAL_BASE_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var(OLLAMA_HOST_ENV).ok())
        .map(|value| normalize_ollama_host(&value))
        .unwrap_or_else(|| DEFAULT_OLLAMA_BASE_URL.to_string())
}

#[must_use]
pub fn local_provider_enabled() -> bool {
    std::env::var(LOCAL_PROVIDER_ENV)
        .ok()
        .is_some_and(|value| value.eq_ignore_ascii_case("ollama"))
        || std::env::var(OLLAMA_HOST_ENV).ok().is_some()
}

fn normalize_ollama_host(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_ollama_host_adds_scheme_when_missing() {
        assert_eq!(
            normalize_ollama_host("127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
    }

    #[test]
    fn normalizes_ollama_host_preserves_scheme() {
        assert_eq!(
            normalize_ollama_host("https://ollama.example.com/"),
            "https://ollama.example.com"
        );
    }

    #[test]
    fn from_model_strips_ollama_prefix() {
        let client = OllamaClient::from_model("ollama/qwen2.5-coder:7b");
        assert_eq!(client.model, "qwen2.5-coder:7b");
    }

    #[test]
    fn from_model_strips_local_prefix() {
        let client = OllamaClient::from_model("local/llama3.2");
        assert_eq!(client.model, "llama3.2");
    }

    #[test]
    fn from_model_falls_back_to_default_when_empty() {
        let client = OllamaClient::from_model("");
        assert_eq!(client.model, DEFAULT_OLLAMA_MODEL);
    }

    #[test]
    fn build_request_includes_system_prompt_as_first_message() {
        let request = MessageRequest {
            model: "llama3.2".to_string(),
            max_tokens: 100,
            messages: vec![InputMessage::user_text("hola")],
            system: Some("You are helpful".to_string()),
            ..Default::default()
        };
        let built = build_ollama_request(&request, "llama3.2", false);
        assert_eq!(built.messages.len(), 2);
        assert_eq!(built.messages[0].role, "system");
        assert_eq!(built.messages[0].content, "You are helpful");
        assert_eq!(built.messages[1].role, "user");
        assert_eq!(built.messages[1].content, "hola");
    }

    #[test]
    fn build_request_omits_system_message_when_missing() {
        let request = MessageRequest {
            model: "llama3.2".to_string(),
            max_tokens: 100,
            messages: vec![InputMessage::user_text("hola")],
            system: None,
            ..Default::default()
        };
        let built = build_ollama_request(&request, "llama3.2", false);
        assert_eq!(built.messages.len(), 1);
        assert_eq!(built.messages[0].role, "user");
    }

    #[test]
    fn build_request_forwards_tools() {
        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: Some("Read a file".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
            }),
        };
        let request = MessageRequest {
            model: "llama3.2".to_string(),
            max_tokens: 100,
            messages: vec![InputMessage::user_text("read foo")],
            tools: Some(vec![tool]),
            ..Default::default()
        };
        let built = build_ollama_request(&request, "llama3.2", false);
        let tools = built.tools.expect("tools should be forwarded");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "read_file");
    }

    #[test]
    fn build_options_returns_none_when_all_defaults() {
        let request = MessageRequest {
            model: "llama3.2".to_string(),
            max_tokens: 0,
            messages: Vec::new(),
            ..Default::default()
        };
        assert!(build_ollama_options(&request).is_none());
    }

    #[test]
    fn build_options_forwards_max_tokens_as_num_predict() {
        let request = MessageRequest {
            model: "llama3.2".to_string(),
            max_tokens: 512,
            messages: Vec::new(),
            ..Default::default()
        };
        let options = build_ollama_options(&request).expect("options should be built");
        assert_eq!(options.num_predict, Some(512));
    }

    #[test]
    fn convert_message_splits_tool_result_into_tool_role_message() {
        let message = InputMessage::user_tool_result("call-1", "file contents", false);
        let mut out = Vec::new();
        convert_single_message(&message, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, "tool");
        assert_eq!(out[0].content, "file contents");
    }

    #[test]
    fn convert_message_groups_assistant_text_and_tool_use() {
        let message = InputMessage {
            role: "assistant".to_string(),
            content: vec![
                InputContentBlock::Text {
                    text: "I'll read it".to_string(),
                },
                InputContentBlock::ToolUse {
                    id: "call-1".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "foo.rs"}),
                },
            ],
        };
        let mut out = Vec::new();
        convert_single_message(&message, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, "assistant");
        assert_eq!(out[0].content, "I'll read it");
        assert_eq!(out[0].tool_calls.len(), 1);
        assert_eq!(out[0].tool_calls[0].function.name, "read_file");
    }

    #[test]
    fn normalize_response_surfaces_tool_calls_as_tool_use_blocks() {
        let payload = OllamaChatResponse {
            message: OllamaMessage {
                content: "Reading now".to_string(),
                tool_calls: vec![OllamaIncomingToolCall {
                    function: OllamaIncomingToolCallFunction {
                        name: "read_file".to_string(),
                        arguments: serde_json::json!({"path": "foo.rs"}),
                    },
                }],
            },
            done: true,
            done_reason: None,
            prompt_eval_count: Some(10),
            eval_count: Some(5),
        };
        let normalized = normalize_response(payload, "llama3.2".to_string());
        assert_eq!(normalized.content.len(), 2);
        assert!(matches!(
            normalized.content[0],
            OutputContentBlock::Text { .. }
        ));
        assert!(matches!(
            normalized.content[1],
            OutputContentBlock::ToolUse { .. }
        ));
        assert_eq!(normalized.stop_reason, Some("tool_use".to_string()));
    }

    #[test]
    fn check_body_error_detects_ollama_error_json() {
        let body = r#"{"error": "model 'foo' not found"}"#;
        let err = check_ollama_body_error(body).expect("should detect error");
        match err {
            ApiError::Api { message, .. } => {
                assert_eq!(message, Some("model 'foo' not found".to_string()));
            }
            other => panic!("expected ApiError::Api, got {other:?}"),
        }
    }

    #[test]
    fn check_body_error_ignores_normal_chat_response() {
        let body = r#"{"message":{"content":"hi"},"done":true}"#;
        assert!(check_ollama_body_error(body).is_none());
    }
}
