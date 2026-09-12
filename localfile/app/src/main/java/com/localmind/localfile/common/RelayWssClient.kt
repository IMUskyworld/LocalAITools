package com.localmind.localfile.common

import android.content.Context
import okhttp3.Request
import okio.ByteString
import org.json.JSONObject
import java.util.concurrent.TimeUnit

/**
 * Relay WSS 客户端 — 连接 Relay，发送远程命令，接收状态回传。
 * 复用 AccountClient 的 TLS 配置（RelayTls）。
 */
class RelayWssClient(
    context: Context,
    private val baseUrl: String = AccountClient.DEFAULT_RELAY_URL
) {
    private val client = RelayTls.relayHttpClient(context, connectTimeoutSeconds = 15, readTimeoutSeconds = 120, writeTimeoutSeconds = 30)
    private var ws: okhttp3.WebSocket? = null

    data class StateUpdate(
        val commandId: String,
        val state: String,
        val resultText: String?
    )

    /**
     * 连接到 Relay WSS 并发送命令。
     * @param deviceId 本机设备 ID
     * @param deviceToken 本机设备 token
     * @param targetDeviceId 目标 Windows 设备 ID
     * @param intentText 用户的自然语言指令
     * @param onStateUpdate 收到状态更新时回调（running/done/failed）
     * @param onConnected 连接成功回调
     * @param onError 错误回调
     */
    fun connectAndSendCommand(
        deviceId: String,
        deviceToken: String,
        targetDeviceId: String,
        intentText: String,
        onStateUpdate: (StateUpdate) -> Unit,
        onConnected: () -> Unit,
        onError: (String) -> Unit
    ) {
        val wsUrl = baseUrl.replace("https://", "wss://").replace("http://", "ws://") + "/ws"
        val request = Request.Builder()
            .url(wsUrl)
            .header("x-device-id", deviceId)
            .header("Authorization", "Bearer $deviceToken")
            .build()

        ws = client.newWebSocket(request, object : okhttp3.WebSocketListener() {
            override fun onOpen(webSocket: okhttp3.WebSocket, response: okhttp3.Response) {
                onConnected()
                // 发送心跳
                // 发送命令
                val envelope = JSONObject().apply {
                    put("version", "v1")
                    put("id", java.util.UUID.randomUUID().toString())
                    put("type", "command")
                    put("from_device_id", deviceId)
                    put("to_device_id", targetDeviceId)
                    put("action_type", "chat_task")
                    put("intent_text", intentText)
                    put("command_id", java.util.UUID.randomUUID().toString())
                    put("timestamp", System.currentTimeMillis())
                }
                webSocket.send(envelope.toString())
            }

            override fun onMessage(webSocket: okhttp3.WebSocket, text: String) {
                try {
                    val json = JSONObject(text)
                    val type = json.optString("type")
                    if (type == "ack" || type == "state") {
                        val cmdId = json.optString("command_id")
                        val state = json.optString("state", "unknown")
                        val result = json.optString("result_text", null)
                        if (cmdId.isNotEmpty()) {
                            onStateUpdate(StateUpdate(cmdId, state, result))
                        }
                        // 收到 done/failed 后关闭连接
                        if (state == "done" || state == "failed") {
                            webSocket.close(1000, "command completed")
                        }
                    }
                } catch (_: Exception) {}
            }

            override fun onFailure(webSocket: okhttp3.WebSocket, t: Throwable, response: okhttp3.Response?) {
                onError(t.message ?: "WebSocket error")
            }

            override fun onClosed(webSocket: okhttp3.WebSocket, code: Int, reason: String) {
                // 正常关闭
            }
        })
    }

    fun disconnect() {
        ws?.close(1000, "user disconnect")
        ws = null
    }
}
