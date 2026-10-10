#!/usr/bin/env python3
"""Điểm khởi động Bulk Model Test."""

import os

from backend import create_app

app = create_app()

if __name__ == "__main__":
    port = int(os.environ.get("PORT", "8686"))
    print("Starting Bulk Model Test Flask Server...")
    print(f"Web UI:  http://localhost:{port}")
    app.run(host="0.0.0.0", port=port, debug=True, threaded=True)
