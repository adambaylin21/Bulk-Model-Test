/* Tab kiểm tra model: form, tiến độ và báo cáo. */

import * as api from "./api.js";
import { $, esc, fmtMs } from "./utils.js";

const FORM_KEY = "bmt.form";
const REPORT_KEY = "bmt.lastRun";

let chart = null;
let jobId = null;
let rows = [];
let total = 0;
let doneCount = 0;
let running = false;
let cancelled = false;
let getActiveProfileId = () => "";
let saveFormQuietly = () => {};
let afterRun = () => {};

/* ------------------------------------------------------ form state (dùng chung) */

/** Đọc các trường ở cột Thiết lập. */
export function collectForm() {
  return {
    base_url: $("base-url").value.trim(),
    api_key: $("api-key").value.trim(),
    model_name: $("model-name").value.trim(),
    test_prompt: $("test-prompt").value.trim() || "Hello",
    timeout: Number($("timeout").value) || 30,
    concurrency: Number($("concurrency").value) || 8,
    auto_detect: $("auto-detect").checked,
    auto_start: $("auto-start").checked,
  };
}

/** Đổ một profile vào cột Thiết lập. */
export function applyProfile(profile) {
  $("base-url").value = profile.base_url || "";
  $("api-key").value = profile.api_key || "";
  $("model-name").value = profile.model_name || "";
  $("test-prompt").value = profile.test_prompt || "Hello";
  $("timeout").value = profile.timeout ?? 30;
  $("concurrency").value = profile.concurrency ?? 8;
  $("auto-detect").checked = profile.auto_detect !== false;
  $("auto-start").checked = !!profile.auto_start;

  syncConcurrency();
  syncModelField();
  saveForm();
}

/* -------------------------------------------------------------------- UI */

function setStatus(state, text, meta = "") {
  $("status-strip").dataset.state = state;
  $("status-text").textContent = text;
  $("status-meta").textContent = meta;
}

function setRunning(on) {
  running = on;
  $("bulk-test-btn").disabled = on;
  $("cancel-btn").hidden = !on;
  $("bulk-test-btn").innerHTML = on ? `<span class="spinner"></span>Đang test…` : "Bulk Test";
}

function sortedRows() {
  const ok = rows.filter((r) => r.status === "success")
    .sort((a, b) => a.response_time_ms - b.response_time_ms);
  const rest = rows.filter((r) => r.status !== "success")
    .sort((a, b) => a.response_time_ms - b.response_time_ms);
  return [...ok, ...rest];
}

function renderTable() {
  if (!rows.length) return;
  const html = sortedRows()
    .map((m, i) => {
      const state = m.status === "success" ? "ok" : m.status === "error" ? "bad" : "pending";
      const label = { success: "OK", error: "Lỗi", pending: "…", running: "…" }[m.status] ?? "…";
      const time = m.status === "pending" || m.status === "running" ? "–" : fmtMs(m.response_time_ms);
      const detail =
        m.status === "success" ? esc((m.test_result || "").slice(0, 100))
        : m.status === "error" ? esc(m.error || "Không rõ nguyên nhân")
        : "";
      return `<tr>
        <td class="rank">${i + 1}</td>
        <td class="name">${esc(m.model)}</td>
        <td class="time">${time}</td>
        <td class="state ${state}">${label}</td>
        <td class="detail">${detail}</td>
      </tr>`;
    })
    .join("");
  $("report-output").innerHTML = `<table class="results-table">
    <thead><tr><th>#</th><th>Model</th><th>Thời gian</th><th>Trạng thái</th><th>Chi tiết</th></tr></thead>
    <tbody>${html}</tbody></table>`;
}

function updateStats() {
  const ok = rows.filter((r) => r.status === "success");
  const bad = rows.filter((r) => r.status === "error");
  const times = ok.map((r) => r.response_time_ms);
  $("stat-total").textContent = total;
  $("stat-success").textContent = ok.length;
  $("stat-error").textContent = bad.length;
  $("stat-avg").textContent = times.length
    ? fmtMs(times.reduce((a, b) => a + b, 0) / times.length) : "–";
  $("stat-fastest").textContent = times.length ? fmtMs(Math.min(...times)) : "–";
}

