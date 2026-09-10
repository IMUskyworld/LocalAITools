#!/usr/bin/env python3
"""LocalMind Agent 服务 — Pydantic AI 2.x Agent 循环 + SSE HTTP 服务。

被 Rust 后端（Tauri）作为子进程启动：
  - 绑定 127.0.0.1 随机端口，启动后向 stdout 打印 `PORT=<n>` 并写入 %TEMP%/localmind_agent_port.txt。
  - 读取 env LOCALMIND_AGENT_TOKEN 作为访问凭证；请求须带 `X-LocalMind-Token` 头。
  - POST /agent/stream 走 SSE，事件契约：
      {"type":"thinking","step":{id,phase,label,status,detail,timestamp}}
      {"type":"tool","log":{name,args,output,success}}
      {"type":"delta","text":"..."}
      {"type":"done","content":"..."}
      {"type":"error","message":"..."}

开发模式独立运行：python scripts/agent_server.py（须设置 LOCALMIND_AGENT_TOKEN）
"""
import asyncio
import functools
import json
import os
import subprocess
import shutil
import sys
import tempfile
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from pydantic_ai import Agent
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
    UserPromptPart,
)
from pydantic_ai.models.ollama import OllamaModel
from pydantic_ai.models.openai import OpenAIChatModel
from pydantic_ai.providers.ollama import OllamaProvider
from pydantic_ai.providers.openai import OpenAIProvider

from tool_policy import ToolPolicy, ToolPolicyError
from tool_registry import ToolRegistry

AGENT_TOKEN = os.environ.get("LOCALMIND_AGENT_TOKEN", "dev-token")
OLLAMA_BASE = os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434/v1")
DEEPSEEK_BASE = "https://api.deepseek.com/v1"


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

def build_system_prompt(desktop_path: str) -> str:
    desktop = desktop_path or "C:/Users/Public/Desktop"
    return f"""你是 LocalMind，一个运行在 Windows 电脑上的 AI 助手。
你能直接操作电脑。可用工具：write_file（写文件）、read_file（读文件）、list_dir（列目录）、move_file（移动/整理文件）、open_app（打开应用）、read_clipboard（读剪贴板）、create_doc（生成 PPT/Word/Excel/PDF 文档）。
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

【关于 create_doc 的重要说明】
1. create_doc 工具能正常生成 PPT/Word/Excel/PDF，不需要任何额外安装（文档生成器已内置）。
2. 只要用户要求生成文档（ppt/word/excel/pdf/演示文稿/文档/表格），你必须调用 create_doc 工具，并把完整内容写进 spec。
3. 严禁编造"工具不可用""生成失败""需要安装依赖"等理由，也不要用 write_file 写脚本代替 create_doc——直接用 create_doc 生成最终文档文件。
4. 如果 create_doc 工具调用确实返回了错误，把工具返回的真实错误信息告诉用户，而不是自行编造。"""


# ========== 工具（纯 Python 实现，不依赖 Rust IPC） ==========

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


def build_tools(emit_thinking, policy: ToolPolicy) -> ToolRegistry:
    """构建 Registry 声明的工具。emit_thinking 用于表达失败与反思事件。"""

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
            out = subprocess.run(
                ["powershell", "-NoProfile", "-Command", ps_cmd],
                capture_output=True, text=True, timeout=8, encoding="utf-8", errors="replace",
            )
            if out.returncode == 0 and out.stdout and out.stdout.strip():
                text = out.stdout.rstrip("\r\n")
                return f"【剪贴板内容】\n\n{text}"
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
                    if child.is_dir():
                        items.append(f"[目录] {child.name}/")
                    else:
                        size = child.stat().st_size
                        items.append(f"[文件] {child.name} ({size} 字节)")
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

    def move_file(src: str, dst: str) -> str:
        """移动/重命名文件或目录。整理文件时使用：dst 可以是目标路径，也可以是目标目录（自动保留文件名）。"""
        s, d = policy.validate_move(src, dst)
        if not s.exists():
            return f"源文件不存在: {src}"
        if d.is_dir():
            d = policy.validate_write(d / s.name)
        try:
            d.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(s), str(d))
            return f"已移动: {s} -> {d}"
        except Exception as e:
            return f"移动失败: {e}"

    def _wrap(name, fn):
        @functools.wraps(fn)
        def wrapped(**kwargs):
            try:
                return fn(**kwargs)
            except Exception as e:
                emit_thinking("reflecting", f"{name} 执行失败，正在分析原因...", "running")
                emit_thinking("retrying", f"失败原因：{e}", "running")
                raise

        return wrapped

    registry = ToolRegistry()
    registry.register("write_file", _wrap("write_file", write_file))
    registry.register("read_file", _wrap("read_file", read_file))
    registry.register("create_doc", _wrap("create_doc", build_create_doc(policy)))
    registry.register("read_clipboard", _wrap("read_clipboard", read_clipboard))
    registry.register("list_dir", _wrap("list_dir", list_dir))
    registry.register("open_app", _wrap("open_app", open_app))
    registry.register("move_file", _wrap("move_file", move_file))
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

