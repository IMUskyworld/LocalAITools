package com.localmind.localfile.common

/**
 * Error code constants matching shared-contract error-code registry.
 * Format: 6-digit [module][category][sequence]
 */
object ErrorCodes {
    // General (1xxxxx)
    const val UNKNOWN = "100001"
    const val INTERNAL_ERROR = "100002"
    const val NETWORK_UNREACHABLE = "100003"
    const val REQUEST_TIMEOUT = "100004"
    const val RATE_LIMITED = "100005"

    // Gateway/Chat (20xxxx)
    const val GATEWAY_ERROR = "201001"
    const val TOKEN_QUOTA_EXHAUSTED = "201002"
    const val OFFLINE_MODEL_NOT_READY = "201003"
    const val ONLINE_SERVICE_UNAVAILABLE = "201004"
    const val MODEL_SWITCHING = "201005"

    // Message (202xxx)
    const val CONTEXT_TOO_LONG = "202001"
    const val MESSAGE_SEND_FAILED = "202002"

    // Model (301xxx)
    const val MODEL_DOWNLOAD_FAILED = "301001"
    const val MODEL_CHECKSUM_MISMATCH = "301002"
    const val INSUFFICIENT_STORAGE = "301003"
    const val DEVICE_BELOW_MINIMUM = "301004"
    const val MODEL_FILE_CORRUPTED = "301005"
    const val INFERENCE_START_FAILED = "301006"
    const val INFERENCE_ABNORMAL = "301007"

    // Remote Control (40xxxx)
    const val DEVICE_NOT_PAIRED = "401001"
    const val PAIRING_CODE_INVALID = "401002"
    const val PAIRING_CODE_USED = "401003"
    const val PAIRING_CODE_FAILED = "401004"
    const val REMOTE_DEVICE_OFFLINE = "402001"
    const val ACTION_NOT_IN_WHITELIST = "402002"
    const val DANGEROUS_ACTION_REJECTED = "402003"
    const val REMOTE_CONTROL_DISABLED = "402004"
    const val AUTH_TICKET_INVALID = "403001"
    const val UNAUTHORIZED_ACCESS = "403002"
    const val EMAIL_ALREADY_REGISTERED = "403003"
    const val INVALID_CREDENTIALS = "403004"
    const val TOKEN_INVALID_OR_EXPIRED = "403005"
    const val ACCOUNT_DISABLED = "403006"
    const val DEVICE_NOT_ENROLLED = "403007"
    const val PAIRING_REQUEST_EXPIRED = "403008"
    const val DEVICE_ALREADY_ENROLLED = "403009"
    const val PAIRING_REQUEST_CONFLICT = "403010"

    // Relay (50xxxx)
    const val PAIRING_SERVICE_ERROR = "501001"
    const val DEVICE_REGISTRATION_FAILED = "501002"
    const val COMMAND_ROUTING_FAILED = "502001"
    const val COMMAND_EXECUTION_TIMEOUT = "502002"

    // AI Channel (60xxxx)
    const val AI_CHANNEL_ERROR = "601001"
    const val TOKEN_AUTH_FAILED = "601002"
    const val MODEL_MAPPING_NOT_CONFIGURED = "602001"

    private val messages: Map<String, String> = mapOf(
        UNKNOWN to "未知错误",
        NETWORK_UNREACHABLE to "网络不可达",
        REQUEST_TIMEOUT to "请求超时",
        GATEWAY_ERROR to "网关异常",
        TOKEN_QUOTA_EXHAUSTED to "令牌额度已耗尽",
        OFFLINE_MODEL_NOT_READY to "离线模型未就绪",
        ONLINE_SERVICE_UNAVAILABLE to "在线服务暂时不可用",
        DEVICE_NOT_PAIRED to "设备未配对",
        PAIRING_CODE_INVALID to "配对码无效或已过期",
        PAIRING_CODE_USED to "配对码已使用",
        PAIRING_CODE_FAILED to "配对码签发失败",
        REMOTE_DEVICE_OFFLINE to "对端设备离线",
        ACTION_NOT_IN_WHITELIST to "操作不在白名单范围内",
        DANGEROUS_ACTION_REJECTED to "危险操作已被拒绝",
        REMOTE_CONTROL_DISABLED to "远程控制未开启",
        AUTH_TICKET_INVALID to "认证票据无效或已过期",
        UNAUTHORIZED_ACCESS to "越权访问拒绝",
        EMAIL_ALREADY_REGISTERED to "该邮箱已注册",
        INVALID_CREDENTIALS to "邮箱或密码错误",
        TOKEN_INVALID_OR_EXPIRED to "登录令牌无效或已过期",
        ACCOUNT_DISABLED to "账号已被禁用",
        DEVICE_NOT_ENROLLED to "设备尚未绑定到该账号",
        PAIRING_REQUEST_EXPIRED to "控制授权请求已过期",
        DEVICE_ALREADY_ENROLLED to "设备已绑定到其他账号",
        PAIRING_REQUEST_CONFLICT to "控制授权请求状态冲突"
    )

    fun getMessage(code: String): String = messages[code] ?: "未知错误 ($code)"
}