function renderChart() {
  const finished = rows.filter((r) => r.status === "success" || r.status === "error");
  if (!finished.length) return;

  const data = sortedRows();
  const labels = data.map((r) => r.model);
  const values = data.map((r) => r.response_time_ms);
  const colors = data.map((r) => (r.status === "success" ? "#1f9d55" : "#d93a2b"));

  $("chart-empty").hidden = true;
  $("response-chart").hidden = false;

  if (chart) {
    chart.data.labels = labels;
    chart.data.datasets[0].data = values;
    chart.data.datasets[0].backgroundColor = colors;
    chart.update();
    return;
  }

  chart = new Chart($("response-chart").getContext("2d"), {
    type: "bar",
    data: {
      labels,
      datasets: [{
        label: "Thời gian phản hồi",
        data: values,
        backgroundColor: colors,
        borderRadius: 4,
        maxBarThickness: 48,
      }],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      animation: { duration: 300, easing: "easeOutQuart" },
      scales: {
        y: { beginAtZero: true, title: { display: true, text: "Thời gian phản hồi (ms)" } },
        x: {
          ticks: { autoSkip: false, maxRotation: 60, minRotation: 30, font: { size: 10 } },
          grid: { display: false },
        },
      },
      plugins: {
        legend: { display: false },
        tooltip: {
          callbacks: {
            label: (ctx) => {
              const row = rows.find((x) => x.model === ctx.label);
              if (!row) return fmtMs(ctx.parsed.y);
              return row.status === "success"
                ? `${fmtMs(ctx.parsed.y)} — OK`
                : `Lỗi sau ${fmtMs(ctx.parsed.y)}`;
            },
          },
        },
      },
    },
  });
}

function resetPanels() {
  rows = [];
  total = 0;
  doneCount = 0;
  cancelled = false;
  $("progress").hidden = false;
  $("progress-fill").style.width = "0%";
  $("progress-count").textContent = "0 / 0";
  updateStats();
  $("report-output").innerHTML = `<p class="empty-state">Đang chạy…</p>`;
  $("response-chart").hidden = true;
  $("chart-empty").hidden = false;
  if (chart) { chart.destroy(); chart = null; }
}

function saveForm() {
  localStorage.setItem(FORM_KEY, JSON.stringify(collectForm()));
  saveFormQuietly();
}

/* ----------------------------------------------------------------- START */

async function start() {
  if (running) return;
  const params = { ...collectForm(), profile_id: getActiveProfileId() };

  if (!params.base_url || !params.api_key) {
    return fail("Vui lòng nhập Base Url và API Key.");
  }
  if (!params.model_name && !params.auto_detect) {
    return fail("Nhập tên model hoặc bật Auto Detect Model.");
  }

  saveForm();
  resetPanels();
  setRunning(true);
  setStatus("running", "Đang kết nối…");

  try {
    jobId = await api.startRun(params, handleEvent);
  } catch (error) {
    finish("error", `Không chạy được: ${error.message}`);
    return;
  }
  if (!jobId && running) finish("error", "Không nhận được mã phiên chạy.");
}

function fail(message) {
  $("report-output").innerHTML = `<p class="error error-pad">${esc(message)}</p>`;
  setStatus("error", message);
}

/* --------------------------------------------------------- tiến độ & kết quả */

function handleEvent(msg) {
  switch (msg.event) {
    case "job":
      jobId = msg.job_id || jobId;
      setStatus("running", "Đang tải danh sách model…");
      break;

    case "status":
      setStatus("running", msg.message);
      break;

    case "models": {
      rows = (msg.models || []).map((m) => ({
        model: m, status: "pending", response_time_ms: 0, error: null, test_result: null,
      }));
      total = rows.length;
      setStatus("running", `Đang test ${total} model…`);
      renderTable();
      updateStats();
      break;
    }

    case "start":
      total = msg.total;
      $("progress-count").textContent = `0 / ${msg.total}`;
      if (msg.concurrency) {
        setStatus("running", `Đang test với ${msg.concurrency} luồng song song…`);
      }
      break;

    case "model_start": {
      const row = rows.find((r) => r.model === msg.model);
      if (row) row.status = "running";
      setStatus("running", `Đang chờ ${msg.model}…`);
      break;
    }

    case "model_done": {
      const idx = rows.findIndex((r) => r.model === msg.model);
      if (idx >= 0) rows[idx] = { ...rows[idx], ...msg.result };
      else rows.push(msg.result);
      doneCount = msg.completed;
      total = msg.total;
      $("progress-fill").style.width = `${Math.round((doneCount / total) * 100)}%`;
      $("progress-count").textContent = `${doneCount} / ${total}`;
      setStatus("running", `Xong ${doneCount}/${total} — ${msg.model}`);
      updateStats();
      renderTable();
      renderChart();
      break;
    }

    case "done":
      rows = msg.report.models.map((m) => ({
        model: m.name,
        status: m.status,
        response_time_ms: m.response_time_ms,
        error: m.error,
        test_result: m.test_result,
      }));
      total = msg.report.total_models;
      doneCount = total;
      finish("done",
        `Hoàn tất — ${msg.report.successful} OK, ${msg.report.errors} lỗi · TB ${fmtMs(msg.report.avg_response_time_ms)}`,
        new Date(msg.report.timestamp).toLocaleTimeString("vi-VN"));
      localStorage.setItem(REPORT_KEY, JSON.stringify(msg.report));
      break;

    case "cancelled":
      finish("cancelled", `Đã dừng sau ${msg.completed} model`);
      break;

    case "closed":
      if (running) finish(cancelled ? "cancelled" : "error",
        cancelled ? "Đã dừng thủ công" : "Server đóng kết nối trước khi test xong.");
      break;

    case "error":
      finish("error", msg.message);
      break;
  }
}

