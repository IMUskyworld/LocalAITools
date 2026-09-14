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

    /**
     * 写入并【同步】落盘（commit 而不是 apply）。
     *
     * 之前用 apply() 是异步写：紧接着删除明文副本的话，进程一旦被强杀，
     * 密文可能还没落盘，凭据就彻底丢了 —— 表现为「每次登录都新注册一台设备」。
     * 返回值表示是否写入成功，调用方可据此决定要不要保留明文兜底。
     */
    fun putString(context: Context, key: String, value: String): Boolean =
        get(context).edit().putString(key, value).commit()

    fun remove(context: Context, key: String) {
        get(context).edit().remove(key).apply()
    }
}
