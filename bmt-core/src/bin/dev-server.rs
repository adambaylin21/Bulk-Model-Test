//! Server HTTP chỉ dùng khi phát triển.
//!
//! Cửa sổ Tauri nói chuyện với Rust qua lệnh IPC; trình duyệt thì không làm được
//! điều đó. Bin này phục vụ thư mục `ui/` và bày cùng các thao tác đó qua HTTP để
//! kiểm tra giao diện bằng trình duyệt thật.
//!
//!     cargo run --bin dev-server
//!     open http://127.0.0.1:8686

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bmt_core::params::{RunParams, RunParamsInput};
use bmt_core::service::Service;
use futures_util::stream::Stream;
use serde_json::{json, Value};
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Clone)]
struct App {
    service: Arc<Service>,
    ui_dir: PathBuf,
    /// Giữ job đang chạy để endpoint /api/test/cancel tìm thấy.
    jobs: Arc<tokio::sync::Mutex<Vec<String>>>,
}

#[tokio::main]
async fn main() {
    let ui_dir = std::env::var("BMT_UI_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui"));
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8686);

    let state = App {
        service: Arc::new(Service::new()),
        ui_dir: ui_dir.clone(),
        jobs: Arc::new(tokio::sync::Mutex::new(Vec::new())),
    };

    let router = Router::new()
        .route("/", get(index))
        .route("/styles.css", get(styles))
        .route("/js/*path", get(js_asset))
        .route("/vendor/*path", get(vendor_asset))
        .route("/:file", get(root_file))
        .route("/api/bootstrap", get(bootstrap))
        .route("/api/profiles", get(profiles_index).post(profiles_save))
        .route("/api/profiles/:id", delete(profiles_delete))
        .route("/api/profiles/:id/used", post(profiles_touch))
        .route("/api/models", post(models_preview))
        .route("/api/models/parse", post(models_parse))
        .route("/api/test/stream", post(test_stream))
        .route("/api/test/cancel", post(test_cancel))
        .with_state(state);

    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .unwrap_or_else(|error| panic!("Không mở được cổng {port}: {error}"));

    println!("Bulk Model Test - bản chạy trong trình duyệt");
    println!("Giao diện: http://127.0.0.1:{port}");
    println!("Giao diện lấy từ: {}", ui_dir.display());
    println!("Dữ liệu:   {}", bmt_core::config::profiles_file().display());
    axum::serve(listener, router).await.unwrap();
}

async fn index(State(app): State<App>) -> Response {
    serve_file(&app.ui_dir.join("index.html")).await
}

async fn styles(State(app): State<App>) -> Response {
    serve_file(&app.ui_dir.join("styles.css")).await
}

/// File nằm ngay trong thư mục giao diện, ví dụ tệp kịch bản kiểm tra.
async fn root_file(State(app): State<App>, Path(file): Path<String>) -> Response {
    if file.contains('/') || file.contains('\\') || file.starts_with('.') {
        return error_response(StatusCode::BAD_REQUEST, "Đường dẫn không hợp lệ.");
    }
    serve_file(&app.ui_dir.join(file)).await
}

/// Phần khớp `*path` không kèm tiền tố, nên phải thêm lại thư mục.
async fn js_asset(State(app): State<App>, Path(path): Path<String>) -> Response {
    asset(app, "js", path).await
}

async fn vendor_asset(State(app): State<App>, Path(path): Path<String>) -> Response {
    asset(app, "vendor", path).await
}

async fn asset(app: App, folder: &str, path: String) -> Response {
    let requested = PathBuf::from(folder).join(&path);
    // Không cho đường dẫn thoát khỏi thư mục giao diện.
    if requested.components().any(|part| {
        matches!(part, std::path::Component::ParentDir | std::path::Component::RootDir)
    }) {
        return error_response(StatusCode::BAD_REQUEST, "Đường dẫn không hợp lệ.");
    }
    serve_file(&app.ui_dir.join(requested)).await
}