function finish(state, text, meta = "") {
  setRunning(false);
  setStatus(state, text, meta);
  jobId = null;
  if (state === "done") {
    $("progress-fill").style.width = "100%";
    $("progress-count").textContent = `${doneCount} / ${total}`;
    updateStats();
    renderTable();
    renderChart();
  } else if (state === "error") {
    $("report-output").innerHTML = `<p class="error error-pad">${esc(text)}</p>`;
    $("response-chart").hidden = true;
    $("chart-empty").hidden = false;
  }
  afterRun();
}

async function cancelRun() {
  cancelled = true;
  try {
    await api.cancelRun(jobId || "");
  } catch (_) { /* phiên chạy có thể đã dừng */ }
  if (running) finish("cancelled", "Đã dừng thủ công");
}

/* ------------------------------------------------------------------ wiring */

function syncModelField() {
  const manual = !$("auto-detect").checked;
  $("model-name").disabled = !manual;
  $("model-name").placeholder = manual ? "gpt-4, llama-3.1-70b" : "Đang bật Auto Detect";
  const n = $("model-name").value.split(/[\n,]+/).map((s) => s.trim()).filter(Boolean).length;
  $("model-count").textContent = manual
    ? (n ? `${n} model sẽ được test` : "Mỗi dòng một model, cách nhau bằng dấu phẩy")
    : "Sẽ test toàn bộ model của API key";
}

function syncConcurrency() {
  const n = Number($("concurrency").value) || 1;
  $("concurrency-value").textContent = n;
  $("concurrency-hint").textContent =
    n <= 2 ? "Tuần tự từng model một — đo chính xác nhất."
    : n <= 8 ? "Mức cân bằng, phù hợp hầu hết API."
    : n <= 16 ? "Nhanh hơn, nhưng thời gian đo có thể bị ảnh hưởng do tải."
    : "Rất cao — dễ bị giới hạn tốc độ hoặc nghẽn.";
}

function restoreReport() {
  let report = null;
  try {
    report = JSON.parse(localStorage.getItem(REPORT_KEY) || "null");
  } catch (_) { /* báo cáo cũ hỏng thì bỏ qua */ }
  if (!report?.models?.length) return;

  rows = report.models.map((m) => ({
    model: m.name,
    status: m.status,
    response_time_ms: m.response_time_ms,
    error: m.error,
    test_result: m.test_result,
  }));
  total = report.total_models;
  doneCount = total;
  updateStats();
  renderTable();
  renderChart();
  $("progress").hidden = false;
  $("progress-fill").style.width = "100%";
  $("progress-count").textContent = `${total} / ${total}`;

  const who = report.profile?.name ? ` · ${report.profile.name}` : "";
  setStatus("idle", `Báo cáo gần nhất${who}`,
    new Date(report.timestamp).toLocaleString("vi-VN"));
}

export function initRun({ getProfileId, saveActiveProfile, onRunFinished } = {}) {
  getActiveProfileId = getProfileId || (() => "");
  saveFormQuietly = saveActiveProfile || (() => {});
  afterRun = onRunFinished || (() => {});

  $("test-form").addEventListener("submit", (e) => { e.preventDefault(); start(); });
  $("cancel-btn").addEventListener("click", cancelRun);
  $("auto-detect").addEventListener("change", syncModelField);
  $("model-name").addEventListener("input", syncModelField);
  $("concurrency").addEventListener("input", syncConcurrency);

  for (const id of ["base-url", "model-name", "test-prompt", "timeout", "auto-start"]) {
    $(id).addEventListener("change", saveForm);
  }

  let saved = null;
  try {
    saved = JSON.parse(localStorage.getItem(FORM_KEY) || "null");
  } catch (_) { /* dữ liệu cũ hỏng thì dùng mặc định */ }

  if (saved) {
    $("base-url").value = saved.base_url || "";
    $("model-name").value = saved.model_name || "";
    $("test-prompt").value = saved.test_prompt || "Hello";
    $("auto-detect").checked = saved.auto_detect !== false;
    $("timeout").value = saved.timeout || 30;
    $("concurrency").value = saved.concurrency || 8;
    $("auto-start").checked = !!saved.auto_start;
  }
  syncConcurrency();
  syncModelField();
  restoreReport();

  return { isRunning: () => running };
}

/** Chạy test nếu người dùng đã bật "Chạy lại khi mở trang". */
export function runIfRequested() {
  if ($("auto-start").checked) start();
}
