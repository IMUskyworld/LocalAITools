#!/usr/bin/env python3
"""LocalMind Agent 服务 — Pydantic AI 2.x Agent 循环 + SSE HTTP 服务。

被 Rust 后端（Tauri）作为子进程启动：
  - 绑定 127.0.0.1 随机端口，启动后向 stdout 打印 `PORT=<n>` 并写入 %TEMP%/localmind_agent_port.txt。
  - 读取 env LOCALMIND_AGENT_TOKEN 作为访问凭证；请求须带 `X-LocalMind-Token` 头。
  - POST /agent/stream 走 SSE，事件契约：
      {"type":"thinking","step":{id,phase,label,status,detail,timestamp}}
      {"type":"tool","log":{name,args,output,success}}
      {"type":"delta","text":"..."}
      {"type":"trace","trace":{...}}
      {"type":"done","content":"..."}
      {"type":"error","message":"..."}

开发模式独立运行：python scripts/agent_server.py（须设置 LOCALMIND_AGENT_TOKEN）
"""
import asyncio
import functools
import hashlib
import json
import re
import os
import subprocess
import threading
import shutil
import sys
import tempfile
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from pydantic_ai import Agent, Tool, UsageLimits
from pydantic_ai.messages import (
    FinalResultEvent,
    FunctionToolCallEvent,
    FunctionToolResultEvent,
    ModelRequest,
    ModelResponse,
    PartDeltaEvent,
    SystemPromptPart,
    TextPart,
    TextPartDelta,
    ToolCallPart,
    ToolReturnPart,
    UserPromptPart,
)
from pydantic_ai.models.ollama import OllamaModel
from pydantic_ai.models.openai import OpenAIChatModel
from pydantic_ai.providers.ollama import OllamaProvider
from pydantic_ai.providers.openai import OpenAIProvider

from harness_trace import TraceRecorder
from tool_policy import ToolPolicy, ToolPolicyError
from tool_registry import TOOL_SPECS, ToolRegistry

AGENT_TOKEN = os.environ.get("LOCALMIND_AGENT_TOKEN", "dev-token")
OLLAMA_BASE = os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434/v1")
DEEPSEEK_BASE = "https://api.deepseek.com/v1"
# 在线模式内置模型（官方 ID；双端一致，见 开发计划/ADR-005-default-model-deepseek-flash.md）
DEFAULT_ONLINE_MODEL = "deepseek-flash"


# ========== Context Manager（Phase 3） ==========

CONTEXT_BUDGET_TOKENS = 50_000  # 为响应预留 ~15K tokens（模型上限 65536）
CHARS_PER_TOKEN_ZH = 2          # 中文约 2 字符/token
CHARS_PER_TOKEN_EN = 4          # 英文约 4 字符/token

def estimate_tokens(text: str) -> int:
    """粗略估算 token 数（中文 2 字符/token，英文 4 字符/token）。"""
    if not text:
        return 0
    zh_count = sum(1 for c in text if '\u4e00' <= c <= '\u9fff')
    en_count = len(text) - zh_count
    return zh_count // CHARS_PER_TOKEN_ZH + en_count // CHARS_PER_TOKEN_EN


def truncate_history(history: list, system_prompt_tokens: int, prompt_tokens: int) -> list:
    """当 history 超过 token 预算时截断，保留最近的消息。

    策略：
    - 固定保留：system prompt + 最后一条 user prompt
    - 优先保留最近的消息（从后往前）
    - 早期消息被截断时生成摘要占位符
    - tool call/result 对必须成对保留或成对丢弃
    """
    budget = CONTEXT_BUDGET_TOKENS - system_prompt_tokens - prompt_tokens - 2000  # 预留余量
    if budget <= 0:
        return []

    # 从后往前计算 token，直到预算用完
    kept = []
    used_tokens = 0
    for msg in reversed(history):
        content = ""
        if hasattr(msg, 'parts'):
            for part in msg.parts:
                if hasattr(part, 'content'):
                    content += part.content or ""
                elif hasattr(part, 'tool_name'):
                    content += f"{part.tool_name} {json.dumps(getattr(part, 'args', {}), ensure_ascii=False)}"
        msg_tokens = estimate_tokens(content)
        if used_tokens + msg_tokens > budget:
            # 预算不够了，如果前面还有消息，加一个摘要占位符
            remaining = len(history) - len(kept)
            if remaining > 0:
                summary_note = ModelRequest(parts=[SystemPromptPart(
                    content=f"【注意：前 {remaining} 条消息因上下文长度限制已被截断。如需引用早期对话内容，请向用户确认。】"
                )])
                kept.append(summary_note)
            break
        kept.append(msg)
        used_tokens += msg_tokens

    kept.reverse()
    return kept
# ========== 高危操作确认机制 ==========

import threading

_pending_confirmations: dict[str, dict] = {}
_confirm_results: dict[str, bool] = {}
_confirm_events: dict[str, threading.Event] = {}
_confirm_lock = threading.Lock()


def request_confirmation(tool_name: str, args: dict, emit_sse_event) -> bool:
    """请求用户确认。发送 confirm SSE 事件并阻塞等待用户响应。返回 True=确认，False=拒绝。"""
    import uuid
    confirm_id = str(uuid.uuid4())
    event = threading.Event()

    with _confirm_lock:
        _pending_confirmations[confirm_id] = {"tool": tool_name, "args": args}
        _confirm_events[confirm_id] = event

    # 发送确认请求到前端
    emit_sse_event({
        "type": "confirm",
        "id": confirm_id,
        "tool": tool_name,
        "args": args,
    })

    # 阻塞等待用户响应（最多60秒）
    if not event.wait(timeout=60):
        with _confirm_lock:
            _pending_confirmations.pop(confirm_id, None)
            _confirm_events.pop(confirm_id, None)
        return False  # 超时视为拒绝

    with _confirm_lock:
        result = _confirm_results.pop(confirm_id, False)
        _pending_confirmations.pop(confirm_id, None)
        _confirm_events.pop(confirm_id, None)
    return result


