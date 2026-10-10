//! Chạy một lượt bulk test và phát tiến độ ngay khi có kết quả.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Instant;

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::OpenAiClient;
use crate::params::RunParams;

/// Kết quả của một model trong lượt chạy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResult {
    pub model: String,
    pub response_time_ms: f64,
    /// `success` hoặc `error`.
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub test_result: Option<String>,
}

impl ModelResult {
    pub fn succeeded(&self) -> bool {
        self.status == "success"
    }

    fn from_error(model: &str, error: String, elapsed_ms: f64) -> Self {
        ModelResult {
            model: model.to_string(),
            response_time_ms: round2(elapsed_ms),
            status: "error".to_string(),
            error: Some(error),
            test_result: None,
        }
    }
}

/// Một sự kiện tiến độ gửi tới giao diện.
pub type Event = Value;

/// Tóm tắt một lượt chạy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub total_models: usize,
    pub successful: usize,
    pub errors: usize,
    pub avg_response_time_ms: f64,
    pub fastest_ms: f64,
    pub slowest_ms: f64,
    pub timestamp: String,
    pub profile: Option<Value>,
    pub models: Vec<ReportModel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportModel {
    pub name: String,
    pub response_time_ms: f64,
    pub status: String,
    pub error: Option<String>,
    pub test_result: Option<String>,
}

