#!/usr/bin/env python3
"""Bản desktop: chạy server nội bộ rồi mở giao diện bằng trình duyệt mặc định."""

import os
import signal
import socket
import sys
import threading
import webbrowser

from backend import create_app
from backend.config import DATA_DIR, PROFILES_FILE


def in_use(port: int) -> bool:
    """Cổng có người nghe chưa.

    bind() một mình không đủ tin cậy: trên macOS nó vẫn thành công khi cổng đã
    có listener nếu bật SO_REUSEADDR, nên phải thử nối thẳng vào cổng.
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.settimeout(0.4)
        if probe.connect_ex(("127.0.0.1", port)) == 0:
            return True

    # Không nối được: vẫn có thể bị giữ bởi socket đang lắng nghe ở địa chỉ khác.
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        try:
            sock.bind(("127.0.0.1", port))
        except OSError:
            return True
    return False


def free_port(preferred: int) -> int:
    """Trả về cổng ưu tiên nếu còn trống, nếu không thì để hệ thống cấp cổng khác."""
    if not in_use(preferred):
        return preferred

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def stop(*_args) -> None:
    """Thoát ngay khi cửa sổ Terminal đóng hoặc khi nhấn Ctrl+C.

    os._exit thay vì sys.exit: luồng chính đang kẹt trong app.run() nên chỉ có
    cách thoát thẳng tiến trình mới nhả được cổng.
    """
    print("\nĐã dừng.")
    sys.stdout.flush()
    os._exit(0)


def main() -> int:
    app = create_app()
    port = free_port(int(os.environ.get("PORT", "8686")))
    url = f"http://127.0.0.1:{port}"

    # Terminal gửi SIGHUP khi cửa sổ đóng; SIGTERM khi tiến trình bị yêu cầu dừng.
    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, stop)

    print("Bulk Model Test")
    print(f"Giao diện: {url}")
    print(f"Dữ liệu:   {PROFILES_FILE}")
    print("Ctrl+C hoặc đóng cửa sổ này để dừng ứng dụng.\n")

    # Mở trình duyệt sau khi server đã lắng nghe.
    threading.Timer(1.0, lambda: webbrowser.open(url)).start()

    try:
        # Không bật debug: chế độ đó chạy tiến trình phụ và tải lại trang liên tục.
        app.run(host="127.0.0.1", port=port, debug=False, threaded=True,
                use_reloader=False)
    except KeyboardInterrupt:
        stop()
    return 0


if __name__ == "__main__":
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    sys.exit(main())
