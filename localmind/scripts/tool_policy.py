"""LocalMind 最小 Tool Guard（Phase 0）。

只解决路径边界、受控打开和最小拒绝清单；不实现完整 Permission Gateway、
用户确认 UI、Undo 或远程权限系统。
"""
from __future__ import annotations

import os
import re
from pathlib import Path
from typing import Iterable
from urllib.parse import urlparse

from tool_registry import TOOL_SPECS


class ToolPolicyError(ValueError):
    """工具调用被策略拒绝。"""


def _normalized_text(path: str | os.PathLike[str]) -> str:
    return os.path.expandvars(os.path.expanduser(str(path)))


def _as_path_set(paths: Iterable[str | os.PathLike[str]] | None) -> set[Path]:
    result: set[Path] = set()
    if not paths:
        return result
    for item in paths:
        try:
            result.add(Path(_normalized_text(item)).resolve(strict=False))
        except OSError:
            continue
    return result


def default_allowed_roots(user_home: str | None = None) -> list[Path]:
    home = Path(user_home or os.environ.get("USERPROFILE") or Path.home())
    candidates = [
        home / "Desktop",
        home / "Documents",
        home / "Downloads",
        home / "OneDrive" / "Desktop",
        home / "OneDrive" / "Documents",
    ]
    return [path.resolve(strict=False) for path in candidates if path.exists()]


