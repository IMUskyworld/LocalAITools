#!/usr/bin/env python3
"""Generate a PDF file from a JSON spec.

Usage: python make_pdf.py <spec.json>
Spec format:
{
  "path": "D:/out.pdf",
  "title": "文档标题",
  "subtitle": "副标题",
  "sections": [
    {"heading": "章节标题", "body": ["段落1", "段落2"]},
    ...
  ]
}
"""
import json
import sys
import os

from fpdf import FPDF
import os


def _find_cjk_font():
    """查找支持中文的字体（Windows 系统字体）"""
    candidates = [
        r"C:\Windows\Fonts\msyh.ttc",    # 微软雅黑
        r"C:\Windows\Fonts\simhei.ttf",   # 黑体
        r"C:\Windows\Fonts\simsun.ttc",   # 宋体
        r"C:\Windows\Fonts\Deng.ttf",     # 等线
    ]
    for c in candidates:
        if os.path.exists(c):
            return c
    return ""


def build(spec):
    path = spec["path"]
    pdf = FPDF()
    pdf.add_page()

    # 使用中文字体（如果找到）
    font_path = _find_cjk_font()
    use_cjk = bool(font_path)
    if use_cjk:
        pdf.add_font("CJK", "", font_path)
    # fpdf 自定义字体不支持粗体/斜体变体，统一用常规样式，靠字号区分层级

    # Title
    title = spec.get("title", "")
    if title:
        pdf.set_font("CJK" if use_cjk else "helvetica", "", 22)
        pdf.cell(0, 16, title, ln=True, align="C")
        pdf.ln(4)

    # Subtitle
    subtitle = spec.get("subtitle", "")
    if subtitle:
        pdf.set_font("CJK" if use_cjk else "helvetica", "", 13)
        pdf.cell(0, 10, subtitle, ln=True, align="C")
        pdf.ln(8)

    # Sections
    for section in spec.get("sections", []):
        heading = section.get("heading", "")
        if heading:
            pdf.set_font("CJK" if use_cjk else "helvetica", "", 15)
            pdf.cell(0, 12, heading, ln=True)
            pdf.ln(2)
        pdf.set_font("CJK" if use_cjk else "helvetica", "", 11)
        for body in section.get("body", []):
            pdf.multi_cell(0, 7, body)
            pdf.ln(2)
        pdf.ln(4)

    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    pdf.output(path)
    return {"success": True, "path": path, "sections": len(spec.get("sections", []))}


if __name__ == "__main__":
    spec_path = sys.argv[1]
    with open(spec_path, "r", encoding="utf-8") as f:
        spec = json.load(f)
    result = build(spec)
    print(json.dumps(result, ensure_ascii=False))
