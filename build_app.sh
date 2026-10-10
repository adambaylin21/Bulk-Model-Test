#!/usr/bin/env bash
# Đóng gói Bulk Model Test thành ứng dụng macOS.
#
#   ./build_app.sh
#
# Kết quả trong target/release/bundle/:
#   macos/Bulk Model Test.app   ứng dụng có icon
#   dmg/Bulk Model Test_*.dmg   bộ cài
#
# Yêu cầu: Rust (cargo), cargo-tauri (hoặc `cargo install tauri-cli --version ^2`),
# và Xcode Command Line Tools.

set -euo pipefail

cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Không thấy cargo. Cài Rust trước: https://rustup.rs" >&2
  exit 1
fi

TAURI="cargo tauri"
if ! cargo tauri --version >/dev/null 2>&1; then
  echo "Không thấy cargo-tauri. Cài bằng:" >&2
  echo "  cargo install tauri-cli --version '^2' --locked" >&2
  exit 1
fi

echo "==> Chạy kiểm thử"
cargo test --workspace

echo
echo "==> Đóng gói (release)"
$TAURI build

echo
echo "Xong."
echo "  ỨNG DỤNG: open 'target/release/bundle/macos/Bulk Model Test.app'"
echo "  BỘ CÀI:   xem target/release/bundle/dmg/"
