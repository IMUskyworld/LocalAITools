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
        private val KEY_DEVICE_NAME = stringPreferencesKey("device_name")
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

    // Gateway token
    val gatewayToken: Flow<String> = context.dataStore.data.map { prefs ->
        prefs[KEY_GATEWAY_TOKEN] ?: ""
    }

    suspend fun setGatewayToken(token: String) {
        context.dataStore.edit { prefs -> prefs[KEY_GATEWAY_TOKEN] = token }
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
