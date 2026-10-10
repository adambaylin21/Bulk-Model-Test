//! Integration test: chạy `bmt-core` với một server OpenAI-compatible giả.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bmt_core::api::OpenAiClient;
use bmt_core::params::RunParams;
use bmt_core::runner::{run_bulk_blocking, JobManager};
use serde_json::{json, Value};
use tiny_http::{Header, Response, Server};

/// Server giả: `/v1/models` và `/v1/chat/completions` như một endpoint thật.
struct MockApi {
    base_url: String,
    chat_calls: Arc<AtomicUsize>,
    auth_headers: Arc<Mutex<Vec<String>>>,
}

impl MockApi {
    fn start() -> Self {
        let server = Server::http("127.0.0.1:0").expect("không mở được cổng test");
        let port = server
            .server_addr()
            .to_ip()
            .expect("thiếu địa chỉ IP")
            .port();
        let chat_calls = Arc::new(AtomicUsize::new(0));
        let auth_headers: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let calls = Arc::clone(&chat_calls);
        let seen_auth = Arc::clone(&auth_headers);
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                // Mỗi yêu cầu một luồng: nếu xử lý tuần tự thì model chậm sẽ chặn
                // các model khác và test về chạy song song không còn ý nghĩa.
                let calls = Arc::clone(&calls);
                let seen_auth = Arc::clone(&seen_auth);
                std::thread::spawn(move || handle_request(request, &calls, &seen_auth));
            }
        });

        MockApi {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            chat_calls,
            auth_headers,
        }
    }
}

/// Xử lý một yêu cầu của server giả.
fn handle_request(
    mut request: tiny_http::Request,
    calls: &AtomicUsize,
    seen_auth: &Mutex<Vec<String>>,
) {
    let url = request.url().to_string();
    let auth = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Authorization"))
        .map(|header| header.value.as_str().to_string())
        .unwrap_or_default();
    seen_auth.lock().unwrap().push(auth);

    if url.ends_with("/models") {
        let body = json!({
            "object": "list",
            "data": [
                {"id": "gpt-4"},
                {"id": "text-embedding-3-small"},
                {"id": "whisper-1"},
                {"id": "llama-3.1-70b"},
                {"id": "gpt-4"},
                {"id": "dall-e-3"},
            ],
        })
        .to_string();
        let _ = request.respond(json_response(200, &body));
        return;
    }

    if url.ends_with("/chat/completions") {
        calls.fetch_add(1, Ordering::SeqCst);
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);
        let model = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|value| value.get("model").and_then(|m| m.as_str()).map(str::to_string))
            .unwrap_or_default();

        let response = if model.starts_with("slow") {
            std::thread::sleep(Duration::from_millis(300));
            json_response(200, &chat_body("Xin chào"))
        } else if model == "bad-model" {
            json_response(
                500,
                r#"{"error":{"message":"Invalid API key","type":"invalid_request_error"}}"#,
            )
        } else if model == "not-json" {
            json_response(200, "day khong phai json")
        } else {
            json_response(200, &chat_body("Xin chào từ mock"))
        };
        let _ = request.respond(response);
        return;
    }

    let _ = request.respond(json_response(404, r#"{"error":"không có route"}"#));
}

fn json_response(status: u16, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        )
}

fn chat_body(content: &str) -> String {
    json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "model": "gpt-4",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": content}}],
    })
    .to_string()
}

fn client_for(api: &MockApi) -> OpenAiClient {
    OpenAiClient::new(&api.base_url, "test-key", 10.0).expect("không tạo được client")
}

fn params_for(api: &MockApi, model_name: &str, concurrency: usize) -> RunParams {
    RunParams {
        base_url: api.base_url.clone(),
        api_key: "test-key".into(),
        test_prompt: Some("Hello".into()),
        model_name: Some(model_name.into()),
        timeout: Some(10.0),
        concurrency: Some(concurrency),
        auto_detect: false,
        profile_id: None,
    }
}

#[test]
fn fetch_models_drops_non_chat_models_and_sorts() {
    let api = MockApi::start();
    let models = client_for(&api).fetch_models().expect("không lấy được model");

    assert_eq!(models, vec!["gpt-4", "llama-3.1-70b"]);
    assert!(api
        .auth_headers
        .lock()
        .unwrap()
        .iter()
        .any(|header| header == "Bearer test-key"));
}

#[test]
fn chat_once_measures_successful_call() {
    let api = MockApi::start();
    let outcome = client_for(&api)
        .chat_once("gpt-4", "Hello")
        .expect("không gọi được model");

    assert!(outcome.succeeded(), "phải thành công: {outcome:?}");
    assert_eq!(outcome.content.as_deref(), Some("Xin chào từ mock"));
    assert!(outcome.response_time_ms > 0.0);
}

