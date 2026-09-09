package com.localmind.localfile.common

import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider
import java.io.File

/**
 * 文件分享工具：用 FileProvider 把文件分享到其他应用（微信/QQ 等）。
 */
object ShareUtils {

    /**
     * 分享一个文件。
     * @param mimeType 文件 MIME 类型，如 docx 的 application/vnd.openxmlformats-officedocument.wordprocessingml.document
     */
    fun shareFile(context: Context, file: File, mimeType: String) {
        try {
            val uri = FileProvider.getUriForFile(
                context,
                "${context.packageName}.fileprovider",
                file
            )
            val intent = Intent(Intent.ACTION_SEND).apply {
                type = mimeType
                putExtra(Intent.EXTRA_STREAM, uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            context.startActivity(Intent.createChooser(intent, "分享文件"))
        } catch (e: Exception) {
            Logger.e("Share file failed", e)
        }
    }

    /** 分享 docx 文件 */
    fun shareDocx(context: Context, file: File) {
        shareFile(context, file, "application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    }

    /** 按文件扩展名自动选 MIME 类型分享 */
    fun shareFileByExtension(context: Context, file: File) {
        val mime = when (file.extension.lowercase()) {
            "docx" -> "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            "pptx" -> "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            "xlsx" -> "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            "pdf" -> "application/pdf"
            "txt" -> "text/plain"
            else -> "application/octet-stream"
        }
        shareFile(context, file, mime)
    }

    /** 分享纯文本（写临时 txt） */
    fun shareText(context: Context, text: String, fileName: String = "分享内容.txt") {
        try {
            val file = File(context.cacheDir, fileName)
            file.writeText(text)
            shareFile(context, file, "text/plain")
        } catch (e: Exception) {
            Logger.e("Share text failed", e)
        }
    }
}