def handle_confirm_response(confirm_id: str, confirmed: bool) -> bool:
    """处理前端发回的确认响应。返回 True=找到并处理，False=ID 不存在或已过期。"""
    with _confirm_lock:
        event = _confirm_events.get(confirm_id)
        if event is None:
            return False
        _confirm_results[confirm_id] = confirmed
        event.set()
        return True
# ========== 桌面路径解析（不依赖前端 IPC，自给自足） ==========

def resolve_desktop_path() -> str:
    """解析真实桌面路径（镜像 Rust tools.rs::get_common_paths 的 OneDrive 优先逻辑）。

    前端传的 desktop_path 可能为空（IPC 失败/旧 build），此时 agent 自己解析，
    避免落到 C:/Users/Public/Desktop 兜底导致权限错误。
    """
    home = os.environ.get("USERPROFILE", "")
    if home:
        onedrive = os.path.join(home, "OneDrive", "Desktop")
        if os.path.isdir(onedrive):
            return onedrive.replace("\\", "/")
        default = os.path.join(home, "Desktop")
        if os.path.isdir(default):
            return default.replace("\\", "/")
    return "C:/Users/Public/Desktop"


# ========== 系统提示词（从原 TS buildSystemPrompt 平移，注入桌面路径） ==========

def build_system_prompt(desktop_path: str, variant: str = "baseline") -> str:
    desktop = desktop_path or "C:/Users/Public/Desktop"
    base = f"""你是 LocalMind，一个运行在 Windows 电脑上的 AI 助手。
你能直接操作电脑。可用工具：write_file（写文件）、read_file（读文件）、list_dir（列目录）、move_file（移动/整理文件）、open_app（打开应用）、read_clipboard（读剪贴板）、create_doc（生成 PPT/Word/Excel/PDF 文档）、run_command（执行任意 shell 命令：安装软件、运行脚本、Git、系统管理等）、delete_path（删除文件或目录）。
当用户要求创建文件、写文件、生成文档时，必须调用对应工具实际执行，绝不能只在回复里口头说"已创建"或"已完成"——只有工具执行返回成功才算真的完成。
如果用户只是聊天，直接回答即可。所有回答请使用中文。

生成文档时使用 create_doc 工具，doc_type 与 spec 格式如下：
- ppt（演示文稿）：spec={{"path":"{desktop}/文件名.pptx","title":"主标题","subtitle":"副标题","slides":[{{"title":"页标题","bullets":["要点1","要点2"]}}]}}
- docx（Word）：spec={{"path":"{desktop}/文件名.docx","title":"文档标题","headings":[{{"level":1,"text":"章节"}}],"paragraphs":["段落1"],"bullets":["要点1"]}}
- xlsx（Excel）：spec={{"path":"{desktop}/文件名.xlsx","sheet_name":"表名","headers":["列1","列2"],"rows":[["值1","值2"]]}}
- pdf（PDF）：spec={{"path":"{desktop}/文件名.pdf","title":"标题","subtitle":"副标题","sections":[{{"heading":"章节","body":["段落"]}}]}}

重要：这台电脑的桌面路径是 {desktop}。
用户说"存到桌面 / 放桌面 / 桌面"时，保存路径固定用 {desktop}/。
用户未指定路径时，默认也保存到桌面 {desktop}/。

【常用操作引导】
- 用户要求"总结/读取剪贴板"时，先调用 read_clipboard 获取剪贴板内容，再基于内容回答或总结。
- 用户要求"整理桌面/整理文件夹/分类文件"时，先调用 list_dir 查看目录内容，再用 move_file 把文件移动到对应子目录（如图片/、文档/、视频/、压缩包/ 等）。
- 用户要求"打开 XX 应用 / 打开文件 / 打开文件夹"时，调用 open_app（参数填应用名或完整路径）。
- 用户要求"浏览/查看文件夹里有什么"时，调用 list_dir。

【关于 create_doc 的强制规则】
1. 生成 PPT/Word/Excel/PDF 时必须调用 create_doc，不允许只输出 Markdown 或口头说明。
2. 严禁编造"工具不可用""生成失败""需要安装依赖"等理由，也不要用 write_file 写脚本代替 create_doc——直接用 create_doc 生成最终文档文件。
3. 如果 create_doc 工具调用确实返回了错误，把工具返回的真实错误信息告诉用户，而不是自行编造。"""
    if variant != "v2":
        return base
    return base + f"""

【Harness v2 执行协议】
1. 先判断任务类型：纯问答直接回答；需要操作文件或系统时必须调用工具，不要虚构执行结果。
2. 调用工具前先确认参数完整，优先使用绝对路径。路径必须位于当前允许目录内，不能访问系统目录、Program Files、UNC 或 device namespace。
3. 每个工具只执行一次相同调用。若第一次成功，直接使用结果；若失败，读取结构化错误并改变策略，不要原样重试。
4. 工具返回 ok=false 时，先解释真实原因，再尝试一个替代方案。策略拒绝（POLICY_DENIED）不得绕过。
5. 达到任务目标后立即停止，不要继续调用无必要工具。最多允许 8 次模型请求和 12 次工具调用。
6. 最终回答只描述真实发生的结果：文件已生成、内容已读取或任务被拒绝，必须有对应工具结果支撑。
7. 默认桌面路径仍然是 {desktop}/；本轮评测或测试若给出新的工作目录，以请求参数为准。"""


def normalize_path_separators(value):
    """递归把 spec JSON 中所有字符串的反斜杠替换为正斜杠（对齐 tools.rs）。"""
    if isinstance(value, str):
        return value.replace("\\", "/")
    if isinstance(value, list):
        return [normalize_path_separators(v) for v in value]
    if isinstance(value, dict):
        return {k: normalize_path_separators(v) for k, v in value.items()}
    return value


