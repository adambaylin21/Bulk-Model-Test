//! Chuẩn hoá tham số chạy test và dữ liệu profile.

use serde::{Deserialize, Serialize};

use crate::config::{
    DEFAULT_PROMPT, DEFAULT_TIMEOUT, MAX_CONCURRENCY, MAX_CONCURRENCY_CEILING, MAX_TIMEOUT,
    MIN_TIMEOUT,
};

/// Các trường của một profile.
pub const PROFILE_FIELDS: &[&str] = &[
    "name",
    "base_url",
    "api_key",
    "model_name",
    "test_prompt",
    "timeout",
    "concurrency",
    "auto_detect",
    "auto_start",
    "notes",
];

/// Tham số của một lượt chạy, đã kiểm tra và chuẩn hoá.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunParams {
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub test_prompt: Option<String>,
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub timeout: Option<f64>,
    #[serde(default)]
    pub concurrency: Option<usize>,
    #[serde(default)]
    pub auto_detect: bool,
    #[serde(default)]
    pub profile_id: Option<String>,
}

impl RunParams {
    pub fn prompt(&self) -> String {
        match self.test_prompt.as_deref() {
            Some(prompt) => trimmed_or(prompt.to_string(), DEFAULT_PROMPT),
            None => DEFAULT_PROMPT.to_string(),
        }
    }

    pub fn timeout(&self) -> f64 {
        match self.timeout {
            Some(timeout) if timeout.is_finite() => timeout.clamp(MIN_TIMEOUT, MAX_TIMEOUT),
            _ => DEFAULT_TIMEOUT,
        }
    }

    pub fn concurrency(&self) -> usize {
        self.concurrency
            .map(|concurrency| concurrency.clamp(1, MAX_CONCURRENCY_CEILING))
            .unwrap_or(MAX_CONCURRENCY)
    }

    pub fn models(&self) -> Vec<String> {
        parse_model_names(self.model_name.as_deref().unwrap_or_default())
    }

    pub fn trimmed_base_url(&self) -> String {
        self.base_url.trim().to_string()
    }

    pub fn trimmed_api_key(&self) -> String {
        self.api_key.trim().to_string()
    }
}

/// Tham số chạy kiểu JSON từ giao diện, chấp nhận cả số dạng chuỗi.
#[derive(Debug, Default, Deserialize)]
pub struct RunParamsInput {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub test_prompt: Option<String>,
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub timeout: Option<serde_json::Value>,
    #[serde(default)]
    pub concurrency: Option<serde_json::Value>,
    #[serde(default)]
    pub auto_detect: Option<bool>,
    #[serde(default)]
    pub profile_id: Option<String>,
}

impl From<RunParamsInput> for RunParams {
    fn from(input: RunParamsInput) -> Self {
        RunParams {
            base_url: input.base_url.unwrap_or_default(),
            api_key: input.api_key.unwrap_or_default(),
            test_prompt: input.test_prompt,
            model_name: input.model_name,
            timeout: Some(clamp_float(
                input.timeout.as_ref(),
                DEFAULT_TIMEOUT,
                MIN_TIMEOUT,
                MAX_TIMEOUT,
            )),
            concurrency: Some(clamp_int(
                input.concurrency.as_ref(),
                MAX_CONCURRENCY,
                1,
                MAX_CONCURRENCY_CEILING,
            )),
            auto_detect: input.auto_detect.unwrap_or(false),
            profile_id: input.profile_id,
        }
    }
}

/// Profile đã chuẩn hoá, sẵn sàng để ghi vào `profiles.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileFields {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub test_prompt: String,
    pub timeout: f64,
    pub concurrency: usize,
    pub auto_detect: bool,
    pub auto_start: bool,
    pub notes: String,
}

