package com.localmind.localfile.files

import android.content.Context
import android.net.Uri
import com.localmind.localfile.common.FileProcessingException
import com.localmind.localfile.common.Logger
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipInputStream
import java.util.zip.ZipOutputStream

/**
 * docx 模板编辑器：保留模板 zip 结构，只替换 document.xml 中的文本。
 * 零第三方依赖（纯 Kotlin + 系统 zip），模板样式/格式保留。
 */
class DocxTemplateEditor(private val context: Context) {

    /**
     * 读取 docx 模板：
     * - 返回段落纯文本列表（按 <w:p> 切分）
     * - 同时保留原始 zip 的所有条目（用于重新打包）
     */
    fun readTemplate(uri: Uri): TemplateData {
        return try {
            val entries = LinkedHashMap<String, ByteArray>()
            val paragraphs = mutableListOf<String>()

            context.contentResolver.openInputStream(uri)?.use { input ->
                ZipInputStream(input).use { zip ->
                    var entry = zip.nextEntry
                    while (entry != null) {
                        val name = entry.name
                        val bytes = zip.readBytes()
                        if (name == "word/document.xml") {
                            val xml = bytes.toString(Charsets.UTF_8)
                            paragraphs.addAll(extractParagraphs(xml))
                        }
                        entries[name] = bytes
                        zip.closeEntry()
                        entry = zip.nextEntry
                    }
                }
            } ?: throw FileProcessingException("无法打开文件")

            if (paragraphs.isEmpty()) {
                throw FileProcessingException("文档中没有可修改的段落")
            }
            TemplateData(entries, paragraphs)
        } catch (e: FileProcessingException) {
            throw e
        } catch (e: Exception) {
            Logger.e("Failed to read docx template", e)
            throw FileProcessingException("读取 docx 模板失败：${e.message}", cause = e)
        }
    }

    /**
     * 用新的段落文本替换 document.xml 里的内容，重新打包成 docx。
     */
    fun rebuildDocx(template: TemplateData, newParagraphs: List<String>): ByteArray {
        val entries = LinkedHashMap(template.entries)
        val oldXml = entries["word/document.xml"]?.toString(Charsets.UTF_8)
            ?: throw FileProcessingException("模板缺少 word/document.xml")

        val newXml = replaceParagraphTexts(oldXml, template.paragraphs, newParagraphs)
        entries["word/document.xml"] = newXml.toByteArray(Charsets.UTF_8)

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

    /**
     * 写 docx 字节到缓存目录。
     */
    fun writeToCache(bytes: ByteArray, baseName: String): File {
        val safeName = baseName.substringBeforeLast(".").ifBlank { "modified" }
        val file = File(context.cacheDir, "${safeName}_modified.docx")
        file.writeBytes(bytes)
        return file
    }

    /**
     * 从 document.xml 提取段落纯文本（按 <w:p> 切分，每段提取 <w:t> 文本）。
     */
    private fun extractParagraphs(xml: String): List<String> {
        val paragraphs = mutableListOf<String>()
        // 按 <w:p> 段切分（含自闭合或嵌套）
        val pPattern = Regex("<w:p[^>]*>.*?</w:p>|<w:p[^>]*/>", RegexOption.DOT_MATCHES_ALL)
        for (pMatch in pPattern.findAll(xml)) {
            val paraXml = pMatch.value
            val text = extractTextFromParagraph(paraXml)
            paragraphs.add(text)
        }
        return paragraphs
    }

    /**
     * 从单个 <w:p> 段落提取文本（收集所有 <w:t> 内容）。
     */
    private fun extractTextFromParagraph(paraXml: String): String {
        val tPattern = Regex("<w:t[^>]*>(.*?)</w:t>", RegexOption.DOT_MATCHES_ALL)
        val sb = StringBuilder()
        for (tMatch in tPattern.findAll(paraXml)) {
            sb.append(decodeXmlEntities(tMatch.groupValues[1]))
        }
        // 表格等复杂结构的段可能取不到，保留标签去标签兜底
        if (sb.isEmpty()) {
            sb.append(paraXml.replace(Regex("<[^>]+>"), " ").trim())
        }
        return sb.toString().trim()
    }

    /**
     * 用新段落文本替换 document.xml 的文本。
     * 策略：遍历每个 <w:p> 段落，第 i 段的所有 <w:t> 合并替换为新段落文本。
     */
    private fun replaceParagraphTexts(xml: String, oldParagraphs: List<String>, newParagraphs: List<String>): String {
        var result = xml
        val pPattern = Regex("<w:p[^>]*>.*?</w:p>|<w:p[^>]*/>", RegexOption.DOT_MATCHES_ALL)
        val matches = pPattern.findAll(result).toList()

        // 从后往前替换，避免索引错位
        for (i in matches.indices.reversed()) {
            val oldText = if (i < oldParagraphs.size) oldParagraphs[i] else ""
            val newText = if (i < newParagraphs.size) newParagraphs[i] else oldText
            val paraXml = matches[i].value
            val replaced = replaceInParagraph(paraXml, newText)
            result = result.replaceFirst(paraXml, replaced)
        }
        return result
    }

    /**
     * 在单个段落 XML 里：把所有 <w:t> 内容替换为新文本。
     * 若段落有多个 <w:t>，把内容放第一个，清空其余。
     */
    private fun replaceInParagraph(paraXml: String, newText: String): String {
        val tPattern = Regex("<w:t[^>]*>.*?</w:t>|<w:t[^>]*/>", RegexOption.DOT_MATCHES_ALL)
        val tMatches = tPattern.findAll(paraXml).toList()
        if (tMatches.isEmpty()) return paraXml

        var result = paraXml
        // 从后往前：先清空后面的 <w:t>，再填第一个
        for (i in tMatches.indices.reversed()) {
            val t = tMatches[i].value
            // 保留 <w:t> 标签属性（如 xml:space）
            val tagMatch = Regex("^<w:t([^>]*)>").find(t)
            val attrs = tagMatch?.groupValues?.get(1) ?: ""
            val replacement = if (i == 0) {
                "<w:t$attrs>${encodeXmlText(newText)}</w:t>"
            } else {
                "<w:t$attrs></w:t>"
            }
            result = result.replaceFirst(t, replacement)
        }
        return result
    }

    private fun decodeXmlEntities(s: String): String =
        s.replace("&lt;", "<").replace("&gt;", ">")
            .replace("&amp;", "&").replace("&quot;", "\"").replace("&apos;", "'")

    private fun encodeXmlText(s: String): String =
        s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace("\"", "&quot;")

    data class TemplateData(
        val entries: Map<String, ByteArray>,
        val paragraphs: List<String>
    )
}
