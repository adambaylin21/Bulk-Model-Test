/* Tiện ích dùng chung cho giao diện. */

export const $ = (id) => document.getElementById(id);

export const esc = (value) =>
  String(value ?? "").replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));

export const fmtMs = (ms) =>
  ms >= 1000 ? `${(ms / 1000).toFixed(2)} s` : `${Math.round(ms)} ms`;

export function fmtDate(iso) {
  if (!iso) return "—";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString("vi-VN");
}

/** Che token, chỉ giữ vài ký tự đầu và cuối. */
export function maskToken(token) {
  if (!token) return "chưa có token";
  if (token.length <= 10) return "•".repeat(token.length);
  return `${token.slice(0, 4)}…${token.slice(-4)}`;
}
