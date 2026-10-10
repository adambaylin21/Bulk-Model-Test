/* Điểm khởi động giao diện: nối tab, tab kiểm tra và tab profile. */

import { $ } from "./utils.js";
import { isDesktop } from "./bridge.js";
import * as api from "./api.js";
import { initTabs } from "./ui/tabs.js";
import { initRun, applyProfile, collectForm, runIfRequested } from "./run.js";
import { initProfiles, activeProfile, saveFromForm, rememberFormValues } from "./profiles.js";

async function main() {
  await showDataLocation();

  initTabs((name) => {
    if (name === "profiles") $("profile-search")?.focus({ preventScroll: true });
  });

  initRun({
    getProfileId: () => activeProfile()?.id || "",
    saveActiveProfile: rememberFormValues,
  });

  const remembered = await initProfiles({ applyProfile, collectForm });
  if (remembered) applyProfile(remembered);

  wireReportAndDataButtons();
  runIfRequested();
}

/** Bản desktop đọc file trong thư mục dữ liệu; bản trình duyệt thì ghi ở backend. */
async function showDataLocation() {
  try {
    const data = await api.bootstrap();
    $("profile-storage-note").textContent = data.path
      ? data.path
      : `data/${data.storage || "profiles.json"}`;
  } catch (_) { /* tab Profile sẽ báo lỗi khi tự đọc */ }
}

function wireReportAndDataButtons() {
  $("report-save-btn").addEventListener("click", async () => {
    const report = lastReport();
    if (!report) return;
    const path = await api.saveReport(report);
    if (path) setFootnote(`Đã lưu báo cáo vào ${path}`);
  });

  const openButton = $("data-open-btn");
  if (!isDesktop) {
    // Mở thư mục dữ liệu chỉ có ý nghĩa trong ứng dụng desktop.
    openButton.hidden = true;
    return;
  }
  openButton.hidden = false;
  openButton.addEventListener("click", async () => {
    await api.openDataDir();
    setFootnote("Đã mở thư mục dữ liệu.");
  });
}

function lastReport() {
  try {
    return JSON.parse(localStorage.getItem("bmt.lastRun") || "null");
  } catch (_) {
    return null;
  }
}

function setFootnote(text) {
  $("app-footnote").textContent = text;
}

main();
