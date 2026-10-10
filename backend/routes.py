"""Định nghĩa các route HTTP của ứng dụng."""

import json
import queue
import threading
from concurrent.futures import ThreadPoolExecutor

from flask import Blueprint, Response, jsonify, request, send_from_directory

from . import profiles
from .config import STATIC_DIR
from .params import parse_model_names, read_run_params
from .runner import (
    build_chart_data,
    build_report,
    cancel_job,
    create_job,
    fetch_models,
    get_openai_client,
    jobs,
    run_job,
    sort_results,
    test_model,
)

bp = Blueprint("main", __name__)


@bp.route("/")
def index() -> Response:
    """Trang chính; các file tĩnh còn lại do static_folder phục vụ."""
    return send_from_directory(STATIC_DIR, "index.html")


# --------------------------------------------------------------------------- #
# Profile
# --------------------------------------------------------------------------- #

@bp.route("/api/profiles", methods=["GET"])
def profiles_index():
    """Toàn bộ profile đã lưu, kèm đường dẫn file JSON."""
    store = profiles.list_profiles()
    return jsonify({
        "profiles": store["profiles"],
        "count": len(store["profiles"]),
        "storage": profiles.PROFILES_FILE.name,
        "updated_at": store.get("updated_at"),
    })


@bp.route("/api/profiles", methods=["POST"])
def profiles_save():
    """Tạo mới hoặc cập nhật một profile."""
    data = request.json or {}
    if not (data.get("base_url") or "").strip():
        return jsonify({"error": "Cần nhập Base Url."}), 400
    profile = profiles.save_profile(data)
    return jsonify({"profile": profile})


@bp.route("/api/profiles/<profile_id>", methods=["DELETE"])
def profiles_delete(profile_id: str):
    if profiles.delete_profile(profile_id):
        return jsonify({"deleted": True, "id": profile_id})
    return jsonify({"error": "Không tìm thấy profile."}), 404


@bp.route("/api/profiles/<profile_id>/used", methods=["POST"])
def profiles_touch(profile_id: str):
    profiles.touch_profile(profile_id)
    return jsonify({"ok": True})


# --------------------------------------------------------------------------- #
# Test
# --------------------------------------------------------------------------- #

def _stream(job_id: str) -> Response:
    """Server-Sent Events cho một job."""
    events = jobs[job_id]["queue"]

    def generate():
        try:
            while True:
                try:
                    payload = events.get(timeout=15)
                except queue.Empty:
                    yield ": keep-alive\n\n"
                    continue
                yield f"data: {json.dumps(payload, ensure_ascii=False)}\n\n"
                if payload.get("event") in ("done", "error", "cancelled"):
                    break
        finally:
            jobs.pop(job_id, None)

    return Response(
        generate(),
        mimetype="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "X-Accel-Buffering": "no",
            "Connection": "keep-alive",
        },
    )


@bp.route("/api/test/stream", methods=["POST"])
def test_stream():
    """Bắt đầu một lượt chạy và stream tiến độ qua SSE."""
    data = request.json or {}
    base_url, api_key, prompt, timeout, model_filter, auto_detect, concurrency = read_run_params(data)
    if not base_url or not api_key:
        return jsonify({"error": "Cần nhập Base Url và API Key."}), 400

    profile = None
    profile_id = (data.get("profile_id") or "").strip()
    if profile_id:
        match = next(
            (p for p in profiles.list_profiles()["profiles"] if p["id"] == profile_id), None)
        if match and match.get("base_url") == base_url:
            profiles.touch_profile(profile_id)
            profile = {"id": match["id"], "name": match.get("name", ""),
                       "base_url": match.get("base_url", "")}

    job_id = create_job()
    jobs[job_id]["queue"].put({"event": "job", "job_id": job_id})

    threading.Thread(
        target=run_job,
        args=(job_id, base_url, api_key, prompt, timeout, model_filter, auto_detect,
              concurrency, profile),
        daemon=True,
    ).start()

    return _stream(job_id)


@bp.route("/api/test/cancel", methods=["POST"])
def test_cancel():
    """Dừng một job đang chạy."""
    cancelled = cancel_job((request.json or {}).get("job_id", ""))
    return jsonify({"cancelled": cancelled})


@bp.route("/api/test/bulk", methods=["POST"])
def test_bulk():
    """Biến thể chạy đồng bộ, trả về báo cáo đã hoàn tất."""
    base_url, api_key, prompt, timeout, model_filter, auto_detect, concurrency = read_run_params(
        request.json or {}
    )
    if not base_url or not api_key:
        return jsonify({"error": "Cần nhập Base Url và API Key."}), 400

    client = get_openai_client(base_url, api_key, timeout)

    if model_filter:
        models = model_filter
    elif auto_detect:
        models = fetch_models(client)
    else:
        return jsonify({"error": "Cần nhập tên model hoặc bật Auto Detect."}), 400

    if not models:
        return jsonify({"error": "Không có model nào để test."}), 400

    with ThreadPoolExecutor(max_workers=min(concurrency, len(models))) as pool:
        results = list(pool.map(lambda m: test_model(client, m, prompt), models))

    ordered = sort_results(results)
    return jsonify({"report": build_report(ordered), "chart_data": build_chart_data(ordered)})


@bp.route("/api/models", methods=["POST"])
def list_models():
    """Xem trước danh sách model mà API key chạm được."""
    base_url, api_key, _, timeout, _, _, _ = read_run_params(request.json or {})
    if not base_url or not api_key:
        return jsonify({"error": "Cần nhập Base Url và API Key."}), 400

    client = get_openai_client(base_url, api_key, timeout)
    models = fetch_models(client)
    return jsonify({"models": models, "count": len(models)})


@bp.route("/api/models/parse", methods=["POST"])
def parse_models():
    """Đếm số model người dùng nhập tay."""
    names = parse_model_names((request.json or {}).get("model_name", ""))
    return jsonify({"models": names, "count": len(names)})
