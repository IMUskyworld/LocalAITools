package com.localmind.localfile.inference

import com.localmind.localfile.common.InferenceException
import com.localmind.localfile.common.Logger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.util.UUID
import kotlin.coroutines.coroutineContext

/**
 * High-level inference interface for llama.cpp.
 * Supports loadModel, generate (streaming), stop, and getSpeed.
 */
class InferenceEngine {

    private var handle: Long = 0L
    private var nativeMode: Boolean = false
    private var isRunning = false
    private var isLoaded = false
    private val mutex = Mutex()
    private val bridge = LlamaBridge()

    val isModelLoaded: Boolean get() = isLoaded
    val currentSpeed: Float get() = if (isLoaded) {
        if (nativeMode) bridge.getSpeed(handle) else bridge.getSpeedStub(handle)
    } else 0f

    /**
     * Load a model from the given path.
     * Tries native JNI first, falls back to stub.
     */
    suspend fun loadModel(modelPath: String, nCtx: Int = 2048): Boolean = mutex.withLock {
        try {
            if (isLoaded) unloadModel()

            if (LlamaBridge.tryLoad()) {
                handle = bridge.create(modelPath, nCtx)
                nativeMode = true
            } else {
                handle = bridge.createStub(modelPath, nCtx)
                nativeMode = false
                Logger.w("Native library not available, using stub inference")
            }

            isLoaded = handle != 0L
            isLoaded
        } catch (e: Exception) {
            Logger.e("Failed to load model", e)
            isLoaded = false
            false
        }
    }

    /**
     * Unload the current model.
     */
    suspend fun unloadModel() = mutex.withLock {
        if (handle != 0L) {
            if (nativeMode) bridge.destroy(handle) else bridge.destroyStub(handle)
            handle = 0L
        }
        isLoaded = false
        isRunning = false
    }

    /**
     * Generate text from a prompt with streaming tokens.
     * Returns a Flow of generated text chunks.
     */
    suspend fun generate(
        prompt: String,
        maxTokens: Int = 512
    ): Flow<String> = flow {
        if (!isLoaded) {
            throw InferenceException("离线模型未加载")
        }

        isRunning = true
        val sessionId = UUID.randomUUID().toString().take(8)
        Logger.d("Generation started: id=$sessionId")

        try {
            val tokens = if (nativeMode) {
                bridge.tokenize(handle, prompt)
            } else {
                bridge.tokenizeStub(handle, prompt)
            }

            var generatedCount = 0
            while (isRunning && generatedCount < maxTokens && coroutineContext.isActive) {
                val outputTokens = if (nativeMode) {
                    bridge.evaluate(handle, tokens)
                } else {
                    bridge.evaluateStub(handle, tokens)
                }

                if (outputTokens.isEmpty()) break

                val text = if (nativeMode) {
                    bridge.detokenize(handle, outputTokens)
                } else {
                    bridge.detokenizeStub(handle, outputTokens)
                }

                emit(text)
                generatedCount += outputTokens.size
            }
        } catch (e: Exception) {
            Logger.e("Generation error: ${e.message}")
            if (e !is InferenceException) {
                throw InferenceException("Generation failed: ${e.message}", cause = e)
            }
            throw e
        } finally {
            isRunning = false
            Logger.d("Generation ended: id=$sessionId")
        }
    }.flowOn(Dispatchers.IO)

    /**
     * Stop the current generation.
     */
    fun stop() {
        isRunning = false
    }
}
