package com.localmind.localfile.common

import android.util.Log

/**
 * Logging utility for consistent log output.
 */
object Logger {
    private const val TAG = "LocalFile"

    fun d(message: String, tag: String = TAG) {
        Log.d(tag, message)
    }

    fun i(message: String, tag: String = TAG) {
        Log.i(tag, message)
    }

    fun w(message: String, tag: String = TAG) {
        Log.w(tag, message)
    }

    fun e(message: String, throwable: Throwable? = null, tag: String = TAG) {
        Log.e(tag, message, throwable)
    }

    fun e(throwable: Throwable, tag: String = TAG) {
        Log.e(tag, throwable.message ?: "Unknown error", throwable)
    }
}
