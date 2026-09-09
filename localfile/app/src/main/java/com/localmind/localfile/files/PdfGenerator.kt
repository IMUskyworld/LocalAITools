package com.localmind.localfile.files

import android.content.Context
import com.tom_roush.pdfbox.pdmodel.PDDocument
import com.tom_roush.pdfbox.pdmodel.PDPage
import com.tom_roush.pdfbox.pdmodel.PDPageContentStream
import com.tom_roush.pdfbox.pdmodel.common.PDRectangle
import com.tom_roush.pdfbox.pdmodel.font.PDType0Font
import java.io.ByteArrayOutputStream
import java.io.File

/**
 * 从零生成 PDF（pdfbox + 中文字体）。
 */
class PdfGenerator(private val context: Context) {

    /**
     * 生成 PDF 字节数组。
     * @param title 标题
     * @param paragraphs 正文段落
     * @param bullets 要点列表
     */
    fun generatePdf(
        title: String,
        paragraphs: List<String>,
        bullets: List<String> = emptyList()
    ): ByteArray {
        // pdfbox 必需初始化
        com.tom_roush.pdfbox.android.PDFBoxResourceLoader.init(context)

        PDDocument().use { doc ->
            // 加载中文字体
            val font = context.assets.open("fonts/simhei.ttf").use { stream ->
                PDType0Font.load(doc, stream)
            }

            // 收集所有内容行（标题 + 段落 + 要点）
            val lines = mutableListOf<Pair<String, Float>>()  // (文本, 字号)
            if (title.isNotBlank()) lines.add(Pair(title, 18f))
            paragraphs.filter { it.isNotBlank() }.forEach { lines.add(Pair(it, 12f)) }
            bullets.filter { it.isNotBlank() }.forEach { lines.add(Pair("• $it", 12f)) }

            val pageWidth = PDRectangle.A4.width  // 595
            val margin = 50f
            val maxWidth = pageWidth - margin * 2
            val leading = 20f
            val yStart = PDRectangle.A4.height - margin  // 792

            var page = PDPage(PDRectangle.A4)
            doc.addPage(page)
            var cs = PDPageContentStream(doc, page)
            var y = yStart

            for ((text, size) in lines) {
                // 按宽度换行
                val wrapped = wrapText(font, size, text, maxWidth)

                for (line in wrapped) {
                    if (y < margin) {
                        // 分页
                        cs.close()
                        page = PDPage(PDRectangle.A4)
                        doc.addPage(page)
                        cs = PDPageContentStream(doc, page)
                        y = yStart
                    }
                    cs.beginText()
                    cs.setFont(font, size)
                    cs.newLineAtOffset(margin, y)
                    cs.showText(line)
                    cs.endText()
                    y -= leading
                }
            }
            cs.close()

            // 输出到字节数组
            val baos = ByteArrayOutputStream()
            doc.save(baos)
            return baos.toByteArray()
        }
    }

    /** 写 PDF 到缓存目录 */
    fun writeToCache(bytes: ByteArray, baseName: String): File {
        val safeName = baseName.ifBlank { "文档" }
        val file = File(context.cacheDir, "${safeName}.pdf")
        file.writeBytes(bytes)
        return file
    }

    /** 按页面宽度换行（中文逐字测量） */
    private fun wrapText(font: PDType0Font, size: Float, text: String, maxWidth: Float): List<String> {
        val lines = mutableListOf<String>()
        val current = StringBuilder()

        for (ch in text) {
            val test = current.toString() + ch
            val width = font.getStringWidth(test) / 1000f * size
            if (width > maxWidth && current.isNotEmpty()) {
                lines.add(current.toString())
                current.setLength(0)
            }
            current.append(ch)
        }
        if (current.isNotEmpty()) lines.add(current.toString())
        return lines
    }
}
