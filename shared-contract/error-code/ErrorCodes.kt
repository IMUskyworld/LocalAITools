package com.localmind.localfile.common

/**
 * 全局错误码常量 — Kotlin 端
 * 对齐 shared-contract/error-code/registry.json
 */
object ErrorCodes {
    // ===== 通用 1xxxxx =====
    const val UNKNOWN = "100001"
    const val INTERNAL_ERROR = "100002"
    const val NETWORK_UNREACHABLE = "100003"
    const val REQUEST_TIMEOUT = "100004"
    const val RATE_LIMITED = "100005"

    // ===== 对话 2xxxxx =====
    const val GATEWAY_ERROR = "201001"
    const val TOKEN_QUOTA_EXHAUSTED = "201002"
    const val OFFLINE_MODEL_NOT_READY = "201003"
    const val ONLINE_SERVICE_UNAVAILABLE = "201004"
    const val MODEL_SWITCHING = "201005"

    // ===== 模型 3xxxxx =====
    const val DOWNLOAD_FAILED = "301001"
    const val CHECKSUM_MISMATCH = "301002"
    const val INSUFFICIENT_STORAGE = "301003"
    const val DEVICE_NOT_COMPATIBLE = "301004"
    const val MODEL_CORRUPTED = "301005"
    const val INFERENCE_START_FAILED = "301006"
    const val INFERENCE_ABNORMAL = "301007"

    // ===== 远程控制 4xxxxx =====
    const val NOT_PAIRED = "401001"
    const val PAIR_CODE_INVALID = "401002"
    const val PAIR_CODE_USED = "401003"
    const val PAIR_CODE_GENERATION_FAILED = "401004"
    const val REMOTE_OFFLINE = "402001"
    const val NOT_IN_WHITELIST = "402002"
    const val DANGEROUS_ACTION_REJECTED = "402003"
    const val REMOTE_CONTROL_DISABLED = "402004"
    const val AUTH_TICKET_INVALID = "403001"
    const val UNAUTHORIZED = "403002"

    val errorMessages: Map<String, String> = mapOf(
        UNKNOWN to "未知错误",
        GATEWAY_ERROR to "网关异常",
        TOKEN_QUOTA_EXHAUSTED to "本月额度已用完",
        OFFLINE_MODEL_NOT_READY to "离线模型未就绪",
        ONLINE_SERVICE_UNAVAILABLE to "在线服务暂时不可用",
        DOWNLOAD_FAILED to "下载失败",
        CHECKSUM_MISMATCH to "文件校验未通过",
        INSUFFICIENT_STORAGE to "存储空间不足",
        DEVICE_NOT_COMPATIBLE to "设备不满足运行要求",
        NOT_PAIRED to "设备未配对",
        PAIR_CODE_INVALID to "配对码无效或已过期",
        REMOTE_OFFLINE to "电脑当前离线",
        NOT_IN_WHITELIST to "该操作不在允许范围",
        DANGEROUS_ACTION_REJECTED to "危险操作已被拒绝",
    )

    fun getMessage(code: String): String =
        errorMessages[code] ?: "未知错误 ($code)"
}