def find_doc_exe() -> str:
    """查找 make_doc.exe（对齐 tools.rs::find_doc_exe 的候选路径）。"""
    candidates = []
    # 打包后（PyInstaller onedir）：sys.executable 为 localmind-agent.exe，
    # make_doc.exe 可能在其父目录（standalone: LocalMindScripts/make_doc.exe）或
    # 父目录/scripts/（安装版: <INSTALL_DIR>/scripts/make_doc.exe）
    if getattr(sys, "frozen", False):
        exe_dir = Path(sys.executable).resolve().parent
        candidates.append(exe_dir.parent / "make_doc.exe")
        candidates.append(exe_dir.parent / "scripts" / "make_doc.exe")
        candidates.append(exe_dir.parent / "LocalMindScripts" / "make_doc.exe")
        candidates.append(exe_dir.parent / "make_doc" / "make_doc.exe")
    # 开发模式：脚本在 localmind/scripts/ 下
    script_dir = Path(__file__).resolve().parent
    candidates.append(script_dir / "make_doc.exe")
    candidates.append(script_dir / "dist" / "make_doc.exe")
    candidates.append(script_dir.parent / "src-tauri/scripts/dist/make_doc.exe")
    # 当前工作目录（Rust 以项目根为 cwd 启动）
    cwd = Path.cwd()
    candidates.append(cwd / "scripts/dist/make_doc.exe")
    candidates.append(cwd / "scripts/make_doc.exe")
    candidates.append(cwd / "LocalMindScripts/make_doc.exe")

    for c in candidates:
        if c.exists():
            return str(c)
    return ""


def build_create_doc(policy: ToolPolicy) -> object:
    def create_doc(doc_type: str, spec: dict) -> str:
        """生成文档文件（PPT/Word/Excel/PDF）。用户要求制作演示文稿、文档、表格、PDF 时使用。
        doc_type 取值：ppt=演示文稿, docx=Word文档, xlsx=Excel表格, pdf=PDF。
        spec 为文档内容规格，必须包含 path 字段指定输出路径：ppt 含 title/slides；docx 含 title/paragraphs/bullets；xlsx 含 sheet_name/headers/rows；pdf 含 title/sections。"""
        doc_type = str(doc_type).lower()
        valid = {
            "ppt", "pptx", "docx", "doc", "word", "xlsx", "xls", "excel", "pdf",
        }
        if doc_type not in valid:
            return f"不支持的文档类型: {doc_type}（支持 ppt/docx/xlsx/pdf）"
        policy.validate_create_doc(spec)
        exe = find_doc_exe()
        if not exe:
            return "未找到 make_doc.exe，无法生成文档。请确认它与 LocalMind.exe 在同一目录（或 scripts/ 子目录）。"
        try:
            normalized = normalize_path_separators(spec)
            spec_json = json.dumps(normalized, ensure_ascii=False)
            fd, tmp_spec = tempfile.mkstemp(suffix=".json", prefix=f"localmind_{doc_type}_spec_")
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(spec_json)
            try:
                proc = subprocess.run(
                    [exe, "--type", doc_type, tmp_spec],
                    capture_output=True,
                    text=True,
                    timeout=120,
                    creationflags=subprocess.CREATE_NO_WINDOW,
                )
            finally:
                try:
                    os.remove(tmp_spec)
                except OSError:
                    pass
            path = spec.get("path", "")
            if proc.returncode == 0:
                stdout = (proc.stdout or "").strip()
                return f"已生成文档: {path}\n{stdout}"
            return f"生成失败: {path}\n{(proc.stderr or '').strip()}\n{(proc.stdout or '').strip()}"
        except Exception as e:
            return f"生成文档失败: {e}"

    return create_doc


