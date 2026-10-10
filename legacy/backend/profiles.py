"""Lưu và đọc danh sách profile trong một file JSON.

File giữ API Key ở dạng thô vì đây là công cụ chạy trên máy cá nhân; thư mục
``data/`` nằm ngoài mã nguồn và không được commit.
"""

import json
import os
import threading
import uuid
from datetime import datetime

from .config import DATA_DIR, PROFILES_FILE
from .params import normalize_profile

_lock = threading.Lock()
SCHEMA_VERSION = 1


def _empty_store() -> dict:
    return {"version": SCHEMA_VERSION, "updated_at": None, "profiles": []}


def _read_store() -> dict:
    if not PROFILES_FILE.exists():
        return _empty_store()
    try:
        with PROFILES_FILE.open("r", encoding="utf-8") as handle:
            store = json.load(handle)
    except (json.JSONDecodeError, OSError):
        return _empty_store()
    if not isinstance(store, dict) or not isinstance(store.get("profiles"), list):
        return _empty_store()
    store.setdefault("version", SCHEMA_VERSION)
    store.setdefault("updated_at", None)
    return store


def _write_store(store: dict) -> None:
    """Ghi nguyên tử để không làm hỏng file khi tiến trình bị dừng giữa chừng."""
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    store["updated_at"] = datetime.now().isoformat(timespec="seconds")
    tmp_path = PROFILES_FILE.with_suffix(f".{os.getpid()}.tmp")
    with tmp_path.open("w", encoding="utf-8") as handle:
        json.dump(store, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    tmp_path.replace(PROFILES_FILE)


def list_profiles() -> dict:
    with _lock:
        return _read_store()


def save_profile(payload: dict) -> dict:
    """Thêm mới khi không có id, cập nhật khi id đã tồn tại."""
    with _lock:
        store = _read_store()
        profile_id = (payload.get("id") or "").strip()
        existing = next((p for p in store["profiles"] if p["id"] == profile_id), None)

        profile = normalize_profile(payload, existing)
        if not profile["name"]:
            profile["name"] = profile["base_url"] or "Profile chưa đặt tên"

        if existing is not None:
            existing.update(profile)
            profile = existing
        else:
            profile = {"id": uuid.uuid4().hex[:12],
                       "created_at": datetime.now().isoformat(timespec="seconds"),
                       **profile}
            store["profiles"].append(profile)

        profile["updated_at"] = datetime.now().isoformat(timespec="seconds")
        _write_store(store)
        return profile


def delete_profile(profile_id: str) -> bool:
    with _lock:
        store = _read_store()
        remaining = [p for p in store["profiles"] if p["id"] != profile_id]
        if len(remaining) == len(store["profiles"]):
            return False
        store["profiles"] = remaining
        _write_store(store)
        return True


def touch_profile(profile_id: str) -> None:
    """Ghi lại thời điểm dùng gần nhất; bỏ qua khi id không còn tồn tại."""
    with _lock:
        store = _read_store()
        profile = next((p for p in store["profiles"] if p["id"] == profile_id), None)
        if profile is None:
            return
        profile["last_used_at"] = datetime.now().isoformat(timespec="seconds")
        _write_store(store)
