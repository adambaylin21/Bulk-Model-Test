//! Dịch vụ dùng chung cho giao diện Tauri và bản dòng lệnh.
//!
//! Mọi thao tác với profile, danh sách model và lượt chạy đều đi qua đây để lớp
//! giao diện chỉ còn việc chuyển dữ liệu sang JSON.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::api::OpenAiClient;
use crate::params::{parse_model_names, ProfileInput, RunParams};
use crate::profiles::{Profile, ProfileStoreHandle};
use crate::runner::{ChartData, Job, JobManager, Report};

pub struct Service {
    pub profiles: ProfileStoreHandle,
    jobs: &'static JobManager,
}

impl Service {
    pub fn new() -> Self {
        Service {
            profiles: ProfileStoreHandle::new(),
            jobs: JobManager::registry(),
        }
    }

    /// Dùng một file profile cụ thể; tiện cho test.
    pub fn with_store(profiles: ProfileStoreHandle) -> Self {
        Service {
            profiles,
            jobs: JobManager::registry(),
        }
    }

    pub fn profiles_path(&self) -> Value {
        json!({ "path": self.profiles.path().display().to_string() })
    }

    /// Toàn bộ profile đã lưu, kèm tên file và thời điểm cập nhật.
    pub fn list_profiles(&self) -> Value {
        let (store, path) = self.profiles.list();
        json!({
            "profiles": store.profiles,
            "count": store.profiles.len(),
            "storage": path.file_name().map(|name| name.to_string_lossy().to_string()),
            "updated_at": store.updated_at,
        })
    }

    pub fn save_profile(&self, input: ProfileInput) -> Result<Profile, String> {
        self.profiles.save(&input).map_err(|error| error.to_string())
    }

    pub fn delete_profile(&self, profile_id: &str) -> Result<bool, String> {
        self.profiles
            .delete(profile_id)
            .map_err(|error| error.to_string())
    }

    pub fn touch_profile(&self, profile_id: &str) -> Result<bool, String> {
        self.profiles
            .touch(profile_id)
            .map_err(|error| error.to_string())
    }

    pub fn profile(&self, profile_id: &str) -> Option<Profile> {
        self.profiles.get(profile_id)
    }

    /// Đếm số model người dùng nhập tay.
    pub fn parse_models(&self, raw: &str) -> Value {
        let names = parse_model_names(raw);
        json!({ "models": names, "count": names.len() })
    }

    /// Xem trước danh sách model mà API key chạm được.
    pub fn preview_models(&self, params: &RunParams) -> Result<Value, String> {
        let client = OpenAiClient::new(
            &params.trimmed_base_url(),
            &params.trimmed_api_key(),
            params.timeout(),
        )
        .map_err(|error| error.to_string())?;
        let models = client.fetch_models().map_err(|error| error.to_string())?;
        Ok(json!({ "models": models, "count": models.len() }))
    }

    /// Bắt đầu một lượt chạy và trả về id của job.
    pub fn start_run(&self, params: RunParams) -> (String, Arc<Job>) {
        let profile = params
            .profile_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .and_then(|id| self.matching_profile(id, &params.trimmed_base_url()));
        let job = self.jobs.start(params, profile);
        (job.id.clone(), job)
    }

    /// Chỉ ghi tên profile khi Base Url trùng với profile đã lưu.
    fn matching_profile(&self, profile_id: &str, base_url: &str) -> Option<Value> {
        let profile = self.profiles.get(profile_id)?;
        if profile.fields.base_url != base_url {
            return None;
        }
        let _ = self.profiles.touch(profile_id);
        Some(json!({
            "id": profile.id,
            "name": profile.fields.name,
            "base_url": profile.fields.base_url,
        }))
    }

    pub fn cancel_run(&self, job_id: &str) -> bool {
        self.jobs.cancel(job_id)
    }

    /// Tách báo cáo và dữ liệu biểu đồ ra khỏi sự kiện `done`.
    pub fn split_done(event: &Value) -> (Report, ChartData) {
        (
            serde_json::from_value(event.get("report").cloned().unwrap_or(Value::Null))
                .unwrap_or_else(|_| crate::runner::build_report(&[], None)),
            serde_json::from_value(event.get("chart_data").cloned().unwrap_or(Value::Null))
                .unwrap_or(ChartData {
                    labels: Vec::new(),
                    values: Vec::new(),
                    colors: Vec::new(),
                }),
        )
    }
}

impl Default for Service {
    fn default() -> Self {
        Self::new()
    }
}