def build_tools(
    emit_thinking,
    policy: ToolPolicy,
    trace: TraceRecorder | None = None,
    variant: str = "baseline",
) -> ToolRegistry:
    """构建 Registry 声明的工具。v2 增加重复调用检测、结构化错误和输出截断。"""
    v2 = variant == "v2"
    seen_calls: set[str] = set()
    seen_calls_lock = threading.Lock()
    cached_results: dict[str, str] = {}

    def _fingerprint(name: str, kwargs: dict) -> str:
        raw = name + "|" + json.dumps(kwargs, ensure_ascii=False, sort_keys=True, default=str)
        return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:16]

    def _structured_error(code: str, message: str, *, retryable: bool = False) -> str:
        return json.dumps(
            {"ok": False, "error": {"code": code, "message": message[:1000], "retryable": retryable}},
            ensure_ascii=False,
        )

    def _output_limit(name: str) -> int:
        spec = TOOL_SPECS.get(name)
        return spec.max_output_chars if spec else 12_000

    def _is_declared_failure(name: str, output: str) -> bool:
        if name == "create_doc":
            return output.startswith(("不支持的", "未找到", "生成失败", "生成文档失败"))
        if name == "move_file":
            return output.startswith(("源文件不存在", "移动失败"))
        if name == "read_clipboard":
            return output.startswith("读取剪贴板失败")
        if name == "open_app":
            return output.startswith("打开失败")
        return False

    def write_file(path: str, content: str) -> str:
        """创建新文件或覆盖写一个文本文件。目录不存在会自动创建。用于生成代码、脚本、文档、配置文件、笔记等。path 为文件的完整路径，content 为要写入的完整文件内容。"""
        p = policy.validate_write(path)
        if p.parent and str(p.parent) != ".":
            p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
        return f"已写入文件: {p}\n大小: {len(content)} 字节"

    def read_file(path: str) -> str:
        """读取文本文件内容。用于查看代码、配置、日志、笔记等文件内容。path 为文件的完整路径。"""
        p = policy.validate_read(path)
        data = p.read_bytes()
        size = len(data)
        text = data.decode("utf-8", errors="replace")
        MAX = 6000
        if len(text) > MAX:
            text = text[:MAX] + f"\n…[输出已截断，剩余 {len(text) - MAX} 字符]"
        return f"【文件: {p} | 大小: {size} 字节】\n\n{text}"

    def read_clipboard() -> str:
        """读取 Windows 剪贴板文本内容（直接读 UTF-16，避免 PowerShell 管道编码乱码）。"""
        try:
            import ctypes
            from ctypes import wintypes
            user32 = ctypes.windll.user32
            kernel32 = ctypes.windll.kernel32
            user32.OpenClipboard.argtypes = [wintypes.HWND]
            user32.OpenClipboard.restype = wintypes.BOOL
            user32.IsClipboardFormatAvailable.argtypes = [wintypes.UINT]
            user32.IsClipboardFormatAvailable.restype = wintypes.BOOL
            user32.GetClipboardData.argtypes = [wintypes.UINT]
            user32.GetClipboardData.restype = wintypes.HANDLE
            kernel32.GlobalLock.argtypes = [wintypes.HGLOBAL]
            kernel32.GlobalLock.restype = wintypes.LPVOID
            kernel32.GlobalUnlock.argtypes = [wintypes.HGLOBAL]
            kernel32.GlobalUnlock.restype = wintypes.BOOL
            CF_UNICODETEXT = 13
            if user32.OpenClipboard(None):
                try:
                    if user32.IsClipboardFormatAvailable(CF_UNICODETEXT):
                        handle = user32.GetClipboardData(CF_UNICODETEXT)
                        if handle:
                            ptr = kernel32.GlobalLock(handle)
                            try:
                                text = ctypes.wstring_at(ptr) if ptr else ""
                                if text and text.strip():
                                    return f"【剪贴板内容】\n\n{text}"
                            finally:
                                kernel32.GlobalUnlock(handle)
                    return "剪贴板为空或无文本内容"
                finally:
                    user32.CloseClipboard()
        except Exception:
            pass
        try:
            ps_cmd = ("[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;"
                      "$OutputEncoding=[System.Text.Encoding]::UTF8;Get-Clipboard -Raw")
            out = subprocess.run(["powershell", "-NoProfile", "-Command", ps_cmd], capture_output=True, text=True, timeout=8, encoding="utf-8", errors="replace")
            if out.returncode == 0 and out.stdout and out.stdout.strip():
                return f"【剪贴板内容】\n\n{out.stdout.rstrip(chr(13) + chr(10))}"
            return "剪贴板为空或无文本内容"
        except Exception as e:
            return f"读取剪贴板失败: {e}"

    def list_dir(path: str) -> str:
        """列出目录内容（子目录与文件，附大小）。用于浏览文件夹、整理桌面前的查看。path 为目录完整路径。"""
        p = policy.validate_read(path)
        if not p.is_dir():
            return f"不是有效目录: {path}"
        items = []
        try:
            for child in sorted(p.iterdir()):
                try:
                    items.append(f"[目录] {child.name}/" if child.is_dir() else f"[文件] {child.name} ({child.stat().st_size} 字节)")
                except Exception:
                    items.append(f"[?] {child.name}")
        except PermissionError as e:
            return f"权限不足，无法访问目录 {path}: {e}"
        MAX_ITEMS = 100
        if len(items) > MAX_ITEMS:
            items = items[:MAX_ITEMS] + [f"…（共 {len(items)} 项，仅显示前 {MAX_ITEMS} 项）"]
        return f"【目录: {p} | 共 {len(items)} 项】\n" + "\n".join(items)

    def open_app(target: str) -> str:
        """打开白名单应用、受控目录/文件或 http(s) URL。target 不接受任意 shell 命令。"""
        kind, value = policy.validate_open_target(target)
        try:
            if kind == "app":
                subprocess.Popen([value], creationflags=subprocess.CREATE_NO_WINDOW)
            else:
                os.startfile(value)
            return f"已启动: {value}"
        except Exception as e:
            return f"打开失败: {e}"

    def run_command(command: str) -> str:
        """执行任意 shell 命令。可用于安装软件、运行脚本、系统管理、Git 操作等。command 为完整的命令行字符串，会通过 PowerShell 执行。返回命令的 stdout 和 stderr。"""
        try:
            result = subprocess.run(
                ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command],
                capture_output=True,
                text=True,
                timeout=30,
                encoding="utf-8",
                errors="replace",
                creationflags=subprocess.CREATE_NO_WINDOW,
            )
            parts = []
            if result.stdout and result.stdout.strip():
                parts.append(f"【stdout】\n{result.stdout.strip()}")
            if result.stderr and result.stderr.strip():
                parts.append(f"【stderr】\n{result.stderr.strip()}")
            if not parts:
                parts.append("(无输出)")
            status = "成功" if result.returncode == 0 else f"退出码 {result.returncode}"
            return f"命令执行{status}:\n" + "\n".join(parts)
        except subprocess.TimeoutExpired:
            return _structured_error("COMMAND_TIMEOUT", "命令执行超时（30秒限制）", retryable=False)
        except Exception as e:
            return f"命令执行失败: {e}"

    def delete_path(path: str) -> str:
        """删除文件或目录（目录会递归删除）。用于清理临时文件、卸载等。path 为要删除的文件或目录的完整路径。"""
        import shutil as _shutil
        p = policy.validate_delete(path)
        if not p.exists():
            return f"路径不存在: {path}"
        try:
            if p.is_dir():
                _shutil.rmtree(str(p))
                return f"已删除目录: {p}"
            else:
                p.unlink()
                return f"已删除文件: {p}"
        except Exception as e:
            return f"删除失败: {e}"
    def move_file(src: str, dst: str) -> str:
        """移动/重命名文件或目录。整理文件时使用：dst 可以是目标路径，也可以为目标目录（自动保留文件名）。"""
        s_path, d_path = policy.validate_move(src, dst)
        if not s_path.exists():
            return f"源文件不存在: {src}"
        if d_path.is_dir():
            d_path = policy.validate_write(d_path / s_path.name)
        try:
            d_path.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(s_path), str(d_path))
            return f"已移动: {s_path} -> {d_path}"
        except Exception as e:
            return f"移动失败: {e}"

    def _wrap(name, fn):
        @functools.wraps(fn)
        def wrapped(**kwargs):
            fingerprint = _fingerprint(name, kwargs)
            duplicate = False
            cached_output: str | None = None
            if v2:
                with seen_calls_lock:
                    if fingerprint in seen_calls:
                        duplicate = True
                        spec = TOOL_SPECS.get(name)
                        if spec and spec.cacheable:
                            cached_output = cached_results.get(fingerprint)
                    else:
                        seen_calls.add(fingerprint)
            if duplicate:
                if cached_output is not None:
                    return cached_output
                if trace:
                    trace.record("duplicate_tool_call", tool=name, args_hash=fingerprint)
                emit_thinking("reflecting", f"{name} 重复调用已被拦截，请改用新参数或基于已有结果继续", "error")
                return _structured_error("DUPLICATE_TOOL_CALL", f"同一 Turn 内已执行过完全相同的 {name} 调用，禁止重复执行")
            if trace:
                trace.record("tool_call_started", tool=name, args_hash=fingerprint)
            # 高危工具需要用户确认
            spec = TOOL_SPECS.get(name)
            if spec and spec.confirmation_required:
                confirmed = request_confirmation(name, kwargs, lambda evt: emit_thinking("confirming", json.dumps(evt, ensure_ascii=False), "running"))
                if not confirmed:
                    return _structured_error("CONFIRMATION_DENIED", f"用户拒绝执行 {name}", retryable=False)

            try:
                output = str(fn(**kwargs) or "")
                if v2 and _is_declared_failure(name, output):
                    if trace:
                        trace.record("tool_call_finished", tool=name, success=False, error_code="TOOL_EXECUTION_FAILED")
                    return _structured_error("TOOL_EXECUTION_FAILED", output, retryable=True)
                limit = _output_limit(name)
                if len(output) > limit:
                    output = output[:limit] + f"\n…[工具输出已截断，原始长度 {len(output)} 字符]"
                if v2 and TOOL_SPECS.get(name) and TOOL_SPECS[name].cacheable:
                    cached_results[fingerprint] = output
                if trace:
                    trace.record("tool_call_finished", tool=name, success=True, output_chars=len(output))
                return output
            except Exception as e:
                if trace:
                    trace.record("retry_started", tool=name, error=str(e)[:300])
                    trace.record("tool_call_finished", tool=name, success=False, error=str(e)[:300])
                emit_thinking("reflecting", f"{name} 执行失败，正在分析原因...", "running")
                emit_thinking("retrying", f"失败原因：{e}", "running")
                if not v2:
                    raise
                if isinstance(e, ToolPolicyError):
                    return _structured_error("POLICY_DENIED", str(e), retryable=False)
                if isinstance(e, TimeoutError):
                    return _structured_error("TOOL_TIMEOUT", str(e), retryable=True)
                if isinstance(e, FileNotFoundError):
                    return _structured_error("FILE_NOT_FOUND", str(e), retryable=True)
                return _structured_error("TOOL_ERROR", str(e), retryable=True)
        return wrapped

    registry = ToolRegistry()
    registry.register("write_file", _wrap("write_file", write_file))
    registry.register("read_file", _wrap("read_file", read_file))
    registry.register("create_doc", _wrap("create_doc", build_create_doc(policy)))
    registry.register("read_clipboard", _wrap("read_clipboard", read_clipboard))
    registry.register("list_dir", _wrap("list_dir", list_dir))
    registry.register("open_app", _wrap("open_app", open_app))
    registry.register("move_file", _wrap("move_file", move_file))
    registry.register("run_command", _wrap("run_command", run_command))
    registry.register("delete_path", _wrap("delete_path", delete_path))
    return registry


