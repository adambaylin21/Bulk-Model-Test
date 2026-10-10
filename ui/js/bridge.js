/* Cầu nối tới Tauri.
 *
 * Trong bản desktop, Tauri chèn `window.__TAURI__`. Khi mở giao diện bằng trình
 * duyệt (lúc phát triển), cùng các hàm đó nói chuyện với server HTTP cục bộ của
 * `cargo run --bin dev-server`. Giao diện không cần biết đang chạy ở đâu.
 */

const tauri = globalThis.__TAURI__;

/** `true` khi giao diện chạy trong cửa sổ Tauri. */
export const isDesktop = Boolean(tauri?.core?.invoke);

/** Gọi một lệnh Rust. */
export function invokeTauri(command, args = {}) {
  if (!isDesktop) {
    return Promise.reject(new Error("Lệnh này chỉ có trong bản desktop."));
  }
  return tauri.core.invoke(command, args);
}

/** Kênh nhận tiến độ từ Rust, chỉ có trong bản desktop. */
export function newChannel(onMessage) {
  if (!isDesktop) return null;
  const channel = new tauri.core.Channel();
  channel.onmessage = onMessage;
  return channel;
}

/** Gọi HTTP cục bộ của bản chạy trong trình duyệt. */
export async function request(url, options = {}) {
  const response = await fetch(url, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });

  let payload = null;
  try {
    payload = await response.json();
  } catch (_) {
    payload = null;
  }

  if (!response.ok) {
    throw new Error(payload?.error || `HTTP ${response.status}`);
  }
  return payload;
}

/** Mở hộp thoại lưu file; trả về đường dẫn hoặc `null` khi người dùng huỷ. */
export async function pickSavePath(defaultName) {
  const save = tauri?.dialog?.save;
  if (!save) return null;
  const path = await save({ defaultPath: defaultName });
  return path || null;
}

/** Chọn file để đọc; trả về nội dung hoặc `null` khi người dùng huỷ. */
export async function readTextFileViaDialog() {
  const open = tauri?.dialog?.open;
  const readTextFile = tauri?.fs?.readTextFile;
  if (!open || !readTextFile) return null;
  const selected = await open({ multiple: false });
  if (!selected || Array.isArray(selected)) return null;
  return readTextFile(selected);
}
