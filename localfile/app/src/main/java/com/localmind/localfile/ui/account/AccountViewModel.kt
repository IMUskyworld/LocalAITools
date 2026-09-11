package com.localmind.localfile.ui.account

import android.app.Application
import android.os.Build
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.localmind.localfile.common.AccountApiException
import com.localmind.localfile.common.AccountClient
import com.localmind.localfile.common.RelayAccountDevice
import com.localmind.localfile.common.RelayAccountUser
import com.localmind.localfile.common.ErrorCodes
import com.localmind.localfile.storage.PreferencesManager
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class AccountUiState(
    val initializing: Boolean = true,
    val authenticated: Boolean = false,
    val user: RelayAccountUser? = null,
    val devices: List<RelayAccountDevice> = emptyList(),
    val busy: Boolean = false,
    val error: String? = null
)

class AccountViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = PreferencesManager(application)
    private val client = AccountClient()

    private val _state = MutableStateFlow(AccountUiState())
    val state: StateFlow<AccountUiState> = _state.asStateFlow()

    init {
        restoreSession()
    }

    fun authenticate(email: String, password: String, register: Boolean) {
        if (email.isBlank() || password.isBlank()) {
            _state.update { it.copy(error = "请输入邮箱和密码") }
            return
        }
        viewModelScope.launch {
            _state.update { it.copy(busy = true, error = null) }
            try {
                val session = if (register) {
                    client.registerAccount(
                        email.trim(),
                        email.substringBefore('@').ifBlank { "LocalFile 用户" },
                        password
                    )
                } else {
                    client.loginAccount(email.trim(), password)
                }
                prefs.setAccountSession(
                    session.accessToken,
                    session.refreshToken,
                    session.user.email,
                    session.user.displayName
                )
                val device = ensureDevice()
                client.claimDevice(session.accessToken, device.id, device.token)
                val devices = client.listDevices(session.accessToken, device.id, device.token)
                _state.update {
                    it.copy(
                        authenticated = true,
                        user = session.user,
                        devices = devices,
                        busy = false,
                        error = null
                    )
                }
            } catch (error: Exception) {
                _state.update { it.copy(busy = false, error = accountError(error)) }
            }
        }
    }

    fun logout() {
        viewModelScope.launch {
            val refresh = prefs.accountRefreshToken.first()
            if (refresh.isNotBlank()) {
                runCatching { client.logout(refresh) }
            }
            prefs.clearAccountSession()
            _state.value = AccountUiState(initializing = false)
        }
    }

    fun refreshDevices() {
        viewModelScope.launch {
            _state.update { it.copy(busy = true, error = null) }
            try {
                val session = loadSession()
                val device = ensureDevice()
                val devices = client.listDevices(session.access, device.id, device.token)
                _state.update { it.copy(devices = devices, busy = false) }
            } catch (error: Exception) {
                _state.update { it.copy(busy = false, error = accountError(error)) }
            }
        }
    }

    fun removeDevice(deviceId: String) {
        viewModelScope.launch {
            _state.update { it.copy(busy = true, error = null) }
            try {
                val session = loadSession()
                val device = ensureDevice()
                client.removeDevice(session.access, device.id, device.token, deviceId)
                val devices = client.listDevices(session.access, device.id, device.token)
                _state.update { it.copy(devices = devices, busy = false) }
            } catch (error: Exception) {
                _state.update { it.copy(busy = false, error = accountError(error)) }
            }
        }
    }

    private fun restoreSession() {
        viewModelScope.launch {
            val refresh = prefs.accountRefreshToken.first()
            val access = prefs.accountAccessToken.first()
            if (refresh.isBlank() || access.isBlank()) {
                _state.update { it.copy(initializing = false) }
                return@launch
            }
            try {
                val user = try {
                    client.currentUser(access)
                } catch (error: AccountApiException) {
                    if (error.code != "403005") throw error
                    val refreshed = client.refreshSession(refresh)
                    prefs.setAccountSession(
                        refreshed.accessToken,
                        refreshed.refreshToken,
                        refreshed.user.email,
                        refreshed.user.displayName
                    )
                    refreshed.user
                }
                val session = loadSession()
                val device = ensureDevice()
                client.claimDevice(session.access, device.id, device.token)
                val devices = client.listDevices(session.access, device.id, device.token)
                _state.value = AccountUiState(
                    initializing = false,
                    authenticated = true,
                    user = user,
                    devices = devices
                )
            } catch (error: Exception) {
                prefs.clearAccountSession()
                _state.value = AccountUiState(initializing = false, error = accountError(error))
            }
        }
    }

    private suspend fun ensureDevice(): LocalDeviceCredentials {
        val existingId = prefs.deviceId.first()
        val existingToken = prefs.deviceToken.first()
        if (existingId.isNotBlank() && existingToken.isNotBlank()) {
            return LocalDeviceCredentials(existingId, existingToken)
        }
        val name = listOf(Build.MANUFACTURER, Build.MODEL)
            .filter { it.isNotBlank() }
            .joinToString(" ")
            .ifBlank { "Android 设备" }
        val registration = client.registerDevice(
            deviceName = name.take(80),
            model = "${Build.MANUFACTURER} ${Build.MODEL}".trim(),
            osVersion = "Android ${Build.VERSION.RELEASE}"
        )
        prefs.setDeviceCredentials(registration.deviceId, registration.deviceToken)
        return LocalDeviceCredentials(registration.deviceId, registration.deviceToken)
    }

    private suspend fun loadSession(): StoredSession {
        val access = prefs.accountAccessToken.first()
        val refresh = prefs.accountRefreshToken.first()
        if (access.isBlank()) throw AccountApiException("登录状态已失效", "403005", 401)
        return StoredSession(access, refresh)
    }

    private fun accountError(error: Exception): String = when (error) {
        is AccountApiException -> ErrorCodes.getMessage(error.code)
        else -> error.message ?: "账号服务暂时不可用"
    }

    private data class LocalDeviceCredentials(val id: String, val token: String)
    private data class StoredSession(val access: String, val refresh: String)
}
