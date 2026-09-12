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
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withTimeoutOrNull
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
            _state.update { it.copy(isSending = true, statusMessage = "\u6b63\u5728\u8fde\u63a5...", statusType = "running", resultText = "") }

            val deviceId = prefs.deviceId.first()
            val deviceToken = prefs.deviceToken.first()

            // \u65ad\u7ebf\u81ea\u52a8\u91cd\u8bd5\uff1a\u590d\u7528\u540c\u4e00\u4e2a commandId\uff0c
            // Relay \u4fa7\u636e\u6b64\u5e42\u7b49\u53bb\u91cd\uff0c\u4e0d\u4f1a\u91cd\u590d\u6267\u884c\u4efb\u52a1\u3002
            val commandId = java.util.UUID.randomUUID().toString()
            val maxAttempts = 3
            var attempt = 0
            var finished = false

            while (attempt < maxAttempts && !finished) {
                attempt++
                if (attempt > 1) {
                    val delayMs = 1500L * (attempt - 1)
                    _state.update {
                        it.copy(
                            statusMessage = "\u8fde\u63a5\u4e2d\u65ad\uff0c${delayMs / 1000}s \u540e\u91cd\u8bd5\uff08\u7b2c $attempt \u6b21\uff09"
                        )
                    }
                    kotlinx.coroutines.delay(delayMs)
                }

                wssClient?.disconnect()
                val client = RelayWssClient(getApplication())
                wssClient = client

                val doneSignal = kotlinx.coroutines.CompletableDeferred<Boolean>()
                client.connectAndSendCommand(
                    deviceId = deviceId,
                    deviceToken = deviceToken,
                    targetDeviceId = targetDeviceId,
                    intentText = intentText,
                    commandId = commandId,
                    onConnected = {
                        _state.update { it.copy(statusMessage = "\u5df2\u8fde\u63a5\uff0c\u7b49\u5f85\u6267\u884c...") }
                    },
                    onStateUpdate = { update ->
                        when (update.state) {
                            "running" -> _state.update {
                                it.copy(statusMessage = "\u6b63\u5728\u6267\u884c...", statusType = "running")
                            }
                            "done" -> {
                                val entry = CommandHistoryEntry(currentState.commandText, "done", update.resultText ?: "")
                                _state.update {
                                    it.copy(
                                        isSending = false,
                                        statusMessage = "\u6267\u884c\u5b8c\u6210",
                                        statusType = "done",
                                        resultText = update.resultText ?: "",
                                        history = it.history + entry,
                                        commandText = ""
                                    )
                                }
                                finished = true
                                doneSignal.complete(true)
                            }
                            "failed" -> {
                                val entry = CommandHistoryEntry(currentState.commandText, "failed", update.resultText ?: "")
                                _state.update {
                                    it.copy(
                                        isSending = false,
                                        statusMessage = "\u6267\u884c\u5931\u8d25",
                                        statusType = "failed",
                                        resultText = update.resultText ?: "",
                                        history = it.history + entry,
                                        commandText = ""
                                    )
                                }
                                finished = true
                                doneSignal.complete(true)
                            }
                        }
                    },
                    onError = { error ->
                        // \u4e0d\u7acb\u5373\u62a5\u9519\uff0c\u4ea4\u7ed9\u5916\u5c42\u91cd\u8bd5\u5faa\u73af\u5904\u7406
                        doneSignal.complete(false)
                        kotlinx.coroutines.runBlocking { }
                        _state.update { it.copy(resultText = error) }
                    }
                )

                // \u7b49\u5f85\u672c\u6b21\u5c1d\u8bd5\u7ed3\u675f\uff08\u6210\u529f\u3001\u5931\u8d25\u6216 30s \u8d85\u65f6\uff09
                val settled = withTimeoutOrNull(30_000L) { doneSignal.await() } ?: false
                if (settled || finished) break
            }

            if (!finished) {
                _state.update {
                    it.copy(
                        isSending = false,
                        statusMessage = "\u8fde\u63a5\u5931\u8d25",
                        statusType = "failed",
                        resultText = it.resultText.ifEmpty { "\u5df2\u91cd\u8bd5 $maxAttempts \u6b21\uff0c\u8bf7\u68c0\u67e5\u7f51\u7edc\u540e\u91cd\u8bd5" }
                    )
                }
            }
        }
    }

    override fun onCleared() {
        super.onCleared()
        wssClient?.disconnect()
    }
}
