package com.localmind.localfile.inference

/**
 * JNI bridge class for llama.cpp.
 * This is a stub implementation. Native methods will link to libllama.so
 * when the native library is available.
 */
class LlamaBridge {

    companion object {
        private const val TAG = "LlamaBridge"
        private var isLoaded = false

        /**
         * Attempt to load the native library.
         * Returns true if libllama.so was successfully loaded.
         */
        fun tryLoad(): Boolean {
            if (isLoaded) return true
            return try {
                System.loadLibrary("llama")
                isLoaded = true
                true
            } catch (e: UnsatisfiedLinkError) {
                // Native library not available — running with stubs
                isLoaded = false
                false
            }
        }

        fun isNativeAvailable(): Boolean = isLoaded
    }

    // Native method declarations
    external fun create(modelPath: String, nCtx: Int): Long
    external fun destroy(handle: Long): Boolean
    external fun reset(handle: Long): Boolean
    external fun evaluate(handle: Long, tokens: IntArray): IntArray
    external fun tokenize(handle: Long, text: String): IntArray
    external fun detokenize(handle: Long, tokens: IntArray): String
    external fun getSpeed(handle: Long): Float

    // Stub implementation for when native library is not available
    private var stubContext: StubContext? = null

    fun createStub(modelPath: String, nCtx: Int = 2048): Long {
        stubContext = StubContext(modelPath, nCtx)
        return 1L // dummy handle
    }

    fun destroyStub(handle: Long): Boolean {
        stubContext = null
        return true
    }

    fun evaluateStub(handle: Long, tokens: IntArray): IntArray {
        // Stub: returns empty result
        return intArrayOf()
    }

    fun tokenizeStub(handle: Long, text: String): IntArray {
        // Stub: simple character-level tokenization approximation
        return text.codePoints().toArray()
    }

    fun detokenizeStub(handle: Long, tokens: IntArray): String {
        // Stub: convert code points back to string
        return String(tokens, 0, tokens.size)
    }

    fun getSpeedStub(handle: Long): Float {
        return 0f
    }

    private data class StubContext(
        val modelPath: String,
        val nCtx: Int
    )
}
