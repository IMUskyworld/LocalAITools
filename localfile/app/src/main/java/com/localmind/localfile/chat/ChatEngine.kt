package com.localmind.localfile.chat

import android.content.Context
import com.localmind.localfile.common.ChatMessage
import com.localmind.localfile.common.ChatResult
import com.localmind.localfile.common.GatewayClient
import com.localmind.localfile.common.Logger
import com.localmind.localfile.common.ToolCall
import com.localmind.localfile.common.ToolDefs
import com.localmind.localfile.files.DocxGenerator
import com.localmind.localfile.files.PdfGenerator
import com.localmind.localfile.files.PptxGenerator
import com.localmind.localfile.files.XlsxGenerator
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import org.json.JSONObject
import java.io.File

/**
 * 聊天引擎：只走在线（DeepSeek），支持 function calling Agent 循环。
 * 离线模式已砍掉（本阶段不做本地推理）。
 */
private const val SYSTEM_PROMPT =
    "你是 LocalFile，一个运行在手机上的 AI 助手。你可以读取用户上传的文件内容，并回答问题。\n" +
    "当用户要求「生成 / 创建 / 制作一个 Word 文档、报告、文档」时，你必须调用 generate_doc 工具，" +
    "把内容结构化成 title/paragraphs 参数传入，绝不能只在回复里口头说「已生成」。\n" +
    "如果用户只是询问或提取信息，直接文本回答即可。所有回答使用中文。"

/** 规划阶段系统提示词 */
private const val PLAN_PROMPT =
    "你是任务规划器。用户提出一个任务，你需要先拆解成清晰的执行步骤，再交给执行者去做。\n" +
    "要求：1. 拆解成 2-5 个有序步骤；2. 每步是单一可执行动作；3. 考虑依赖关系。\n" +
    "严格输出 JSON 格式：{\"plan\": [\"步骤1\", \"步骤2\", ...]}，不要输出其他内容。"

/** 反思阶段系统提示词 */
private const val REFLECT_PROMPT =
    "你是执行反思器。刚才的某个工具执行失败了，请你：\n" +
    "1. 分析失败的可能原因（参数错误、路径问题、方法不对等）。\n" +
    "2. 给出具体的修正建议（调整参数、换方法、或重试）。\n" +
    "严格输出 JSON 格式：{\"reflection\": \"失败原因分析\", \"suggestion\": \"具体修正建议\"}，不要输出其他内容。"

/** 任务关键词：含这些词的用户指令触发规划轮 */
private val TASK_KEYWORDS = Regex("生成|创建|制作|写|保存|总结|分析|整理|读|改|打开|处理|提取|翻译|压缩|转换|列出|查找|设计|做一个")

class ChatEngine(private val appContext: Context) {

    private val gatewayClient = GatewayClient()
    private val docxGenerator = DocxGenerator(appContext)
    private val pptxGenerator = PptxGenerator(appContext)
    private val xlsxGenerator = XlsxGenerator(appContext)
    private val pdfGenerator = PdfGenerator(appContext)

