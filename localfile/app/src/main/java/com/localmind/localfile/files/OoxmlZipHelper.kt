package com.localmind.localfile.files

import java.io.ByteArrayOutputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/**
 * OOXML 打包辅助：把多个 XML 部件打包成 zip（docx/pptx/xlsx 通用）。
 */
object OoxmlZipHelper {

    /** 把 (路径 -> 内容) 打包成 zip 字节数组 */
    fun buildZip(entries: LinkedHashMap<String, String>): ByteArray {
        return ByteArrayOutputStream().use { baos ->
            ZipOutputStream(baos).use { zip ->
                for ((name, content) in entries) {
                    zip.putNextEntry(ZipEntry(name))
                    zip.write(content.toByteArray(Charsets.UTF_8))
                    zip.closeEntry()
                }
            }
            baos.toByteArray()
        }
    }

    fun escapeXml(s: String): String =
        s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace("\"", "&quot;")
}
