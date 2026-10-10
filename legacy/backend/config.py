"""Đường dẫn và hằng số dùng chung cho backend.

Chạy từ mã nguồn thì đọc file tĩnh ngay trong thư mục legacy. Khi đã đóng gói
(PyInstaller) thì mã nguồn nằm trong thư mục giải nén tạm, còn dữ liệu người
dùng phải nằm ở thư mục ghi được lâu dài.

Bản Python nằm trong legacy/ để tham chiếu; bản chính của chương trình là Rust
+ Tauri ở thư mục gốc.
"""

import os
import sys
from pathlib import Path


def _is_frozen() -> bool:
    return getattr(sys, "frozen", False)


def _bundle_dir() -> Path:
    """Nơi chứa file đi kèm (static, icon)."""
    if _is_frozen():
        # PyInstaller onefile giải nén vào sys._MEIPASS; onedir dùng thư mục chứa app.
        return Path(getattr(sys, "_MEIPASS", Path(sys.executable).parent))
    return Path(__file__).resolve().parent.parent


def _data_dir() -> Path:
    """Nơi ghi dữ liệu lâu dài, không nằm trong gói chỉ đọc."""
    override = os.environ.get("BMT_DATA_DIR")
    if override:
        return Path(override).expanduser()

    # Chạy từ mã nguồn thì giữ nguyên data/ của dự án (một cấp trên legacy/).
    if not _is_frozen():
        return Path(__file__).resolve().parent.parent.parent / "data"

    if sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support" / "Bulk Model Test"
    if os.name == "nt":
        base = os.environ.get("APPDATA") or Path.home() / "AppData" / "Roaming"
        return Path(base) / "Bulk Model Test"
    return Path.home() / ".local" / "share" / "bulk-model-test"


BASE_DIR = Path(__file__).resolve().parent.parent
BUNDLE_DIR = _bundle_dir()
STATIC_DIR = BUNDLE_DIR / "static"
DATA_DIR = _data_dir()
PROFILES_FILE = DATA_DIR / "profiles.json"

MAX_CONCURRENCY = 8
MAX_CONCURRENCY_CEILING = 64
DEFAULT_TIMEOUT = 30.0
DEFAULT_PROMPT = "Hello"

# Model trả lời được /v1/models nhưng không phục vụ chat completion.
NON_CHAT_PREFIXES = (
    "embedding", "text-embedding", "whisper", "tts-", "dall-e", "moderation",
    "rerank", "stable-diffusion", "text-to-image", "guard", "audio", "realtime",
    "instruct-embed", "vision-embed",
)
