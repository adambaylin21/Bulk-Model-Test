//! Bản dòng lệnh: chạy test rồi in báo cáo.
//!
//!     cargo run --bin bmt-cli -- --base-url http://localhost:8899/v1 --api-key mock

use std::path::PathBuf;
use std::process::ExitCode;

use bmt_core::params::RunParams;
use bmt_core::runner::{build_chart_data, run_bulk_blocking, sort_results, ModelResult};

struct Args {
    base_url: String,
    api_key: String,
    model: String,
    test_prompt: String,
    timeout: f64,
    concurrency: usize,
    auto_detect: bool,
    output: Option<PathBuf>,
    chart_data: Option<PathBuf>,
}

fn usage() -> String {
    format!(
        "Bulk Model Test - đo thời gian phản hồi của mọi model trên một API key\n\n\
         Cách dùng:\n  {name} --base-url <URL> --api-key <KEY> [tuỳ chọn]\n\n\
         Tuỳ chọn:\n\
         \x20 --base-url <URL>       Endpoint tương thích OpenAI SDK, thường kết thúc bằng /v1\n\
         \x20 --api-key <KEY>        Khoá API\n\
         \x20 --model <TÊN>          Tên model cần test; nhiều model cách nhau bằng dấu phẩy\n\
         \x20 --test-prompt <CÂU>    Câu hỏi gửi tới model (mặc định: Hello)\n\
         \x20 --auto-detect          Lấy danh sách model từ /v1/models\n\
         \x20 --timeout <GIÂY>       Thời gian chờ mỗi model (mặc định: 30)\n\
         \x20 --concurrency <SỐ>     Số model chạy song song, 1..64 (mặc định: 8)\n\
         \x20 --output, -o <TỆP>     Ghi báo cáo JSON\n\
         \x20 --chart-data <TỆP>     Ghi dữ liệu biểu đồ\n\
         \x20 --help, -h             In hướng dẫn này\n",
        name = "bmt-cli"
    )
}

/// Đọc đối số; thiếu hoặc sai thì trả về thông báo lỗi bằng tiếng Việt.
fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        base_url: String::new(),
        api_key: String::new(),
        model: String::new(),
        test_prompt: "Hello".to_string(),
        timeout: 30.0,
        concurrency: 8,
        auto_detect: false,
        output: None,
        chart_data: None,
    };

    let mut items = std::env::args().skip(1);
    while let Some(item) = items.next() {
        let mut value = || {
            items
                .next()
                .ok_or_else(|| format!("Thiếu giá trị cho {item}"))
        };
        match item.as_str() {
            "--help" | "-h" => return Err(usage()),
            "--base-url" => args.base_url = value()?,
            "--api-key" => args.api_key = value()?,
            "--model" => args.model = value()?,
            "--test-prompt" => args.test_prompt = value()?,
            "--timeout" => {
                args.timeout = value()?
                    .parse()
                    .map_err(|_| "--timeout phải là một số giây.".to_string())?
            }
            "--concurrency" => {
                args.concurrency = value()?
                    .parse()
                    .map_err(|_| "--concurrency phải là một số nguyên.".to_string())?
            }
            "--auto-detect" => args.auto_detect = true,
            "--output" | "-o" => args.output = Some(PathBuf::from(value()?)),
            "--chart-data" => args.chart_data = Some(PathBuf::from(value()?)),
            other => return Err(format!("Đối số không hiểu được: {other}\n\n{}", usage())),
        }
    }

    if args.base_url.is_empty() {
        return Err(format!("Thiếu --base-url.\n\n{}", usage()));
    }
    if args.api_key.is_empty() {
        return Err(format!("Thiếu --api-key.\n\n{}", usage()));
    }
    if args.model.is_empty() && !args.auto_detect {
        // Không có tên model thì mặc định lấy danh sách từ API.
        args.auto_detect = true;
    }
    Ok(args)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            println!("{message}");
            return ExitCode::from(1);
        }
    };

    println!("Endpoint: {}", args.base_url);
    println!("Prompt:   {:?}", args.test_prompt);
    println!("Danh sách model: {}", if args.model.is_empty() {
        "lấy từ API".to_string()
    } else {
        args.model.clone()
    });

    let params = RunParams {
        base_url: args.base_url.clone(),
        api_key: args.api_key.clone(),
        test_prompt: Some(args.test_prompt.clone()),
        model_name: Some(args.model.clone()),
        timeout: Some(args.timeout),
        concurrency: Some(args.concurrency),
        auto_detect: args.auto_detect,
        profile_id: None,
    };

    let (report, chart) = match run_bulk_blocking(&params, None) {
        Ok(outcome) => outcome,
        Err(message) => {
            eprintln!("Lỗi: {message}");
            return ExitCode::from(1);
        }
    };

    print_report(&report);

    if let Some(path) = &args.output {
        match write_json(path, &serde_json::to_value(&report).unwrap_or_default()) {
            Ok(()) => println!("\nĐã ghi báo cáo: {}", path.display()),
            Err(error) => {
                eprintln!("Không ghi được {}: {error}", path.display());
                return ExitCode::from(1);
            }
        }
    }
    if let Some(path) = &args.chart_data {
        let chart = if chart.labels.is_empty() {
            let results: Vec<ModelResult> = report
                .models
                .iter()
                .map(|model| ModelResult {
                    model: model.name.clone(),
                    response_time_ms: model.response_time_ms,
                    status: model.status.clone(),
                    error: model.error.clone(),
                    test_result: model.test_result.clone(),
                })
                .collect();
            build_chart_data(&sort_results(results))
        } else {
            chart
        };
        match write_json(path, &serde_json::to_value(&chart).unwrap_or_default()) {
            Ok(()) => println!("Đã ghi dữ liệu biểu đồ: {}", path.display()),
            Err(error) => {
                eprintln!("Không ghi được {}: {error}", path.display());
                return ExitCode::from(1);
            }
        }
    }

    if report.errors > 0 && report.successful == 0 {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn write_json(path: &PathBuf, value: &serde_json::Value) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value).unwrap_or_default();
    std::fs::write(path, format!("{text}\n"))
}

fn print_report(report: &bmt_core::runner::Report) {
    println!("\n{}", "=".repeat(64));
    println!("BÁO CÁO BULK MODEL TEST");
    println!("{}", "=".repeat(64));
    println!("Tổng số model: {}", report.total_models);
    println!("Thành công:    {}", report.successful);
    println!("Lỗi:           {}", report.errors);
    if report.successful > 0 {
        println!("Thời gian TB:  {:.2} ms", report.avg_response_time_ms);
        println!("Nhanh nhất:    {:.2} ms", report.fastest_ms);
        println!("Chậm nhất:     {:.2} ms", report.slowest_ms);
    }
    if let Some(profile) = &report.profile {
        if let Some(name) = profile.get("name").and_then(|name| name.as_str()) {
            println!("Profile:       {name}");
        }
    }
    println!("Thời điểm:     {}", report.timestamp);
    println!("{}", "-".repeat(64));
    println!("\nKết quả (nhanh → chậm, model lỗi ở cuối):\n");

    for model in &report.models {
        if model.status == "success" {
            println!("  ✓ {:<40} {:>10.2} ms", model.name, model.response_time_ms);
        } else {
            println!(
                "  ✗ {:<40} {:>10.2} ms (LỖI)",
                model.name, model.response_time_ms
            );
            if let Some(error) = &model.error {
                println!("      └─ {error}");
            }
        }
    }
    println!("\n{}", "=".repeat(64));
}