/// Chuỗi dữ liệu cho biểu đồ cột thời gian phản hồi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartData {
    pub labels: Vec<String>,
    pub values: Vec<f64>,
    pub colors: Vec<String>,
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Model thành công nhanh nhất lên trước, model lỗi xuống cuối.
pub fn sort_results(mut results: Vec<ModelResult>) -> Vec<ModelResult> {
    results.sort_by(|left, right| {
        let left_failed = !left.succeeded();
        let right_failed = !right.succeeded();
        left_failed
            .cmp(&right_failed)
            .then_with(|| {
                left.response_time_ms
                    .partial_cmp(&right.response_time_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.model.cmp(&right.model))
    });
    results
}

/// Tóm tắt một lượt chạy đã xếp thứ tự.
pub fn build_report(results: &[ModelResult], profile: Option<Value>) -> Report {
    let times: Vec<f64> = results
        .iter()
        .filter(|result| result.succeeded())
        .map(|result| result.response_time_ms)
        .collect();
    let fastest = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let slowest = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    Report {
        total_models: results.len(),
        successful: results.iter().filter(|r| r.succeeded()).count(),
        errors: results.iter().filter(|r| !r.succeeded()).count(),
        avg_response_time_ms: if times.is_empty() {
            0.0
        } else {
            round2(times.iter().sum::<f64>() / times.len() as f64)
        },
        fastest_ms: if times.is_empty() { 0.0 } else { fastest },
        slowest_ms: if times.is_empty() { 0.0 } else { slowest },
        timestamp: Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        profile,
        models: results
            .iter()
            .map(|result| ReportModel {
                name: result.model.clone(),
                response_time_ms: result.response_time_ms,
                status: result.status.clone(),
                error: result.error.clone(),
                test_result: result.test_result.clone(),
            })
            .collect(),
    }
}

/// Chuỗi dữ liệu biểu đồ: xanh là thành công, đỏ là lỗi.
pub fn build_chart_data(results: &[ModelResult]) -> ChartData {
    ChartData {
        labels: results.iter().map(|r| r.model.clone()).collect(),
        values: results.iter().map(|r| r.response_time_ms).collect(),
        colors: results
            .iter()
            .map(|r| {
                if r.succeeded() {
                    "#1f9d55".to_string()
                } else {
                    "#d93a2b".to_string()
                }
            })
            .collect(),
    }
}

/// Một lượt chạy đang diễn ra: nguồn sự kiện và cờ huỷ.
///
/// Bên đọc luôn giữ được đầu nhận, nên job kết thúc bằng cách đóng đầu gửi;
/// sự kiện còn trong hàng đợi vẫn đọc được trước khi `recv` trả về `None`.
pub struct Job {
    pub id: String,
    pub total: AtomicUsize,
    cancel: AtomicBool,
    events: Mutex<Option<crossbeam_channel::Sender<Event>>>,
    receiver: crossbeam_channel::Receiver<Event>,
}

impl Job {
    fn new(id: String) -> Self {
        let (events, receiver) = crossbeam_channel::unbounded();
        Job {
            id,
            total: AtomicUsize::new(0),
            cancel: AtomicBool::new(false),
            events: Mutex::new(Some(events)),
            receiver,
        }
    }

    /// Sự kiện kế tiếp; `None` khi job đã phát xong sự kiện cuối.
    pub fn recv(&self) -> Option<Event> {
        self.receiver.recv().ok()
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.receiver.try_recv().ok()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    fn emit(&self, payload: Event) {
        let sender = self
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(sender) = sender {
            let _ = sender.send(payload);
        }
    }

    /// Kết thúc job: đóng đầu gửi để bên đọc biết đã hết sự kiện.
    fn finish(&self) {
        self.events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
}

/// Bảng các job đang chạy, dùng chung cho giao diện và bản dòng lệnh.
pub struct JobManager {
    jobs: Mutex<Vec<Arc<Job>>>,
}

impl JobManager {
    pub fn new() -> Self {
        JobManager {
            jobs: Mutex::new(Vec::new()),
        }
    }

    /// Bảng job dùng chung cho cả tiến trình.
    pub fn registry() -> &'static JobManager {
        static REGISTRY: OnceLock<JobManager> = OnceLock::new();
        REGISTRY.get_or_init(JobManager::new)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Arc<Job>>> {
        self.jobs.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Bắt đầu một lượt chạy ở luồng nền và trả về job để đọc tiến độ.
    pub fn start(&self, params: RunParams, profile: Option<Value>) -> Arc<Job> {
        let job = Arc::new(Job::new(uuid::Uuid::new_v4().simple().to_string()));
        {
            let mut jobs = self.lock();
            // Giữ lại vài job gần nhất để job đã xong không tích lại mãi.
            if jobs.len() >= 8 {
                let done = jobs.len() - 4;
                jobs.drain(..done);
            }
            jobs.push(Arc::clone(&job));
        }

        // Tất cả job dùng cùng một client nên model đông vẫn chạy song song.
        let runner = Arc::clone(&job);
        thread::spawn(move || {
            run_job(&runner, &params, profile);
            runner.finish();
        });

        job
    }

    pub fn get(&self, job_id: &str) -> Option<Arc<Job>> {
        self.lock()
            .iter()
            .find(|job| job.id == job_id)
            .map(Arc::clone)
    }

    /// Dừng job theo id; không có id thì dừng job mới nhất.
    pub fn cancel(&self, job_id: &str) -> bool {
        let jobs = self.lock();
        let found = if job_id.is_empty() {
            jobs.last().map(Arc::clone)
        } else {
            jobs.iter().find(|job| job.id == job_id).map(Arc::clone)
        };
        match found {
            Some(job) => {
                job.cancel();
                true
            }
            None => false,
        }
    }

    pub fn latest_id(&self) -> Option<String> {
        self.lock().last().map(|job| job.id.clone())
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Chạy mọi model trong một worker pool, phát tiến độ ngay khi có.
pub fn run_job(job: &Job, params: &RunParams, profile: Option<Value>) {
    let base_url = params.trimmed_base_url();
    let api_key = params.trimmed_api_key();
    let prompt = params.prompt();
    let timeout = params.timeout();
    let concurrency = params.concurrency();
    let model_filter = params.models();

    let client = match OpenAiClient::new(&base_url, &api_key, timeout) {
        Ok(client) => client,
        Err(error) => {
            job.emit(json!({"event": "error", "message": error.to_string()}));
            return;
        }
    };

    let models = if !model_filter.is_empty() {
        job.emit(json!({"event": "models", "models": model_filter, "source": "manual"}));
        model_filter
    } else if params.auto_detect {
        job.emit(json!({"event": "status", "message": "Đang tải danh sách model..."}));
        match client.fetch_models() {
            Ok(models) => models,
            Err(error) => {
                job.emit(json!({
                    "event": "error",
                    "message": format!("Không lấy được danh sách model: {error}"),
                }));
                return;
            }
        }
    } else {
        job.emit(json!({
            "event": "error",
            "message": "Cần nhập tên model hoặc bật Auto Detect.",
        }));
        return;
    };

    if models.is_empty() {
        job.emit(json!({"event": "error", "message": "API không trả về model nào dùng được."}));
        return;
    }

    let total = models.len();
    let workers = concurrency.min(total);
    job.total.store(total, Ordering::SeqCst);
    job.emit(json!({"event": "start", "total": total, "concurrency": workers}));

    let results: Mutex<Vec<ModelResult>> = Mutex::new(Vec::new());
    let next_index = AtomicUsize::new(0);

    let worker = || {
        loop {
            let index = next_index.fetch_add(1, Ordering::SeqCst);
            if index >= total || job.is_cancelled() {
                return;
            }
            let model = &models[index];
            job.emit(json!({"event": "model_start", "model": model}));

            let started = Instant::now();
            let result = match client.chat_once(model, &prompt) {
                Ok(outcome) => ModelResult {
                    model: model.clone(),
                    response_time_ms: round2(outcome.response_time_ms),
                    status: if outcome.succeeded() { "success" } else { "error" }.to_string(),
                    error: outcome.error,
                    test_result: outcome.content.map(|content| truncate(&content, 120)),
                },
                Err(error) => ModelResult::from_error(
                    model,
                    error.to_string(),
                    started.elapsed().as_secs_f64() * 1000.0,
                ),
            };

            let completed = {
                let mut guard = results.lock().unwrap_or_else(|error| error.into_inner());
                guard.push(result.clone());
                guard.len()
            };

            job.emit(json!({
                "event": "model_done",
                "model": model,
                "result": serde_json::to_value(&result).unwrap_or(Value::Null),
                "completed": completed,
                "total": total,
            }));
        }
    };

    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(worker);
        }
    });

    let results = results.into_inner().unwrap_or_else(|error| error.into_inner());
    if job.is_cancelled() {
        job.emit(json!({"event": "cancelled", "completed": results.len()}));
        return;
    }

    let ordered = sort_results(results);
    let report = build_report(&ordered, profile);
    job.emit(json!({
        "event": "done",
        "report": serde_json::to_value(&report).unwrap_or(Value::Null),
        "chart_data": serde_json::to_value(build_chart_data(&ordered)).unwrap_or(Value::Null),
    }));
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Chạy đồng bộ tới khi xong; dùng cho bản dòng lệnh.
pub fn run_bulk_blocking(
    params: &RunParams,
    profile: Option<Value>,
) -> Result<(Report, ChartData), String> {
    let job = JobManager::new().start(params.clone(), profile);

    let mut outcome = None;
    while let Some(event) = job.recv() {
        match event.get("event").and_then(Value::as_str) {
            Some("done") => {
                let report: Report =
                    serde_json::from_value(event.get("report").cloned().unwrap_or(Value::Null))
                        .map_err(|error| error.to_string())?;
                let chart: ChartData = serde_json::from_value(
                    event.get("chart_data").cloned().unwrap_or(Value::Null),
                )
                .map_err(|error| error.to_string())?;
                outcome = Some(Ok((report, chart)));
            }
            Some("error") => {
                outcome = Some(Err(event
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Lượt chạy thất bại.")
                    .to_string()));
            }
            Some("cancelled") => {
                outcome = Some(Err("Lượt chạy đã bị dừng.".to_string()));
            }
            _ => {}
        }
    }

    // `Option<Result<…>>` do vòng lặp tạo ra: chưa có kết quả nghĩa là lỗi.
    outcome.unwrap_or_else(|| Err("Lượt chạy kết thúc mà không có kết quả.".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(model: &str, ms: f64, status: &str) -> ModelResult {
        ModelResult {
            model: model.to_string(),
            response_time_ms: ms,
            status: status.to_string(),
            error: if status == "error" { Some("boom".into()) } else { None },
            test_result: if status == "success" { Some("chào".into()) } else { None },
        }
    }

    #[test]
    fn sorts_success_by_time_then_errors() {
        let ordered = sort_results(vec![
            result("b", 300.0, "error"),
            result("a", 200.0, "success"),
            result("c", 100.0, "success"),
            result("d", 50.0, "error"),
        ]);
        let names: Vec<&str> = ordered.iter().map(|r| r.model.as_str()).collect();
        assert_eq!(names, vec!["c", "a", "d", "b"]);
    }

    #[test]
    fn report_summarises_successful_calls() {
        let ordered = sort_results(vec![
            result("a", 200.0, "success"),
            result("b", 100.0, "success"),
            result("c", 700.0, "error"),
        ]);
        let report = build_report(&ordered, None);

        assert_eq!(report.total_models, 3);
        assert_eq!(report.successful, 2);
        assert_eq!(report.errors, 1);
        assert_eq!(report.avg_response_time_ms, 150.0);
        assert_eq!(report.fastest_ms, 100.0);
        assert_eq!(report.slowest_ms, 200.0);
        assert_eq!(report.models[0].name, "b");
        assert!(!report.timestamp.is_empty());
    }

    #[test]
    fn report_without_success_has_zero_times() {
        let report = build_report(&[result("a", 700.0, "error")], None);
        assert_eq!(report.avg_response_time_ms, 0.0);
        assert_eq!(report.fastest_ms, 0.0);
        assert_eq!(report.slowest_ms, 0.0);
    }

    #[test]
    fn chart_colours_success_green_and_error_red() {
        let chart = build_chart_data(&[result("a", 10.0, "success"), result("b", 20.0, "error")]);
        assert_eq!(chart.labels, vec!["a", "b"]);
        assert_eq!(chart.values, vec![10.0, 20.0]);
        assert_eq!(chart.colors, vec!["#1f9d55", "#d93a2b"]);
    }

    #[test]
    fn truncates_long_answers() {
        let long = "x".repeat(300);
        assert_eq!(truncate(&long, 120).chars().count(), 120);
    }

    #[test]
    fn cancel_marks_job_and_skips_missing_ids() {
        let manager = JobManager::new();
        let job = manager.start(
            RunParams {
                base_url: "http://127.0.0.1:1/v1".into(),
                api_key: "k".into(),
                model_name: Some("manually".into()),
                ..Default::default()
            },
            None,
        );

        assert!(manager.get(&job.id).is_some());
        assert!(manager.cancel(&job.id));
        assert!(job.is_cancelled());
        assert!(!manager.cancel("khong-ton-tai"));
    }
}
