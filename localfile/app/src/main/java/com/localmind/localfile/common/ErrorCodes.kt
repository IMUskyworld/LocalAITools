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

    // Relay (50xxxx)
    const val PAIRING_SERVICE_ERROR = "501001"
    const val DEVICE_REGISTRATION_FAILED = "501002"
    const val COMMAND_ROUTING_FAILED = "502001"
    const val COMMAND_EXECUTION_TIMEOUT = "502002"

    // AI Channel (60xxxx)
    const val AI_CHANNEL_ERROR = "601001"
    const val TOKEN_AUTH_FAILED = "601002"
    const val MODEL_MAPPING_NOT_CONFIGURED = "602001"
}
