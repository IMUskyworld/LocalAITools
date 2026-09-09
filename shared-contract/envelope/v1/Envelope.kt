package com.localmind.localfile.common

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

/**
 * WSS 指令信封 v1 — Kotlin 类型定义（LocalFile Android 端使用）
 * 对齐 shared-contract/envelope/v1/schema.json
 */
@Serializable
data class CommandEnvelope(
    val version: String = "v1",
    val id: String,
    @SerialName("type")
    val type: EnvelopeType,
    @SerialName("from_device_id")
    val fromDeviceId: String,
    @SerialName("to_device_id")
    val toDeviceId: String = "",
    @SerialName("tenant_id")
    val tenantId: String = "",
    @SerialName("command_id")
    val commandId: String = "",
    @SerialName("intent_text")
    val intentText: String = "",
    @SerialName("action_type")
    val actionType: String = "",
    @SerialName("action_params")
    val actionParams: JsonObject? = null,
    val state: CommandState? = null,
    @SerialName("result_text")
    val resultText: String = "",
    @SerialName("error_code")
    val errorCode: String = "",
    val timestamp: Long = System.currentTimeMillis(),
    @SerialName("pairing_code")
    val pairingCode: String = "",
    @SerialName("device_info")
    val deviceInfo: DeviceInfo? = null
)

@Serializable
enum class EnvelopeType {
    @SerialName("command") COMMAND,
    @SerialName("ack") ACK,
    @SerialName("state") STATE,
    @SerialName("pair") PAIR,
    @SerialName("pair_confirm") PAIR_CONFIRM,
    @SerialName("heartbeat") HEARTBEAT,
    @SerialName("error") ERROR
}

@Serializable
enum class CommandState {
    @SerialName("sent") SENT,
    @SerialName("delivered") DELIVERED,
    @SerialName("running") RUNNING,
    @SerialName("done") DONE,
    @SerialName("failed") FAILED
}

@Serializable
data class DeviceInfo(
    @SerialName("device_name")
    val deviceName: String,
    val platform: String,
    val model: String = "",
    @SerialName("os_version")
    val osVersion: String = ""
)

fun createAck(original: CommandEnvelope): CommandEnvelope {
    return CommandEnvelope(
        id = java.util.UUID.randomUUID().toString(),
        type = EnvelopeType.ACK,
        fromDeviceId = original.toDeviceId,
        toDeviceId = original.fromDeviceId,
        tenantId = original.tenantId,
        commandId = original.commandId,
        timestamp = System.currentTimeMillis()
    )
}

fun createHeartbeat(deviceId: String): CommandEnvelope {
    return CommandEnvelope(
        id = java.util.UUID.randomUUID().toString(),
        type = EnvelopeType.HEARTBEAT,
        fromDeviceId = deviceId,
        timestamp = System.currentTimeMillis()
    )
}