    /**
     * 发送消息并流式接收回复，支持 function calling Agent 循环。
     * 流程：非流式带 tools 请求 → 若返回 tool_calls → 执行工具（生成文件）→ 回传 → 再请求
     *      → 直到无 tool_calls → 流式输出最终回复
     */
    suspend fun sendMessage(
        messages: List<ChatMessage>,
        newMessage: String
    ): Flow<ChatStreamEvent> = flow {
        // 注入 system prompt：明确告知可用工具，强制在生成文档时调用
        val systemMessage = ChatMessage("system", SYSTEM_PROMPT)
        val updatedMessages = mutableListOf<ChatMessage>().apply {
            add(systemMessage)
            addAll(messages)
            add(ChatMessage("user", newMessage))
        }
        val tools = ToolDefs.generateDocTools()

        emit(ChatStreamEvent.StreamStart)

        // 规划轮：任务关键词触发，先拆解步骤再执行
        if (shouldPlan(newMessage)) {
            emit(ChatStreamEvent.ThinkingStep(ThinkingPhase.PLANNING, "正在分析任务并制定计划..."))
            val plan = planTask(updatedMessages, newMessage)
            if (plan.isNotEmpty()) {
                val planText = plan.mapIndexed { i, s -> "${i + 1}. $s" }.joinToString("\n")
                emit(ChatStreamEvent.ThinkingStep(ThinkingPhase.PLANNING, "计划已制定", planText))
                updatedMessages.add(ChatMessage("system",
                    "你的执行计划如下，请按步骤依次完成：\n$planText\n\n如果某步工具失败，分析原因后调整重试。"))
            } else {
                emit(ChatStreamEvent.ThinkingStep(ThinkingPhase.PLANNING, "任务无需工具，直接回答"))
            }
        }

        try {
            // === Agent 循环：非流式拿 tool_calls 决策 ===
            val maxRounds = 4
            var result: ChatResult
            var currentMessages = updatedMessages

            for (round in 0 until maxRounds) {
                result = gatewayClient.chatCompletion(currentMessages, tools = tools)
                val toolCalls = result.toolCalls

                // 无工具调用 → 停止循环
                if (toolCalls.isNullOrEmpty()) break

                // 有工具调用：先回传 assistant 消息（带 tool_calls）
                emit(ChatStreamEvent.ToolCallStarted)
                currentMessages += ChatMessage(
                    role = "assistant",
                    content = result.content,
                    toolCalls = toolCalls
                )

                // 反思轮状态
                var hasFailure = false
                var reflectDone = false
                val failInfos = mutableListOf<String>()

                for (tc in toolCalls) {
                    emit(ChatStreamEvent.ToolCall(tc.name, tc.arguments))
                    emit(ChatStreamEvent.ThinkingStep(ThinkingPhase.EXECUTING, "正在执行：${tc.name}"))

                    // 执行工具，拿到结果（可能生成文件）
                    val (toolResult, generatedFile) = executeTool(tc)

                    // 生成文件事件
                    generatedFile?.let { emit(ChatStreamEvent.GeneratedFile(it)) }

                    // 记录失败（用于反思轮）
                    if (isToolFailure(toolResult)) {
                        hasFailure = true
                        failInfos.add("工具 ${tc.name}: $toolResult")
                    }

                    // 回传 tool 结果
                    currentMessages += ChatMessage(
                        role = "tool",
                        content = toolResult,
                        toolCallId = tc.id,
                        name = tc.name
                    )
                }
                emit(ChatStreamEvent.ToolCallEnd)

                // 反思轮：本轮有工具失败时，让模型分析原因并给修正建议
                if (hasFailure && !reflectDone) {
                    emit(ChatStreamEvent.ThinkingStep(ThinkingPhase.REFLECTING, "检测到工具失败，正在分析原因..."))
                    val reflection = reflectOnFailure(currentMessages, failInfos.joinToString("\n"))
                    if (reflection != null) {
                        emit(ChatStreamEvent.ThinkingStep(ThinkingPhase.REFLECTING, "反思完成，采纳修正建议", reflection))
                        currentMessages += ChatMessage("system", reflection)
                        reflectDone = true
                    } else {
                        emit(ChatStreamEvent.ThinkingStep(ThinkingPhase.REFLECTING, "反思失败，将按原计划重试"))
                    }
                }
            }

            emit(ChatStreamEvent.ThinkingStep(ThinkingPhase.DONE, "任务处理完成"))

            // === 流式输出最终回复 ===
            val finalMessages = currentMessages
            gatewayClient.chatCompletionStream(finalMessages).collect { chunk ->
                emit(ChatStreamEvent.Token(chunk))
            }

            emit(ChatStreamEvent.StreamEnd)
        } catch (e: Exception) {
            Logger.e("Chat error", e)
            emit(ChatStreamEvent.Token("[在线] 已收到您的消息：$newMessage\n"))
            emit(ChatStreamEvent.Token("\n\n提示：连接出现异常，这是本地回显。请检查网络后重试。"))
            emit(ChatStreamEvent.StreamEnd)
        }
    }.flowOn(Dispatchers.IO)

