package com.localmind.localfile.storage

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(
    name = "localfile_preferences"
)

class PreferencesManager(private val context: Context) {

    companion object {
        private val KEY_THEME_MODE = stringPreferencesKey("theme_mode")
        private val KEY_LAST_CHAT_MODE = stringPreferencesKey("last_chat_mode")
        private val KEY_GATEWAY_HOST = stringPreferencesKey("gateway_host")
        private val KEY_WSS_RELAY = stringPreferencesKey("wss_relay")
        private val KEY_DEVICE_ID = stringPreferencesKey("device_id")
        private val KEY_DEVICE_TOKEN = stringPreferencesKey("device_token")
        private val KEY_DEVICE_NAME = stringPreferencesKey("device_name")
        private val KEY_ACCOUNT_ACCESS_TOKEN = stringPreferencesKey("account_access_token")
        private val KEY_ACCOUNT_REFRESH_TOKEN = stringPreferencesKey("account_refresh_token")
        private val KEY_ACCOUNT_EMAIL = stringPreferencesKey("account_email")
        private val KEY_ACCOUNT_DISPLAY_NAME = stringPreferencesKey("account_display_name")
        private val KEY_PAIRED_DEVICE_ID = stringPreferencesKey("paired_device_id")
        private val KEY_PAIRED_DEVICE_NAME = stringPreferencesKey("paired_device_name")
        private val KEY_TENANT_ID = stringPreferencesKey("tenant_id")
        private val KEY_GATEWAY_TOKEN = stringPreferencesKey("gateway_token")
        private val KEY_OFFLINE_MODEL_ID = stringPreferencesKey("offline_model_id")
        private val KEY_IS_FIRST_LAUNCH = booleanPreferencesKey("is_first_launch")
        private val KEY_LAST_SESSION_ID = stringPreferencesKey("last_session_id")
    }

