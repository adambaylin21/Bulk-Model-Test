#!/usr/bin/env bash
# Đóng gói Bulk Model Test thành ứng dụng macOS một tệp.
#
#   ./build_app.sh
#
# Kết quả:
#   dist/Bulk Model Test.app      bundle có icon
#   dist/Bulk Model Test.command  nhấp đôi để chạy kèm cửa sổ Terminal
# Yêu cầu: macOS, Xcode Command Line Tools (cho sips/iconutil).

set -euo pipefail

cd "$(dirname "$0")"

PY=".venv/bin/python"

if [[ ! -x "$PY" ]]; then
  echo "Không thấy $PY. Tạo môi trường trước:" >&2
  echo "  uv venv && uv pip install -r requirements.txt" >&2
  exit 1
fi

if ! "$PY" -c "import PyInstaller" 2>/dev/null; then
  echo "Chưa có PyInstaller. Cài bằng:" >&2
  echo "  uv pip install pyinstaller" >&2
  exit 1
fi

# Icon và tệp .command đều do spec tự dựng, nên chỉ cần gọi PyInstaller.
echo "==> Đóng gói"
"$PY" -m PyInstaller --noconfirm --clean bulk_model_test.spec

echo
echo "Xong."
echo "  Có cửa sổ Terminal: nhấp đôi 'dist/Bulk Model Test.command'"
echo "  Không có Terminal:  open 'dist/Bulk Model Test.app'"
