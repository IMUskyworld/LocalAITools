package com.localmind.localfile.common

/**
 * Application exception hierarchy with error codes.
 */
sealed class AppException(
    message: String,
    val errorCode: String,
    cause: Throwable? = null
) : Exception(message, cause)

class NetworkException(
    message: String = "Network error",
    errorCode: String = ErrorCodes.NETWORK_UNREACHABLE,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)

class GatewayException(
    message: String = "Gateway error",
    errorCode: String = ErrorCodes.GATEWAY_ERROR,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)

class ChatException(
    message: String,
    errorCode: String = ErrorCodes.MESSAGE_SEND_FAILED,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)

class ModelException(
    message: String,
    errorCode: String = ErrorCodes.MODEL_DOWNLOAD_FAILED,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)

class RemoteControlException(
    message: String,
    errorCode: String = ErrorCodes.DEVICE_NOT_PAIRED,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)

class PairingException(
    message: String,
    errorCode: String = ErrorCodes.PAIRING_CODE_INVALID,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)

class StorageException(
    message: String,
    errorCode: String = ErrorCodes.INSUFFICIENT_STORAGE,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)

class InferenceException(
    message: String,
    errorCode: String = ErrorCodes.INFERENCE_START_FAILED,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)

class FileProcessingException(
    message: String,
    errorCode: String = ErrorCodes.INTERNAL_ERROR,
    cause: Throwable? = null
) : AppException(message, errorCode, cause)
