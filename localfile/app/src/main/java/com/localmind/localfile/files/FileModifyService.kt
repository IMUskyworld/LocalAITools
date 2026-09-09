package com.localmind.localfile.files

import com.localmind.localfile.common.ChatMessage
import com.localmind.localfile.common.DeepSeekConfig
import com.localmind.localfile.common.GatewayClient
import com.localmind.localfile.common.Logger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray

/**
 * 按模板修改 docx：让 AI 根据模板的段落结构，产出对应段落的新文本（JSON 数组）。
 */
class FileModifyService(
    private val gatewayClient: GatewayClient = GatewayClient()
) {

    /**
     * 根据模板段落，让 AI 产出新的段落文本列表。
     * @param templateParagraphs 模板的段落纯文本
     * @param instruction 用户的修改要求（如「改成李白介绍」）
     * @return 与模板段落一一对应的新文本列表
     */
    suspend fun modifyByTemplate(
        templateParagraphs: List<String>,
        instruction: String
    ): List<String> = withContext(Dispatchers.IO) {
        val systemPrompt = """
            你是一个文档编辑助手。用户给了一个文档的段落列表，以及修改要求。
            请根据修改要求，为【每一段】生成对应的新内容，保持段落数量和顺序与原文一致。
            如果某段是空段落或格式标记（如只有空格），返回空字符串。
            严格输出一个 JSON 数组，每个元素对应一段的新文本，不要输出任何其他内容。
        """.trimIndent()

        val templateText = templateParagraphs.mapIndexed { i, p ->
            "[段落${i + 1}] ${p}"
        }.joinToString("\n")

        val userPrompt = """
            修改要求：$instruction

            原文段落：
            $templateText

            请输出修改后的段落 JSON 数组（每个元素对应原文一个段落，共 ${templateParagraphs.size} 段）。
        """.trimIndent()

        try {
            val messages = listOf(
                ChatMessage("system", systemPrompt),
                ChatMessage("user", userPrompt)
            )
            val result = gatewayClient.chatCompletion(messages)
            parseJsonArray(result.content) ?: fallbackModify(templateParagraphs, instruction)
        } catch (e: Exception) {
            Logger.e("AI modify failed", e)
            fallbackModify(templateParagraphs, instruction)
        }
    }

    /** 解析 AI 输出的 JSON 数组；失败返回 null */
    private fun parseJsonArray(content: String): List<String>? {
        if (content.isBlank()) return null
        return try {
            // 提取第一个 [ ... ] 块
            val start = content.indexOf('[')
            val end = content.lastIndexOf(']')
            if (start < 0 || end <= start) return null
            val jsonStr = content.substring(start, end + 1)
            val arr = JSONArray(jsonStr)
            (0 until arr.length()).map { arr.optString(it) }
        } catch (e: Exception) {
            Logger.w("AI modify JSON parse failed: ${e.message}")
            null
        }
    }

    /** 兜底：AI 返回不可用时的降级（提示用户，不真正改动） */
    private fun fallbackModify(templateParagraphs: List<String>, instruction: String): List<String> {
        // 无法从 AI 获得可靠修改时，返回原文（保持段落结构）
        return templateParagraphs
    }
}
