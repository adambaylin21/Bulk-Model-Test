//! Ứng dụng desktop: các lệnh Tauri mà giao diện gọi tới.

use std::sync::Arc;

use bmt_core::params::{ProfileInput, RunParams, RunParamsInput};
use bmt_core::service::Service;
use serde_json::{json, Value};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

mod selftest;

struct AppState {
    service: Arc<Service>,
}

/// Thông tin dùng chung lúc khởi động: nơi lưu profile, tên phiên bản.
#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> Value {
    let store = state.service.list_profiles();
    json!({
        "profiles": store["profiles"],
        "count": store["count"],
        "storage": store["storage"],
        "updated_at": store["updated_at"],
        "path": state.service.profiles_path()["path"],
        "version": env!("CARGO_PKG_VERSION"),
    })
}

#[tauri::command]
fn profiles_list(state: State<'_, AppState>) -> Result<Value, String> {
    let mut store = state.service.list_profiles();
    if let Some(object) = store.as_object_mut() {
        object.insert("path".into(), state.service.profiles_path()["path"].clone());
    }
    Ok(store)
}

#[tauri::command]
fn profile_save(state: State<'_, AppState>, input: Value) -> Result<Value, String> {
    let payload: ProfileInput = serde_json::from_value(input).unwrap_or_default();
    if payload.base_url.as_deref().unwrap_or_default().trim().is_empty() {
        return Err("Cần nhập Base Url.".to_string());
    }
    let profile = state.service.save_profile(payload)?;
    Ok(json!({ "profile": profile }))
}

#[tauri::command]
fn profile_delete(state: State<'_, AppState>, profile_id: String) -> Result<Value, String> {
    match state.service.delete_profile(&profile_id)? {
        true => Ok(json!({ "deleted": true, "id": profile_id })),
        false => Err("Không tìm thấy profile.".to_string()),
    }
}

#[tauri::command]
fn profile_touch(state: State<'_, AppState>, profile_id: String) -> Result<Value, String> {
    state.service.touch_profile(&profile_id)?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
fn models_parse(state: State<'_, AppState>, model_name: String) -> Value {
    state.service.parse_models(&model_name)
}

#[tauri::command]
async fn models_preview(state: State<'_, AppState>, params: Value) -> Result<Value, String> {
    let params = run_params(params);
    let service = Arc::clone(&state.service);
    match tauri::async_runtime::spawn_blocking(move || service.preview_models(&params)).await {
        Ok(outcome) => outcome,
        Err(error) => Err(error.to_string()),
    }
}

/// Bắt đầu một lượt chạy; tiến độ gửi về qua `on_event`.
#[tauri::command]
async fn test_start(
    state: State<'_, AppState>,
    params: Value,
    on_event: Channel<Value>,
) -> Result<String, String> {
    let params = run_params(params);
    if params.trimmed_base_url().is_empty() || params.trimmed_api_key().is_empty() {
        return Err("Cần nhập Base Url và API Key.".to_string());
    }

    let (job_id, job) = state.service.start_run(params);
    let _ = on_event.send(json!({ "event": "job", "job_id": job_id }));

    let channel = on_event.clone();
    tauri::async_runtime::spawn_blocking(move || {
        while let Some(payload) = job.recv() {
            // Giao diện đã đóng: dừng luôn cho khỏi tốn công.
            if channel.send(payload).is_err() {
                job.cancel();
                break;
            }
        }
    });

    Ok(job_id)
}

#[tauri::command]
async fn test_cancel(state: State<'_, AppState>, job_id: Option<String>) -> Result<Value, String> {
    let service = Arc::clone(&state.service);
    let requested = job_id.unwrap_or_default();
    let cancelled = tauri::async_runtime::spawn_blocking(move || service.cancel_run(&requested))
        .await
        .unwrap_or(false);
    Ok(json!({ "cancelled": cancelled }))
}

#[tauri::command]
fn data_open(state: State<'_, AppState>) -> Result<Value, String> {
    let path = bmt_core::config::data_dir();
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    let _ = state;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }

    Ok(json!({ "opened": path.display().to_string() }))
}

/// Lưu báo cáo JSON vào file người dùng chọn.
#[tauri::command]
async fn report_save(
    app: AppHandle,
    default_name: String,
    contents: String,
) -> Result<Value, String> {
    use tauri_plugin_dialog::DialogExt;

    let (sender, receiver) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_file_name(&default_name)
        .save_file(move |path| {
            let _ = sender.send(path);
        });

    let chosen = tauri::async_runtime::spawn_blocking(move || receiver.recv().ok().flatten())
        .await
        .map_err(|error| error.to_string())?;

    let Some(path) = chosen else {
        return Ok(json!({ "saved": false }));
    };
    let target = path
        .into_path()
        .map_err(|error| error.to_string())?;
    std::fs::write(&target, contents).map_err(|error| error.to_string())?;
    Ok(json!({ "saved": true, "path": target.display().to_string() }))
}

fn run_params(value: Value) -> RunParams {
    serde_json::from_value::<RunParamsInput>(value)
        .unwrap_or_default()
        .into()
}

/// Khởi động ứng dụng desktop.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(AppState {
                service: Arc::new(Service::new()),
            });

            // Chế độ tự kiểm tra giao diện: thao tác trong webview rồi thoát.
            if std::env::var_os("BMT_UITEST").is_some() {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    selftest::ui_selftest(&handle);
                });
                return Ok(());
            }

            // Chế độ tự kiểm tra: chạy các bước rồi thoát.
            if std::env::var_os("BMT_SELFTEST").is_some() {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    let result = selftest::selftest();
                    let path = std::env::var("BMT_SELFTEST_OUT")
                        .unwrap_or_else(|_| "/tmp/bmt-selftest.json".to_string());
                    let rendered = serde_json::to_string_pretty(&result)
                        .unwrap_or_else(|_| "{}".to_string());
                    let _ = std::fs::write(&path, format!("{rendered}\n"));
                    println!("=== KẾT QUẢ TỰ KIỂM TRA ===");
                    println!("{rendered}");
                    println!("(đã ghi vào {path})");
                    handle.exit(0);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            profiles_list,
            profile_save,
            profile_delete,
            profile_touch,
            models_parse,
            models_preview,
            selftest::ui_result,
            selftest::ui_done,
            test_start,
            test_cancel,
            data_open,
            report_save,
        ])
        .run(tauri::generate_context!())
        .expect("Không chạy được ứng dụng Bulk Model Test");
}
