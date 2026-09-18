#!/usr/bin/env python3
"""Generate an Excel (.xlsx) file from a JSON spec.

Usage: python make_xlsx.py <spec.json>
Spec format:
{
  "path": "D:/out.xlsx",
  "sheet_name": "数据表",
  "headers": ["列1", "列2", "列3"],
  "rows": [["a", "b", "c"], ["d", "e", "f"]]
}
"""
import json
import sys
import os

from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill


def _column_letter(index: int) -> str:
    """1 -> A, 26 -> Z, 27 -> AA。

    旧实现用 chr(64 + col)，只对 A–Z 有效：超过 26 列时全部写到 A 列。
    """
    letters = ""
    while index > 0:
        index, rem = divmod(index - 1, 26)
        letters = chr(65 + rem) + letters
    return letters


def build(spec):
    path = spec["path"]
    wb = Workbook()
    ws = wb.active
    ws.title = spec.get("sheet_name", "Sheet1")

    headers = spec.get("headers", [])
    rows = spec.get("rows", [])

    # Write headers
    header_font = Font(bold=True, color="FFFFFF")
    header_fill = PatternFill(start_color="1F4E79", end_color="1F4E79", fill_type="solid")
    for col, h in enumerate(headers, start=1):
        cell = ws.cell(row=1, column=col, value=h)
        cell.font = header_font
        cell.fill = header_fill

    # Write data rows
    for r, row in enumerate(rows, start=2):
        for c, val in enumerate(row, start=1):
            ws.cell(row=r, column=c, value=val)

    # Auto-width (rough)：列字母要支持 AA/AB…，且要覆盖比表头更宽的列
    column_count = max([len(headers)] + [len(row) for row in rows] + [0])
    for col in range(1, column_count + 1):
        max_len = 0
        if col <= len(headers):
            max_len = len(str(headers[col - 1] or ""))
        for r in range(2, len(rows) + 2):
            v = ws.cell(row=r, column=col).value
            if v is not None:
                max_len = max(max_len, len(str(v)))
        ws.column_dimensions[_column_letter(col)].width = min(max_len + 4, 60)

    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    wb.save(path)
    return {"success": True, "path": path, "rows": len(rows)}


if __name__ == "__main__":
    spec_path = sys.argv[1]
    with open(spec_path, "r", encoding="utf-8") as f:
        spec = json.load(f)
    result = build(spec)
    print(json.dumps(result, ensure_ascii=False))
