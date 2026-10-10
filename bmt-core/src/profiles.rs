//! Lưu và đọc danh sách profile trong một file JSON.
//!
//! File giữ API Key ở dạng thô vì đây là công cụ chạy trên máy cá nhân; thư mục
//! `data/` nằm ngoài mã nguồn và không được commit.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::profiles_file;
use crate::params::{normalize_profile, ProfileFields, ProfileInput};

pub const SCHEMA_VERSION: u32 = 1;

/// Một profile đã lưu, kèm các trường do backend sinh ra.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    #[serde(flatten)]
    pub fields: ProfileFields,
    /// Trường lạ trong file (do bản khác ghi) được giữ nguyên khi ghi lại.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Nội dung của `profiles.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStore {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

fn default_version() -> u32 {
    SCHEMA_VERSION
}

impl Default for ProfileStore {
    fn default() -> Self {
        ProfileStore {
            version: SCHEMA_VERSION,
            updated_at: None,
            profiles: Vec::new(),
        }
    }
}

fn now() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Đọc/ghi `profiles.json` với một khóa để hai lệnh gọi cùng lúc không đụng nhau.
pub struct ProfileStoreHandle {
    path: PathBuf,
    lock: Mutex<()>,
}

impl ProfileStoreHandle {
    /// Mở file profile ở đường dẫn mặc định của ứng dụng.
    pub fn new() -> Self {
        Self::at(profiles_file())
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        ProfileStoreHandle {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        // Một lệnh gọi khác panic trong lúc giữ khóa không nên làm hỏng cả app.
        self.lock.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Toàn bộ profile đã lưu, kèm đường dẫn file.
    pub fn list(&self) -> (ProfileStore, PathBuf) {
        let _guard = self.guard();
        (read_store(&self.path), self.path.clone())
    }

    pub fn get(&self, profile_id: &str) -> Option<Profile> {
        let _guard = self.guard();
        read_store(&self.path)
            .profiles
            .into_iter()
            .find(|profile| profile.id == profile_id)
    }

    /// Thêm mới khi không có id, cập nhật khi id đã tồn tại.
    pub fn save(&self, input: &ProfileInput) -> Result<Profile, StoreError> {
        let _guard = self.guard();
        let mut store = read_store(&self.path);
        let profile_id = input.id.as_deref().unwrap_or_default().trim().to_string();
        let existing_index = store
            .profiles
            .iter()
            .position(|profile| profile.id == profile_id);

        let fields = match existing_index {
            Some(index) => normalize_profile(input, Some(&store.profiles[index].fields)),
            None => normalize_profile(input, None),
        };

        let profile = match existing_index {
            Some(index) => {
                let existing = &mut store.profiles[index];
                existing.fields = fields;
                existing.updated_at = Some(now());
                existing.clone()
            }
            None => {
                let mut fields = fields;
                if fields.name.is_empty() {
                    fields.name = if fields.base_url.is_empty() {
                        "Profile chưa đặt tên".to_string()
                    } else {
                        fields.base_url.clone()
                    };
                }
                let profile = Profile {
                    id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
                    created_at: Some(now()),
                    updated_at: Some(now()),
                    last_used_at: None,
                    fields,
                    extra: serde_json::Map::new(),
                };
                store.profiles.push(profile.clone());
                profile
            }
        };

        write_store(&self.path, &mut store)?;
        Ok(profile)
    }

    /// Xoá một profile; trả về `false` khi id không tồn tại.
    pub fn delete(&self, profile_id: &str) -> Result<bool, StoreError> {
        let _guard = self.guard();
        let mut store = read_store(&self.path);
        let before = store.profiles.len();
        store.profiles.retain(|profile| profile.id != profile_id);
        if store.profiles.len() == before {
            return Ok(false);
        }
        write_store(&self.path, &mut store)?;
        Ok(true)
    }

    /// Ghi lại thời điểm dùng gần nhất; bỏ qua khi id không còn tồn tại.
    pub fn touch(&self, profile_id: &str) -> Result<bool, StoreError> {
        let _guard = self.guard();
        let mut store = read_store(&self.path);
        let Some(profile) = store
            .profiles
            .iter_mut()
            .find(|profile| profile.id == profile_id)
        else {
            return Ok(false);
        };
        profile.last_used_at = Some(now());
        write_store(&self.path, &mut store)?;
        Ok(true)
    }
}

impl Default for ProfileStoreHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Encode(serde_json::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(error) => write!(formatter, "Không ghi được file profile: {error}"),
            StoreError::Encode(error) => write!(formatter, "Không mã hoá được profile: {error}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Đọc file; file hỏng hoặc sai định dạng thì coi như chưa có profile nào.
fn read_store(path: &Path) -> ProfileStore {
    let Ok(raw) = fs::read_to_string(path) else {
        return ProfileStore::default();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return ProfileStore::default();
    };
    if !value.is_object() || !value.get("profiles").map(Value::is_array).unwrap_or(false) {
        return ProfileStore::default();
    }
    serde_json::from_value(value).unwrap_or_default()
}

/// Ghi nguyên tử để không làm hỏng file khi tiến trình bị dừng giữa chừng.
fn write_store(path: &Path, store: &mut ProfileStore) -> Result<(), StoreError> {
    store.version = SCHEMA_VERSION;
    store.updated_at = Some(now());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(StoreError::Io)?;
    }
    let tmp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    let encoded = serde_json::to_vec_pretty(store).map_err(StoreError::Encode)?;
    let mut encoded = encoded;
    encoded.push(b'\n');
    fs::write(&tmp_path, encoded).map_err(StoreError::Io)?;
    fs::rename(&tmp_path, path).map_err(StoreError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::ProfileInput;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bmt-store-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn input(base_url: &str, name: &str) -> ProfileInput {
        ProfileInput {
            name: Some(name.to_string()),
            base_url: Some(base_url.to_string()),
            api_key: Some("jr_secret".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn saves_lists_and_deletes_profiles() {
        let path = temp_path("profiles.json");
        let store = ProfileStoreHandle::at(&path);

        let created = store.save(&input("http://127.0.0.1:8899/v1", "Mock local")).unwrap();
        assert_eq!(created.id.len(), 12);
        assert_eq!(created.fields.test_prompt, "Hello");

        let (listed, listed_path) = store.list();
        assert_eq!(listed.profiles.len(), 1);
        assert_eq!(listed_path, path);
        assert_eq!(listed.version, SCHEMA_VERSION);

        // File phải là JSON đọc được và có cùng nội dung.
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.ends_with('\n'));
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["profiles"][0]["name"], "Mock local");

        assert!(store.delete(&created.id).unwrap());
        assert!(!store.delete(&created.id).unwrap());
        assert!(store.list().0.profiles.is_empty());
    }

    #[test]
    fn update_keeps_stored_token_and_unknown_fields() {
        let path = temp_path("profiles.json");
        fs::write(
            &path,
            r#"{
              "version": 1,
              "updated_at": "2026-10-10T17:50:54",
              "profiles": [
                {
                  "id": "ffdfbbc526f3",
                  "created_at": "2026-10-10T17:31:50",
                  "name": "Jerouter",
                  "base_url": "https://je.example/v1",
                  "api_key": "jr_cup_secret",
                  "model_name": "",
                  "test_prompt": "Xin chào",
                  "timeout": 30.0,
                  "concurrency": 8,
                  "auto_detect": true,
                  "auto_start": false,
                  "notes": "",
                  "updated_at": "2026-10-10T17:31:50",
                  "last_used_at": "2026-10-10T17:50:54",
                  "so_thich": "giữ lại"
                }
              ]
            }"#,
        )
        .unwrap();

        let store = ProfileStoreHandle::at(&path);
        let updated = store
            .save(&ProfileInput {
                id: Some("ffdfbbc526f3".to_string()),
                name: Some("Jerouter".to_string()),
                base_url: Some("https://je.example/v1".to_string()),
                concurrency: Some(serde_json::json!(16)),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(updated.fields.api_key, "jr_cup_secret");
        assert_eq!(updated.fields.concurrency, 16);
        assert_eq!(updated.created_at.as_deref(), Some("2026-10-10T17:31:50"));
        assert_eq!(updated.last_used_at.as_deref(), Some("2026-10-10T17:50:54"));

        let raw = fs::read_to_string(&path).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["profiles"][0]["so_thich"], "giữ lại");
    }

    #[test]
    fn corrupt_file_is_treated_as_empty() {
        let path = temp_path("profiles.json");
        fs::write(&path, "{ not json").unwrap();
        let store = ProfileStoreHandle::at(&path);
        assert!(store.list().0.profiles.is_empty());

        fs::write(&path, "[1, 2, 3]").unwrap();
        assert!(store.list().0.profiles.is_empty());
    }

    #[test]
    fn missing_name_falls_back_to_base_url() {
        let path = temp_path("profiles.json");
        let store = ProfileStoreHandle::at(&path);
        let created = store
            .save(&ProfileInput {
                base_url: Some("https://api.example.com/v1".to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(created.fields.name, "https://api.example.com/v1");
    }

    #[test]
    fn touch_records_last_used() {
        let path = temp_path("profiles.json");
        let store = ProfileStoreHandle::at(&path);
        let created = store.save(&input("https://api.example.com/v1", "A")).unwrap();
        assert!(store.touch(&created.id).unwrap());
        assert!(!store.touch("khong-co").unwrap());
        assert!(store.get(&created.id).unwrap().last_used_at.is_some());
    }
}