/// Dữ liệu profile gửi từ giao diện lên.
#[derive(Debug, Default, Deserialize)]
pub struct ProfileInput {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub test_prompt: Option<String>,
    #[serde(default)]
    pub timeout: Option<serde_json::Value>,
    #[serde(default)]
    pub concurrency: Option<serde_json::Value>,
    #[serde(default)]
    pub auto_detect: Option<bool>,
    #[serde(default)]
    pub auto_start: Option<bool>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Tách danh sách model người dùng nhập theo dòng, dấu phẩy.
pub fn parse_model_names(raw: &str) -> Vec<String> {
    raw.lines()
        .flat_map(|line| line.split(','))
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

/// Bỏ khoảng trắng thừa; chuỗi rỗng trả về `None`.
fn non_empty(value: Option<&str>) -> Option<String> {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Bỏ khoảng trắng thừa; chuỗi rỗng trả về giá trị mặc định.
fn trimmed_or(value: String, default: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Ép một giá trị JSON về `f64` trong khoảng cho phép, sai kiểu thì lấy mặc định.
pub fn clamp_float(value: Option<&serde_json::Value>, default: f64, low: f64, high: f64) -> f64 {
    let number = match value {
        Some(serde_json::Value::Number(number)) => number.as_f64(),
        Some(serde_json::Value::String(text)) => text.trim().parse::<f64>().ok(),
        _ => None,
    };
    match number {
        Some(number) if number.is_finite() => number.clamp(low, high),
        _ => default,
    }
}

/// Ép một giá trị JSON về `usize` trong khoảng cho phép, sai kiểu thì lấy mặc định.
pub fn clamp_int(
    value: Option<&serde_json::Value>,
    default: usize,
    low: usize,
    high: usize,
) -> usize {
    let number = match value {
        Some(serde_json::Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|float| float as i64)),
        Some(serde_json::Value::String(text)) => text.trim().parse::<i64>().ok(),
        _ => None,
    };
    match number {
        Some(number) => (number.max(low as i64).min(high as i64)) as usize,
        None => default,
    }
}

/// Chuẩn hoá một profile; giữ API Key đã lưu khi giao diện không gửi lại.
pub fn normalize_profile(input: &ProfileInput, existing: Option<&ProfileFields>) -> ProfileFields {
    fn pick(input: Option<&String>, existing: Option<&String>, default: &str) -> String {
        match input {
            Some(value) => value.clone(),
            None => existing.cloned().unwrap_or_else(|| default.to_string()),
        }
    }

    let base = existing;
    let api_key = non_empty(input.api_key.as_deref())
        .or_else(|| base.map(|profile| profile.api_key.clone()))
        .unwrap_or_default();
    let test_prompt = match input.test_prompt.as_ref() {
        Some(value) => trimmed_or(value.clone(), DEFAULT_PROMPT),
        None => base
            .map(|profile| profile.test_prompt.clone())
            .unwrap_or_else(|| DEFAULT_PROMPT.to_string()),
    };
    let timeout = match input.timeout.as_ref() {
        Some(value) => clamp_float(Some(value), DEFAULT_TIMEOUT, MIN_TIMEOUT, MAX_TIMEOUT),
        None => base.map(|profile| profile.timeout).unwrap_or(DEFAULT_TIMEOUT),
    };
    let concurrency = match input.concurrency.as_ref() {
        Some(value) => clamp_int(Some(value), MAX_CONCURRENCY, 1, MAX_CONCURRENCY_CEILING),
        None => base
            .map(|profile| profile.concurrency)
            .unwrap_or(MAX_CONCURRENCY),
    };

    ProfileFields {
        name: pick(input.name.as_ref(), base.map(|p| &p.name), "")
            .trim()
            .to_string(),
        base_url: pick(input.base_url.as_ref(), base.map(|p| &p.base_url), "")
            .trim()
            .to_string(),
        api_key,
        model_name: pick(input.model_name.as_ref(), base.map(|p| &p.model_name), "")
            .trim()
            .to_string(),
        test_prompt,
        timeout,
        concurrency,
        auto_detect: input
            .auto_detect
            .or_else(|| base.map(|p| p.auto_detect))
            .unwrap_or(true),
        auto_start: input
            .auto_start
            .or_else(|| base.map(|p| p.auto_start))
            .unwrap_or(false),
        notes: pick(input.notes.as_ref(), base.map(|p| &p.notes), "")
            .trim()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splits_model_names_on_lines_and_commas() {
        let names = parse_model_names("gpt-4, llama-3.1-70b\n\n  mistral ,\nqwen2.5\n");
        assert_eq!(
            names,
            vec!["gpt-4", "llama-3.1-70b", "mistral", "qwen2.5"]
        );
        assert!(parse_model_names("  \n , ").is_empty());
    }

    #[test]
    fn clamps_values_and_falls_back_on_garbage() {
        assert_eq!(clamp_float(Some(&json!(0.2)), 30.0, 1.0, 600.0), 1.0);
        assert_eq!(clamp_float(Some(&json!(9999)), 30.0, 1.0, 600.0), 600.0);
        assert_eq!(clamp_float(Some(&json!("12.5")), 30.0, 1.0, 600.0), 12.5);
        assert_eq!(clamp_float(Some(&json!("abc")), 30.0, 1.0, 600.0), 30.0);
        assert_eq!(clamp_int(Some(&json!(0)), 8, 1, 64), 1);
        assert_eq!(clamp_int(Some(&json!(500)), 8, 1, 64), 64);
        assert_eq!(clamp_int(None, 8, 1, 64), 8);
    }

    #[test]
    fn run_params_apply_defaults() {
        let params: RunParams = RunParamsInput {
            base_url: Some(" https://api.example.com/v1 ".into()),
            api_key: Some(" sk-test ".into()),
            test_prompt: Some("   ".into()),
            ..Default::default()
        }
        .into();

        assert_eq!(params.trimmed_base_url(), "https://api.example.com/v1");
        assert_eq!(params.trimmed_api_key(), "sk-test");
        assert_eq!(params.prompt(), "Hello");
        assert_eq!(params.timeout(), 30.0);
        assert_eq!(params.concurrency(), 8);
        assert!(!params.auto_detect);
    }

    #[test]
    fn run_params_accept_string_numbers() {
        let params: RunParams = RunParamsInput {
            timeout: Some(json!("5")),
            concurrency: Some(json!("16")),
            ..Default::default()
        }
        .into();

        assert_eq!(params.timeout(), 5.0);
        assert_eq!(params.concurrency(), 16);
    }

    #[test]
    fn normalize_keeps_stored_api_key() {
        let existing = ProfileFields {
            name: "Jerouter".into(),
            base_url: "https://je.example/v1".into(),
            api_key: "jr_secret".into(),
            model_name: "gpt-4".into(),
            test_prompt: "Xin chào".into(),
            timeout: 45.0,
            concurrency: 4,
            auto_detect: false,
            auto_start: true,
            notes: "key của team".into(),
        };

        let input = ProfileInput {
            base_url: Some("https://je.example/v1".into()),
            ..Default::default()
        };
        let updated = normalize_profile(&input, Some(&existing));

        assert_eq!(updated.api_key, "jr_secret");
        assert_eq!(updated.timeout, 45.0);
        assert_eq!(updated.concurrency, 4);
        assert_eq!(updated.model_name, "gpt-4");
        assert!(!updated.auto_detect);
        assert!(updated.auto_start);
        assert_eq!(updated.notes, "key của team");
    }

    #[test]
    fn normalize_fills_defaults_for_new_profile() {
        let input = ProfileInput {
            base_url: Some(" https://api.example.com/v1 ".into()),
            ..Default::default()
        };
        let profile = normalize_profile(&input, None);

        assert_eq!(profile.base_url, "https://api.example.com/v1");
        assert_eq!(profile.test_prompt, "Hello");
        assert_eq!(profile.timeout, 30.0);
        assert_eq!(profile.concurrency, 8);
        assert!(profile.auto_detect);
        assert!(!profile.auto_start);
    }
}