def split_messages(messages):
    """前端 AgentMessage[] → (prompt, message_history)。

    - 最后一条 user 消息作为本次 prompt（不进 history）
    - 其余按 role 转成 Pydantic AI 的 ModelRequest/ModelResponse
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
            history.append(ModelResponse(parts=[TextPart(content=content)]))
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
        mode = body.get("mode", "online")
        # 前端传的 desktop_path 可能为空，用 agent 自解析的真实桌面兜底
        desktop = body.get("desktop_path") or resolve_desktop_path()

        # 模型：online=DeepSeek，offline=Ollama（都走 OpenAI 兼容协议）
        if mode == "offline":
            model_name = body.get("model") or "qwen2.5:7b"
            model = OllamaModel(
                model_name,
                provider=OllamaProvider(base_url=OLLAMA_BASE, api_key="not-needed"),
            )
        else:
            # 生产路径：Rust 启动时通过环境变量 LOCALMIND_DEEPSEEK_KEY 注入 key，避免 key 进 HTTP 请求体。
            # 请求体 token 仅作独立测试（直接 python agent_server.py 跑）的回退。
            token = os.environ.get("LOCALMIND_DEEPSEEK_KEY") or body.get("token", "")
            model = OpenAIChatModel(
                "deepseek-chat",
                provider=OpenAIProvider(base_url=DEEPSEEK_BASE, api_key=token),
            )

        selected_attachment_paths = body.get("selected_attachment_paths") or []
        policy = ToolPolicy(selected_attachment_paths=selected_attachment_paths)
        tools = build_tools(self._emit_thinking, policy)
        prompt, history = split_messages(body.get("messages", []))
        agent = Agent(
            model,
            system_prompt=build_system_prompt(desktop),
            tools=tools.callables(),
            retries=1,  # 工具失败自动重试 1 次（Pydantic AI 内建）
        )

        self._emit_thinking("planning", "正在分析任务...", "running")

        try:
            content_parts = []
            tool_calls = {}  # tool_call_id → {name, args}
            async with agent.run_stream_events(prompt, message_history=history) as events:
                async for event in events:
                    if isinstance(event, PartDeltaEvent):
                        d = event.delta
                        if isinstance(d, TextPartDelta) and d.content_delta:
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
                        success = outcome != "failed"
                        output = getattr(part, "content", "")
                        if isinstance(output, (list, tuple)):
                            output = " ".join(str(x) for x in output)
                        output = str(output or "")
                        self._write_event({
                            "type": "tool",
                            "log": {
                                "name": call.get("name", tool_name),
                                "args": call.get("args", "{}"),
                                "output": output[:4000],
                                "success": success,
                            },
                        })
                        if success:
                            self._emit_thinking("executing", f"{tool_name} 完成", "success")
                        else:
                            self._emit_thinking("executing", f"{tool_name} 失败", "error")
                    elif isinstance(event, FinalResultEvent):
                        pass  # 不作为结束标记（以流结束为准）

            result = getattr(events, "result", None)
            content = ""
            if result is not None:
                content = getattr(result, "output", None)
                if content is None:
                    content = getattr(result, "data", "")
            content = str(content or "")
            if not content:
                content = "".join(content_parts)

            # 文本工具调用兜底：结构化 tool_calls 未发生，但模型在文本里输出了工具调用 JSON。
            # 小模型（如 qwen 7b）通病，对齐原 TS parseToolCallsFromText 的兜底行为。
            if not tool_calls:
                fallback = parse_tool_call_from_text(content)
                if fallback and tools.contains(fallback["name"]):
                    name, args = fallback["name"], fallback["args"]
                    self._emit_thinking("executing", f"正在执行：{name}（本地模型文本指令）", "running")
                    try:
                        output = tools.get(name)(**args)
                        output = str(output or "")
                        success = self._fallback_success(name, output)
                        self._write_event({
                            "type": "tool",
                            "log": {"name": name, "args": json.dumps(args, ensure_ascii=False), "output": output[:4000], "success": success},
                        })
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

            # 清理最终内容里残留的工具调用文本
            content = strip_tool_call_text(content)
            self._write_event({"type": "done", "content": content})
        except (BrokenPipeError, ConnectionResetError, OSError):
            raise  # 交给上层静默处理
        except Exception as e:
            try:
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
