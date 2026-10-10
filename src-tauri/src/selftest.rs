//! Kiểm tra nội bộ: chạy đúng các lệnh Tauri trên một cửa sổ thật rồi tự thoát.
//!
//! ```text
//! BMT_SELFTEST=1 BMT_DATA_DIR=/tmp/bmt-appdata cargo run -p bulk-model-test
//! ```
//!
//! Kết quả in ra stdout và ghi vào `$BMT_SELFTEST_OUT` (mặc định
//! `/tmp/bmt-selftest.json`), nên chạy được cả khi không có ai ngồi trước máy.

use std::sync::Arc;

use bmt_core::params::{ProfileInput, RunParams};
use bmt_core::service::Service;
use serde_json::{json, Value};

fn service_for_run() -> Arc<Service> {
    if let Some(dir) = std::env::var_os("BMT_SELFTEST_DATA") {
        std::env::set_var("BMT_DATA_DIR", dir);
    }
    Arc::new(Service::new())
}

/// Nhận kết quả tự kiểm tra giao diện từ trong trang.
#[tauri::command]
pub fn ui_result(payload: Value) -> Value {
    let path = std::env::var("BMT_UITEST_OUT")
        .unwrap_or_else(|_| "/tmp/bmt-uitest.json".to_string());
    let rendered = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
    let _ = std::fs::write(&path, format!("{rendered}\n"));
    println!("=== KẾT QUẢ TỰ KIỂM TRA GIAO DIỆN ===");
    println!("{rendered}");
    println!("(đã ghi vào {path})");
    json!({ "ok": true })
}

/// Nhận tín hiệu giao diện đã kiểm tra xong.
#[tauri::command]
pub fn ui_done(app: tauri::AppHandle) -> Value {
    app.exit(0);
    json!({ "ok": true })
}

/// Kiểm tra giao diện trong webview của ứng dụng; kết quả gửi về qua `ui_result`.
pub fn ui_selftest(app: &tauri::AppHandle) {

    // Kịch bản thao tác như người dùng: gõ form, bấm chạy, xem báo cáo, lưu rồi xoá profile.
    let script = r##"
      (async () => {
        const D = (id) => document.getElementById(id);
        const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
        const log = {};
        const errors = [];
        window.addEventListener("error", (e) => errors.push(String(e.message)));
        window.addEventListener("unhandledrejection", (e) => errors.push(String(e.reason)));
        const status = () => ({ state: D("status-strip").dataset.state, text: D("status-text").textContent });
        const waitFor = async (label, test, timeout = 30000) => {
          const t0 = Date.now();
          while (Date.now() - t0 < timeout) { if (test()) return; await sleep(150); }
          throw new Error("hết thời gian chờ: " + label);
        };
        const chartBars = () => {
          try { return Chart.getChart(D("response-chart"))?.data?.datasets[0]?.data?.length ?? null; }
          catch (_) { return null; }
        };

        try {
          log.bridge = { desktop: Boolean(window.__TAURI__), invoke: typeof window.__TAURI__?.core?.invoke };
          log.initial = { storageNote: D("profile-storage-note").textContent,
                          dataButtonShown: !D("data-open-btn").hidden,
                          profileCount: D("tab-profile-count").textContent,
                          chartLib: typeof window.Chart };

          D("base-url").value = "http://127.0.0.1:8899/v1";
          D("api-key").value = "mock-key-1234567890";
          D("auto-detect").checked = true;
          D("auto-detect").dispatchEvent(new Event("change"));
          D("concurrency").value = "8";
          D("concurrency").dispatchEvent(new Event("input"));

          D("bulk-test-btn").click();
          await sleep(500);
          log.duringRun = { disabled: D("bulk-test-btn").disabled,
                            cancelShown: !D("cancel-btn").hidden, status: status() };

          await waitFor("lượt chạy", () => D("status-strip").dataset.state === "done", 60000);
          const rows = Array.from(document.querySelectorAll("#report-output tbody tr"))
            .map((tr) => Array.from(tr.children).map((td) => td.textContent.trim()));
          log.run = {
            status: status(),
            stats: { total: D("stat-total").textContent, ok: D("stat-success").textContent,
                     bad: D("stat-error").textContent, avg: D("stat-avg").textContent,
                     fastest: D("stat-fastest").textContent },
            progress: D("progress-count").textContent,
            chartShown: !D("response-chart").hidden,
            chartBars: chartBars(),
            rows: rows.length,
            order: rows.map((r) => r[1]),
            lastStatus: rows[rows.length - 1]?.[3],
            stored: Boolean(localStorage.getItem("bmt.lastRun")),
            cancelHidden: D("cancel-btn").hidden,
          };

          D("profile-save-btn").click();
          await waitFor("lưu profile", () => /Đã lưu/.test(D("profile-form-message").textContent));
          log.saved = { message: D("profile-form-message").textContent,
                        badge: D("tab-profile-count").textContent };

          D("tab-profiles").click();
          await sleep(300);
          const card = document.querySelector(".profile-card");
          log.profileTab = {
            shown: !D("panel-profiles").hidden,
            cardName: card?.querySelector("h3")?.textContent.trim() ?? null,
            cardValues: card ? Array.from(card.querySelectorAll("dd")).map((dd) => dd.textContent.trim()) : null,
          };

          window.confirm = () => true;
          document.querySelector('.profile-card button[data-action="delete"]')?.click();
          await waitFor("xoá profile", () => /Đã xoá/.test(D("profile-form-message").textContent));
          log.deleted = { message: D("profile-form-message").textContent,
                          cards: document.querySelectorAll(".profile-card").length,
                          badge: D("tab-profile-count").textContent };

          log.buttons = { saveReport: Boolean(D("report-save-btn")),
                          openDataDir: Boolean(D("data-open-btn")) };
        } catch (error) {
          log.fatal = String(error && error.message ? error.message : error);
        }

        try {
          await window.__TAURI__.core.invoke("ui_result", { payload: { log, errors } });
          await window.__TAURI__.core.invoke("ui_done", {});
        } catch (error) {
          console.error("không gửi được kết quả", error);
        }
      })()
    "##;

    match tauri::Manager::get_webview_window(app, "main") {
        Some(window) => {
            if let Err(error) = window.eval(script) {
                println!("UI tự kiểm tra: không chạy được kịch bản: {error}");
            }
        }
        None => println!("UI tự kiểm tra: không thấy cửa sổ chính"),
    }
}

