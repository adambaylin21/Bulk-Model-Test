"""Bulk Model Test - Flask backend package."""

from flask import Flask

from .config import STATIC_DIR
from .routes import bp


def create_app() -> Flask:
    app = Flask(__name__, static_folder=str(STATIC_DIR), static_url_path="")
    app.register_blueprint(bp)
    return app
