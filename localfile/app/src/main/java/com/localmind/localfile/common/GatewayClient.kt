package com.localmind.localfile.common

import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.suspendCancellableCoroutine
import okhttp3.Call
import okhttp3.Callback
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.sse.EventSource
import okhttp3.sse.EventSourceListener
import okhttp3.sse.EventSources
import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

class GatewayClient(
    private val baseUrl: String = DeepSeekConfig.BASE_URL
) {
    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .writeTimeout(30, TimeUnit.SECONDS)
        .build()

    private val jsonMediaType = "application/json; charset=utf-8".toMediaType()

    suspend fun chatCompletion(
        messages: List<ChatMessage>,
        model: String = DeepSeekConfig.MODEL,
        token: String = DeepSeekConfig.API_KEY,
        onStream: ((String) -> Unit)? = null,
        tools: JSONArray? = null
    ): ChatResult = suspendCancellableCoroutine { continuation ->
        // 注意：不能用 put(key, List)——Android 系统精简版 org.json 没有该重载，会 NoSuchMethodError
        val messagesJson = JSONArray().apply {
            messages.forEach { put(it.toJson()) }
        }
        val requestBody = JSONObject().apply {
            put("model", model)
            put("messages", messagesJson)
            put("stream", onStream != null)
            tools?.let { put("tools", it) }
        }

        val request = Request.Builder()
            .url("$baseUrl/chat/completions")
            .addHeader("Content-Type", "application/json")
            .apply {
                if (token.isNotEmpty()) addHeader("Authorization", "Bearer $token")
            }
            .post(requestBody.toString().toRequestBody(jsonMediaType))
            .build()

        if (onStream != null) {
            val factory = EventSources.createFactory(client)
            val listener = object : EventSourceListener() {
                override fun onEvent(eventSource: EventSource, id: String?, type: String?, data: String) {
                    if (data == "[DONE]") return
                    try {
                        val json = JSONObject(data)
                        val choicesArray = json.optJSONArray("choices")
                        val delta = if (choicesArray != null && choicesArray.length() > 0) {
                            choicesArray.optJSONObject(0)?.optJSONObject("delta")
                        } else null
                        // 注意：推理模型的 delta 里 content 可能是 JSON null，
                        // org.json 的 optString 会把 JSONObject.NULL 转成字符串 "null"，
                        // 必须先判 isNull 再取值，否则回复开头会出现一串 "null"。
                        val content = if (delta != null && !delta.isNull("content")) {
                            delta.optString("content", "")
                        } else ""
                    } catch (e: Exception) {
                        Logger.w("SSE parse error: ${e.message}")
                    }
                }

                override fun onFailure(eventSource: EventSource, t: Throwable?, response: Response?) {
                    if (!continuation.isCancelled) {
                        continuation.resumeWithException(
                            GatewayException("SSE connection failed: ${t?.message}", cause = t)
                        )
                    }
                }

                override fun onClosed(eventSource: EventSource) {
                    if (!continuation.isCancelled) {
                        continuation.resume(ChatResult("", ""))
                    }
                }
            }
            factory.newEventSource(request, listener)
        } else {
            client.newCall(request).enqueue(object : Callback {
                override fun onFailure(call: Call, e: IOException) {
                    if (!continuation.isCancelled) {
                        continuation.resumeWithException(GatewayException("Request failed: ${e.message}", cause = e))
                    }
                }

                override fun onResponse(call: Call, response: Response) {
                    try {
                        val body = response.body?.string() ?: ""
                        if (!response.isSuccessful) {
                            // 解析 DeepSeek 的错误信息，如 401 无效 key / 402 余额不足 / 429 限流
                            val msg = try {
                                JSONObject(body).optJSONObject("error")?.optString("message") ?: body.take(200)
                            } catch (e: Exception) {
                                body.take(200)
                            }
                            if (!continuation.isCancelled) {
                                continuation.resumeWithException(
                                    GatewayException("DeepSeek API 错误 ${response.code}: $msg")
                                )
                            }
                            return
                        }
                        val json = JSONObject(body)
                        val choicesArray = json.optJSONArray("choices")
                        val message = if (choicesArray != null && choicesArray.length() > 0) {
                            choicesArray.optJSONObject(0)?.optJSONObject("message")
                        } else null
                        val content = if (message != null && !message.isNull("content")) {
                            message.optString("content", "")
                        } else ""
                        val finishReason = if (choicesArray != null && choicesArray.length() > 0) {
                            choicesArray.optJSONObject(0)?.optString("finish_reason", "") ?: ""
                        } else ""
                        // 解析 tool_calls
                        val toolCalls = message?.optJSONArray("tool_calls")?.let { arr ->
                            (0 until arr.length()).mapNotNull { i ->
                                val tc = arr.optJSONObject(i)
                                val fn = tc?.optJSONObject("function")
                                if (fn != null) {
                                    ToolCall(
                                        id = tc.optString("id", ""),
                                        name = fn.optString("name", ""),
                                        arguments = fn.optString("arguments", "{}")
                                    )
                                } else null
                            }
                        }
                        continuation.resume(ChatResult(content, finishReason, toolCalls))
                    } catch (e: Exception) {
                        if (!continuation.isCancelled) {
                            continuation.resumeWithException(GatewayException("Parse error: ${e.message}", cause = e))
                        }
                    }
                }
            })
        }
    }

    fun chatCompletionStream(
        messages: List<ChatMessage>,
        model: String = DeepSeekConfig.MODEL,
        token: String = DeepSeekConfig.API_KEY
    ): Flow<String> = callbackFlow {
        // 注意：不能用 put(key, List)——Android 系统精简版 org.json 没有该重载，会 NoSuchMethodError
        val messagesJson = JSONArray().apply {
            messages.forEach { put(it.toJson()) }
        }
        val requestBody = JSONObject().apply {
            put("model", model)
            put("messages", messagesJson)
            put("stream", true)
        }

        val request = Request.Builder()
            .url("$baseUrl/chat/completions")
            .addHeader("Content-Type", "application/json")
            .apply {
                if (token.isNotEmpty()) addHeader("Authorization", "Bearer $token")
            }
            .post(requestBody.toString().toRequestBody(jsonMediaType))
            .build()

        val factory = EventSources.createFactory(client)
        val listener = object : EventSourceListener() {
            override fun onEvent(eventSource: EventSource, id: String?, type: String?, data: String) {
                if (data == "[DONE]") {
                    close()
                    return
                }
                try {
                    val json = JSONObject(data)
                    val choicesArray = json.optJSONArray("choices")
                    val delta = if (choicesArray != null && choicesArray.length() > 0) {
                        choicesArray.optJSONObject(0)?.optJSONObject("delta")
                    } else null
                    val content = if (delta != null && !delta.isNull("content")) {
                        delta.optString("content", "")
                    } else ""
                    if (content.isNotEmpty()) trySend(content)
                } catch (e: Exception) {
                    Logger.w("SSE parse error: ${e.message}")
                }
            }

            override fun onFailure(eventSource: EventSource, t: Throwable?, response: Response?) {
                close(t ?: IOException("SSE failed"))
            }

            override fun onClosed(eventSource: EventSource) {
                close()
            }
        }

        val eventSource = factory.newEventSource(request, listener)

        awaitClose {
            eventSource.cancel()
        }
    }
}

data class ChatMessage(
    val role: String,
    val content: String,
    val toolCallId: String? = null,
    val name: String? = null,
    val toolCalls: List<ToolCall>? = null
) {
    fun toJson(): JSONObject = JSONObject().apply {
        put("role", role)
        put("content", content)
        toolCallId?.let { put("tool_call_id", it) }
        name?.let { put("name", it) }
        if (toolCalls != null) {
            val arr = JSONArray()
            toolCalls.forEach { tc ->
                arr.put(JSONObject().apply {
                    put("id", tc.id)
                    put("type", "function")
                    put("function", JSONObject().apply {
                        put("name", tc.name)
                        put("arguments", tc.arguments)
                    })
                })
            }
            put("tool_calls", arr)
        }
    }
}

data class ToolCall(
    val id: String,
    val name: String,
    val arguments: String
)

data class ChatResult(
    val content: String,
    val finishReason: String,
    val toolCalls: List<ToolCall>? = null
)
