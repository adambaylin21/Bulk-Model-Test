"""Chuẩn hoá tham số chạy test và dữ liệu profile."""

from .config import (
    DEFAULT_PROMPT,
    DEFAULT_TIMEOUT,
    MAX_CONCURRENCY,
    MAX_CONCURRENCY_CEILING,
)

PROFILE_FIELDS = (
    "name", "base_url", "api_key", "model_name", "test_prompt",
    "timeout", "concurrency", "auto_detect", "auto_start", "notes",
)


def parse_model_names(raw: str) -> list[str]:
    """Tách danh sách model người dùng nhập theo dòng, dấu phẩy."""
    parts = [p.strip() for chunk in (raw or "").splitlines() for p in chunk.split(",")]
    return [p for p in parts if p]


def clamp_float(value, default: float, low: float, high: float) -> float:
    try:
        number = float(value)
    except (TypeError, ValueError):
        return default
    return max(low, min(number, high))


def clamp_int(value, default: int, low: int, high: int) -> int:
    try:
        number = int(value)
    except (TypeError, ValueError):
        return default
    return max(low, min(number, high))


def read_run_params(data: dict) -> tuple:
    """Kiểm tra và chuẩn hoá tham số dùng chung cho một lần chạy."""
    base_url = (data.get("base_url") or "").strip()
    api_key = (data.get("api_key") or "").strip()
    prompt = (data.get("test_prompt") or "").strip() or DEFAULT_PROMPT
    auto_detect = bool(data.get("auto_detect"))
    model_filter = parse_model_names(data.get("model_name") or "")
    timeout = clamp_float(data.get("timeout") or DEFAULT_TIMEOUT, DEFAULT_TIMEOUT, 1.0, 600.0)
    concurrency = clamp_int(data.get("concurrency") or MAX_CONCURRENCY,
                            MAX_CONCURRENCY, 1, MAX_CONCURRENCY_CEILING)
    return base_url, api_key, prompt, timeout, model_filter, auto_detect, concurrency


def normalize_profile(payload: dict, existing: dict | None = None) -> dict:
    """Chuẩn hoá một profile; giữ API Key đã lưu khi client không gửi lại."""
    payload = payload or {}
    base = existing or {}

    def pick(key, default=""):
        value = payload.get(key)
        if value is None:
            return base.get(key, default)
        return value

    api_key = (payload.get("api_key") or "").strip() or base.get("api_key", "")
    base_url = (pick("base_url") or "").strip()
    name = (pick("name") or "").strip()

    return {
        "name": name,
        "base_url": base_url,
        "api_key": api_key,
        "model_name": (pick("model_name") or "").strip(),
        "test_prompt": (pick("test_prompt") or "").strip() or DEFAULT_PROMPT,
        "timeout": clamp_float(pick("timeout", DEFAULT_TIMEOUT), DEFAULT_TIMEOUT, 1.0, 600.0),
        "concurrency": clamp_int(pick("concurrency", MAX_CONCURRENCY),
                                 MAX_CONCURRENCY, 1, MAX_CONCURRENCY_CEILING),
        "auto_detect": bool(pick("auto_detect", True)),
        "auto_start": bool(pick("auto_start", False)),
        "notes": (pick("notes") or "").strip(),
    }