    /** 判断用户指令是否含明确任务（需要规划） */
    private fun shouldPlan(newMessage: String): Boolean = TASK_KEYWORDS.containsMatchIn(newMessage)

    /**
     * 规划轮：非流式不带 tools，让模型拆解任务步骤。
     * 返回步骤列表；失败返回空列表。
     */
    private suspend fun planTask(
        messages: List<ChatMessage>,
        newMessage: String
    ): List<String> {
        return try {
            val planMessages = listOf(ChatMessage("system", PLAN_PROMPT)) +
                messages.takeLast(4) +
                ChatMessage("user", newMessage)
            val result = gatewayClient.chatCompletion(planMessages)  // 不带 tools
            parsePlan(result.content)
        } catch (e: Exception) {
            Logger.e("Plan failed", e)
            emptyList()
        }
    }

    /** 解析计划 JSON {"plan":[...]} */
    private fun parsePlan(content: String): List<String> {
        if (content.isBlank()) return emptyList()
        return try {
            val start = content.indexOf('{')
            val end = content.lastIndexOf('}')
            if (start < 0 || end <= start) return emptyList()
            val json = JSONObject(content.substring(start, end + 1))
            val plan = json.optJSONArray("plan") ?: return emptyList()
            (0 until plan.length()).mapNotNull { i ->
                plan.optString(i).trim().ifEmpty { null }
            }
        } catch (e: Exception) {
            emptyList()
        }
    }

    /**
     * 反思轮：非流式不带 tools，让模型分析工具失败原因并给建议。
     * 返回反思文本；失败返回 null。
     */
    private suspend fun reflectOnFailure(
        messages: List<ChatMessage>,
        failInfo: String
    ): String? {
        return try {
            val reflectMessages = listOf(ChatMessage("system", REFLECT_PROMPT)) +
                messages.takeLast(4) +
                ChatMessage("user", "以下工具执行失败了，请分析原因并给出修正建议：\n$failInfo")
            val result = gatewayClient.chatCompletion(reflectMessages)  // 不带 tools
            parseReflect(result.content)
        } catch (e: Exception) {
            Logger.e("Reflect failed", e)
            null
        }
    }

    /** 解析反思 JSON {"reflection","suggestion"} */
    private fun parseReflect(content: String): String? {
        if (content.isBlank()) return null
        return try {
            val start = content.indexOf('{')
            val end = content.lastIndexOf('}')
            if (start < 0 || end <= start) return null
            val json = JSONObject(content.substring(start, end + 1))
            val reflection = json.optString("reflection", "工具执行失败")
            val suggestion = json.optString("suggestion", "请调整参数后重试")
            "【反思】$reflection\n建议：$suggestion"
        } catch (e: Exception) {
            // 没解析出 JSON，含关键词就用原文
            if (content.contains("建议") || content.contains("原因")) content else null
        }
    }

    /** 判断工具结果是否失败 */
    private fun isToolFailure(toolResult: String): Boolean =
        toolResult.contains("失败") || toolResult.contains("错误") || toolResult.contains("异常")

