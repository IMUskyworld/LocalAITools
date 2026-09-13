from pathlib import Path

import pytest

from tool_policy import ToolPolicy, ToolPolicyError
from tool_registry import TOOL_SPECS, ToolRegistry


def policy_for(tmp_path: Path, allowed: Path | None = None, selected: list[Path] | None = None) -> ToolPolicy:
    allowed_root = allowed or tmp_path
    return ToolPolicy(
        allowed_roots=[allowed_root],
        selected_attachment_paths=selected or [],
        user_home=str(tmp_path),
        windows_root=r"C:\Windows",
    )


def test_allowed_root_read_and_write(tmp_path: Path):
    policy = policy_for(tmp_path)
    source = tmp_path / "notes.txt"
    source.write_text("hello", encoding="utf-8")
    assert policy.validate_read(source) == source.resolve()
    target = tmp_path / "new.txt"
    assert policy.validate_write(target) == target.resolve()


def test_outside_root_write_is_rejected(tmp_path: Path):
    allowed = tmp_path / "allowed"
    allowed.mkdir()
    outside = tmp_path / "outside.txt"
    policy = policy_for(tmp_path, allowed=allowed)
    with pytest.raises(ToolPolicyError):
        policy.validate_write(outside)


def test_selected_attachment_is_read_only(tmp_path: Path):
    outside = tmp_path / "outside.txt"
    outside.write_text("selected", encoding="utf-8")
    policy = policy_for(tmp_path, allowed=tmp_path / "allowed", selected=[outside])
    assert policy.validate_read(outside) == outside.resolve()
    with pytest.raises(ToolPolicyError):
        policy.validate_write(outside)


def test_path_traversal_and_unc_are_rejected(tmp_path: Path):
    policy = policy_for(tmp_path)
    with pytest.raises(ToolPolicyError):
        policy.validate_write(tmp_path / "allowed" / ".." / ".." / "Windows" / "x.txt")
    with pytest.raises(ToolPolicyError):
        policy.validate_read(r"\\server\share\file.txt")
    with pytest.raises(ToolPolicyError):
        policy.validate_write("..\\Desktop\\escape.txt")


def test_open_url_and_executable_policy(tmp_path: Path):
    policy = policy_for(tmp_path)
    kind, _ = policy.validate_open_target("https://example.com")
    assert kind == "url"

    exe = tmp_path / "bad.exe"
    exe.write_bytes(b"not a real executable")
    with pytest.raises(ToolPolicyError):
        policy.validate_open_target(str(exe))

    notes = tmp_path / "notes.txt"
    notes.write_text("hello", encoding="utf-8")
    kind, value = policy.validate_open_target(str(notes))
    assert kind == "path"
    assert Path(value) == notes.resolve()

def test_ollama_models_open_but_not_write(tmp_path: Path):
    allowed = tmp_path / "allowed"
    allowed.mkdir()
    models = tmp_path / ".ollama" / "models"
    models.mkdir(parents=True)
    policy = policy_for(tmp_path, allowed=allowed)
    kind, value = policy.validate_open_target(str(models))
    assert kind == "path"
    assert Path(value) == models.resolve()
    with pytest.raises(ToolPolicyError):
        policy.validate_write(models / "new.bin")

def test_create_doc_requires_allowed_output_path(tmp_path: Path):
    policy = policy_for(tmp_path)
    with pytest.raises(ToolPolicyError):
        policy.validate_create_doc({"path": str(tmp_path.parent / "outside.pptx")})
    expected = tmp_path / "deck.pptx"
    assert policy.validate_create_doc({"path": str(expected)}) == expected.resolve()


def test_registry_rejects_unregistered_tool():
    """Registry 只允许注册 TOOL_SPECS 中声明过的工具（防止偷偷注册未声明能力）。"""
    registry = ToolRegistry()
    with pytest.raises(KeyError):
        registry.register("format_disk", lambda: None)


def test_high_risk_tools_require_confirmation():
    """run_command / delete_path 已从「永久禁止」改为「高危 + 强制用户确认」。

    这条断言跟随 ADR 变更：工具必须留在 TOOL_SPECS 中显式声明，
    且 confirmation_required=True（Python 侧 request_confirmation，前端确认卡片）。
    """
    for name in ("run_command", "delete_path"):
        spec = TOOL_SPECS[name]
        assert spec.confirmation_required is True, f"{name} 必须强制确认"
        assert spec.risk_level in {"L3", "L4"}, f"{name} 风险等级不应低于 L3"

def test_missing_file_is_file_not_found_not_policy_denied(tmp_path: Path):
    policy = policy_for(tmp_path)
    with pytest.raises(FileNotFoundError):
        policy.validate_read(tmp_path / "missing.txt")
