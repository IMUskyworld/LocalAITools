# -*- coding: utf-8 -*-
"""create_doc 的 spec 规整回归测试。

背景（2026-09-18）：模型把 sections[].body 传成**字符串**而不是数组，
`for body in section["body"]` 于是逐字符迭代，生成的 PDF 变成"每行一个字"，
一份简介被撑成 45 页。这里锁死"字符串不会被拆开"这条不变量。
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import make_doc_cli as cli


def test_string_body_is_not_split_into_characters():
    assert cli._as_text_list("内蒙古大学简介") == ["内蒙古大学简介"]


def test_char_split_body_is_rejoined_with_spaces():
    chars = list("Inner Mongolia University")
    assert cli._as_text_list(chars) == ["Inner Mongolia University"]


def test_normal_list_is_preserved():
    assert cli._as_text_list(["第一段", "第二段"]) == ["第一段", "第二段"]


def test_short_single_char_items_are_not_joined():
    assert cli._as_text_list(["上", "中", "下"]) == ["上", "中", "下"]


def test_scalars_and_none():
    assert cli._as_text_list(None) == []
    assert cli._as_text_list(123) == ["123"]
    assert cli._as_list("abc") == ["abc"]
    assert cli._as_list({"heading": "x"}) == [{"heading": "x"}]
    assert cli._as_str(None, "默认") == "默认"
    assert cli._as_str(5) == "5"


def test_pdf_builder_accepts_string_body(tmp_path):
    out = tmp_path / "out.pdf"
    result = cli.build_pdf({
        "path": str(out),
        "title": "内蒙古大学简介",
        "sections": [{"heading": "一、学校概况", "body": "内蒙古大学位于呼和浩特市。"}],
    })
    assert result["success"] is True
    assert out.exists() and out.stat().st_size > 0


def test_column_letter_supports_beyond_z():
    assert cli._column_letter(1) == "A"
    assert cli._column_letter(26) == "Z"
    assert cli._column_letter(27) == "AA"
    assert cli._column_letter(28) == "AB"
    assert cli._column_letter(52) == "AZ"
    assert cli._column_letter(53) == "BA"


def test_xlsx_sets_width_on_correct_columns_beyond_26(tmp_path):
    """>26 列时列宽必须写到真正的列（旧实现会全写到 A）。"""
    out = tmp_path / "wide.xlsx"
    headers = [f"字段{i}" for i in range(1, 31)]
    rows = [[f"值{i}" for i in range(1, 31)]]
    result = cli.build_xlsx({"path": str(out), "sheet_name": "wide", "headers": headers, "rows": rows})
    assert result["success"] is True
    from openpyxl import load_workbook
    ws = load_workbook(out).active
    assert ws.column_dimensions["AA"].width and ws.column_dimensions["AA"].width > 0
    assert ws.column_dimensions["AD"].width and ws.column_dimensions["AD"].width > 0
    assert ws.column_dimensions["A"].width != ws.column_dimensions["AA"].width
