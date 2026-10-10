# Bulk Model Test

Đo thời gian phản hồi của mọi model mà một API key OpenAI-compatible chạm được, rồi báo cáo xếp từ nhanh đến chậm. Có thể lưu nhiều profile (nhiều token) để chuyển qua lại nhanh.

Ứng dụng desktop viết bằng **Rust + Tauri 2**: cửa sổ riêng, giao diện và toàn bộ phần gọi API chạy trong máy, không mở cổng mạng và không cần trình duyệt.

## Chạy

```bash
cargo run -p bulk-model-test
```

Lần chạy đầu sẽ tải và biên dịch các gói phụ thuộc, mất vài phút.

## Bản ứng dụng

Đóng gói thành ứng dụng macOS. Cần Rust và `cargo-tauri`:

```bash
cargo install tauri-cli --version '^2' --locked
./build_app.sh
```

Kết quả:

| Tệp | Dùng khi |
| --- | --- |
| `target/release/bundle/macos/Bulk Model Test.app` | Mở như ứng dụng thường |
| `target/release/bundle/dmg/Bulk Model Test_1.0.0_*.dmg` | Bộ cài để chia sẻ |

Icon lấy từ `Icon.png` ở gốc dự án và đã được tạo sẵn vào `src-tauri/icons/` (dựng lại bằng `sips` và `iconutil` khi cần). Dữ liệu profile nằm ở `~/Library/Application Support/Bulk Model Test/profiles.json`, đổi được bằng biến môi trường `BMT_DATA_DIR`. Nút **Mở thư mục dữ liệu** trong tab Báo cáo mở thẳng thư mục đó.

## Cấu trúc thư mục

```
Cargo.toml            workspace Rust (bmt-core, src-tauri)
bmt-core/             lõi dùng chung cho ứng dụng, CLI và server lúc phát triển
  src/config.rs       đường dẫn, hằng số, bộ lọc model không phục vụ chat
  src/params.rs       chuẩn hoá tham số chạy và dữ liệu profile
  src/profiles.rs     đọc/ghi profiles.json (ghi nguyên tử)
  src/api.rs          gọi /v1/models và /v1/chat/completions, đo thời gian
  src/runner.rs       chạy bulk test song song, phát tiến độ
  src/service.rs      mặt tiền cho các lệnh của Tauri
  src/bin/bmt-cli.rs  bản dòng lệnh
  src/bin/dev-server.rs  server HTTP cục bộ để mở giao diện bằng trình duyệt
  tests/mock_api.rs   test tích hợp với server OpenAI giả
src-tauri/            ứng dụng desktop
  src/lib.rs          các lệnh Tauri (IPC) mà giao diện gọi
  src/selftest.rs     tự kiểm tra lệnh và giao diện
  tauri.conf.json     cấu hình cửa sổ và gói
  capabilities/       quyền tối thiểu cấp cho cửa sổ
ui/                   giao diện (HTML, CSS, ES module)
  index.html
  styles.css
  js/bridge.js        cầu nối Tauri, có nhánh dự phòng qua HTTP
  js/api.js           nghiệp vụ: profile, model, lượt chạy
  js/run.js           tab Kiểm tra model
  js/profiles.js      tab Profile
  js/main.js          khởi động, nối các tab
  js/utils.js         tiện ích chung
  js/ui/tabs.js       chuyển tab
  vendor/chart.umd.js Chart.js 4 (đi kèm, không tải từ mạng)
legacy/               bản Python cũ (Flask + PyInstaller) để tham chiếu
data/profiles.json    nơi lưu profile khi chạy CLI (tự tạo khi lưu lần đầu)
```

## Hai tab

### Tab Kiểm tra model

1. **Profile đã lưu** — chọn một profile rồi bấm **Nạp** để điền sẵn mọi trường, hoặc bấm **Lưu lại** để lưu các giá trị đang nhập thành profile
2. **Base Url** — endpoint tương thích OpenAI SDK, thường kết thúc bằng `/v1`
3. **API Key** — khoá của bạn
4. **Tên Model** — bỏ trống khi muốn test toàn bộ, hoặc ghi mỗi model một dòng
5. **Auto Detect Model** — bật để lấy danh sách từ `/v1/models`
6. **Prompt Test** — câu hỏi gửi tới, mặc định `Hello`
7. **Timeout** — giây chờ mỗi model, mặc định 30
8. **Test đồng thời** — số model chạy song song, từ 1 đến 32

Trong lúc chạy, bảng cập nhật từng dòng, thanh tiến độ đếm số model đã xong, biểu đồ vẽ dần. Nút **Dừng** huỷ phiên test. Nút **Lưu JSON** ghi báo cáo gần nhất ra file.

### Tab Profile

Lưu nhiều bộ tuỳ chọn, mỗi bộ gồm tên, Base Url, token, danh sách model, prompt, timeout, số luồng và ghi chú.

