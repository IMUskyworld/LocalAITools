"""LocalMind Agent Tool Registry（Phase 0 轻量版）。

只负责工具元数据与受控 callable 注册，不实现插件市场、动态加载或代码生成。
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable


@dataclass(frozen=True)
class ToolSpec:
    name: str
    description: str
    risk_level: str
    path_policy: str
    timeout_ms: int
    max_output_chars: int
    cacheable: bool
    enabled: bool = True


TOOL_SPECS: dict[str, ToolSpec] = {
    "write_file": ToolSpec(
        name="write_file",
        description="创建或覆盖写入文本文件",
        risk_level="L2",
        path_policy="allowed_roots_write",
        timeout_ms=10_000,
        max_output_chars=4_000,
        cacheable=False,
    ),
    "read_file": ToolSpec(
        name="read_file",
        description="读取允许目录或本轮已选择附件中的文本文件",
        risk_level="L0",
        path_policy="allowed_roots_or_selected_read",
        timeout_ms=10_000,
        max_output_chars=12_000,
        cacheable=True,
    ),
    "read_clipboard": ToolSpec(
        name="read_clipboard",
        description="读取 Windows 剪贴板文本",
        risk_level="L1",
        path_policy="none",
        timeout_ms=8_000,
        max_output_chars=12_000,
        cacheable=False,
    ),
    "list_dir": ToolSpec(
        name="list_dir",
        description="列出允许目录中的文件与子目录",
        risk_level="L0",
        path_policy="allowed_roots_read",
        timeout_ms=10_000,
        max_output_chars=12_000,
        cacheable=True,
    ),
    "open_app": ToolSpec(
        name="open_app",
        description="打开白名单应用、受控目录/文件或 http(s) URL",
        risk_level="L1",
        path_policy="open_whitelist",
        timeout_ms=8_000,
        max_output_chars=2_000,
        cacheable=False,
    ),
    "move_file": ToolSpec(
        name="move_file",
        description="在允许目录范围内移动或重命名文件/目录",
        risk_level="L2",
        path_policy="allowed_roots_move",
        timeout_ms=15_000,
        max_output_chars=4_000,
        cacheable=False,
    ),
    "run_command": ToolSpec(
        name="run_command",
        description="执行任意 shell 命令（PowerShell / cmd / 可执行文件）。用于安装软件、运行脚本、系统管理等。",
        risk_level="L4",
        path_policy="none",
        timeout_ms=30_000,
        max_output_chars=50_000,
        cacheable=False,
    ),
    "delete_path": ToolSpec(
        name="delete_path",
        description="删除文件或目录。用于清理临时文件、卸载等。",
        risk_level="L3",
        path_policy="allowed_roots_write",
        timeout_ms=15_000,
        max_output_chars=4_000,
        cacheable=False,
    ),
    "create_doc": ToolSpec(
        name="create_doc",
        description="在允许目录内生成 PPT/Word/Excel/PDF",
        risk_level="L2",
        path_policy="allowed_roots_write",
        timeout_ms=120_000,
        max_output_chars=4_000,
        cacheable=False,
    ),
}

ORDERED_TOOL_NAMES = tuple(TOOL_SPECS.keys())


class ToolRegistry:
    """只允许注册 TOOL_SPECS 中声明的工具。"""

    def __init__(self) -> None:
        self._callables: dict[str, Callable[..., Any]] = {}

    def register(self, name: str, callable_: Callable[..., Any]) -> None:
        spec = TOOL_SPECS.get(name)
        if spec is None:
            raise KeyError(f"未在 Tool Registry 声明的工具: {name}")
        if not spec.enabled:
            raise ValueError(f"工具已禁用: {name}")
        self._callables[name] = callable_

    def get(self, name: str) -> Callable[..., Any]:
        if name not in TOOL_SPECS or name not in self._callables:
            raise KeyError(f"工具不可用: {name}")
        return self._callables[name]

    def contains(self, name: str) -> bool:
        return name in TOOL_SPECS and name in self._callables

    def callables(self) -> list[Callable[..., Any]]:
        return [self._callables[name] for name in ORDERED_TOOL_NAMES if name in self._callables]

    def names(self) -> tuple[str, ...]:
        return tuple(name for name in ORDERED_TOOL_NAMES if name in self._callables)

    def specs(self) -> tuple[ToolSpec, ...]:
        return tuple(TOOL_SPECS[name] for name in self.names())