    /**
     * 执行工具调用，返回 (工具结果字符串, 生成的 File?).
     */
    private fun executeTool(tc: ToolCall): Pair<String, File?> {
        return try {
            when (tc.name) {
                "generate_doc" -> {
                    val args = JSONObject(tc.arguments)
                    val format = args.optString("format", "docx").lowercase()
                    val title = args.optString("title").ifBlank { "文档" }
                    val paragraphs = args.optJSONArray("paragraphs")?.let { arr ->
                        (0 until arr.length()).map { arr.optString(it) }
                    } ?: emptyList()
                    val bullets = args.optJSONArray("bullets")?.let { arr ->
                        (0 until arr.length()).map { arr.optString(it) }
                    } ?: emptyList()

                    when (format) {
                        "pptx" -> {
                            // slides: [{title, bullets[]}]
                            val slides = args.optJSONArray("slides")?.let { arr ->
                                (0 until arr.length()).map { i ->
                                    val s = arr.optJSONObject(i)
                                    val sTitle = s?.optString("title") ?: ""
                                    val sBullets = s?.optJSONArray("bullets")?.let { ba ->
                                        (0 until ba.length()).map { ba.optString(it) }
                                    } ?: emptyList()
                                    Pair(sTitle, sBullets)
                                }
                            } ?: listOf(Pair(title, bullets))
                            val bytes = pptxGenerator.generatePptx(title, slides)
                            val file = pptxGenerator.writeToCache(bytes, title)
                            Pair("已生成演示文稿：${file.name}", file)
                        }
                        "xlsx" -> {
                            val sheetName = args.optString("sheetName", "数据")
                            val headers = args.optJSONArray("headers")?.let { arr ->
                                (0 until arr.length()).map { arr.optString(it) }
                            } ?: emptyList()
                            val rows = args.optJSONArray("rows")?.let { arr ->
                                (0 until arr.length()).map { i ->
                                    arr.optJSONArray(i)?.let { ra ->
                                        (0 until ra.length()).map { ra.optString(it) }
                                    } ?: emptyList()
                                }
                            } ?: emptyList()
                            val bytes = xlsxGenerator.generateXlsx(sheetName, headers, rows)
                            val file = xlsxGenerator.writeToCache(bytes, title)
                            Pair("已生成表格：${file.name}", file)
                        }
                        "pdf" -> {
                            val bytes = pdfGenerator.generatePdf(title, paragraphs, bullets)
                            val file = pdfGenerator.writeToCache(bytes, title)
                            Pair("已生成 PDF：${file.name}", file)
                        }
                        else -> {  // docx 默认
                            val bytes = docxGenerator.generateDocx(title, paragraphs, bullets)
                            val file = docxGenerator.writeToCache(bytes, title)
                            Pair("已生成文档：${file.name}", file)
                        }
                    }
                }
                else -> Pair("错误：工具 ${tc.name} 不存在。可用工具: generate_doc", null)
            }
        } catch (e: Exception) {
            Logger.e("Tool execute failed", e)
            Pair("工具执行失败：${e.message}", null)
        }
    }

    /**
     * 停止当前生成（在线流式由 Flow 取消，保留方法签名兼容）。
     */
    fun stopGeneration() {
        // 在线流式无法强制中断（SSE 由 Flow 取消），保留方法签名
    }

    /**
     * 检查离线可用性（已砍离线，永远 false）。
     */
    suspend fun checkOfflineAvailability(): Boolean = false
}

sealed class ChatStreamEvent {
    data object StreamStart : ChatStreamEvent()
    data class Token(val text: String) : ChatStreamEvent()
    data class Speed(val text: String) : ChatStreamEvent()
    data object ToolCallStarted : ChatStreamEvent()
    data class ToolCall(val name: String, val arguments: String) : ChatStreamEvent()
    data class GeneratedFile(val file: java.io.File) : ChatStreamEvent()
    data object ToolCallEnd : ChatStreamEvent()
    data object StreamEnd : ChatStreamEvent()
    data class Error(val message: String) : ChatStreamEvent()
    data class ThinkingStep(val phase: ThinkingPhase, val label: String, val detail: String? = null) : ChatStreamEvent()
}

/** Agent 思考阶段 */
enum class ThinkingPhase {
    PLANNING,   // 规划中
    EXECUTING,  // 执行工具
    REFLECTING, // 反思
    RETRYING,   // 重试
    DONE        // 完成
}