/// Chạy một lượt test và gom các sự kiện lại.
fn run_once(service: &Service, params: RunParams) -> Value {
    let (job_id, job) = service.start_run(params);
    let mut events: Vec<String> = Vec::new();
    let mut report = Value::Null;
    let mut chart = Value::Null;
    let mut messages: Vec<String> = Vec::new();

    while let Some(event) = job.recv() {
        let name = event.get("event").and_then(Value::as_str).unwrap_or("?");
        events.push(name.to_string());
        match name {
            "done" => {
                report = event.get("report").cloned().unwrap_or(Value::Null);
                chart = event.get("chart_data").cloned().unwrap_or(Value::Null);
            }
            "error" => messages.push(
                event
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
            _ => {}
        }
    }

    json!({
        "job_id": job_id,
        "events": events,
        "report": report,
        "chart_data": chart,
        "messages": messages,
    })
}

/// Kiểm tra các lệnh mà giao diện gọi qua IPC.
pub fn selftest() -> Value {
    let service = service_for_run();
    let mut out = serde_json::Map::new();
    let mut checks: Vec<Value> = Vec::new();

    // 1. Khởi động: đọc danh sách profile
    let bootstrap = service.list_profiles();
    checks.push(json!({
        "bước": "bootstrap/profiles_list",
        "ok": bootstrap.get("profiles").map(Value::is_array).unwrap_or(false),
        "path": service.profiles_path()["path"],
    }));

    // 2. Lưu profile
    let saved = service
        .save_profile(ProfileInput {
            name: Some("Self test".into()),
            base_url: serde_json::from_str::<Value>(
                &std::env::var("BMT_SELFTEST_BASE_URL")
                    .map(|url| format!("{url:?}"))
                    .unwrap_or_else(|_| "\"http://127.0.0.1:8899/v1\"".into()),
            )
            .ok()
            .and_then(|v| v.as_str().map(str::to_string)),
            api_key: Some("mock-key-1234567890".into()),
            concurrency: Some(serde_json::json!(8)),
            ..Default::default()
        })
        .map_err(|error| error.to_string());
    match &saved {
        Ok(profile) => checks.push(json!({
            "bước": "profile_save",
            "ok": true,
            "id": profile.id,
            "name": profile.fields.name,
            "token": profile.fields.api_key,
        })),
        Err(error) => checks.push(json!({ "bước": "profile_save", "ok": false, "error": error })),
    }
    let profile_id = saved.as_ref().map(|p| p.id.clone()).unwrap_or_default();

    // 3. Xem trước danh sách model
    let preview = service.preview_models(&RunParams {
        base_url: std::env::var("BMT_SELFTEST_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8899/v1".into()),
        api_key: "mock-key-1234567890".into(),
        timeout: Some(15.0),
        ..Default::default()
    });
    match preview {
        Ok(payload) => checks.push(json!({
            "bước": "models_preview",
            "ok": true,
            "count": payload["count"],
            "models": payload["models"],
        })),
        Err(error) => checks.push(json!({ "bước": "models_preview", "ok": false, "error": error })),
    }

    // 4. Đếm model gõ tay
    checks.push(json!({
        "bước": "models_parse",
        "ok": true,
        "count": service.parse_models("a\nb, c\n\n d")["count"],
    }));

    // 5. Chạy test với Auto Detect
    let auto = run_once(
        &service,
        RunParams {
            base_url: std::env::var("BMT_SELFTEST_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8899/v1".into()),
            api_key: "mock-key-1234567890".into(),
            timeout: Some(20.0),
            concurrency: Some(8),
            auto_detect: true,
            ..Default::default()
        },
    );
    checks.push(json!({
        "bước": "test_start (auto detect)",
        "ok": auto["report"]["total_models"].as_u64().unwrap_or(0) > 0,
        "events": auto["events"],
        "ok_count": auto["report"]["successful"],
        "errors": auto["report"]["errors"],
        "order": auto["report"]["models"].as_array().map(|rows| rows.iter()
            .map(|row| json!([row["name"], row["status"]]))
            .collect::<Vec<Value>>()),
        "chart": { "labels": auto["chart_data"]["labels"].as_array().map(Vec::len),
                   "colours": auto["chart_data"]["colors"] },
    }));

    // 6. Chạy test với danh sách gõ tay, có profile để ghi kèm tên
    let manual = run_once(
        &service,
        RunParams {
            base_url: std::env::var("BMT_SELFTEST_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8899/v1".into()),
            api_key: "mock-key-1234567890".into(),
            test_prompt: Some("Xin chào".into()),
            model_name: Some("gpt-4o-mini, rate-limited".into()),
            timeout: Some(20.0),
            concurrency: Some(2),
            auto_detect: false,
            profile_id: if profile_id.is_empty() { None } else { Some(profile_id.clone()) },
        },
    );
    checks.push(json!({
        "bước": "test_start (nhập tay, kèm profile)",
        "ok": manual["report"]["total_models"].as_u64() == Some(2),
        "ok_count": manual["report"]["successful"],
        "errors": manual["report"]["errors"],
        "profile_in_report": manual["report"]["profile"]["name"],
        "error_detail": manual["report"]["models"][1]["error"],
    }));

    // 7. Báo cáo có tách được thành JSON để lưu file
    let report_text = serde_json::to_string_pretty(&manual["report"]).unwrap_or_default();
    checks.push(json!({
        "bước": "report_save (dữ liệu)",
        "ok": report_text.len() > 100 && report_text.contains("response_time_ms"),
        "bytes": report_text.len(),
    }));

    // 8. Dừng giữa chừng
    let (job_id, job) = service.start_run(RunParams {
        base_url: std::env::var("BMT_SELFTEST_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8899/v1".into()),
        api_key: "mock-key-1234567890".into(),
        model_name: Some("very-slow-large, slow-medium, slow-small, gpt-4o".into()),
        timeout: Some(20.0),
        concurrency: Some(1),
        ..Default::default()
    });
    let mut saw_model_done = false;
    let mut cancelled_event = Value::Null;
    while let Some(event) = job.recv() {
        match event.get("event").and_then(Value::as_str) {
            Some("model_done") if !saw_model_done => {
                saw_model_done = true;
                let _ = service.cancel_run(&job_id);
            }
            Some("cancelled") => {
                cancelled_event = event;
                break;
            }
            Some("done") => break,
            _ => {}
        }
    }
    checks.push(json!({
        "bước": "test_cancel",
        "ok": !cancelled_event.is_null(),
        "completed": cancelled_event["completed"],
        "cancel_unknown": service.cancel_run("khong-ton-tai"),
    }));

    // 9. Dọn profile của lượt tự kiểm tra
    let deleted = service.delete_profile(&profile_id).unwrap_or(false);
    checks.push(json!({
        "bước": "profile_delete",
        "ok": deleted,
        "remaining": service.list_profiles()["count"],
        "delete_twice": service.delete_profile(&profile_id).unwrap_or(false),
    }));

    out.insert("checks".into(), Value::Array(checks.clone()));
    let failed: Vec<&Value> = checks
        .iter()
        .filter(|check| check.get("ok").and_then(Value::as_bool) != Some(true))
        .collect();
    out.insert("passed".into(), json!(checks.len() - failed.len()));
    out.insert("failed".into(), json!(failed.len()));
    Value::Object(out)
}
