package com.localmind.localfile.storage

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * 加密存储层 — 用 EncryptedSharedPreferences 保护敏感数据（API key、token）。
 * 对上层透明：读写接口与普通 SharedPreferences 相同。
 */
object EncryptedPrefs {
    private const val FILE_NAME = "localmind_secure_prefs"

    private fun get(context: Context): SharedPreferences {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        return EncryptedSharedPreferences.create(
            context,
            FILE_NAME,
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
    }

    fun getString(context: Context, key: String, default: String = ""): String {
        return get(context).getString(key, default) ?: default
    }

    fun putString(context: Context, key: String, value: String) {
        get(context).edit().putString(key, value).apply()
    }

    fun remove(context: Context, key: String) {
        get(context).edit().remove(key).apply()
    }
}
