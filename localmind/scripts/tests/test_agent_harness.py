import json
from pathlib import Path

from agent_server import build_tools
from tool_policy import ToolPolicy


def _registry(tmp_path: Path):
    policy = ToolPolicy(allowed_roots=[tmp_path])
    return build_tools(lambda *args, **kwargs: None, policy, variant="v2")


def test_v2_duplicate_call_is_structured(tmp_path: Path):
    tools = _registry(tmp_path)
    write_file = tools.get("write_file")
    first = write_file(path=str(tmp_path / "a.txt"), content="a")
    second = write_file(path=str(tmp_path / "a.txt"), content="a")
    assert "已写入文件" in first
    payload = json.loads(second)
    assert payload["ok"] is False
    assert payload["error"]["code"] == "DUPLICATE_TOOL_CALL"


def test_v2_missing_file_is_file_not_found(tmp_path: Path):
    tools = _registry(tmp_path)
    payload = json.loads(tools.get("read_file")(path=str(tmp_path / "missing.txt")))
    assert payload["ok"] is False
    assert payload["error"]["code"] == "FILE_NOT_FOUND"


def test_v2_policy_denied_is_structured(tmp_path: Path):
    allowed = tmp_path / "allowed"
    allowed.mkdir()
    policy = ToolPolicy(allowed_roots=[allowed])
    tools = build_tools(lambda *args, **kwargs: None, policy, variant="v2")
    payload = json.loads(tools.get("write_file")(path=str(tmp_path / "outside.txt"), content="x"))
    assert payload["ok"] is False
    assert payload["error"]["code"] == "POLICY_DENIED"
