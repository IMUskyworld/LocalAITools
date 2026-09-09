package com.localmind.localfile.files

import android.content.Context
import java.io.File

/**
 * 从零生成 xlsx 文件（手写 OOXML zip，标准 sharedStrings 写法）。
 * 单元格字符串用 sharedStrings.xml 索引引用，数字直接存值。
 */
class XlsxGenerator(private val context: Context) {

    /**
     * 生成 xlsx 字节数组。
     * @param sheetName 工作表名
     * @param headers 表头
     * @param rows 数据行（每行一个字符串列表）
     */
    fun generateXlsx(
        sheetName: String = "数据",
        headers: List<String> = emptyList(),
        rows: List<List<String>> = emptyList()
    ): ByteArray {
        // 收集所有字符串单元格（去重）
        val allStrings = mutableListOf<String>()
        val stringIndex = mutableMapOf<String, Int>()
        fun getIndex(s: String): Int {
            return stringIndex.getOrPut(s) {
                allStrings.add(s)
                allStrings.size - 1
            }
        }

        // 构建 sheet 数据：先收集，统一算索引
        val sheetRows = mutableListOf<List<Pair<String, Boolean>>>()  // (值, 是否字符串)
        val headerRow = headers.map { h -> Pair(h, true) }
        if (headerRow.isNotEmpty()) sheetRows.add(headerRow)
        rows.forEach { r ->
            sheetRows.add(r.map { v -> Pair(v, true) })
        }

        // 为所有字符串单元格注册索引
        sheetRows.flatten().forEach { (v, isStr) -> if (isStr) getIndex(v) }

        val entries = LinkedHashMap<String, String>()

        // sharedStrings.xml
        val ssSb = StringBuilder()
        ssSb.append("""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>""")
        ssSb.append("""<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="${sheetRows.sumOf { it.size }}" uniqueCount="${allStrings.size}">""")
        allStrings.forEach { s ->
            ssSb.append("""<si><t xml:space="preserve">${OoxmlZipHelper.escapeXml(s)}</t></si>""")
        }
        ssSb.append("</sst>")

        // worksheet XML（sharedStrings 索引引用）
        val wsSb = StringBuilder()
        wsSb.append("""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>""")
        wsSb.append("""<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>""")
        sheetRows.forEachIndexed { rowIdx, row ->
            val r = rowIdx + 1
            wsSb.append("""<row r="$r">""")
            row.forEachIndexed { colIdx, (value, isStr) ->
                val ref = columnName(colIdx) + r
                if (isStr) {
                    val idx = stringIndex[value] ?: 0
                    wsSb.append("""<c r="$ref" t="s"><v>$idx</v></c>""")
                } else {
                    wsSb.append("""<c r="$ref"><v>${OoxmlZipHelper.escapeXml(value)}</v></c>""")
                }
            }
            wsSb.append("</row>")
        }
        wsSb.append("</sheetData></worksheet>")

        // Content_Types（加 sharedStrings）
        val ctSb = StringBuilder()
        ctSb.append("""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>""")
        ctSb.append("""<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">""")
        ctSb.append("""<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>""")
        ctSb.append("""<Default Extension="xml" ContentType="application/xml"/>""")
        ctSb.append("""<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>""")
        ctSb.append("""<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>""")
        ctSb.append("""<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>""")
        ctSb.append("</Types>")

        // workbook.xml
        val wbSb = StringBuilder()
        wbSb.append("""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>""")
        wbSb.append("""<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">""")
        wbSb.append("""<sheets><sheet name="${OoxmlZipHelper.escapeXml(sheetName)}" sheetId="1" r:id="rId1"/></sheets></workbook>""")

        // workbook.xml.rels（加 sharedStrings 关系）
        val wbRelsSb = StringBuilder()
        wbRelsSb.append("""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>""")
        wbRelsSb.append("""<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">""")
        wbRelsSb.append("""<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>""")
        wbRelsSb.append("""<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>""")
        wbRelsSb.append("</Relationships>")

        entries["[Content_Types].xml"] = ctSb.toString()
        entries["_rels/.rels"] = buildRootRels()
        entries["xl/workbook.xml"] = wbSb.toString()
        entries["xl/_rels/workbook.xml.rels"] = wbRelsSb.toString()
        entries["xl/worksheets/sheet1.xml"] = wsSb.toString()
        entries["xl/sharedStrings.xml"] = ssSb.toString()

        return OoxmlZipHelper.buildZip(entries)
    }

    /** 写 xlsx 到缓存目录 */
    fun writeToCache(bytes: ByteArray, baseName: String): File {
        val safeName = baseName.ifBlank { "表格" }
        val file = File(context.cacheDir, "${safeName}.xlsx")
        file.writeBytes(bytes)
        return file
    }

    /** 列名 A, B, C, ... Z, AA, AB */
    private fun columnName(index: Int): String {
        var i = index
        val sb = StringBuilder()
        while (i >= 0) {
            sb.insert(0, ('A' + (i % 26)))
            i = i / 26 - 1
        }
        return sb.toString()
    }

    private fun buildRootRels(): String {
        return """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"""
    }
}
