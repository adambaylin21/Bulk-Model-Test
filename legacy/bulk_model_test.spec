# -*- mode: python ; coding: utf-8 -*-
"""Đóng gói Bulk Model Test thành một bundle .app chạy độc lập.

Build:  .venv/bin/python -m PyInstaller --noconfirm bulk_model_test.spec
Kết quả: dist/Bulk Model Test.app
"""

import subprocess
import sys
from pathlib import Path

PROJECT_DIR = Path(SPECPATH).resolve()
SOURCE_ICON = PROJECT_DIR / "Icon.png"
ICON_FILE = PROJECT_DIR / "build" / "Icon.icns"

ICONSET_SIZES = [
    (16, "icon_16x16"), (32, "icon_16x16@2x"),
    (32, "icon_32x32"), (64, "icon_32x32@2x"),
    (128, "icon_128x128"), (256, "icon_128x128@2x"),
    (256, "icon_256x256"), (512, "icon_256x256@2x"),
    (512, "icon_512x512"), (1024, "icon_512x512@2x"),
]


def build_icns() -> Path:
    """Dựng build/Icon.icns từ Icon.png; báo lỗi rõ nếu không dựng được.

    Không được để thiếu icon một cách im lặng: PyInstaller sẽ lặng lẽ dùng icon
    mặc định của nó và bundle ra đời mang icon sai.
    """
    if ICON_FILE.exists() and ICON_FILE.stat().st_mtime >= SOURCE_ICON.stat().st_mtime:
        return ICON_FILE

    if not SOURCE_ICON.exists():
        sys.exit(f"Thiếu {SOURCE_ICON.name}; cần file này để tạo icon cho ứng dụng.")

    iconset = PROJECT_DIR / "build" / "icon.iconset"
    subprocess.run(["rm", "-rf", str(iconset)], check=True)
    iconset.mkdir(parents=True, exist_ok=True)

    for pixels, name in ICONSET_SIZES:
        subprocess.run(
            ["sips", "-z", str(pixels), str(pixels), str(SOURCE_ICON),
             "--out", str(iconset / f"{name}.png")],
            check=True, capture_output=True,
        )

    result = subprocess.run(
        ["iconutil", "-c", "icns", str(iconset), "-o", str(ICON_FILE)],
        capture_output=True, text=True,
    )
    if result.returncode != 0 or not ICON_FILE.exists():
        sys.exit(f"iconutil thất bại: {result.stderr.strip()}")

    print(f"[spec] Đã dựng icon: {ICON_FILE}")
    return ICON_FILE


ICON = build_icns()

# Flask cần các template tĩnh và một số module được nạp theo tên.
hidden_imports = [
    "backend",
    "backend.config",
    "backend.params",
    "backend.profiles",
    "backend.routes",
    "backend.runner",
    "werkzeug",
    "werkzeug.serving",
    "jinja2",
    "jinja2.ext",
    "click",
    "flask",
    "dotenv",
    "openai",
    "httpx2",
    "httpcore2",
    "anyio",
    "sniffio",
    "idna",
    "h11",
]

# Giao diện nằm ngoài mã Python nên phải khai báo thủ công.
datas = [
    (str(PROJECT_DIR / "static"), "static"),
]

a = Analysis(
    ["desktop.py"],
    pathex=[str(PROJECT_DIR)],
    binaries=[],
    datas=datas,
    hiddenimports=hidden_imports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    # Không dùng tkinter, matplotlib hay test của thư viện để gói nhẹ hơn.
    excludes=[
        "tkinter", "unittest", "pydoc", "doctest", "pytest",
        "numpy", "pandas", "matplotlib", "PIL", "test",
    ],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="Bulk Model Test",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    upx_exclude=[],
    runtime_tmpdir=None,
    # console=True: giữ cửa sổ terminal để còn thấy log và tắt ứng dụng.
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=str(ICON),
)

APP_NAME = "Bulk Model Test"
APP_PATH = PROJECT_DIR / "dist" / f"{APP_NAME}.app"

app = BUNDLE(
    exe,
    name=f"{APP_NAME}.app",
    icon=str(ICON),
    bundle_identifier="com.adambaylin.bulkmodeltest",
    info_plist={
        "CFBundleName": "Bulk Model Test",
        "CFBundleDisplayName": "Bulk Model Test",
        "CFBundleShortVersionString": "1.0.0",
        "CFBundleVersion": "1.0.0",
        "NSHighResolutionCapable": True,
        # PyInstaller tự đặt LSBackgroundOnly=True khi console=True, khiến macOS
        # chạy app nền nên không mở cửa sổ Terminal. Ghi đè lại để cửa sổ hiện ra.
        "LSBackgroundOnly": False,
        # Ứng dụng mở trình duyệt thay vì tự vẽ giao diện.
        "LSApplicationCategoryType": "public.app-category.developer-tools",
        "LSMinimumSystemVersion": "11.0",
    },
)


# Nhấp đôi một .app trên macOS luôn bị hệ thống đẩy sang launchd nên không kèm
# cửa sổ Terminal. Chỉ file .command mới mở được Terminal; nó được sinh ngay ở
# bước build (không phải file thủ công trong dist) để không mất khi build lại.
COMMAND_NAME = f"{APP_NAME}.command"

COMMAND_SCRIPT = f"""#!/bin/bash
# Nhấp đôi tệp này để chạy ứng dụng kèm cửa sổ Terminal.
# Ctrl+C hoặc đóng cửa sổ Terminal đều tắt ứng dụng.
cd "$(dirname "$0")" || exit 1
if [ ! -x "./{APP_NAME}.app/Contents/MacOS/{APP_NAME}" ]; then
    echo "Không thấy {APP_NAME}.app cạnh tệp này." >&2
    echo "Hãy chạy build_app.sh để tạo lại." >&2
    echo
    read -r -p "Nhấn Enter để đóng..."
    exit 1
fi
echo "{APP_NAME} đang chạy."
echo "Ctrl+C hoặc đóng cửa sổ này để tắt ứng dụng."
echo
exec "./{APP_NAME}.app/Contents/MacOS/{APP_NAME}"
"""


def write_launcher() -> None:
    """Ghi dist/Bulk Model Test.command và cấp quyền chạy.

    Chạy ngay khi đọc spec, tức trước lúc PyInstaller đóng gói. PyInstaller chỉ
    xoá đúng mục tiêu cũ của nó trong dist/ nên tệp này ở lại sau khi build.
    """
    APP_PATH.parent.mkdir(parents=True, exist_ok=True)

    launcher = PROJECT_DIR / "dist" / COMMAND_NAME
    launcher.write_text(COMMAND_SCRIPT, encoding="utf-8")
    launcher.chmod(0o755)
    print(f"[spec] Đã tạo: {launcher}")


write_launcher()
