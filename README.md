# Bulk Model Test

Đo thời gian phản hồi của mọi model mà một API key OpenAI-compatible chạm được, rồi báo cáo xếp từ nhanh đến chậm. Có thể lưu nhiều profile (nhiều token) để chuyển qua lại nhanh.

## Chạy

```bash
uv venv
uv pip install -r requirements.txt
uv run app.py
```

Mở http://localhost:8686

## Cấu trúc thư mục

```
app.py              điểm khởi động Flask
backend/            package backend
  config.py         đường dẫn, hằng số, bộ lọc model không phục vụ chat
  params.py         chuẩn hoá tham số chạy và dữ liệu profile
  profiles.py       đọc/ghi data/profiles.json
  runner.py         gọi API, chạy bulk test, phát tiến độ
  routes.py         các route HTTP
static/             giao diện
  index.html
  styles.css
  js/main.js        khởi động, nối các tab
  js/run.js         tab Kiểm tra model
  js/profiles.js    tab Profile
  js/api.js         gọi endpoint backend
  js/utils.js       tiện ích chung
  js/ui/tabs.js     chuyển tab
cli/bulk_test.py    bản dòng lệnh
data/profiles.json  nơi lưu profile (tự tạo khi lưu lần đầu)
```

## Hai tab

### Tab Kiểm tra model

1. **Profile đã lưu** — chọn một profile rồi bấm **Nạp** để điền sẵn mọi trường, hoặc bấm **Lưu lại** để lưu các giá trị đang nhập thành profile
2. **Base Url** — endpoint tương thích OpenAI SDK, thường kết thúc bằng `/v1`
3. **API Key** — khoá của bạn, gửi tới backend để gọi API
4. **Tên Model** — bỏ trống khi muốn test toàn bộ, hoặc ghi mỗi model một dòng
5. **Auto Detect Model** — bật để lấy danh sách từ `/v1/models`
6. **Prompt Test** — câu hỏi gửi tới, mặc định `Hello`
7. **Timeout** — giây chờ mỗi model, mặc định 30
8. **Test đồng thời** — số model chạy song song, từ 1 đến 32

Trong lúc chạy, bảng cập nhật từng dòng, thanh tiến độ đếm số model đã xong, biểu đồ vẽ dần. Nút **Dừng** huỷ phiên test.

### Tab Profile

Lưu nhiều bộ tuỳ chọn, mỗi bộ gồm tên, Base Url, token, danh sách model, prompt, timeout, số luồng và ghi chú.

- **Lưu profile** — thêm mới, hoặc cập nhật khi đang sửa một profile có sẵn
- **Dùng ngay** — nạp profile vào tab Kiểm tra model
- **Sửa** — mở profile ra biểu mẫu bên trái
- **Xoá** — bỏ profile, có hỏi xác nhận
- **Tìm theo tên hoặc Base Url** — lọc danh sách đang hiển thị

Token hiện trong danh sách ở dạng che bớt (`gsk_…mnop`). Bỏ trống ô Token lúc cập nhật thì token cũ được giữ nguyên. Đổi Base Url rồi bấm **Lưu lại** sẽ ghi đè token đang nhập.

Dữ liệu nằm trong `data/profiles.json`:

```json
{
  "version": 1,
  "updated_at": "2026-10-10T17:20:30",
  "profiles": [
    {
      "id": "541665d154cd",
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

Ghi file theo cách nguyên tử (ghi file tạm rồi đổi tên) nên không hỏng dữ liệu khi tiến trình dừng giữa chừng. Thư mục `data/` nằm ngoài mã nguồn và không được commit.

**Lưu ý bảo mật:** tool lưu token dạng thô trong `data/profiles.json`. Đây là công cụ chạy trên máy cá nhân; đừng dùng chung file này hoặc commit nó lên.

## Kết quả

Bảng xếp model thành công theo thời gian tăng dần, các model lỗi nằm cuối. Mỗi dòng hiện thời gian, trạng thái và nội dung trả về (hoặc thông báo lỗi). Cửa sổ báo cáo cao tối thiểu 460px và tối đa 72% chiều cao màn hình.

Biểu đồ cột tương ứng: xanh là thành công, đỏ là lỗi. Trục ngang là tên model, trục dọc là mili giây.

## Bản dòng lệnh

```bash
uv run cli/bulk_test.py --base-url http://localhost:8899/v1 --api-key mock --test-prompt "Hello"
```

Thêm `--output report.json` để lưu báo cáo và `--chart-data chart.json` để lưu dữ liệu biểu đồ.

## API

| Endpoint | Method | Mô tả |
| --- | --- | --- |
| `/` | GET | Giao diện |
| `/api/profiles` | GET | Danh sách profile đã lưu |
| `/api/profiles` | POST | Thêm mới hoặc cập nhật một profile |
| `/api/profiles/<id>` | DELETE | Xoá một profile |
| `/api/profiles/<id>/used` | POST | Ghi nhận thời điểm dùng gần nhất |
| `/api/test/stream` | POST | Chạy và stream tiến độ (SSE) |
| `/api/test/bulk` | POST | Chạy xong rồi mới trả kết quả |
| `/api/test/cancel` | POST | Dừng phiên đang chạy |
| `/api/models` | POST | Xem trước danh sách model |
| `/api/models/parse` | POST | Đếm số model nhập tay |

Body cho các endpoint POST chạy test:

```json
{
  "base_url": "https://api.example.com/v1",
  "api_key": "sk-...",
  "model_name": "gpt-4, llama-3.1-70b",
  "auto_detect": true,
  "test_prompt": "Hello",
  "timeout": 30,
  "concurrency": 8,
  "profile_id": "541665d154cd"
}
```

`model_name` và `auto_detect` chỉ cần một trong hai. `profile_id` là tuỳ chọn: khi Base Url trùng với profile đã lưu, báo cáo ghi kèm tên profile.

Body để lưu profile:

```json
{
  "id": "541665d154cd",
  "name": "Mock local",
  "base_url": "http://localhost:8899/v1",
  "api_key": "mock-key",
  "model_name": "",
  "test_prompt": "Hello",
  "timeout": 30,
  "concurrency": 4,
  "auto_detect": true,
  "auto_start": false,
  "notes": "key của team"
}
```

Bỏ `id` để thêm mới. Bỏ `api_key` khi cập nhật để giữ token cũ. Base Url là bắt buộc.

## Ghi chú kỹ thuật

- Số model chạy song song lấy từ thanh trượt, tối đa 32. Backend cũng chặn cứng ở 64.
- Test song song tốn tài nguyên hơn nhưng cho kết quả nhanh hơn
- Ở mức cao, thời gian đo được có thể phản ánh tải của server thay vì tốc độ thật của model
- Lọc bỏ các model không phục vụ chat: embedding, whisper, tts, dall-e, moderation, rerank…
- Mỗi model gửi một chat completion với `max_tokens=16`, `temperature=0`
- Tiến độ truyền qua Server-Sent Events tại `POST /api/test/stream`
- Tab đang mở và báo cáo gần nhất được nhớ trong `localStorage`; danh sách profile nằm ở backend

## Cổng

Mặc định 8686. Đổi bằng biến môi trường:

```bash
PORT=5000 uv run app.py
```
