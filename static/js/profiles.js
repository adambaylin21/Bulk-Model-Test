/* Tab Profile: CRUD profile lưu trong data/profiles.json và thanh chọn nhanh. */

import { api } from "./api.js";
import { $, esc, fmtDate, maskToken } from "./utils.js";

const ACTIVE_KEY = "bmt.activeProfile";

let profiles = [];
let editingId = "";
let activeId = "";
let applyProfile = () => {};
let collectForm = () => ({});

export function getProfile(id) {
  return profiles.find((p) => p.id === id) || null;
}

export function activeProfile() {
  return getProfile(activeId);
}

export function activeProfileId() {
  return activeId;
}

/* -------------------------------------------------------------- rendering */

function describeModels(profile) {
  if (profile.auto_detect !== false) return "Auto Detect toàn bộ model";
  const count = (profile.model_name || "").split(/[\n,]+/).filter((s) => s.trim()).length;
  return count ? `${count} model nhập tay` : "Chưa nhập model";
}

function profileCard(profile) {
  const isActive = profile.id === activeId;
  const title = esc(profile.name || "Profile chưa đặt tên");

  return `<article class="profile-card${isActive ? " is-active" : ""}" data-id="${esc(profile.id)}">
    <header class="profile-card-head">
      <h3>${title}</h3>
      ${isActive ? '<span class="profile-chip">đang dùng</span>' : ""}
    </header>
    <dl class="profile-meta">
      <div><dt>Base Url</dt><dd class="mono">${esc(profile.base_url || "—")}</dd></div>
      <div><dt>Token</dt><dd class="mono">${esc(maskToken(profile.api_key))}</dd></div>
      <div><dt>Model</dt><dd>${esc(describeModels(profile))}</dd></div>
      <div><dt>Timeout</dt><dd>${esc(profile.timeout)}s · ${esc(profile.concurrency)} luồng · prompt “${esc(profile.test_prompt || "Hello")}”</dd></div>
      <div><dt>Cập nhật</dt><dd>${esc(fmtDate(profile.updated_at || profile.created_at))}</dd></div>
      ${profile.notes ? `<div><dt>Ghi chú</dt><dd>${esc(profile.notes)}</dd></div>` : ""}
    </dl>
    <div class="profile-card-actions">
      <button class="mini-button accent" type="button" data-action="use">Dùng ngay</button>
      <button class="mini-button" type="button" data-action="edit">Sửa</button>
      <button class="mini-button danger" type="button" data-action="delete">Xoá</button>
    </div>
  </article>`;
}

function renderList() {
  const list = $("profile-list");
  const query = ($("profile-search")?.value || "").trim().toLowerCase();
  $("tab-profile-count").textContent = String(profiles.length);

  if (!profiles.length) {
    list.innerHTML = `<p class="empty-state">Chưa có profile nào được lưu.
      Điền biểu mẫu bên trái rồi bấm <strong>Lưu profile</strong>.</p>`;
    return;
  }

  const shown = profiles.filter((p) =>
    !query || `${p.name} ${p.base_url}`.toLowerCase().includes(query));

  list.innerHTML = shown.length
    ? shown.map(profileCard).join("")
    : `<p class="empty-state">Không có profile nào khớp “${esc(query)}”.</p>`;
}

function renderSelect() {
  const select = $("profile-select");
  const known = (id) => profiles.some((p) => p.id === id);
  const previous = select.value;

  select.innerHTML = profiles.length
    ? [`<option value="">— Chọn profile —</option>`].concat(
        profiles.map((p) =>
          `<option value="${esc(p.id)}">${esc(p.name || p.base_url)}</option>`)).join("")
    : `<option value="">— Chưa lưu profile nào —</option>`;

  // Ô chọn phải phản chiếu profile đang dùng; chỉ giữ lựa chọn cũ khi nó là profile đang dùng.
  if (known(activeId)) select.value = activeId;
  else if (known(previous)) select.value = previous;
  else select.value = "";
}

function setActive(id) {
  activeId = id || "";
  if (activeId) localStorage.setItem(ACTIVE_KEY, activeId);
  else localStorage.removeItem(ACTIVE_KEY);
  renderList();
  renderSelect();
}

function message(text, kind = "") {
  const el = $("profile-form-message");
  el.textContent = text;
  el.dataset.kind = kind;
}

/* ------------------------------------------------------------- form <-> state */

function readForm() {
  return {
    id: editingId,
    name: $("profile-name").value.trim(),
    base_url: $("profile-base-url").value.trim(),
    api_key: $("profile-api-key").value.trim(),
    model_name: $("profile-model-name").value.trim(),
    test_prompt: $("profile-prompt").value.trim() || "Hello",
    timeout: Number($("profile-timeout").value) || 30,
    concurrency: Number($("profile-concurrency").value) || 8,
    auto_detect: $("profile-auto-detect").checked,
    auto_start: $("profile-auto-start").checked,
    notes: $("profile-notes").value.trim(),
  };
}

