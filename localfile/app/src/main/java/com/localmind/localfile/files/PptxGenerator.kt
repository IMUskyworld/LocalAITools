package com.localmind.localfile.files

import android.content.Context
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipInputStream
import java.util.zip.ZipOutputStream

/**
 * 基于完整模板生成 pptx（保证 WPS/Office 兼容）。
 * 模板由 python-pptx 生成（含 slideMaster/slideLayout/theme），打进 assets。
 * 生成时替换 slide1.xml 内容，多页时复制 slide + 更新 presentation.xml/rels。
 */
class PptxGenerator(private val context: Context) {

    /**
     * 生成 pptx 字节数组。
     * @param title 演示文稿标题
     * @param slides 页面列表：每页 {title, bullets[]}
     */
    fun generatePptx(title: String, slides: List<Pair<String, List<String>>>): ByteArray {
        // 读取模板
        val templateEntries = readTemplateEntries()
        val slideCount = slides.size.coerceAtLeast(1)

        // 生成每页 slide XML（用模板 slide1 的结构，替换 title + content 文本）
        val slideXmls = (0 until slideCount).map { i ->
            val slide = slides.getOrElse(i) { Pair(title, emptyList()) }
            buildSlideXml(slide.first, slide.second)
        }

        // 复制模板条目，替换/添加 slides
        val entries = LinkedHashMap(templateEntries)
        (0 until slideCount).forEach { i ->
            entries["ppt/slides/slide${i + 1}.xml"] = slideXmls[i].toByteArray(Charsets.UTF_8)
            // slide rels（指向 layout rId1）
            entries["ppt/slides/_rels/slide${i + 1}.xml.rels"] =
                buildSlideRels().toByteArray(Charsets.UTF_8)
        }

        // 更新 presentation.xml：sldIdLst 引用所有页
        entries["ppt/presentation.xml"] = updatePresentationXml(
            templateEntries["ppt/presentation.xml"]?.toString(Charsets.UTF_8) ?: "",
            slideCount
        ).toByteArray(Charsets.UTF_8)

        // 更新 presentation.xml.rels：为每页加 slide 关系
        entries["ppt/_rels/presentation.xml.rels"] = updatePresentationRels(
            templateEntries["ppt/_rels/presentation.xml.rels"]?.toString(Charsets.UTF_8) ?: "",
            slideCount
        ).toByteArray(Charsets.UTF_8)

        // 更新 Content_Types：为每页加 slide override
        entries["[Content_Types].xml"] = updateContentTypes(
            templateEntries["[Content_Types].xml"]?.toString(Charsets.UTF_8) ?: "",
            slideCount
        ).toByteArray(Charsets.UTF_8)

        // 打包
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

    /** 写 pptx 到缓存目录 */
    fun writeToCache(bytes: ByteArray, baseName: String): File {
        val safeName = baseName.ifBlank { "演示文稿" }
        val file = File(context.cacheDir, "${safeName}.pptx")
        file.writeBytes(bytes)
        return file
    }

    /** 读取模板 zip 所有条目 */
    private fun readTemplateEntries(): LinkedHashMap<String, ByteArray> {
        val entries = LinkedHashMap<String, ByteArray>()
        context.assets.open("pptx_template.pptx").use { input ->
            ZipInputStream(input).use { zip ->
                var entry = zip.nextEntry
                while (entry != null) {
                    if (!entry.isDirectory) {
                        entries[entry.name] = zip.readBytes()
                    }
                    zip.closeEntry()
                    entry = zip.nextEntry
                }
            }
        }
        return entries
    }

    /** 生成 slide XML（用模板的结构：title + content 占位符） */
    private fun buildSlideXml(slideTitle: String, bullets: List<String>): String {
        val bulletsXml = if (bullets.isEmpty()) {
            "<a:p><a:r><a:t></a:t></a:r></a:p>"
        } else {
            bullets.joinToString("") { b ->
                "<a:p><a:r><a:t>• ${OoxmlZipHelper.escapeXml(b)}</a:t></a:r></a:p>"
            }
        }
        return """<?xml version='1.0' encoding='UTF-8' standalone='yes'?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<p:cSld><p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr/>
<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>${OoxmlZipHelper.escapeXml(slideTitle)}</a:t></a:r></a:p></p:txBody></p:sp>
<p:sp><p:nvSpPr><p:cNvPr id="3" name="Content Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>$bulletsXml</p:txBody></p:sp>
</p:spTree></p:cSld>
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"""
    }

    /** slide rels：指向 slideLayout */
    private fun buildSlideRels(): String {
        return """<?xml version='1.0' encoding='UTF-8' standalone='yes'?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"""
    }

    /** 更新 presentation.xml 的 sldIdLst 引用所有页 */
    private fun updatePresentationXml(xml: String, slideCount: Int): String {
        // 替换 sldIdLst
        val sldIdLst = buildString {
            append("<p:sldIdLst>")
            (0 until slideCount).forEach { i ->
                append("""<p:sldId id="${256 + i}" r:id="rId${7 + i}"/>""")
            }
            append("</p:sldIdLst>")
        }
        return xml.replace(Regex("<p:sldIdLst>.*?</p:sldIdLst>", RegexOption.DOT_MATCHES_ALL), sldIdLst)
    }

    /** 更新 presentation.xml.rels：加每页 slide 关系（从 rId7 开始） */
    private fun updatePresentationRels(xml: String, slideCount: Int): String {
        // 移除旧的 slide 关系（rId7 及以后）
        val baseXml = xml.replace(
            Regex("<Relationship Id=\"rId7\"[^>]*/>"),
            ""
        )
        val slideRels = buildString {
            (0 until slideCount).forEach { i ->
                append("""<Relationship Id="rId${7 + i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide${i + 1}.xml"/>""")
            }
        }
        // 在 </Relationships> 前插入
        return baseXml.replace("</Relationships>", slideRels + "</Relationships>")
    }

    /** 更新 Content_Types：移除旧的 slide override，加每页 slide override */
    private fun updateContentTypes(xml: String, slideCount: Int): String {
        // 移除已有的 slide override
        val baseXml = xml.replace(
            Regex("<Override PartName=\"/ppt/slides/slide\\d+\\.xml\"[^>]*/>"),
            ""
        )
        val overrides = buildString {
            (0 until slideCount).forEach { i ->
                append("""<Override PartName="/ppt/slides/slide${i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>""")
            }
        }
        return baseXml.replace("</Types>", overrides + "</Types>")
    }
}
