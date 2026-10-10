/* Các thao tác nghiệp vụ của giao diện: profile, danh sách model, lượt chạy.
 *
 * Mỗi hàm đi qua lệnh Tauri khi chạy trong ứng dụng, và qua HTTP cục bộ khi mở
 * bằng trình duyệt. Đổi lại, phần giao diện chỉ gọi một API duy nhất.
 */

import { isDesktop, invokeTauri, newChannel, request } from "./bridge.js";

/** Dữ liệu dùng chung lúc khởi động: đường dẫn lưu profile, tên file… */
export async function bootstrap() {
  if (isDesktop) return invokeTauri("bootstrap");
  return request("/api/profiles");
}

export async function listProfiles() {
  if (isDesktop) return invokeTauri("profiles_list");
  const data = await request("/api/profiles");
  return { ...data, path: null };
}

export async function saveProfile(profile) {
  const payload = { input: profile };
  if (isDesktop) {
    const data = await invokeTauri("profile_save", payload);
    return { profile: data.profile };
  }
  return request("/api/profiles", { method: "POST", body: JSON.stringify(profile) });
}

export async function deleteProfile(id) {
  if (isDesktop) {
    const data = await invokeTauri("profile_delete", { profileId: id });
    return { deleted: data.deleted, id };
  }
  return request(`/api/profiles/${encodeURIComponent(id)}`, { method: "DELETE" });
}

export async function markUsed(id) {
  if (isDesktop) return invokeTauri("profile_touch", { profileId: id });
  return request(`/api/profiles/${encodeURIComponent(id)}/used`, { method: "POST", body: "{}" });
}

/** Số model người dùng gõ tay. */
export async function parseModels(modelName) {
  if (isDesktop) return invokeTauri("models_parse", { modelName });
  return request("/api/models/parse", { method: "POST", body: JSON.stringify({ model_name: modelName }) });
}

/** Xem trước danh sách model mà API key chạm được. */
export async function previewModels(params) {
  if (isDesktop) return invokeTauri("models_preview", { params });
  return request("/api/models", { method: "POST", body: JSON.stringify(params) });
}

/**
 * Bắt đầu một lượt chạy và gọi `onEvent` cho từng sự kiện tiến độ.
 * Trả về id của job để dừng khi cần.
 */
export async function startRun(params, onEvent) {
  if (isDesktop) {
    const channel = newChannel(onEvent);
    return invokeTauri("test_start", { params, onEvent: channel });
  }
  return startRunOverSse(params, onEvent);
}

export async function cancelRun(jobId) {
  if (isDesktop) {
    const data = await invokeTauri("test_cancel", { jobId });
    return { cancelled: data.cancelled };
  }
  return request("/api/test/cancel", { method: "POST", body: JSON.stringify({ job_id: jobId }) });
}

/** Lưu báo cáo ra file JSON; trình duyệt thì tải xuống. */
export async function saveReport(report) {
  const name = `bmt-report-${stamp(report.timestamp)}.json`;
  const contents = JSON.stringify(report, null, 2);
  if (isDesktop) {
    const path = await invokeTauri("report_save", { defaultName: name, contents });
    return path;
  }
  const url = URL.createObjectURL(new Blob([contents], { type: "application/json" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
  return name;
}

/** Mở thư mục chứa profiles.json (chỉ bản desktop). */
export async function openDataDir() {
  if (!isDesktop) return null;
  return invokeTauri("data_open");
}

function stamp(iso) {
  return (iso || new Date().toISOString()).replace(/[:.]/g, "-").slice(0, 19);
}

/** Bản dự phòng khi mở giao diện bằng trình duyệt: đọc Server-Sent Events. */
function startRunOverSse(params, onEvent) {
  let resolveJob;
  const jobReady = new Promise((resolve) => { resolveJob = resolve; });

  fetch("/api/test/stream", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(params),
  })
    .then(async (response) => {
      if (!response.ok || !response.body) {
        let detail = `HTTP ${response.status}`;
        try {
          detail = (await response.json()).error || detail;
        } catch (_) { /* giữ mã trạng thái */ }
        onEvent({ event: "error", message: detail });
        resolveJob("");
        return;
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        let split;
        while ((split = buffer.indexOf("\n\n")) >= 0) {
          const chunk = buffer.slice(0, split).trim();
          buffer = buffer.slice(split + 2);
          if (!chunk || chunk.startsWith(":")) continue;
          try {
            const message = JSON.parse(chunk.replace(/^data:\s*/, ""));
            if (message.event === "job") resolveJob(message.job_id);
            onEvent(message);
          } catch (_) { /* bỏ qua gói hỏng */ }
        }
      }
      // Hết luồng mà chưa có sự kiện kết thúc nghĩa là server đã dừng.
      onEvent({ event: "closed" });
    })
    .catch((error) => {
      onEvent({ event: "error", message: `Không kết nối được server: ${error.message}` });
      resolveJob("");
    });

  return jobReady;
}