function fillForm(profile) {
  editingId = profile?.id || "";
  $("profile-name").value = profile?.name || "";
  $("profile-base-url").value = profile?.base_url || "";
  $("profile-api-key").value = profile?.api_key || "";
  $("profile-model-name").value = profile?.model_name || "";
  $("profile-prompt").value = profile?.test_prompt || "Hello";
  $("profile-timeout").value = profile?.timeout ?? 30;
  $("profile-concurrency").value = profile?.concurrency ?? 8;
  $("profile-auto-detect").checked = profile ? profile.auto_detect !== false : true;
  $("profile-auto-start").checked = !!profile?.auto_start;
  $("profile-notes").value = profile?.notes || "";

  $("profile-editor-title").textContent = profile
    ? `Sửa profile: ${profile.name || profile.base_url}`
    : "Thêm profile mới";
  $("profile-editor-state").textContent = profile ? `ID ${profile.id}` : "Chưa lưu";
  $("profile-submit-btn").textContent = profile ? "Cập nhật profile" : "Lưu profile";
}

async function refresh() {
  try {
    const data = await api.listProfiles();
    profiles = data.profiles || [];
    $("profile-storage-note").textContent = `data/${data.storage || "profiles.json"}`;
    renderList();
    renderSelect();
  } catch (err) {
    $("profile-list").innerHTML =
      `<p class="error error-pad">Không đọc được file profile: ${esc(err.message)}</p>`;
  }
}

/* ------------------------------------------------------------------ actions */

async function submitForm(event) {
  event.preventDefault();
  const payload = readForm();
  if (!payload.base_url) {
    message("Cần nhập Base Url.", "bad");
    return;
  }
  try {
    const { profile } = await api.saveProfile(payload);
    await refresh();
    fillForm(null);
    setActive(profile.id);
    message(`Đã lưu “${profile.name}” vào data/profiles.json.`, "ok");
  } catch (err) {
    message(`Không lưu được: ${err.message}`, "bad");
  }
}

async function removeProfile(id) {
  const profile = getProfile(id);
  const label = profile?.name || id;
  if (!confirm(`Xoá profile “${label}”? Thao tác này không hoàn tác được.`)) return;

  try {
    await api.deleteProfile(id);
    if (editingId === id) fillForm(null);
    if (activeId === id) setActive("");
    await refresh();
    message(`Đã xoá “${label}”.`, "ok");
  } catch (err) {
    message(`Không xoá được: ${err.message}`, "bad");
  }
}

function useProfile(profile) {
  applyProfile(profile);
  setActive(profile.id);
  api.markUsed(profile.id).catch(() => {});
}

/** Lưu các giá trị đang có ở cột Thiết lập thành profile. */
async function saveFromForm() {
  const existing = getProfile($("profile-select").value) || getProfile(editingId);
  const { base_url, api_key, model_name, test_prompt, timeout, concurrency,
    auto_detect, auto_start } = collectForm();

  if (!base_url) {
    message("Cần nhập Base Url ở tab Kiểm tra model trước.", "bad");
    return;
  }

  try {
    const { profile } = await api.saveProfile({
      id: existing?.id || "",
      name: existing?.name || base_url,
      base_url, api_key, model_name, test_prompt, timeout, concurrency,
      auto_detect, auto_start,
      notes: existing?.notes || "",
    });
    await refresh();
    setActive(profile.id);
    message(`Đã lưu “${profile.name}” vào data/profiles.json.`, "ok");
  } catch (err) {
    message(`Không lưu được: ${err.message}`, "bad");
  }
}

/* --------------------------------------------------------------------- init */

export function initProfiles({ applyProfile: apply, collectForm: collect } = {}) {
  applyProfile = apply || (() => {});
  collectForm = collect || (() => ({}));

  $("profile-form").addEventListener("submit", submitForm);
  $("profile-reset-btn").addEventListener("click", () => { fillForm(null); message(""); });
  $("profile-refresh-btn").addEventListener("click", refresh);
  $("profile-search").addEventListener("input", renderList);

  $("profile-list").addEventListener("click", (event) => {
    const button = event.target.closest("button[data-action]");
    if (!button) return;
    const id = button.closest(".profile-card").dataset.id;
    const profile = getProfile(id);
    if (!profile) return;

    if (button.dataset.action === "use") useProfile(profile);
    else if (button.dataset.action === "edit") { fillForm(profile); $("profile-name").focus(); }
    else if (button.dataset.action === "delete") removeProfile(id);
  });

  $("profile-select").addEventListener("change", (event) => {
    const profile = getProfile(event.target.value);
    if (profile) useProfile(profile);
    else setActive("");
  });

  $("profile-load-btn").addEventListener("click", () => {
    const profile = getProfile($("profile-select").value);
    if (profile) useProfile(profile);
    else message("Chọn một profile trong danh sách trước.", "bad");
  });

  $("profile-save-btn").addEventListener("click", saveFromForm);

  return refresh().then(() => {
    const remembered = localStorage.getItem(ACTIVE_KEY);
    if (remembered && getProfile(remembered)) setActive(remembered);
    return activeProfile();
  });
}
