package com.localmind.localfile.ui.remote

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.localmind.localfile.common.AccountClient
import com.localmind.localfile.common.RelayAccountDevice
import com.localmind.localfile.common.RelayWssClient
import com.localmind.localfile.storage.PreferencesManager
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class CommandHistoryEntry(
    val command: String,
    val status: String,
    val result: String
)

data class RemoteControlUiState(
    val devices: List<RelayAccountDevice> = emptyList(),
    val selectedDeviceId: String? = null,
    val commandText: String = "",
    val isSending: Boolean = false,
    val statusMessage: String = "",
    val statusType: String = "", // "running", "done", "failed"
    val resultText: String = "",
    val history: List<CommandHistoryEntry> = emptyList(),
    val error: String? = null
)

class RemoteControlViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = PreferencesManager(application)
    private val accountClient = AccountClient(application)
    private var wssClient: RelayWssClient? = null

    private val _state = MutableStateFlow(RemoteControlUiState())
    val state: StateFlow<RemoteControlUiState> = _state.asStateFlow()

    init {
        loadDevices()
    }

    private fun loadDevices() {
        viewModelScope.launch {
            val token = prefs.accountAccessToken.first()
            if (token.isEmpty()) {
                _state.update { it.copy(error = "请先登录账号") }
                return@launch
            }
            val deviceId = prefs.deviceId.first()
            val deviceToken = prefs.deviceToken.first()
            if (deviceId.isEmpty() || deviceToken.isEmpty()) {
                _state.update { it.copy(error = "设备未注册") }
                return@launch
            }
            try {
                val devices = accountClient.listDevices(token, deviceId, deviceToken)
                val windowsDevices = devices.filter { it.platform == "windows" }
                _state.update { it.copy(devices = windowsDevices) }
            } catch (e: Exception) {
                _state.update { it.copy(error = "加载设备失败: ${e.message}") }
            }
        }
    }

    fun selectDevice(deviceId: String) {
        _state.update { it.copy(selectedDeviceId = deviceId, statusMessage = "", statusType = "", resultText = "") }
    }

    fun updateCommand(text: String) {
        _state.update { it.copy(commandText = text) }
    }

    fun sendCommand() {
        val currentState = _state.value
        val targetDeviceId = currentState.selectedDeviceId ?: return
        val intentText = currentState.commandText.trim()
        if (intentText.isEmpty()) return

        viewModelScope.launch {
            _state.update { it.copy(isSending = true, statusMessage = "正在连接...", statusType = "running", resultText = "") }

            val deviceId = prefs.deviceId.first()
            val deviceToken = prefs.deviceToken.first()

            wssClient?.disconnect()
            wssClient = RelayWssClient(getApplication())

            wssClient?.connectAndSendCommand(
                deviceId = deviceId,
                deviceToken = deviceToken,
                targetDeviceId = targetDeviceId,
                intentText = intentText,
                onConnected = {
                    _state.update { it.copy(statusMessage = "已连接，等待执行...") }
                },
                onStateUpdate = { update ->
                    when (update.state) {
                        "running" -> _state.update { it.copy(statusMessage = "正在执行...", statusType = "running") }
                        "done" -> _state.update {
                            it.copy(isSending = false, statusMessage = "执行完成", statusType = "done", resultText = update.resultText ?: "")
                        }
                        "failed" -> _state.update {
                            it.copy(isSending = false, statusMessage = "执行失败", statusType = "failed", resultText = update.resultText ?: "")
                        }
                    }
                },
                onError = { error ->
                    _state.update { it.copy(isSending = false, statusMessage = "连接失败", statusType = "failed", resultText = error) }
                }
            )
        }
    }

    override fun onCleared() {
        super.onCleared()
        wssClient?.disconnect()
    }
}
