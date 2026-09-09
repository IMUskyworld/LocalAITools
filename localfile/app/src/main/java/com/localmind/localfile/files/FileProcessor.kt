package com.localmind.localfile.files

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import com.localmind.localfile.common.FileProcessingException
import com.localmind.localfile.common.Logger
import java.io.BufferedReader
import java.io.InputStreamReader

data class FileInfo(
    val uri: Uri,
    val name: String,
    val size: Long,
    val mimeType: String,
    val content: String = ""
)

enum class ProcessingMode(val displayName: String) {
    SUMMARIZE("摘要"),
    TRANSLATE("翻译"),
    SMART_RENAME("智能重命名"),
    EXTRACT_KEY_POINTS("提取要点")
}

class FileProcessor(private val context: Context) {

    /**
     * Read file metadata from SAF URI.
     */
    fun getFileInfo(uri: Uri): FileInfo {
        val cursor = context.contentResolver.query(uri, null, null, null, null)
        return cursor?.use { c ->
            if (c.moveToFirst()) {
                val nameIndex = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                val sizeIndex = c.getColumnIndex(OpenableColumns.SIZE)
                val name = if (nameIndex >= 0) c.getString(nameIndex) else "unknown"
                val size = if (sizeIndex >= 0) c.getLong(sizeIndex) else 0L
                val mimeType = context.contentResolver.getType(uri) ?: "application/octet-stream"
                FileInfo(uri = uri, name = name, size = size, mimeType = mimeType)
            } else {
                throw FileProcessingException("无法读取文件信息")
            }
        } ?: throw FileProcessingException("无法读取文件")
    }

    /**
     * 读取文件内容（智能分发）。支持：文本、PDF、Office(docx/pptx/xlsx)。
     * 图片降级提示（DeepSeek 纯文本模型，不支持图片）。
     */
    fun readFileContent(uri: Uri): String {
        val fileInfo = getFileInfo(uri)
        if (fileInfo.size > MAX_SIZE) {
            throw FileProcessingException("文件太大（最大 ${MAX_SIZE / 1024 / 1024}MB）")
        }

        return when {
            isImageType(fileInfo.mimeType) ->
                throw FileProcessingException("当前模型不支持图片，请上传文本、PDF 或 Office 文件")
            isPdfType(fileInfo.mimeType) -> readPdfContent(uri)
            isOfficeType(fileInfo.mimeType) -> readOfficeContent(uri)
            isTextType(fileInfo.mimeType) -> readTextContent(uri)
            else -> throw FileProcessingException("不支持的文件类型：${fileInfo.mimeType}")
        }
    }

    /** 读取纯文本内容 */
    fun readTextContent(uri: Uri): String {
        return try {
            val sb = StringBuilder()
            context.contentResolver.openInputStream(uri)?.use { inputStream ->
                BufferedReader(InputStreamReader(inputStream)).use { reader ->
                    var line: String?
                    while (reader.readLine().also { line = it } != null) {
                        sb.append(line).append("\n")
                    }
                }
            } ?: throw FileProcessingException("无法打开文件")
            sb.toString()
        } catch (e: Exception) {
            Logger.e("Failed to read file content", e)
            throw FileProcessingException("读取文件失败：${e.message}", cause = e)
        }
    }

    /** 读取 PDF 文本（pdfbox-android） */
    private fun readPdfContent(uri: Uri): String {
        return try {
            // pdfbox-android 必需初始化：加载 glyphlist 等资源
            com.tom_roush.pdfbox.android.PDFBoxResourceLoader.init(context)
            val sb = StringBuilder()
            context.contentResolver.openInputStream(uri)?.use { inputStream ->
                com.tom_roush.pdfbox.pdmodel.PDDocument.load(inputStream).use { doc ->
                    val stripper = com.tom_roush.pdfbox.text.PDFTextStripper()
                    sb.append(stripper.getText(doc))
                }
            } ?: throw FileProcessingException("无法打开文件")
            sb.toString()
        } catch (e: Exception) {
            Logger.e("Failed to read PDF content", e)
            throw FileProcessingException("PDF 读取失败：${e.message}", cause = e)
        }
    }

    /** 读取 Office Open XML 文本（docx/pptx/xlsx，原生解 zip 读 XML） */
    private fun readOfficeContent(uri: Uri): String {
        return try {
            val fileName = getFileInfo(uri).name.lowercase()
            val sb = StringBuilder()
            context.contentResolver.openInputStream(uri)?.use { inputStream ->
                java.util.zip.ZipInputStream(inputStream).use { zip ->
                    var entry = zip.nextEntry
                    while (entry != null) {
                        val name = entry.name
                        when {
                            fileName.endsWith(".docx") && name == "word/document.xml" ->
                                sb.append(extractTextFromXml(zip.readBytes().toString(Charsets.UTF_8))).append("\n")
                            fileName.endsWith(".pptx") && name.startsWith("ppt/slides/slide") && name.endsWith(".xml") ->
                                sb.append(extractTextFromXml(zip.readBytes().toString(Charsets.UTF_8))).append("\n")
                            fileName.endsWith(".xlsx") && (name.startsWith("xl/worksheets/sheet") || name == "xl/sharedStrings.xml") ->
                                sb.append(extractTextFromXml(zip.readBytes().toString(Charsets.UTF_8))).append("\n")
                        }
                        zip.closeEntry()
                        entry = zip.nextEntry
                    }
                }
            } ?: throw FileProcessingException("无法打开文件")
            sb.toString()
        } catch (e: Exception) {
            Logger.e("Failed to read Office content", e)
            throw FileProcessingException("Office 文件读取失败：${e.message}", cause = e)
        }
    }

    /** 从 Word/PPT/Excel 的 XML 里提取文本（去标签、去实体） */
    private fun extractTextFromXml(xml: String): String {
        return xml
            .replace(Regex("<[^>]+>"), " ")  // 去标签
            .replace("&lt;", "<").replace("&gt;", ">")
            .replace("&amp;", "&").replace("&quot;", "\"").replace("&apos;", "'")
            .replace(Regex("\\s+"), " ")
            .trim()
    }

    /**
     * Check if the MIME type is a readable text type.
     */
    fun isTextType(mimeType: String): Boolean {
        return mimeType.startsWith("text/") ||
            mimeType == "application/json" ||
            mimeType == "application/xml" ||
            mimeType == "application/javascript" ||
            mimeType == "application/csv" ||
            mimeType == "application/x-yaml" ||
            mimeType == "application/yaml" ||
            mimeType == "application/rtf" ||
            mimeType.endsWith("+xml") ||
            mimeType.endsWith("+json")
    }

    fun isPdfType(mimeType: String): Boolean = mimeType == "application/pdf"

    fun isImageType(mimeType: String): Boolean = mimeType.startsWith("image/")

    /** Office Open XML 或旧版 Office */
    fun isOfficeType(mimeType: String): Boolean {
        return mimeType.startsWith("application/vnd") &&
            (mimeType.contains("wordprocessing") ||
             mimeType.contains("presentation") ||
             mimeType.contains("spreadsheet"))
    }

    companion object {
        private const val MAX_SIZE = 30L * 1024 * 1024  // 30MB
    }

    /**
     * Extract file extension from name.
     */
    fun getFileExtension(fileName: String): String {
        val dot = fileName.lastIndexOf('.')
        return if (dot >= 0) fileName.substring(dot) else ""
    }
}
