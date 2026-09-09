#!/usr/bin/env python3
"""把 make_doc_cli.py 打包成 make_doc.exe（PyInstaller onefile）。

用法: python pack_doc_exe.py
产物: localmind/scripts/dist/make_doc.exe

- onefile: 单个 exe，解压到临时目录运行
- noconsole: 不弹 CMD 黑窗口
- 隐藏导入 4 个 make_*.py（make_doc_cli.py 里 import 它们）
"""
import os
import subprocess
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
ENTRY = os.path.join(SCRIPT_DIR, "make_doc_cli.py")
OUT_DIR = os.path.join(SCRIPT_DIR, "dist")

cmd = [
    sys.executable, "-m", "PyInstaller",
    "--onefile",
    "--noconsole",
    "--name", "make_doc",
    "--distpath", OUT_DIR,
    "--workpath", os.path.join(SCRIPT_DIR, "build", "pyinstaller"),
    "--specpath", os.path.join(SCRIPT_DIR, "build"),
    "--clean",
    "--noconfirm",
]

cmd.append(ENTRY)

print("=== 打包 make_doc.exe ===")
print("命令:", " ".join(cmd))
print("工作目录:", SCRIPT_DIR)

result = subprocess.run(cmd, cwd=SCRIPT_DIR, capture_output=True, text=True)
if result.stdout:
    print(result.stdout[-3000:])
if result.stderr:
    print("STDERR:", result.stderr[-3000:])
if result.returncode != 0:
    print(f"打包失败，退出码 {result.returncode}")
    sys.exit(1)

exe = os.path.join(OUT_DIR, "make_doc.exe")
if os.path.exists(exe):
    size = os.path.getsize(exe) / (1024 * 1024)
    print(f"OK: {exe}（{size:.1f} MB）")
else:
    print("警告：未找到 make_doc.exe")
    sys.exit(1)
