//! Đường dẫn và hằng số dùng chung.
//!
//! Chạy từ mã nguồn (CLI, test) thì đọc `data/` ngay trong thư mục dự án. Khi
//! đã đóng gói thì bundle chỉ đọc, nên dữ liệu người dùng phải nằm ở thư mục
//! ghi được lâu dài của hệ điều hành.

use std::path::PathBuf;

pub const APP_NAME: &str = "Bulk Model Test";

/// Số model chạy song song mặc định.
pub const MAX_CONCURRENCY: usize = 8;
/// Trần cứng của backend, thanh trượt trên giao diện chỉ cho tới 32.
pub const MAX_CONCURRENCY_CEILING: usize = 64;
pub const DEFAULT_TIMEOUT: f64 = 30.0;
pub const DEFAULT_PROMPT: &str = "Hello";
pub const MIN_TIMEOUT: f64 = 1.0;
pub const MAX_TIMEOUT: f64 = 600.0;

/// Model trả lời được `/v1/models` nhưng không phục vụ chat completion.
pub const NON_CHAT_PREFIXES: &[&str] = &[
    "embedding",
    "text-embedding",
    "whisper",
    "tts-",
    "dall-e",
    "moderation",
    "rerank",
    "stable-diffusion",
    "text-to-image",
    "guard",
    "audio",
    "realtime",
    "instruct-embed",
    "vision-embed",
];

/// `true` khi model không phục vụ chat completion và bị bỏ khỏi danh sách.
pub fn is_non_chat_model(model: &str) -> bool {
    let lowered = model.to_ascii_lowercase();
    NON_CHAT_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
}

/// Thư mục dữ liệu lâu dài. `BMT_DATA_DIR` được ưu tiên khi có.
pub fn data_dir() -> PathBuf {
    if let Some(override_dir) = std::env::var_os("BMT_DATA_DIR") {
        let path = PathBuf::from(override_dir);
        if !path.as_os_str().is_empty() {
            return path;
        }
    }
    if let Some(dir) = dirs_style_data_dir() {
        return dir;
    }
    std::env::temp_dir().join("bulk-model-test")
}

/// Thư mục dữ liệu của hệ điều hành, giữ tên cũ của bản Python để profile đã
/// lưu vẫn dùng được.
fn dirs_style_data_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME")?;
        return Some(PathBuf::from(home).join("Library/Application Support").join(APP_NAME));
    }
    if cfg!(target_os = "windows") {
        let base = std::env::var_os("APPDATA")?;
        return Some(PathBuf::from(base).join(APP_NAME));
    }
    if let Some(base) = std::env::var_os("XDG_DATA_HOME") {
        return Some(PathBuf::from(base).join("bulk-model-test"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local/share/bulk-model-test"))
}

/// File profile: `profiles.json` trong thư mục dữ liệu.
pub fn profiles_file() -> PathBuf {
    data_dir().join("profiles.json")
}

/// Nơi ghi dữ liệu lâu dài khi chạy từ mã nguồn (thư mục `data/` của dự án).
/// `None` khi thư mục hiện tại không phải gốc dự án.
pub fn project_data_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    if cwd.join("bmt-core").is_dir() && cwd.join("Cargo.toml").is_file() {
        return Some(cwd.join("data"));
    }
    None
}