#[test]
fn chat_once_reports_http_error_with_message() {
    let api = MockApi::start();
    let outcome = client_for(&api)
        .chat_once("bad-model", "Hello")
        .expect("lỗi HTTP vẫn là kết quả");

    assert!(!outcome.succeeded());
    let error = outcome.error.unwrap_or_default();
    assert!(error.contains("500"), "thiếu mã lỗi: {error}");
    assert!(error.contains("Invalid API key"), "thiếu thông báo: {error}");
}

#[test]
fn chat_once_reports_broken_body() {
    let api = MockApi::start();
    let outcome = client_for(&api)
        .chat_once("not-json", "Hello")
        .expect("body hỏng vẫn là kết quả");
    let error = outcome.error.unwrap_or_default();
    assert!(error.contains("định dạng"), "thiếu thông báo: {error}");
}

#[test]
fn run_bulk_blocking_produces_sorted_report() {
    let api = MockApi::start();
    let params = params_for(&api, "gpt-4, bad-model, llama-3.1-70b", 3);
    let (report, chart) = run_bulk_blocking(&params, None).expect("lượt chạy thất bại");

    assert_eq!(report.total_models, 3);
    assert_eq!(report.successful, 2);
    assert_eq!(report.errors, 1);
    assert_eq!(chart.labels.len(), 3);
    // Model lỗi nằm cuối, hai model thành công đứng trước.
    assert_eq!(report.models.last().unwrap().name, "bad-model");
    assert!(report.avg_response_time_ms > 0.0);
    assert_eq!(chart.colors[2], "#d93a2b");
}

#[test]
fn auto_detect_uses_models_from_endpoint() {
    let api = MockApi::start();
    let mut params = params_for(&api, "", 2);
    params.auto_detect = true;
    let (report, _) = run_bulk_blocking(&params, None).expect("lượt chạy thất bại");

    assert_eq!(report.total_models, 2);
    assert_eq!(api.chat_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn missing_models_and_auto_detect_is_an_error() {
    let api = MockApi::start();
    let params = params_for(&api, "", 2);
    let error = run_bulk_blocking(&params, None).unwrap_err();
    assert!(error.contains("Auto Detect"), "thông báo lạ: {error}");
}

#[test]
fn run_job_emits_progress_and_done_events() {
    let api = MockApi::start();
    let manager = JobManager::new();
    let job = manager.start(params_for(&api, "gpt-4, llama-3.1-70b, bad-model", 3), None);

    let mut events: Vec<String> = Vec::new();
    while let Some(event) = job.recv() {
        events.push(event["event"].as_str().unwrap_or("?").to_string());
    }

    assert_eq!(events.first().map(String::as_str), Some("models"));
    assert_eq!(events.get(1).map(String::as_str), Some("start"));
    assert_eq!(events.iter().filter(|e| *e == "model_done").count(), 3);
    assert_eq!(events.last().map(String::as_str), Some("done"));
}

#[test]
fn cancel_stops_the_run() {
    let api = MockApi::start();
    let manager = JobManager::new();
    let job = manager.start(
        params_for(&api, "slow-1, slow-2, slow-3, slow-4", 1),
        None,
    );

    // Chờ model đầu tiên xong rồi dừng.
    let mut seen_done = 0;
    while let Some(event) = job.recv() {
        match event["event"].as_str() {
            Some("model_done") => {
                seen_done += 1;
                job.cancel();
            }
            Some("cancelled") => {
                assert_eq!(event["completed"].as_u64(), Some(seen_done));
                return;
            }
            Some("done") => panic!("job phải bị huỷ, không được phát done"),
            _ => {}
        }
    }
    panic!("không nhận được sự kiện cancelled");
}

#[test]
fn models_run_in_parallel_up_to_the_concurrency_limit() {
    let api = MockApi::start();
    let manager = JobManager::new();

    let started = Instant::now();
    let serial = manager.start(params_for(&api, "slow-1, slow-2, slow-3, slow-4", 1), None);
    while serial.recv().is_some() {}
    let serial_elapsed = started.elapsed();

    let started = Instant::now();
    let parallel = manager.start(params_for(&api, "slow-1, slow-2, slow-3, slow-4", 4), None);
    while parallel.recv().is_some() {}
    let parallel_elapsed = started.elapsed();

    assert!(
        serial_elapsed >= Duration::from_millis(1200),
        "chạy tuần tự phải chậm: {serial_elapsed:?}"
    );
    assert!(
        parallel_elapsed < Duration::from_millis(900),
        "chạy song song phải nhanh hơn nhiều: {parallel_elapsed:?}"
    );
}
