"""Đường dẫn và hằng số dùng chung cho backend."""

from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
STATIC_DIR = BASE_DIR / "static"
DATA_DIR = BASE_DIR / "data"
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
