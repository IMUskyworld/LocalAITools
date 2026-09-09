#!/usr/bin/env python3
"""把 agent_server.py 打包成 localmind-agent/（PyInstaller onedir）。

用法: python scripts/pack_agent_exe.py
产物: localmind/src-tauri/scripts/localmind-agent/（tauri.conf.json bundle.resources 引用此路径）

- onedir：进程即主进程，Rust 可直接 kill，无 onefile bootloader 孤儿进程问题；启动快。
- noconsole：不弹 CMD 黑窗口。
- 在干净 venv（build/agent-venv，只装 pydantic-ai-slim[openai]）里打包——避免系统 Python 里的
  tensorflow/torch/cv2 等无关库被 PyInstaller 依赖分析误抓，把产物从 1.9GB 压到 ~80MB。
- openai SDK 有动态 import（openai.lib._parsing.*），PyInstaller 常漏，必须 --collect-all openai。
"""
import os
import subprocess
import sys

# Windows 控制台默认 GBK，PyInstaller 输出可能含非 GBK 字符，放宽编码避免 print 崩溃
sys.stdout.reconfigure(errors="replace")
sys.stderr.reconfigure(errors="replace")

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
ENTRY = os.path.join(SCRIPT_DIR, "agent_server.py")
# 输出到 src-tauri/scripts/localmind-agent/（bundle.resources 引用）
OUT_DIR = os.path.join(SCRIPT_DIR, "..", "src-tauri", "scripts")
# 干净打包环境：只含 pydantic-ai-slim[openai]，避免系统库误抓
VENV_PYTHON = os.path.join(SCRIPT_DIR, "build", "agent-venv", "Scripts", "python.exe")
if not os.path.exists(VENV_PYTHON):
    print(f"缺少打包环境 {VENV_PYTHON}，请先运行：")
    print("  D:/APP/python.exe -m venv scripts/build/agent-venv")
    print("  scripts/build/agent-venv/Scripts/python.exe -m pip install 'pydantic-ai-slim[openai]' pyinstaller")
    sys.exit(1)

cmd = [
    VENV_PYTHON, "-m", "PyInstaller",
    "--onedir",
    "--noconsole",
    "--name", "localmind-agent",
    "--distpath", OUT_DIR,
    "--workpath", os.path.join(SCRIPT_DIR, "build", "pyinstaller-agent"),
    "--specpath", os.path.join(SCRIPT_DIR, "build"),
    "--clean",
    "--noconfirm",
    # openai 动态 import（_completions/_responses）；genai_prices / pydantic-ai-slim 需 metadata
    "--collect-all", "openai",
    "--collect-all", "genai_prices",
    "--copy-metadata", "pydantic-ai-slim",
    "--copy-metadata", "pydantic",
    "--copy-metadata", "pydantic-core",
    "--collect-submodules", "pydantic_ai",
    "--collect-submodules", "pydantic_graph",
]

cmd.append(ENTRY)

print("=== 打包 localmind-agent ===")
print("命令:", " ".join(cmd))
print("工作目录:", SCRIPT_DIR)

result = subprocess.run(cmd, cwd=SCRIPT_DIR, capture_output=True, text=True, encoding="utf-8", errors="replace")
if result.stdout:
    print(result.stdout[-3000:])
if result.stderr:
    print("STDERR:", result.stderr[-3000:])
if result.returncode != 0:
    print(f"打包失败，退出码 {result.returncode}")
    sys.exit(1)

exe = os.path.join(OUT_DIR, "localmind-agent", "localmind-agent.exe")
if os.path.exists(exe):
    size = os.path.getsize(exe) / (1024 * 1024)
    print(f"OK: {exe}（{size:.1f} MB）")
else:
    print("打包结束但未找到产物 exe")
    sys.exit(1)