    // Theme mode
    val themeMode: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_THEME_MODE] ?: "system"
    }

    suspend fun setThemeMode(mode: String) {
        context.dataStore.edit { prefs -> prefs[KEY_THEME_MODE] = mode }
    }

    // Chat mode
    val lastChatMode: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_LAST_CHAT_MODE] ?: "online"
    }

    suspend fun setLastChatMode(mode: String) {
        context.dataStore.edit { prefs -> prefs[KEY_LAST_CHAT_MODE] = mode }
    }

    // Gateway host
    val gatewayHost: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_GATEWAY_HOST] ?: "https://api.deepseek.com"
    }

    suspend fun setGatewayHost(host: String) {
        context.dataStore.edit { prefs -> prefs[KEY_GATEWAY_HOST] = host }
    }

    // WSS relay (已废弃，远程控制功能已移除)
    val wssRelay: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_WSS_RELAY] ?: ""
    }

    suspend fun setWssRelay(relay: String) {
        context.dataStore.edit { prefs -> prefs[KEY_WSS_RELAY] = relay }
    }

    // Device ID
    val deviceId: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_DEVICE_ID] ?: ""
    }

    suspend fun setDeviceId(id: String) {
        context.dataStore.edit { prefs -> prefs[KEY_DEVICE_ID] = id }
    }

    val deviceToken: Flow<String> = encryptedFlow(KEY_DEVICE_TOKEN.name, mirrorPlaintext = true)

    suspend fun setDeviceCredentials(id: String, token: String) {
        context.dataStore.edit { prefs -> prefs[KEY_DEVICE_ID] = id }
        setEncrypted(KEY_DEVICE_TOKEN.name, token, mirrorPlaintext = true)
    }

    // Account session（access/refresh token 属于凭据，加密存储）
    val accountAccessToken: Flow<String> = encryptedFlow(KEY_ACCOUNT_ACCESS_TOKEN.name, mirrorPlaintext = true)

    val accountRefreshToken: Flow<String> = encryptedFlow(KEY_ACCOUNT_REFRESH_TOKEN.name, mirrorPlaintext = true)

    val accountEmail: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_ACCOUNT_EMAIL] ?: ""
    }

    val accountDisplayName: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_ACCOUNT_DISPLAY_NAME] ?: ""
    }

    suspend fun setAccountSession(
        accessToken: String,
        refreshToken: String,
        email: String,
        displayName: String
    ) {
        context.dataStore.edit { prefs ->
            prefs[KEY_ACCOUNT_EMAIL] = email
            prefs[KEY_ACCOUNT_DISPLAY_NAME] = displayName
        }
        setEncrypted(KEY_ACCOUNT_ACCESS_TOKEN.name, accessToken, mirrorPlaintext = true)
        setEncrypted(KEY_ACCOUNT_REFRESH_TOKEN.name, refreshToken, mirrorPlaintext = true)
    }

    suspend fun clearAccountSession() {
        context.dataStore.edit { prefs ->
            prefs.remove(KEY_ACCOUNT_ACCESS_TOKEN)
            prefs.remove(KEY_ACCOUNT_REFRESH_TOKEN)
            prefs.remove(KEY_ACCOUNT_EMAIL)
            prefs.remove(KEY_ACCOUNT_DISPLAY_NAME)
        }
        setEncrypted(KEY_ACCOUNT_ACCESS_TOKEN.name, "")
        setEncrypted(KEY_ACCOUNT_REFRESH_TOKEN.name, "")
    }

    // Device name
    val deviceName: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_DEVICE_NAME] ?: android.os.Build.MODEL
    }

    suspend fun setDeviceName(name: String) {
        context.dataStore.edit { prefs -> prefs[KEY_DEVICE_NAME] = name }
    }

    // Paired device
    val pairedDeviceId: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_PAIRED_DEVICE_ID] ?: ""
    }

    val pairedDeviceName: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_PAIRED_DEVICE_NAME] ?: ""
    }

    suspend fun setPairedDevice(id: String, name: String) {
        context.dataStore.edit { prefs ->
            prefs[KEY_PAIRED_DEVICE_ID] = id
            prefs[KEY_PAIRED_DEVICE_NAME] = name
        }
    }

    suspend fun clearPairedDevice() {
        context.dataStore.edit { prefs ->
            prefs.remove(KEY_PAIRED_DEVICE_ID)
            prefs.remove(KEY_PAIRED_DEVICE_NAME)
        }
    }

    // Tenant ID
    val tenantId: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_TENANT_ID] ?: ""
    }

    suspend fun setTenantId(id: String) {
        context.dataStore.edit { prefs -> prefs[KEY_TENANT_ID] = id }
    }

    // Gateway token（用户自填的 DeepSeek API Key）— 加密存储
    val gatewayToken: Flow<String> = encryptedFlow(KEY_GATEWAY_TOKEN.name)

    suspend fun setGatewayToken(token: String) {
        setEncrypted(KEY_GATEWAY_TOKEN.name, token)
    }

    // ===== 敏感字段的加密读写 =====
    //
    // EncryptedSharedPreferences（AES-256-GCM，密钥存 Android Keystore）。
    // 兼容策略：
    //  - 首次读到旧版本写入 DataStore 的明文 → 迁移进加密存储并删除明文；
    //  - 加密层不可用（Keystore 异常/恢复出厂）时退回 DataStore，保证功能不中断。
    private fun encryptedFlow(key: String, mirrorPlaintext: Boolean = false): Flow<String> = flow {
        val encrypted = runCatching { EncryptedPrefs.getString(context, key) }.getOrNull().orEmpty()
        if (encrypted.isNotEmpty()) {
            emit(encrypted)
            return@flow
        }
        // 密文读不到（首次迁移、Keystore 失效、写入未落盘）→ 退回 DataStore 明文
        val legacyKey = stringPreferencesKey(key)
        val legacy = context.dataStore.data.map { it[legacyKey] ?: "" }.first()
        if (legacy.isNotEmpty()) {
            val migrated = runCatching { EncryptedPrefs.putString(context, key, legacy) }.getOrDefault(false)
            // 设备/账号 token 保留明文副本作为兜底（丢了会让手机被当成新设备重新注册）；
            // DeepSeek Key 不保留副本，迁移成功即清除明文。
            if (migrated && !mirrorPlaintext) {
                context.dataStore.edit { prefs -> prefs.remove(legacyKey) }
            }
        }
        emit(legacy)
    }

    private suspend fun setEncrypted(key: String, value: String, mirrorPlaintext: Boolean = false) {
        val legacyKey = stringPreferencesKey(key)
        if (value.isEmpty()) {
            runCatching { EncryptedPrefs.remove(context, key) }
            context.dataStore.edit { prefs -> prefs.remove(legacyKey) }
            return
        }
        val encryptedOk = runCatching { EncryptedPrefs.putString(context, key, value) }.getOrDefault(false)
        context.dataStore.edit { prefs ->
            // mirrorPlaintext=true：始终保留一份明文兜底，避免密文层异常导致凭据彻底丢失；
            // mirrorPlaintext=false（DeepSeek Key）：加密成功就清掉明文。
            if (encryptedOk && !mirrorPlaintext) prefs.remove(legacyKey) else prefs[legacyKey] = value
        }
    }

    // Offline model
    val offlineModelId: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_OFFLINE_MODEL_ID] ?: ""
    }

    suspend fun setOfflineModelId(id: String) {
        context.dataStore.edit { prefs -> prefs[KEY_OFFLINE_MODEL_ID] = id }
    }

    // First launch
    val isFirstLaunch: Flow<Boolean> = context.dataStore.data.map { prefs ->
        prefs[KEY_IS_FIRST_LAUNCH] ?: true
    }

    suspend fun setFirstLaunchComplete() {
        context.dataStore.edit { prefs -> prefs[KEY_IS_FIRST_LAUNCH] = false }
    }

    // Last session
    val lastSessionId: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_LAST_SESSION_ID] ?: ""
    }

    suspend fun setLastSessionId(id: String) {
        context.dataStore.edit { prefs -> prefs[KEY_LAST_SESSION_ID] = id }
    }
}
