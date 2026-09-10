"""Phase 1 Trace 记录器。

每个 Turn 生成一份不含完整消息正文的结构化 Trace，默认持久化到：
    %APPDATA%\\LocalMind\\traces\\<turn_id>.json

同时可通过 emit 回调把完整 Trace 作为 SSE `trace` 事件发给前端或评测器。
"""
from __future__ import annotations

import json
import os
import tempfile
import time
from dataclasses import asdict, is_dataclass
from decimal import Decimal
from pathlib import Path
from typing import Any, Callable

TraceEmitter = Callable[[dict[str, Any]], None]


def _jsonable(value: Any) -> Any:
    if is_dataclass(value):
        return {field.name: _jsonable(getattr(value, field.name)) for field in value.__dataclass_fields__.values()}
    if isinstance(value, Decimal):
        return float(value)
    if isinstance(value, dict):
        return {str(key): _jsonable(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_jsonable(item) for item in value]
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    return str(value)


def _safe_file_part(value: str) -> str:
    cleaned = ''.join(ch if ch.isalnum() or ch in '-_.' else '_' for ch in value)
    return cleaned[:120] or 'turn'


class TraceRecorder:
    def __init__(self, turn_id: str, variant: str, emit: TraceEmitter | None = None) -> None:
        self.turn_id = turn_id or f"eval-{int(time.time() * 1000)}"
        self.variant = variant
        self.emit = emit
        self.started_perf = time.perf_counter()
        self.events: list[dict[str, Any]] = []
        self.first_token_recorded = False

    def record(self, event: str, **data: Any) -> dict[str, Any]:
        now = time.perf_counter()
        entry = {
            'event': event,
            'timestamp_ms': int(time.time() * 1000),
            'elapsed_ms': round((now - self.started_perf) * 1000, 2),
            'data': _jsonable(data),
        }
        self.events.append(entry)
        return entry

    def first_token(self) -> None:
        if not self.first_token_recorded:
            self.first_token_recorded = True
            self.record('model_response_received')

    def finish(
        self,
        status: str,
        *,
        content: str = '',
        usage: Any = None,
        tool_logs: list[dict[str, Any]] | None = None,
        error: str = '',
    ) -> dict[str, Any]:
        event_name = {
            'completed': 'turn_completed',
            'failed': 'turn_failed',
            'cancelled': 'turn_cancelled',
        }.get(status, f'turn_{status}')
        self.record(
            event_name,
            status=status,
            output_chars=len(content or ''),
            tool_calls=len(tool_logs or []),
            error=error[:500],
        )
        payload = {
            'turn_id': self.turn_id,
            'variant': self.variant,
            'status': status,
            'duration_ms': round((time.perf_counter() - self.started_perf) * 1000, 2),
            'events': self.events,
            'usage': _jsonable(usage) if usage is not None else None,
            'tool_logs': _jsonable(tool_logs or []),
        }
        self._persist(payload)
        if self.emit is not None:
            try:
                self.emit({'type': 'trace', 'trace': payload})
            except (BrokenPipeError, ConnectionResetError, OSError):
                pass
        return payload

    @staticmethod
    def _trace_dir() -> Path:
        override = os.environ.get('LOCALMIND_TRACE_DIR')
        if override:
            return Path(override)
        appdata = os.environ.get('APPDATA')
        if appdata:
            return Path(appdata) / 'LocalMind' / 'traces'
        return Path(tempfile.gettempdir()) / 'localmind-traces'

    def _persist(self, payload: dict[str, Any]) -> None:
        try:
            directory = self._trace_dir()
            directory.mkdir(parents=True, exist_ok=True)
            target = directory / f"{_safe_file_part(self.turn_id)}.json"
            temp = target.with_suffix('.json.tmp')
            temp.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding='utf-8')
            temp.replace(target)
        except OSError:
            pass
