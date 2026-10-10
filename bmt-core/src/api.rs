//! Gọi endpoint OpenAI-compatible: liệt kê model và đo thời gian một chat completion.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::is_non_chat_model;

/// Một lượt gọi API hỏng.
#[derive(Debug)]
pub enum ApiError {
    /// Không gọi được endpoint (mạng, DNS, TLS…).
    Transport(reqwest::Error),
    /// Endpoint trả về mã lỗi HTTP.
    Status { status: u16, body: String },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Transport(error) => write!(formatter, "Không gọi được API: {error}"),
            ApiError::Status { status, body } => {
                let detail = error_message(body);
                if detail.is_empty() {
                    write!(formatter, "HTTP {status}")
                } else {
                    write!(formatter, "HTTP {status}: {detail}")
                }
            }
        }
    }
}

impl std::error::Error for ApiError {}

/// Lấy `error.message` trong body lỗi kiểu OpenAI, nếu có.
fn error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
                .map(str::to_string)
                .or_else(|| value.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .or_else(|| {
                    value
                        .get("message")
                        .and_then(|message| message.as_str())
                        .map(str::to_string)
                })
        })
        .unwrap_or_else(|| body.trim().chars().take(200).collect())
}

/// Client tối thiểu cho endpoint tương thích OpenAI SDK.
pub struct OpenAiClient {
    http: reqwest::blocking::Client,
    base_url: String,
    api_key: String,
}

impl OpenAiClient {
    /// `timeout` là số giây chờ cho mỗi lần gọi.
    pub fn new(base_url: &str, api_key: &str, timeout: f64) -> Result<Self, ApiError> {
        let http = build_client(timeout).map_err(ApiError::Transport)?;
        Ok(OpenAiClient {
            http,
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            api_key: api_key.trim().to_string(),
        })
    }

    /// Các model phục vụ chat mà endpoint công bố.
    pub fn fetch_models(&self) -> Result<Vec<String>, ApiError> {
        let response = self
            .http
            .get(format!("{}/models", self.base_url))
            .headers(headers(&self.api_key))
            .send()
            .map_err(ApiError::Transport)?;

        let status = response.status();
        let body = response.text().map_err(ApiError::Transport)?;
        if !status.is_success() {
            return Err(ApiError::Status {
                status: status.as_u16(),
                body,
            });
        }

        let payload: ModelsResponse = serde_json::from_str(&body).map_err(|error| ApiError::Status {
            status: status.as_u16(),
            body: format!("Danh sách model không đúng định dạng: {error}"),
        })?;

        let mut names: Vec<String> = payload
            .data
            .into_iter()
            .filter_map(|model| model.id)
            .filter(|id| !id.is_empty() && !is_non_chat_model(id))
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Gửi một chat completion tối thiểu và đo thời gian khứ hồi.
    pub fn chat_once(&self, model: &str, prompt: &str) -> Result<ChatOutcome, ApiError> {
        let started = Instant::now();
        let body = json!({
            "model": model,
            "messages": [{ "role": "user", "content": prompt }],
            "max_tokens": 16,
            "temperature": 0,
        });

        let response = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .headers(headers(&self.api_key))
            .json(&body)
            .send()
            .map_err(ApiError::Transport)?;

        let status = response.status();
        let text = response.text().map_err(ApiError::Transport)?;
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

        if !status.is_success() {
            return Ok(ChatOutcome {
                response_time_ms: elapsed_ms,
                content: None,
                error: Some(ApiError::Status {
                    status: status.as_u16(),
                    body: text,
                }
                .to_string()),
            });
        }

        let payload: ChatResponse = match serde_json::from_str(&text) {
            Ok(payload) => payload,
            // Body hỏng chỉ là lỗi của riêng model này, không phải lỗi cả lượt chạy.
            Err(error) => {
                return Ok(ChatOutcome {
                    response_time_ms: elapsed_ms,
                    content: None,
                    error: Some(format!("Phản hồi không đúng định dạng: {error}")),
                })
            }
        };

        // Model có thể trả `reasoning` trước khi trả `content`; lấy trường nào có nội dung.
        let content = payload
            .choices
            .into_iter()
            .find_map(|choice| match choice.message {
                Some(message) => message
                    .content
                    .filter(|text| !text.is_empty())
                    .or(message.reasoning),
                None => choice.text,
            })
            .unwrap_or_default();

        Ok(ChatOutcome {
            response_time_ms: elapsed_ms,
            content: Some(content),
            error: None,
        })
    }
}

/// Kết quả một lần gọi model: HTTP lỗi vẫn là kết quả, chỉ lỗi mạng mới là `Err`.
#[derive(Debug, Clone)]
pub struct ChatOutcome {
    pub response_time_ms: f64,
    pub content: Option<String>,
    pub error: Option<String>,
}

impl ChatOutcome {
    pub fn succeeded(&self) -> bool {
        self.error.is_none()
    }
}

fn build_client(timeout: f64) -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs_f64(timeout.max(0.1)))
        .connect_timeout(Duration::from_secs_f64(timeout.max(0.1).min(30.0)))
        .user_agent(user_agent())
        .build()
}

/// Các header giống SDK: `Authorization` khi có token, `Content-Type` cho JSON.
fn headers(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    if !api_key.is_empty() {
        if let Ok(value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}")) {
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
    }
    headers
}

fn user_agent() -> String {
    static USER_AGENT: OnceLock<String> = OnceLock::new();
    USER_AGENT
        .get_or_init(|| format!("{}/{}", crate::config::APP_NAME, env!("CARGO_PKG_VERSION")))
        .clone()
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatMessage>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_openai_error_message() {
        let body = r#"{"error":{"message":"Invalid API key","type":"invalid_request_error"}}"#;
        assert_eq!(error_message(body), "Invalid API key");
        assert_eq!(error_message(r#"{"message":"Thiếu token"}"#), "Thiếu token");
        assert_eq!(error_message("  server sập  "), "server sập");
    }

    #[test]
    fn sends_auth_header_only_with_token() {
        assert!(headers("sk-test").contains_key(reqwest::header::AUTHORIZATION));
        assert!(!headers("").contains_key(reqwest::header::AUTHORIZATION));
    }

    #[test]
    fn strips_trailing_slash_from_base_url() {
        let client = OpenAiClient::new("https://api.example.com/v1/", "k", 5.0).unwrap();
        assert_eq!(client.base_url, "https://api.example.com/v1");
    }
}