- **Lưu profile** — thêm mới, hoặc cập nhật khi đang sửa một profile có sẵn
- **Dùng ngay** — nạp profile vào tab Kiểm tra model
- **Sửa** — mở profile ra biểu mẫu bên trái
- **Xoá** — bỏ profile, có hỏi xác nhận
- **Tìm theo tên hoặc Base Url** — lọc danh sách đang hiển thị

Token hiện trong danh sách ở dạng che bớt (`gsk_…mnop`). Bỏ trống ô Token lúc cập nhật thì token cũ được giữ nguyên.

Dữ liệu nằm trong `profiles.json`:

```json
{
  "version": 1,
  "updated_at": "2026-10-10T17:20:30",
  "profiles": [
    {
      "id": "541665d154cd",
      "created_at": "2026-10-10T17:20:30",
      "updated_at": "2026-10-10T17:20:30",
      "name": "Mock local",
      "base_url": "http://127.0.0.1:8899/v1",
      "api_key": "mock-key",
      "model_name": "",
      "test_prompt": "Hello",
      "timeout": 30.0,
      "concurrency": 4,
      "auto_detect": true,
      "auto_start": false,
      "notes": ""
    }
  ]
}
```

Ghi file theo cách nguyên tử (ghi file tạm rồi đổi tên) nên không hỏng dữ liệu khi tiến trình dừng giữa chừng. Trường lạ trong file do bản khác ghi vẫn được giữ nguyên khi ghi lại.

**Lưu ý bảo mật:** tool lưu token dạng thô trong `profiles.json`. Đây là công cụ chạy trên máy cá nhân; đừng dùng chung file này hoặc commit nó lên.

## Kết quả

Bảng xếp model thành công theo thời gian tăng dần, các model lỗi nằm cuối. Mỗi dòng hiện thứ hạng, tên model, thời gian, trạng thái và nội dung trả về (hoặc thông báo lỗi).

Biểu đồ cột tương ứng: xanh là thành công, đỏ là lỗi. Trục ngang là tên model, trục dọc là mili giây.

## Bản dòng lệnh

```bash
cargo run --bin bmt-cli -- --base-url http://localhost:8899/v1 --api-key mock --test-prompt "Hello"
```

Thêm `--output report.json` để lưu báo cáo và `--chart-data chart.json` để lưu dữ liệu biểu đồ. `--auto-detect` lấy danh sách model từ API; `--timeout` và `--concurrency` chỉnh thời gian chờ và số luồng.

## Giao diện lúc phát triển

Cửa sổ Tauri và trình duyệt dùng chung một giao diện. Muốn xem nhanh bằng trình duyệt (thanh công cụ DevTools quen thuộc, không cần build lại):

```bash
cargo run --bin dev-server
```

Mở http://127.0.0.1:8686. Lệnh này phục vụ `ui/` và bày các thao tác giống hệt qua HTTP, chỉ dùng khi phát triển; ứng dụng desktop không mở cổng nào.

## Tự kiểm tra

```bash
cargo test --workspace            # test lõi + test với server OpenAI giả

BMT_SELFTEST=1 BMT_DATA_DIR=/tmp/bmt-appdata cargo run --release
BMT_UITEST=1   BMT_DATA_DIR=/tmp/bmt-appdata cargo run --release
```

`BMT_SELFTEST` chạy đúng các lệnh IPC (đọc/ghi profile, xem trước model, chạy test, dừng giữa chừng, lưu báo cáo) rồi in kết quả. `BMT_UITEST` thao tác thật trong cửa sổ: gõ form, bấm Bulk Test, xem bảng và biểu đồ, lưu rồi xoá profile. Cả hai tự thoát khi xong và ghi kết quả ra `/tmp/bmt-selftest.json` và `/tmp/bmt-uitest.json`.

## Ghi chú kỹ thuật

- Số model chạy song song lấy từ thanh trượt, tối đa 32. Lõi cũng chặn cứng ở 64.
- Ứng dụng gửi yêu cầu bằng một HTTP client dùng chung nên danh sách model dài vẫn chạy song song thật.
- Ở mức cao, thời gian đo được có thể phản ánh tải của server thay vì tốc độ thật của model
- Lọc bỏ các model không phục vụ chat: embedding, whisper, tts, dall-e, moderation, rerank…
- Mỗi model gửi một chat completion với `max_tokens=16`, `temperature=0`
- Tiến độ truyền từ Rust sang giao diện qua kênh IPC; bản chạy trong trình duyệt dùng Server-Sent Events
- Model có thể trả nội dung ở `message.content` hoặc `message.reasoning`; cả hai đều được hiển thị
- Tab đang mở và báo cáo gần nhất được nhớ trong `localStorage`; danh sách profile nằm ở file JSON
- Token không bao giờ được ghi vào `localStorage`

## Bản Python cũ

`legacy/` giữ nguyên bản Flask + PyInstaller trước đây để tham chiếu:

```bash
cd legacy && uv pip install -r requirements.txt && BMT_DATA_DIR=../data uv run app.py
```