async fn serve_file(path: &std::path::Path) -> Response {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let mime = match path.extension().and_then(|ext| ext.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("js") => "text/javascript; charset=utf-8",
                Some("json") => "application/json; charset=utf-8",
                _ => "application/octet-stream",
            };
            ([(axum::http::header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        Err(_) => error_response(StatusCode::NOT_FOUND, "Không tìm thấy file."),
    }
}

async fn bootstrap(State(app): State<App>) -> Json<Value> {
    let store = app.service.list_profiles();
    let mut payload = store;
    payload["path"] = json!(app.service.profiles_path()["path"]);
    Json(payload)
}

async fn profiles_index(State(app): State<App>) -> Json<Value> {
    Json(app.service.list_profiles())
}

async fn profiles_save(State(app): State<App>, Json(input): Json<Value>) -> Response {
    match app.service.save_profile(serde_json::from_value(input).unwrap_or_default()) {
        Ok(profile) => Json(json!({ "profile": profile })).into_response(),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
    }
}

async fn profiles_delete(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.service.delete_profile(&id) {
        Ok(true) => Json(json!({ "deleted": true, "id": id })).into_response(),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Không tìm thấy profile."),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
    }
}

async fn profiles_touch(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.service.touch_profile(&id) {
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
    }
}

async fn models_parse(Json(body): Json<Value>) -> Json<Value> {
    let raw = body.get("model_name").and_then(Value::as_str).unwrap_or_default();
    Json(Service::default().parse_models(raw))
}

async fn models_preview(State(app): State<App>, Json(body): Json<Value>) -> Response {
    let params = run_params(body);
    if params.trimmed_base_url().is_empty() || params.trimmed_api_key().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Cần nhập Base Url và API Key.");
    }
    // Client trong bmt-core là bản blocking: gọi thẳng trên luồng async của tokio
    // sẽ panic khi runtime bị huỷ, nên phải đẩy sang luồng riêng.
    let outcome = tokio::task::spawn_blocking(move || app.service.preview_models(&params)).await;
    match outcome {
        Ok(Ok(payload)) => Json(payload).into_response(),
        Ok(Err(error)) => error_response(StatusCode::BAD_REQUEST, &error),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

async fn test_stream(State(app): State<App>, Json(body): Json<Value>) -> Response {
    let params = run_params(body);
    if params.trimmed_base_url().is_empty() || params.trimmed_api_key().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Cần nhập Base Url và API Key.");
    }

    let (job_id, job) = app.service.start_run(params);
    app.jobs.lock().await.push(job_id.clone());

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, std::convert::Infallible>>();
    let job_id_for_stream = job_id.clone();
    let _ = tx.send(Ok(Event::default().data(
        json!({ "event": "job", "job_id": job_id_for_stream }).to_string(),
    )));

    std::thread::spawn(move || {
        while let Some(payload) = job.recv() {
            if tx.send(Ok(Event::default().data(payload.to_string()))).is_err() {
                job.cancel();
                break;
            }
        }
    });

    Sse::new(UnboundedReceiverStream::new(rx) as UnboundedReceiverStream<_>)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Giữ tên kiểu dữ liệu để `Sse` suy ra loại luồng đúng.
type _StreamShape = Box<dyn Stream<Item = Result<Event, std::convert::Infallible>> + Send>;

async fn test_cancel(State(app): State<App>, Json(body): Json<Value>) -> Response {
    let requested = body
        .get("job_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let job_id = if requested.is_empty() {
        app.jobs.lock().await.last().cloned().unwrap_or_default()
    } else {
        requested
    };
    let service = Arc::clone(&app.service);
    let cancelled = tokio::task::spawn_blocking(move || service.cancel_run(&job_id))
        .await
        .unwrap_or(false);
    Json(json!({ "cancelled": cancelled })).into_response()
}

fn run_params(body: Value) -> RunParams {
    serde_json::from_value::<RunParamsInput>(body)
        .unwrap_or_default()
        .into()
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}
