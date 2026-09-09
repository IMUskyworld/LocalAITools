package com.localmind.localfile.files

import com.localmind.localfile.common.ChatMessage
import com.localmind.localfile.common.DeepSeekConfig
import com.localmind.localfile.common.GatewayClient
import com.localmind.localfile.common.Logger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

class FileAiService(
    private val gatewayClient: GatewayClient = GatewayClient()
) {

    suspend fun process(
        content: String,
        fileName: String,
        mode: ProcessingMode
    ): String = withContext(Dispatchers.IO) {
        val systemPrompt = when (mode) {
            ProcessingMode.SUMMARIZE -> """
                你是一个文本摘要助手。请用中文简洁地总结以下文件内容，保留关键信息。
            """.trimIndent()

            ProcessingMode.TRANSLATE -> """
                你是一个翻译助手。请将以下文件内容翻译成中文。
                如果原文已经是中文，则翻译成英文。保持原意和语气。
            """.trimIndent()

            ProcessingMode.SMART_RENAME -> """
                你是一个文件重命名助手。请根据文件内容，给出一个简洁、有描述性的文件名
                （不含扩展名）。只返回建议的文件名，不要任何额外文字或解释。
            """.trimIndent()

            ProcessingMode.EXTRACT_KEY_POINTS -> """
                你是一个要点提取助手。请从以下文件内容中提取最重要的关键点，
                用简洁的项目符号列表呈现，使用中文回复。
            """.trimIndent()
        }

        try {
            // 超长文件截断，防止撑爆 DeepSeek 上下文
            val truncatedContent = if (content.length > DeepSeekConfig.MAX_FILE_CHARS) {
                content.take(DeepSeekConfig.MAX_FILE_CHARS) + "\n...[文件过长，内容已截断]"
            } else {
                content
            }
            val messages = listOf(
                ChatMessage("system", systemPrompt),
                ChatMessage("user", "文件名：$fileName\n\n文件内容：\n$truncatedContent")
            )
            val result = gatewayClient.chatCompletion(messages)
            result.content
        } catch (e: Exception) {
            Logger.e("AI processing failed", e)
            // Fallback: simple local processing
            fallbackProcess(content, fileName, mode)
        }
    }

    /**
     * Fallback local processing when gateway is unavailable.
     */
    private fun fallbackProcess(
        content: String,
        fileName: String,
        mode: ProcessingMode
    ): String {
        return when (mode) {
            ProcessingMode.SUMMARIZE -> {
                val words = content.split("\\s+".toRegex())
                val preview = if (words.size > 100) {
                    words.take(100).joinToString(" ") + "...\n\n[摘要截断：全文共 ${words.size} 词]"
                } else {
                    content
                }
                "摘要：\n$preview"
            }
            ProcessingMode.TRANSLATE -> {
                "[无法连接 DeepSeek，翻译不可用]\n\n原文内容：\n${content.take(500)}"
            }
            ProcessingMode.SMART_RENAME -> {
                val base = fileName.substringBeforeLast(".")
                val ext = fileName.substringAfterLast(".", "")
                val words = base.split("[-_\\s]+".toRegex())
                    .filter { it.length > 2 }
                    .take(3)
                    .joinToString("_")
                if (words.isNotEmpty()) "${words}_已处理.$ext" else "已处理_${System.currentTimeMillis()}.$ext"
            }
            ProcessingMode.EXTRACT_KEY_POINTS -> {
                val lines = content.lines().filter { it.isNotBlank() }
                val keyLines = lines.take(10).mapIndexed { i, line ->
                    "- ${line.take(80)}${if (line.length > 80) "..." else ""}"
                }
                keyLines.joinToString("\n")
            }
        }
    }
}