# ========== 文本工具调用兜底（小模型通病） ==========

def extract_json_objects(text):
    """从文本提取所有平衡的 JSON 对象块（支持嵌套花括号，对齐原 TS extractJsonObjects）。"""
    results = []
    i = 0
    n = len(text)
    while i < n:
        if text[i] == "{":
            depth = 0
            in_string = False
            j = i
            while j < n:
                c = text[j]
                if in_string:
                    if c == "\\":
                        j += 1
                    elif c == '"':
                        in_string = False
                else:
                    if c == '"':
                        in_string = True
                    elif c == "{":
                        depth += 1
                    elif c == "}":
                        depth -= 1
                        if depth == 0:
                            results.append(text[i : j + 1])
                            break
                j += 1
            i = j + 1
        else:
            i += 1
    return results


def parse_tool_call_from_text(text):
    """从模型纯文本输出解析工具调用（返回 name + args dict；没有则 None）。

    小模型（如 qwen 7b）在复杂工具集下可能把工具调用输出成 JSON 文本而非标准 tool_calls，
    支持两种格式：
      1. {"name": "write_file", "arguments": {...}}
      2. <tool_call>{"name":"...","arguments":{...}}</tool_call>
    """
    if not text:
        return None
    for block in extract_json_objects(text):
        try:
            parsed = json.loads(block)
        except Exception:
            continue
        name = parsed.get("name") or (parsed.get("function") or {}).get("name")
        if not isinstance(name, str) or not name:
            continue
        args = parsed.get("arguments") or (parsed.get("function") or {}).get("arguments") or {}
        if isinstance(args, str):
            try:
                args = json.loads(args)
            except Exception:
                args = {}
        return {"name": name, "args": args if isinstance(args, dict) else {}}
    return None


