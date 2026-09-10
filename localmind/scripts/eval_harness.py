"""LocalMind Phase 1 Harness 固定评测器。

在隔离目录中启动本地 agent_server，按 eval_tasks.json 逐任务请求 SSE，并汇总
成功率、工具调用、重复调用、越权拒绝、token、首 token 延迟和总耗时。

用法示例：
    python localmind/scripts/eval_harness.py --variant both --model deepseek-chat
    python localmind/scripts/eval_harness.py --variant v2 --task single_tool_write
"""
from __future__ import annotations

import argparse
import http.client
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_TASKS = SCRIPT_DIR / "eval_tasks.json"
AGENT_SERVER = SCRIPT_DIR / "agent_server.py"


class EvalError(RuntimeError):
    pass


class AgentServer:
    def __init__(self, trace_dir: Path, token: str, api_key: str | None = None, agent_exe: Path | None = None) -> None:
        self.trace_dir = trace_dir
        self.token = token
        self.port = 0
        self.proc: subprocess.Popen[str] | None = None
        self.stdout_lines: list[str] = []
        self.stderr_lines: list[str] = []
        self.api_key = api_key
        self.agent_exe = agent_exe

    def start(self, timeout_s: float = 30.0) -> None:
        env = os.environ.copy()
        env["LOCALMIND_AGENT_TOKEN"] = self.token
        env["LOCALMIND_EVAL_MODE"] = "1"
        env["LOCALMIND_TRACE_DIR"] = str(self.trace_dir)
        env["PYTHONUNBUFFERED"] = "1"
        if self.api_key:
            env["LOCALMIND_DEEPSEEK_KEY"] = self.api_key
        command = [str(self.agent_exe)] if self.agent_exe else [sys.executable, str(AGENT_SERVER)]
        cwd = self.agent_exe.parent if self.agent_exe else SCRIPT_DIR
        self.proc = subprocess.Popen(
            command,
            cwd=str(cwd),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        threading.Thread(target=self._drain_stdout, daemon=True).start()
        threading.Thread(target=self._drain_stderr, daemon=True).start()
        deadline = time.monotonic() + timeout_s
        while time.monotonic() < deadline:
            for line in list(self.stdout_lines):
                if line.startswith("PORT="):
                    self.port = int(line.split("=", 1)[1].strip())
                    return
            if self.proc.poll() is not None:
                raise EvalError(f"agent_server 启动失败: {self._stderr_text()}")
            time.sleep(0.05)
        raise EvalError(f"等待 agent_server 端口超时: {self._stderr_text()}")

    def _drain_stdout(self) -> None:
        assert self.proc and self.proc.stdout
        for line in self.proc.stdout:
            self.stdout_lines.append(line.rstrip("\r\n"))

    def _drain_stderr(self) -> None:
        assert self.proc and self.proc.stderr
        for line in self.proc.stderr:
            self.stderr_lines.append(line.rstrip("\r\n"))

    def _stderr_text(self) -> str:
        return "\n".join(self.stderr_lines[-20:])

    def stop(self) -> None:
        if not self.proc:
            return
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=5)

    def request_stream(
        self,
        body: dict[str, Any],
        *,
        timeout_s: float,
        cancel_after_first_delta: bool = False,
    ) -> dict[str, Any]:
        payload = json.dumps(body, ensure_ascii=False).encode("utf-8")
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=timeout_s)
        result: dict[str, Any] = {
            "events": [],
            "trace": None,
            "done": None,
            "error": None,
            "client_cancelled": False,
            "elapsed_ms": 0.0,
        }
        started = time.perf_counter()
        try:
            conn.request(
                "POST",
                "/agent/stream",
                body=payload,
                headers={
                    "Content-Type": "application/json",
                    "Content-Length": str(len(payload)),
                    "X-LocalMind-Token": self.token,
                },
            )
            response = conn.getresponse()
            if response.status != 200:
                detail = response.read().decode("utf-8", errors="replace")
                raise EvalError(f"HTTP {response.status}: {detail}")
            got_delta = False
            while True:
                raw = response.readline()
                if not raw:
                    break
                line = raw.decode("utf-8", errors="replace").strip()
                if not line.startswith("data:"):
                    continue
                try:
                    event = json.loads(line[5:].strip())
                except json.JSONDecodeError:
                    continue
                result["events"].append(event)
                event_type = event.get("type")
                if event_type == "trace":
                    result["trace"] = event.get("trace")
                elif event_type == "done":
                    result["done"] = event
                elif event_type == "error":
                    result["error"] = event
                elif event_type == "delta":
                    got_delta = True
                    if cancel_after_first_delta:
                        _reset_connection(conn)
                        result["client_cancelled"] = True
                        break
            if got_delta and cancel_after_first_delta:
                result["client_cancelled"] = True
        finally:
            result["elapsed_ms"] = round((time.perf_counter() - started) * 1000, 2)
            try:
                conn.close()
            except OSError:
                pass
        return result