class ToolPolicy:
    def __init__(
        self,
        allowed_roots: Iterable[str | os.PathLike[str]] | None = None,
        selected_attachment_paths: Iterable[str | os.PathLike[str]] | None = None,
        *,
        user_home: str | None = None,
        windows_root: str | None = None,
    ) -> None:
        roots = list(allowed_roots) if allowed_roots is not None else default_allowed_roots(user_home)
        self.allowed_roots = [Path(_normalized_text(root)).resolve(strict=False) for root in roots]
        self.selected_attachment_paths = _as_path_set(selected_attachment_paths)
        self.windows_root = Path(
            windows_root or os.environ.get("WINDIR") or r"C:\Windows"
        ).resolve(strict=False)
        self.user_home = Path(user_home or os.environ.get("USERPROFILE") or Path.home()).resolve(strict=False)
        self.ollama_models_root = self._resolve_ollama_models_root()

    def _resolve_ollama_models_root(self) -> Path:
        return (self.user_home / ".ollama" / "models").resolve(strict=False)

    @staticmethod
    def _reject_unsafe_text(raw: str) -> None:
        if not raw or "\x00" in raw:
            raise ToolPolicyError("路径为空或包含非法字符")
        normalized = raw.replace("/", "\\")
        if normalized.startswith("\\\\"):
            raise ToolPolicyError("不允许 UNC 或 device namespace 路径")
        lower = normalized.lower()
        if lower == ".." or lower.startswith("..\\") or "\\..\\" in lower or lower.endswith("\\.."):
            raise ToolPolicyError("路径不得包含 ..")
        if re.match(r"^[a-z]:\\windows(?:\\|$)", lower):
            raise ToolPolicyError("不允许访问 C:\\Windows")
        if re.match(r"^[a-z]:\\program files(?: \(x86\))?(?:\\|$)", lower):
            raise ToolPolicyError("不允许访问 Program Files")

    @classmethod
    def canonicalize_path(
        cls,
        path: str | os.PathLike[str],
        *,
        must_exist: bool = False,
    ) -> Path:
        raw = _normalized_text(path)
        cls._reject_unsafe_text(raw)
        candidate = Path(raw)
        try:
            resolved = candidate.resolve(strict=must_exist)
        except FileNotFoundError as exc:
            raise FileNotFoundError(f"路径不存在: {candidate}") from exc
        except (OSError, RuntimeError) as exc:
            raise ToolPolicyError(f"路径无效: {path}") from exc
        if must_exist and not resolved.exists():
            raise FileNotFoundError(f"路径不存在: {resolved}")
        return resolved

    @staticmethod
    def _is_relative_to(path: Path, root: Path) -> bool:
        try:
            path.relative_to(root)
            return True
        except ValueError:
            return False

    def is_within_allowed_roots(self, path: str | os.PathLike[str]) -> bool:
        resolved = self.canonicalize_path(path)
        return any(self._is_relative_to(resolved, root) for root in self.allowed_roots)

    def validate_read(self, path: str | os.PathLike[str]) -> Path:
        resolved = self.canonicalize_path(path, must_exist=True)
        if self.is_within_allowed_roots(resolved):
            return resolved
        if any(self._is_relative_to(resolved, selected) for selected in self.selected_attachment_paths):
            return resolved
        raise ToolPolicyError(f"读取路径不在允许范围内: {resolved}")

    def validate_write(self, path: str | os.PathLike[str]) -> Path:
        resolved = self.canonicalize_path(path)
        if not self.is_within_allowed_roots(resolved):
            raise ToolPolicyError(f"写入路径不在允许范围内: {resolved}")
        return resolved

    def validate_delete(self, path: str | os.PathLike[str]) -> Path:
        resolved = self.canonicalize_path(path)
        if not self.is_within_allowed_roots(resolved):
            raise ToolPolicyError(f"删除路径不在允许范围内: {resolved}")
        return resolved
    def validate_move(self, source: str | os.PathLike[str], destination: str | os.PathLike[str]) -> tuple[Path, Path]:
        src = self.validate_write(source)
        dst = self.validate_write(destination)
        return src, dst

    def validate_create_doc(self, spec: dict) -> Path:
        path = spec.get("path")
        if not isinstance(path, str) or not path.strip():
            raise ToolPolicyError("create_doc 的 spec.path 必须是允许范围内的完整路径")
        return self.validate_write(path)

    def _app_whitelist(self) -> dict[str, Path]:
        return {
            "notepad": self.windows_root / "System32" / "notepad.exe",
            "notepad.exe": self.windows_root / "System32" / "notepad.exe",
            "calc": self.windows_root / "System32" / "calc.exe",
            "calc.exe": self.windows_root / "System32" / "calc.exe",
            "mspaint": self.windows_root / "System32" / "mspaint.exe",
            "mspaint.exe": self.windows_root / "System32" / "mspaint.exe",
            "explorer": self.windows_root / "explorer.exe",
            "explorer.exe": self.windows_root / "explorer.exe",
        }

    def validate_open_target(self, target: str) -> tuple[str, str]:
        target = str(target or "").strip()
        if not target:
            raise ToolPolicyError("打开目标为空")

        parsed = urlparse(target)
        if parsed.scheme.lower() in {"http", "https"}:
            if not parsed.netloc:
                raise ToolPolicyError("open_app 只允许完整 http/https URL")
            return "url", target

        app = self._app_whitelist().get(target.lower())
        if app is not None:
            if not app.exists():
                raise ToolPolicyError(f"白名单应用不存在: {app}")
            return "app", str(app)

        path = self.canonicalize_path(target, must_exist=True)
        if path.is_file() and path.suffix.lower() in {
            ".exe", ".com", ".bat", ".cmd", ".ps1", ".vbs", ".js", ".jse",
            ".wsf", ".wsh", ".msi", ".lnk", ".reg",
        }:
            raise ToolPolicyError("不允许通过 open_app 启动任意可执行文件")
        in_models_root = self._is_relative_to(path, self.ollama_models_root)
        in_allowed_root = any(self._is_relative_to(path, root) for root in self.allowed_roots)
        if not (in_models_root or in_allowed_root):
            raise ToolPolicyError(f"打开路径不在允许范围内: {path}")
        return "path", str(path)

    @staticmethod
    def spec_for(name: str):
        spec = TOOL_SPECS.get(name)
        if spec is None:
            raise ToolPolicyError(f"工具未注册: {name}")
        return spec
