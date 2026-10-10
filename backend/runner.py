"""Gọi endpoint OpenAI-compatible và chạy một lượt bulk test."""

import queue
import threading
import time
import uuid
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from statistics import mean

from openai import OpenAI

from .config import NON_CHAT_PREFIXES

# job_id -> {"queue": Queue, "cancel": Event}
jobs: dict[str, dict] = {}


def get_openai_client(base_url: str, api_key: str, timeout: float) -> OpenAI:
    """Tạo client tương thích OpenAI SDK."""
    return OpenAI(base_url=base_url.rstrip("/"), api_key=api_key, timeout=timeout)


def fetch_models(client: OpenAI) -> list[str]:
    """Trả về các model phục vụ chat mà endpoint công bố."""
    try:
        response = client.models.list()
    except Exception as exc:
        raise RuntimeError(f"Không lấy được danh sách model: {exc}") from exc

    names = []
    for model in response.data:
        model_id = getattr(model, "id", None)
        if not model_id:
            continue
        if model_id.lower().startswith(NON_CHAT_PREFIXES):
            continue
        names.append(model_id)

    return sorted(set(names))


def test_model(client: OpenAI, model: str, test_prompt: str) -> dict:
    """Gửi một chat completion tối thiểu và đo thời gian khứ hồi."""
    started = time.perf_counter()
    result = {
        "model": model,
        "response_time_ms": 0.0,
        "status": "error",
        "error": None,
        "test_result": None,
    }

    try:
        response = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": test_prompt}],
            max_tokens=16,
            temperature=0,
        )
        result["status"] = "success"
        result["test_result"] = (response.choices[0].message.content or "")[:120]
    except Exception as exc:
        result["status"] = "error"
        result["error"] = str(exc)

    result["response_time_ms"] = round((time.perf_counter() - started) * 1000, 2)
    return result


def sort_results(results: list[dict]) -> list[dict]:
    """Model thành công nhanh nhất lên trước, model lỗi xuống cuối."""
    successful = sorted(
        (r for r in results if r["status"] == "success"),
        key=lambda r: r["response_time_ms"],
    )
    failed = sorted(
        (r for r in results if r["status"] == "error"),
        key=lambda r: r["response_time_ms"],
    )
    return successful + failed


def build_report(results: list[dict], profile: dict | None = None) -> dict:
    """Tóm tắt một lượt chạy."""
    times = [r["response_time_ms"] for r in results if r["status"] == "success"]
    return {
        "total_models": len(results),
        "successful": sum(1 for r in results if r["status"] == "success"),
        "errors": sum(1 for r in results if r["status"] == "error"),
        "avg_response_time_ms": round(mean(times), 2) if times else 0.0,
        "fastest_ms": min(times) if times else 0.0,
        "slowest_ms": max(times) if times else 0.0,
        "timestamp": datetime.now().isoformat(timespec="seconds"),
        "profile": profile or None,
        "models": [
            {
                "name": r["model"],
                "response_time_ms": r["response_time_ms"],
                "status": r["status"],
                "error": r["error"],
                "test_result": r["test_result"],
            }
            for r in results
        ],
    }


def build_chart_data(results: list[dict]) -> dict:
    """Chuỗi dữ liệu cho biểu đồ cột thời gian phản hồi."""
    return {
        "labels": [r["model"] for r in results],
        "values": [r["response_time_ms"] for r in results],
        "colors": ["#1f9d55" if r["status"] == "success" else "#d93a2b" for r in results],
    }


def run_job(job_id: str, base_url: str, api_key: str, prompt: str, timeout: float,
            model_filter: list[str], auto_detect: bool, concurrency: int,
            profile: dict | None = None) -> None:
    """Chạy mọi model trong một worker pool, phát tiến độ ngay khi có."""
    events = jobs[job_id]["queue"]
    cancel = jobs[job_id]["cancel"]

    def emit(payload: dict) -> None:
        events.put(payload)

    try:
        client = get_openai_client(base_url, api_key, timeout)

        if model_filter:
            models = model_filter
            emit({"event": "models", "models": models, "source": "manual"})
        elif auto_detect:
            emit({"event": "status", "message": "Đang tải danh sách model..."})
            models = fetch_models(client)
            if not models:
                emit({"event": "error", "message": "API không trả về model nào dùng được."})
                return
            emit({"event": "models", "models": models, "source": "auto"})
        else:
            emit({"event": "error", "message": "Cần nhập tên model hoặc bật Auto Detect."})
            return

        total = len(models)
        workers = min(concurrency, total)
        emit({"event": "start", "total": total, "concurrency": workers})

        results: list[dict] = []
        lock = threading.Lock()
        completed = 0

        def worker(model: str) -> None:
            nonlocal completed
            if cancel.is_set():
                return
            emit({"event": "model_start", "model": model})
            result = test_model(client, model, prompt)
            with lock:
                results.append(result)
                completed += 1
                done = completed
            emit({
                "event": "model_done",
                "model": model,
                "result": result,
                "completed": done,
                "total": total,
            })

        with ThreadPoolExecutor(max_workers=workers) as pool:
            list(pool.map(worker, models))

        if cancel.is_set():
            emit({"event": "cancelled", "completed": len(results)})
            return

        ordered = sort_results(results)
        emit({
            "event": "done",
            "report": build_report(ordered, profile),
            "chart_data": build_chart_data(ordered),
        })
    except Exception as exc:
        emit({"event": "error", "message": str(exc)})


def create_job() -> str:
    job_id = uuid.uuid4().hex
    jobs[job_id] = {"queue": queue.Queue(), "cancel": threading.Event()}
    return job_id


def cancel_job(job_id: str = "") -> bool:
    """Dừng job theo id; không có id thì dừng job mới nhất còn sống."""
    job = jobs.get(job_id) if job_id else None
    if job is None and jobs:
        job = jobs[next(reversed(jobs))]
    if job is None:
        return False
    job["cancel"].set()
    return True
