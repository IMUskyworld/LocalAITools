package com.localmind.localfile.files

import android.content.Context
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/**
 * 从零生成 docx 文件（最小 OOXML 骨架）。
 * 不依赖模板，直接构造 Word 文档内容。
 */
class DocxGenerator(private val context: Context) {

    /**
     * 生成 docx 字节数组。
     * @param title 文档标题（可选）
     * @param paragraphs 正文段落列表
     * @param bullets 要点列表（可选）
     */
    fun generateDocx(
        title: String? = null,
        paragraphs: List<String>,
        bullets: List<String> = emptyList()
    ): ByteArray {
        val documentXml = buildDocumentXml(title, paragraphs, bullets)

        val entries = linkedMapOf<String, ByteArray>()
        entries["[Content_Types].xml"] = CONTENT_TYPES.toByteArray(Charsets.UTF_8)
        entries["_rels/.rels"] = RELS.toByteArray(Charsets.UTF_8)
        entries["word/document.xml"] = documentXml.toByteArray(Charsets.UTF_8)
        entries["word/_rels/document.xml.rels"] = DOC_RELS.toByteArray(Charsets.UTF_8)

        return ByteArrayOutputStream().use { baos ->
            ZipOutputStream(baos).use { zip ->
                for ((name, bytes) in entries) {
                    zip.putNextEntry(ZipEntry(name))
                    zip.write(bytes)
                    zip.closeEntry()
                }
            }
            baos.toByteArray()
        }
    }

    /** 写 docx 到缓存目录 */
    fun writeToCache(bytes: ByteArray, baseName: String): File {
        val safeName = baseName.ifBlank { "document" }
        val file = File(context.cacheDir, "${safeName}.docx")
        file.writeBytes(bytes)
        return file
    }

    private fun buildDocumentXml(
        title: String?,
        paragraphs: List<String>,
        bullets: List<String>
    ): String {
        val sb = StringBuilder()
        sb.append("""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>""")
        sb.append("""<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">""")
        sb.append("<w:body>")

        // 标题（Heading 样式）
        if (!title.isNullOrBlank()) {
            sb.append(paragraphXml(title, style = "Title"))
        }

        // 正文段落
        paragraphs.filter { it.isNotBlank() }.forEach { p ->
            sb.append(paragraphXml(p))
        }

        // 要点列表
        bullets.filter { it.isNotBlank() }.forEach { b ->
            sb.append(paragraphXml("• $b"))
        }

        sb.append("</w:body></w:document>")
        return sb.toString()
    }

    private fun paragraphXml(text: String, style: String? = null): String {
        val escaped = escapeXml(text)
        val styleTag = style?.let { "<w:pPr><w:pStyle w:val=\"$it\"/></w:pPr>" } ?: ""
        return "<w:p>$styleTag<w:r><w:t xml:space=\"preserve\">$escaped</w:t></w:r></w:p>"
    }

    private fun escapeXml(s: String): String =
        s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace("\"", "&quot;")

    companion object {
        private val CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"""

        private val RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"""

        private val DOC_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"""
    }
}