def strip_tool_call_text(text):
    """从模型回复中移除工具调用文本，保留纯对话内容（对齐原 TS stripToolCallText）。"""
    if not text:
        return text
    cleaned = text
    # 去掉 <tool_call>...</tool_call> 块
    import re
    cleaned = re.sub(r"<tool_call>[\s\S]*?</tool_call>", "", cleaned)
    # 去掉独立的 {"name":"...","arguments":{...}} JSON 块（支持嵌套）
    for block in extract_json_objects(cleaned):
        try:
            p = json.loads(block)
            if p.get("name") and p.get("arguments") is not None:
                cleaned = cleaned.replace(block, "")
        except Exception:
            pass
    return cleaned.strip()


# ========== 消息转换 ==========

_FINAL_REPLY_MARKER = "[最终回复]"

_TOOL_BLOCK_RE = re.compile(
    r"\[调用工具:(?P<name>[^\]]+)\]\s*(?P<args>.*?)\n"
    r"\[工具结果:(?P=name)\]\s*(?P<result>.*?)"
    r"(?=\n\[调用工具:|\n" + re.escape(_FINAL_REPLY_MARKER) + r"|\Z)",
    re.DOTALL,
)


def parse_tool_blocks(content: str):
    """解析前端嵌入的工具调用标记，返回 (tool_calls, trailing_text)。

    前端 `embedToolLogs()` 写入的格式：
        [调用工具:write_file] {"path":"a.txt"}
        [工具结果:write_file] 已写入文件: a.txt
        [调用工具:read_file] {...}
        [工具结果:read_file] ...
        <最终的 assistant 文本>

    返回 [] 表示这段内容里没有工具调用，调用方按普通文本处理。
    """
    content = content.replace(_FINAL_REPLY_MARKER, "").strip() if _FINAL_REPLY_MARKER in content and not content.lstrip().startswith("[调用工具:") else content
    matches = list(_TOOL_BLOCK_RE.finditer(content))
    if not matches:
        return [], content

    calls = []
    for idx, m in enumerate(matches):
        name = m.group("name").strip()
        raw_args = (m.group("args") or "").strip()
        raw_result = (m.group("result") or "").strip()

        try:
            args = json.loads(raw_args) if raw_args else {}
            if not isinstance(args, dict):
                args = {"value": args}
        except (ValueError, TypeError):
            args = {"raw": raw_args}

        failed = raw_result.startswith("(失败)")
        if failed:
            raw_result = raw_result[len("(失败)"):].strip()

        calls.append({
            "tool_call_id": f"hist-{idx}-{abs(hash(name)) % 100000}",
            "tool_name": name,
            "args": args,
            "result": raw_result,
            "failed": failed,
        })

    # 优先用显式分隔符切出助手正文，避免把最后一段工具输出和正文混在一起
    if _FINAL_REPLY_MARKER in content:
        trailing = content.split(_FINAL_REPLY_MARKER, 1)[1].strip()
    else:
        trailing = content[matches[-1].end():].strip()
    return calls, trailing


def _assistant_history_entries(content: str):
    """把一条 assistant 消息转成 Pydantic AI 的历史条目。

    - 含工具调用：重建 ToolCallPart + ToolReturnPart（保留跨轮工具上下文）
    - 不含工具调用：退化成 TextPart
    解析失败时也安全退化为 TextPart。
    """
    try:
        calls, trailing = parse_tool_blocks(content)
    except Exception:
        return [ModelResponse(parts=[TextPart(content=content)])]

    if not calls:
        return [ModelResponse(parts=[TextPart(content=content)])]

    call_parts = []
    return_parts = []
    for call in calls:
        call_parts.append(
            ToolCallPart(
                tool_name=call["tool_name"],
                args=call["args"],
                tool_call_id=call["tool_call_id"],
            )
        )
        return_parts.append(
            ToolReturnPart(
                tool_name=call["tool_name"],
                content=call["result"],
                tool_call_id=call["tool_call_id"],
                outcome="failed" if call["failed"] else "success",
            )
        )

    entries = [ModelResponse(parts=call_parts), ModelRequest(parts=return_parts)]
    if trailing:
        entries.append(ModelResponse(parts=[TextPart(content=trailing)]))
    return entries


def split_messages(messages):
    """前端 AgentMessage[] → (prompt, message_history)。

    - 最后一条 user 消息作为本次 prompt（不进 history）
    - 其余按 role 转成 Pydantic AI 的 ModelRequest/ModelResponse
    - assistant 消息里的工具调用标记会重建成 ToolCallPart / ToolReturnPart，
      让模型在后续轮次能看到「之前调用过哪些工具、结果是什么」
    """
    history = []
    prompt = ""
    user_contents = [m.get("content", "") for m in messages if m.get("role") == "user"]
    last_user = user_contents[-1] if user_contents else ""

    for m in messages:
        role = m.get("role")
        content = m.get("content", "")
        if role == "user" and content == last_user and not prompt:
            prompt = content
            continue
        if role == "system":
            history.append(ModelRequest(parts=[SystemPromptPart(content=content)]))
        elif role == "user":
            history.append(ModelRequest(parts=[UserPromptPart(content=content)]))
        elif role == "assistant":
            history.extend(_assistant_history_entries(content))
    if not prompt:
        prompt = last_user
    return prompt, history


# ========== HTTP + SSE ==========

def _log(msg: str):
    """写入 agent 诊断日志：%TEMP%/localmind-agent.log"""
    try:
        path = os.path.join(tempfile.gettempdir(), "localmind-agent.log")
        with open(path, "a", encoding="utf-8") as f:
            f.write(f"[{time.strftime('%Y-%m-%d %H:%M:%S')}] {msg}\n")
    except Exception:
        pass

_think_seq = [0]


class AgentHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):  # 静默默认访问日志
        pass

    # ---- CORS 工具 ----

    def _handle_confirm(self):
        """处理前端发回的确认/拒绝响应。"""
        length = int(self.headers.get("Content-Length", 0))
        if length <= 0:
            self._send_json({"error": "empty body"}, 400)
            return
        try:
            body = json.loads(self.rfile.read(length).decode("utf-8"))
        except Exception as e:
            self._send_json({"error": f"bad json: {e}"}, 400)
            return
        confirm_id = body.get("id", "")
        confirmed = body.get("confirmed", False)
        if not confirm_id:
            self._send_json({"error": "missing id"}, 400)
            return
        handled = handle_confirm_response(confirm_id, bool(confirmed))
        self._send_json({"ok": handled})
    def _cors_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "content-type, x-localmind-token")
        self.send_header("Access-Control-Max-Age", "86400")

    def _send_json(self, obj, status=200):
        data = json.dumps(obj, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self._cors_headers()
        self.end_headers()
        self.wfile.write(data)

    # ---- 路由 ----

    def do_OPTIONS(self):
        self.send_response(204)
        self._cors_headers()
        self.end_headers()

    def do_GET(self):
        if self.path.startswith("/health"):
            self._send_json({"ok": True, "token_valid": self.headers.get("X-LocalMind-Token") == AGENT_TOKEN})
        else:
            self._send_json({"error": "not found"}, 404)

    def do_POST(self):
        if self.path.startswith("/agent/stream"):
            self._stream_agent()
        elif self.path == "/agent/confirm":
            self._handle_confirm()
        else:
            self._send_json({"error": "not found"}, 404)

    # ---- Agent 流式处理 ----

    def _stream_agent(self):
        if self.headers.get("X-LocalMind-Token") != AGENT_TOKEN:
            self._send_json({"error": "unauthorized"}, 403)
            _log("UNAUTHORIZED")
            return
        length = int(self.headers.get("Content-Length", 0))
        if length <= 0:
            self._send_json({"error": "empty body"}, 400)
            return
        try:
            body = json.loads(self.rfile.read(length).decode("utf-8"))
        except Exception as e:
            self._send_json({"error": f"bad json: {e}"}, 400)
            return

        _log(f"REQUEST mode={body.get('mode')} msgs={len(body.get('messages', []) or [])}")
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self._cors_headers()
        self.end_headers()
        self.wfile.flush()
        self.close_connection = True  # 每次消息一条 SSE 流，无需 keep-alive 复用

        try:
            asyncio.run(self._run_agent(body))
        except (BrokenPipeError, ConnectionResetError, OSError):
            pass  # 前端已断开（停止生成），静默结束

    def _write_event(self, obj):
        data = json.dumps(obj, ensure_ascii=False)
        self.wfile.write(f"data: {data}\n\n".encode("utf-8"))
        self.wfile.flush()

    def _fallback_success(self, name, output):
        """文本兜底执行时判断工具返回是否成功（工具返回错误字符串而非抛异常的情况）。"""
        try:
            parsed = json.loads(output)
            if isinstance(parsed, dict) and parsed.get("ok") is False:
                return False
        except Exception:
            pass
        if name == "read_file":
            return not output.startswith("文件不存在")
        if name == "create_doc":
            return not (
                output.startswith("不支持的")
                or output.startswith("未找到")
                or output.startswith("生成失败")
                or output.startswith("生成文档失败")
            )
        return True

    def _emit_thinking(self, phase, label, status="running", detail=None):
        _think_seq[0] += 1
        self._write_event({
            "type": "thinking",
            "step": {
                "id": f"think_{_think_seq[0]}",
                "phase": phase,
                "label": label,
                "detail": detail,
                "status": status,
                "timestamp": int(time.time() * 1000),
            },
        })

    async def _run_agent(self, body):
        variant = str(body.get("harness_variant") or os.environ.get("LOCALMIND_HARNESS_VARIANT", "v2")).lower()
        if variant not in {"baseline", "v2"}:
            variant = "v2"
        v2 = variant == "v2"
        turn_id = str(body.get("turn_id") or f"anonymous-{int(time.time() * 1000)}")
        trace = TraceRecorder(turn_id, variant, emit=self._write_event)
        tool_logs: list[dict] = []
        trace.record("turn_started", mode=body.get("mode", "online"), message_count=len(body.get("messages", []) or []))

        try:
            mode = body.get("mode", "online")
            desktop = body.get("desktop_path") or resolve_desktop_path()

            if mode == "offline":
                model_name = body.get("model") or "qwen2.5:7b"
                model = OllamaModel(
                    model_name,
                    provider=OllamaProvider(base_url=OLLAMA_BASE, api_key="not-needed"),
                )
            else:
                model_name = body.get("model") or DEFAULT_ONLINE_MODEL
                # 用户自填的 key 优先；环境变量仅作开发调试兜底
                token = body.get("token") or os.environ.get("LOCALMIND_DEEPSEEK_KEY", "")
                model = OpenAIChatModel(
                    model_name,
                    provider=OpenAIProvider(base_url=DEEPSEEK_BASE, api_key=token),
                )

            selected_attachment_paths = body.get("selected_attachment_paths") or []
            allowed_roots = body.get("allowed_roots") if os.environ.get("LOCALMIND_EVAL_MODE") == "1" else None
            policy = ToolPolicy(
                allowed_roots=allowed_roots,
                selected_attachment_paths=selected_attachment_paths,
            )
            tools = build_tools(self._emit_thinking, policy, trace=trace, variant=variant)
            prompt, history = split_messages(body.get("messages", []))

            system_prompt = build_system_prompt(desktop, variant)

            # Context Manager
            system_prompt_tokens = estimate_tokens(system_prompt)
            prompt_tokens = estimate_tokens(prompt)
            history = truncate_history(history, system_prompt_tokens, prompt_tokens)
            trace.record(
                "context_built",
                model=model_name,
                mode=mode,
                prompt_chars=len(prompt),
                history_messages=len(history),
                system_prompt_chars=len(system_prompt),
                tools=list(tools.names()),
            )

            if v2:
                agent_tools = [
                    Tool(
                        tools.get(spec.name),
                        name=spec.name,
                        timeout=max(1.0, spec.timeout_ms / 1000),
                    )
                    for spec in tools.specs()
                ]
            else:
                agent_tools = tools.callables()

            agent = Agent(
                model,
                system_prompt=system_prompt,
                tools=agent_tools,
                retries=1,
                tool_timeout=120 if v2 else None,
            )

            self._emit_thinking("planning", "正在分析任务...", "running")
            trace.record("model_request_started", model=model_name)

            content_parts = []
            tool_calls = {}
            started_at = time.time()
            usage_limits = UsageLimits(request_limit=8, tool_calls_limit=12) if v2 else None
            async with agent.run_stream_events(
                prompt,
                message_history=history,
                usage_limits=usage_limits,
            ) as events:
                async for event in events:
                    if isinstance(event, PartDeltaEvent):
                        d = event.delta
                        if isinstance(d, TextPartDelta) and d.content_delta:
                            trace.first_token()
                            content_parts.append(d.content_delta)
                            self._write_event({"type": "delta", "text": d.content_delta})
                    elif isinstance(event, FunctionToolCallEvent):
                        part = event.part
                        args = part.args
                        if not isinstance(args, str):
                            args = json.dumps(args, ensure_ascii=False) if args is not None else "{}"
                        tool_calls[part.tool_call_id] = {"name": part.tool_name, "args": args}
                        self._emit_thinking("executing", f"正在执行：{part.tool_name}", "running")
                    elif isinstance(event, FunctionToolResultEvent):
                        part = event.part
                        tool_name = getattr(part, "tool_name", "")
                        call = tool_calls.get(getattr(part, "tool_call_id", ""), {})
                        outcome = getattr(part, "outcome", "success")
                        output = getattr(part, "content", "")
                        if isinstance(output, (list, tuple)):
                            output = " ".join(str(x) for x in output)
                        output = str(output or "")
                        success = outcome != "failed" and self._fallback_success(tool_name, output)
                        log = {
                            "name": call.get("name", tool_name),
                            "args": call.get("args", "{}"),
                            "output": output[:4000],
                            "success": success,
                        }
                        tool_logs.append(log)
                        self._write_event({"type": "tool", "log": log})
                        if success:
                            self._emit_thinking("executing", f"{tool_name} 完成", "success")
                        else:
                            self._emit_thinking("executing", f"{tool_name} 失败", "error")
                    elif isinstance(event, FinalResultEvent):
                        pass

            usage = None
            try:
                usage = events.usage
            except Exception:
                pass

            result = getattr(events, "result", None)
            content = ""
            if result is not None:
                content = getattr(result, "output", None)
                if content is None:
                    content = getattr(result, "data", "")
            content = str(content or "")
            if not content:
                content = "".join(content_parts)

            if not tool_calls:
                fallback = parse_tool_call_from_text(content)
                if fallback and tools.contains(fallback["name"]):
                    name, args = fallback["name"], fallback["args"]
                    self._emit_thinking("executing", f"正在执行：{name}（本地模型文本指令）", "running")
                    try:
                        output = str(tools.get(name)(**args) or "")
                        success = self._fallback_success(name, output)
                        log = {
                            "name": name,
                            "args": json.dumps(args, ensure_ascii=False),
                            "output": output[:4000],
                            "success": success,
                        }
                        tool_logs.append(log)
                        self._write_event({"type": "tool", "log": log})
                        if success:
                            self._emit_thinking("executing", f"{name} 完成", "success")
                            content = f"（本地模型未走标准工具调用，已按其文本指令执行 {name}）\n\n{output}"
                        else:
                            self._emit_thinking("executing", f"{name} 执行失败", "error")
                            content = f"（尝试执行本地模型文本指令失败：{output}）"
                    except Exception as e:
                        self._emit_thinking("executing", f"{name} 执行失败", "error")
                        self._write_event({
                            "type": "tool",
                            "log": {"name": name, "args": json.dumps(args, ensure_ascii=False), "output": str(e), "success": False},
                        })
                        content = f"（尝试执行本地模型文本指令失败：{e}）"

            content = strip_tool_call_text(content)
            trace.finish("completed", content=content, usage=usage, tool_logs=tool_logs)
            elapsed_ms = int((time.time() - started_at) * 1000)
            usage_payload = None
            if usage is not None:
                usage_payload = {
                    "input_tokens": int(getattr(usage, "input_tokens", 0) or 0),
                    "output_tokens": int(getattr(usage, "output_tokens", 0) or 0),
                    "total_tokens": int(getattr(usage, "total_tokens", 0) or 0),
                    "cache_read_tokens": int(getattr(usage, "cache_read_tokens", 0) or 0),
                    "cache_write_tokens": int(getattr(usage, "cache_write_tokens", 0) or 0),
                    "requests": int(getattr(usage, "requests", 0) or 0),
                    "tool_calls": int(getattr(usage, "tool_calls", 0) or 0),
                }
            self._write_event({
                "type": "done",
                "content": content,
                "usage": usage_payload,
                "elapsed_ms": elapsed_ms,
            })

        except (BrokenPipeError, ConnectionResetError, OSError):
            trace.finish("cancelled", error="client disconnected", tool_logs=tool_logs)
            raise
        except Exception as e:
            try:
                trace.finish("failed", error=str(e), tool_logs=tool_logs)
                self._write_event({"type": "error", "message": str(e)})
            except (BrokenPipeError, ConnectionResetError, OSError):
                pass
            _log(f"RUN_ERROR {e}")


# ========== 入口 ==========

def main():
    server = ThreadingHTTPServer(("127.0.0.1", 0), AgentHandler)
    _log(f"START pid={os.getpid()}")
    port = server.server_address[1]

    # 双通道握手：stdout + 临时文件（文件名带 pid，Rust 用 child.id() 匹配，避免读到过期文件）
    print(f"PORT={port}", flush=True)
    try:
        port_file = os.path.join(
            tempfile.gettempdir(), f"localmind_agent_port_{os.getpid()}.txt"
        )
        with open(port_file, "w", encoding="utf-8") as f:
            f.write(str(port))
    except OSError:
        pass

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