def _reset_connection(conn: http.client.HTTPConnection) -> None:
    """用 SO_LINGER=0 发送 TCP RST，确保服务端感知客户端取消而不是只收 FIN。"""
    sock = getattr(conn, "sock", None)
    if sock is not None:
        try:
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        except OSError:
            pass
    try:
        conn.close()
    except OSError:
        pass


def load_tasks(path: Path) -> list[dict[str, Any]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    tasks = data.get("tasks")
    if not isinstance(tasks, list) or not tasks:
        raise EvalError(f"评测集为空或格式错误: {path}")
    return tasks


def _json_output(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True)


def _is_declared_failure(output: str) -> bool:
    try:
        parsed = json.loads(output)
        if isinstance(parsed, dict) and parsed.get("ok") is False:
            return True
    except Exception:
        pass
    return output.startswith(("读取剪贴板失败", "打开失败", "移动失败", "生成失败", "生成文档失败"))


def _trace_path(trace_dir: Path, turn_id: str) -> Path:
    safe = "".join(ch if ch.isalnum() or ch in "-_." else "_" for ch in turn_id)[:120] or "turn"
    return trace_dir / f"{safe}.json"


def _wait_trace(trace_dir: Path, turn_id: str, timeout_s: float = 4.0) -> dict[str, Any] | None:
    target = _trace_path(trace_dir, turn_id)
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if target.exists():
            try:
                return json.loads(target.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                pass
        time.sleep(0.1)
    return None


def _metric(observation: dict[str, Any]) -> dict[str, Any]:
    trace = observation.get("trace") or {}
    events = trace.get("events") if isinstance(trace, dict) else []
    if not isinstance(events, list):
        events = []
    tool_logs = trace.get("tool_logs") if isinstance(trace, dict) else None
    if not isinstance(tool_logs, list):
        tool_logs = [
            event.get("log", {})
            for event in observation.get("events", [])
            if event.get("type") == "tool"
        ]
    usage = trace.get("usage") if isinstance(trace, dict) else None
    if not isinstance(usage, dict):
        usage = {}
    first_token_ms = None
    for event in events:
        if event.get("event") == "model_response_received":
            first_token_ms = event.get("elapsed_ms")
            break
    if first_token_ms is None:
        for event in observation.get("events", []):
            if event.get("type") == "delta":
                first_token_ms = observation.get("elapsed_ms")
                break
    tool_names = [str(item.get("name", "")) for item in tool_logs if isinstance(item, dict)]
    duplicate = any(event.get("event") == "duplicate_tool_call" for event in events)
    denied = any(
        event.get("event") == "tool_call_finished" and event.get("data", {}).get("error_code") == "POLICY_DENIED"
        for event in events
    )
    error_count = 0
    for item in tool_logs:
        output = str(item.get("output", "")) if isinstance(item, dict) else ""
        if _is_declared_failure(output) or "POLICY_DENIED" in output or "DUPLICATE_TOOL_CALL" in output:
            error_count += 1
            if "POLICY_DENIED" in output:
                denied = True
    if observation.get("error"):
        error_count += 1
    trace_status = trace.get("status") if isinstance(trace, dict) else None
    if trace_status == "cancelled":
        error_count += 0
    return {
        "status": trace_status or ("completed" if observation.get("done") else "failed"),
        "tool_names": tool_names,
        "tool_calls": len(tool_names),
        "duplicate_blocked": duplicate,
        "policy_denied": denied,
        "error_count": error_count,
        "input_tokens": int(usage.get("input_tokens") or 0),
        "output_tokens": int(usage.get("output_tokens") or 0),
        "requests": int(usage.get("requests") or sum(1 for e in events if e.get("event") == "model_request_started")),
        "first_token_ms": first_token_ms,
        "duration_ms": trace.get("duration_ms") if isinstance(trace, dict) else observation.get("elapsed_ms"),
        "final_content": str((observation.get("done") or {}).get("content", "")),
    }


def _check_task(task: dict[str, Any], metrics: dict[str, Any], root: Path) -> tuple[bool, list[dict[str, Any]]]:
    checks = task.get("checks") or {}
    final_content = metrics.get("final_content", "")
    tool_names = metrics.get("tool_names", [])
    results: list[dict[str, Any]] = []

    def add(name: str, passed: bool, detail: str = "") -> None:
        results.append({"name": name, "passed": bool(passed), "detail": detail})

    expected_any = checks.get("require_final_contains") or []
    if expected_any:
        hit = [text for text in expected_any if str(text) in final_content]
        add("final_contains", bool(hit), f"hit={hit!r}")

    forbidden = checks.get("forbid_final_contains") or []
    if forbidden:
        hit = [text for text in forbidden if str(text) in final_content]
        add("final_forbidden_text", not hit, f"hit={hit!r}")

    required_tools = checks.get("require_tools") or []
    if required_tools:
        missing = [name for name in required_tools if name not in tool_names]
        add("required_tools", not missing, f"missing={missing!r}")

    if checks.get("no_tool_calls") is True:
        add("no_tool_calls", metrics.get("tool_calls", 0) == 0, f"calls={metrics.get('tool_calls', 0)}")

    for file_check in checks.get("require_files") or []:
        rel = str(file_check.get("path", ""))
        target = root / rel
        exists = target.exists() and target.is_file()
        detail = f"path={target}"
        passed = exists
        if exists and file_check.get("contains") is not None:
            text = target.read_text(encoding="utf-8", errors="replace")
            expected = str(file_check["contains"])
            passed = expected in text
            detail += f", contains={expected!r}"
        if exists and file_check.get("minimum_size") is not None:
            passed = target.stat().st_size >= int(file_check["minimum_size"]) and passed
            detail += f", size={target.stat().st_size}"
        add(f"file:{rel}", passed, detail)

    if checks.get("require_policy_denied") is True:
        add("policy_denied", bool(metrics.get("policy_denied")), "expected POLICY_DENIED trace")
    if checks.get("require_duplicate_blocked") is True:
        add("duplicate_blocked", bool(metrics.get("duplicate_blocked")), "expected duplicate_tool_call")

    max_tool_calls = checks.get("max_tool_calls")
    if max_tool_calls is not None:
        add("max_tool_calls", metrics.get("tool_calls", 0) <= int(max_tool_calls), f"calls={metrics.get('tool_calls')}")
    max_requests = checks.get("max_requests")
    if max_requests is not None:
        add("max_requests", metrics.get("requests", 0) <= int(max_requests), f"requests={metrics.get('requests')}")
    max_input_tokens = checks.get("max_input_tokens")
    if max_input_tokens is not None:
        add("max_input_tokens", metrics.get("input_tokens", 0) <= int(max_input_tokens), f"input_tokens={metrics.get('input_tokens')}")
    max_duration_ms = checks.get("max_duration_ms")
    if max_duration_ms is not None:
        add("max_duration_ms", (metrics.get("duration_ms") or 0) <= int(max_duration_ms), f"duration_ms={metrics.get('duration_ms')}")

    required_status = checks.get("require_trace_status")
    if required_status:
        add("trace_status", metrics.get("status") == required_status, f"status={metrics.get('status')}")

    return all(item["passed"] for item in results), results


def run_task(
    *,
    server: AgentServer,
    task: dict[str, Any],
    variant: str,
    model: str,
    mode: str,
    trace_dir: Path,
    work_root: Path,
    request_timeout_s: float,
    run_id: str,
) -> dict[str, Any]:
    task_id = str(task["id"])
    root = work_root / variant / task_id
    root.mkdir(parents=True, exist_ok=True)
    for rel, content in (task.get("setup_files") or {}).items():
        target = root / str(rel)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(str(content), encoding="utf-8")

    steps = task.get("steps") or [task.get("prompt", "")]
    seed_history = task.get("seed_history") or []
    messages: list[dict[str, str]] = list(seed_history)
    observations: list[dict[str, Any]] = []
    traces: list[dict[str, Any]] = []
    for index, prompt_template in enumerate(steps, start=1):
        prompt = str(prompt_template).format(root=root)
        messages.append({"role": "user", "content": prompt})
        turn_id = f"eval-{run_id}-{variant}-{task_id}-{index}"
        body = {
            "mode": mode,
            "model": model,
            "messages": messages,
            "desktop_path": str(root),
            "allowed_roots": [str(root)],
            "selected_attachment_paths": [],
            "turn_id": turn_id,
            "harness_variant": variant,
        }
        observation = server.request_stream(
            body,
            timeout_s=request_timeout_s,
            cancel_after_first_delta=bool(task.get("cancel_after_first_delta")),
        )
        trace = observation.get("trace")
        if not isinstance(trace, dict):
            trace = _wait_trace(trace_dir, turn_id)
            observation["trace"] = trace
        if trace:
            traces.append(trace)
        observation["turn_id"] = turn_id
        observations.append(observation)
        assistant_content = str((observation.get("done") or {}).get("content", ""))
        if assistant_content:
            messages.append({"role": "assistant", "content": assistant_content})
        if task.get("cancel_after_first_delta"):
            break

    per_step = [_metric(item) for item in observations]
    tool_logs: list[dict[str, Any]] = []
    trace_events: list[dict[str, Any]] = []
    for trace in traces:
        if isinstance(trace.get("tool_logs"), list):
            tool_logs.extend(trace["tool_logs"])
        if isinstance(trace.get("events"), list):
            trace_events.extend(trace["events"])
    aggregate = {
        "status": per_step[-1]["status"] if per_step else "failed",
        "tool_names": [str(item.get("name", "")) for item in tool_logs if isinstance(item, dict)],
        "tool_calls": len(tool_logs),
        "duplicate_blocked": any(item.get("duplicate_blocked") for item in per_step),
        "policy_denied": any(item.get("policy_denied") for item in per_step),
        "error_count": sum(int(item.get("error_count") or 0) for item in per_step),
        "input_tokens": sum(int(item.get("input_tokens") or 0) for item in per_step),
        "output_tokens": sum(int(item.get("output_tokens") or 0) for item in per_step),
        "requests": sum(int(item.get("requests") or 0) for item in per_step),
        "first_token_ms": next((item.get("first_token_ms") for item in per_step if item.get("first_token_ms") is not None), None),
        "duration_ms": round(sum(float(item.get("duration_ms") or 0) for item in per_step), 2),
        "final_content": per_step[-1]["final_content"] if per_step else "",
    }
    passed, checks = _check_task(task, aggregate, root)
    return {
        "id": task_id,
        "category": task.get("category", ""),
        "variant": variant,
        "success": passed,
        "root": str(root),
        "metrics": aggregate,
        "checks": checks,
        "steps": [
            {
                "turn_id": item.get("turn_id"),
                "status": metric.get("status"),
                "client_cancelled": item.get("client_cancelled"),
                "error": item.get("error"),
                "trace": item.get("trace"),
                "events": item.get("events"),
            }
            for item, metric in zip(observations, per_step)
        ],
    }


def summarize(runs: list[dict[str, Any]]) -> dict[str, Any]:
    total = len(runs)
    successes = sum(1 for run in runs if run.get("success"))
    durations = [float(run["metrics"].get("duration_ms") or 0) for run in runs]
    first_tokens = [float(run["metrics"]["first_token_ms"]) for run in runs if run["metrics"].get("first_token_ms") is not None]
    input_tokens = [int(run["metrics"].get("input_tokens") or 0) for run in runs]
    output_tokens = [int(run["metrics"].get("output_tokens") or 0) for run in runs]
    tool_calls = [int(run["metrics"].get("tool_calls") or 0) for run in runs]
    return {
        "tasks": total,
        "successes": successes,
        "success_rate": round(successes / total, 4) if total else 0.0,
        "avg_duration_ms": round(sum(durations) / total, 2) if total else 0.0,
        "avg_first_token_ms": round(sum(first_tokens) / len(first_tokens), 2) if first_tokens else None,
        "total_input_tokens": sum(input_tokens),
        "total_output_tokens": sum(output_tokens),
        "avg_input_tokens": round(sum(input_tokens) / total, 2) if total else 0.0,
        "total_tool_calls": sum(tool_calls),
        "duplicates_blocked": sum(1 for run in runs if run["metrics"].get("duplicate_blocked")),
        "policy_denials": sum(1 for run in runs if run["metrics"].get("policy_denied")),
        "failed_tasks": [run["id"] for run in runs if not run.get("success")],
    }


def load_api_key(explicit: str | None) -> str | None:
    if explicit:
        return explicit
    return os.environ.get("LOCALMIND_DEEPSEEK_KEY") or None


def git_revision() -> str:
    try:
        return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=str(SCRIPT_DIR.parent.parent), text=True).strip()
    except Exception:
        return ""


def main() -> int:
    parser = argparse.ArgumentParser(description="LocalMind Phase 1 Harness A/B evaluator")
    parser.add_argument("--tasks", type=Path, default=DEFAULT_TASKS)
    parser.add_argument("--variant", choices=["baseline", "v2", "both"], default="both")
    parser.add_argument("--model", default="deepseek-chat")
    parser.add_argument("--mode", choices=["online", "offline"], default="online")
    parser.add_argument("--task", action="append", default=[], help="只运行指定 task id，可重复")
    parser.add_argument("--max-tasks", type=int, default=0)
    parser.add_argument("--request-timeout", type=float, default=180.0)
    parser.add_argument("--api-key", default=None, help="仅测试时覆盖；默认读取 LOCALMIND_DEEPSEEK_KEY")
    parser.add_argument("--agent-exe", type=Path, default=None, help="使用已打包的 localmind-agent.exe 进行发布验证")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--keep-server", action="store_true")
    args = parser.parse_args()

    tasks = load_tasks(args.tasks)
    if args.task:
        wanted = set(args.task)
        tasks = [task for task in tasks if task.get("id") in wanted]
    if args.max_tasks > 0:
        tasks = tasks[: args.max_tasks]
    if not tasks:
        raise EvalError("没有匹配的评测任务")

    api_key = load_api_key(args.api_key) if args.mode == "online" else None
    if args.mode == "online" and not api_key:
        raise EvalError("在线评测需要 LOCALMIND_DEEPSEEK_KEY 或 --api-key")

    timestamp = time.strftime("%Y%m%d-%H%M%S")
    run_id = uuid.uuid4().hex[:8]
    temp_root = Path(tempfile.mkdtemp(prefix="localmind-eval-"))
    trace_dir = temp_root / "traces"
    trace_dir.mkdir(parents=True, exist_ok=True)
    server = AgentServer(trace_dir=trace_dir, token=uuid.uuid4().hex, api_key=api_key, agent_exe=args.agent_exe)
    variants = ["baseline", "v2"] if args.variant == "both" else [args.variant]
    report: dict[str, Any] = {
        "schema": 1,
        "created_at": timestamp,
        "model": args.model,
        "mode": args.mode,
        "tasks_file": str(args.tasks.resolve()),
        "git_revision": git_revision(),
        "run_id": run_id,
        "variants": {},
    }
    try:
        server.start()
        for variant in variants:
            runs = []
            for task in tasks:
                print(f"[{variant}] {task['id']} ...", flush=True)
                run = run_task(
                    server=server,
                    task=task,
                    variant=variant,
                    model=args.model,
                    mode=args.mode,
                    trace_dir=trace_dir,
                    work_root=temp_root / "work",
                    request_timeout_s=args.request_timeout,
                    run_id=run_id,
                )
                runs.append(run)
                status = "PASS" if run["success"] else "FAIL"
                print(
                    f"[{variant}] {task['id']}: {status} "
                    f"tools={run['metrics']['tool_calls']} duration={run['metrics']['duration_ms']}ms "
                    f"tokens={run['metrics']['input_tokens']}/{run['metrics']['output_tokens']}",
                    flush=True,
                )
            report["variants"][variant] = {"summary": summarize(runs), "runs": runs}
    finally:
        if args.keep_server:
            print(f"agent_server pid={server.proc.pid if server.proc else '-'} port={server.port}", flush=True)
        else:
            server.stop()

    if len(variants) == 2:
        baseline = report["variants"]["baseline"]["summary"]
        v2 = report["variants"]["v2"]["summary"]
        report["comparison"] = {
            "success_rate_delta": round(v2["success_rate"] - baseline["success_rate"], 4),
            "avg_duration_ms_delta": round(v2["avg_duration_ms"] - baseline["avg_duration_ms"], 2),
            "avg_first_token_ms_delta": (
                round(v2["avg_first_token_ms"] - baseline["avg_first_token_ms"], 2)
                if v2.get("avg_first_token_ms") is not None and baseline.get("avg_first_token_ms") is not None
                else None
            ),
            "avg_input_tokens_delta": round(v2["avg_input_tokens"] - baseline["avg_input_tokens"], 2),
            "total_tool_calls_delta": v2["total_tool_calls"] - baseline["total_tool_calls"],
            "duplicates_blocked_delta": v2["duplicates_blocked"] - baseline["duplicates_blocked"],
        }

    output = args.output or (SCRIPT_DIR / "eval_reports" / f"harness-{timestamp}-{args.model.replace('/', '_')}.json")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print("\n=== Summary ===")
    for variant in variants:
        summary = report["variants"][variant]["summary"]
        print(
            f"{variant:8s} success={summary['successes']}/{summary['tasks']} "
            f"avg_duration={summary['avg_duration_ms']}ms "
            f"first_token={summary['avg_first_token_ms']}ms "
            f"tokens={summary['total_input_tokens']}/{summary['total_output_tokens']} "
            f"tool_calls={summary['total_tool_calls']} "
            f"duplicates={summary['duplicates_blocked']} denied={summary['policy_denials']}"
        )
    if report.get("comparison"):
        print(f"comparison: {_json_output(report['comparison'])}")
    print(f"report: {output}")
    print(f"artifacts: {temp_root}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except EvalError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        raise SystemExit(2)
