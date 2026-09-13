package com.localmind.localfile.ui.remote

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.localmind.localfile.common.AccountClient
import com.localmind.localfile.common.Logger
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

/**
 * 一个可远控的目标（来自已批准的控制配对）。
 * tenantId 是 Relay 路由命令的必要字段。
 */
data class RemoteTarget(
    val tenantId: String,
    val targetDeviceId: String,
    val deviceName: String,
    val permissions: List<String> = emptyList()
)

data class RemoteControlUiState(
    val targets: List<RemoteTarget> = emptyList(),
    /** 账号下已登记、但还没建立控制配对的 Windows 设备（可发起申请） */
    val requestable: List<RemoteTarget> = emptyList(),
    /** 已发出申请、等待电脑端确认的设备 id */
    val pendingRequestDeviceIds: Set<String> = emptySet(),
    val selectedTargetId: String? = null,
    val commandText: String = "",
    val isSending: Boolean = false,
    val statusMessage: String = "",
    val statusType: String = "", // "running", "done", "failed"
    val resultText: String = "",
    val history: List<CommandHistoryEntry> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null
)

class RemoteControlViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = PreferencesManager(application)
    private val accountClient = AccountClient(application)
    private var wssClient: RelayWssClient? = null

    private val _state = MutableStateFlow(RemoteControlUiState())
    val state: StateFlow<RemoteControlUiState> = _state.asStateFlow()

    init {
        refresh()
    }

    /**
     * 拉取可用于远控的 Windows 设备。
     *
     * 注意：以前只在 init 里调用一次，如果用户「先打开远控页、后登录账号」，
     * 页面会永远停在"暂无已配对设备"，且错误信息不显示。现在改为可重复调用，
     * 页面每次进入都会刷新，用户也能手动点刷新。
     */
    fun refresh() {
        viewModelScope.launch {
            _state.update { it.copy(loading = true, error = null) }

            val token = prefs.accountAccessToken.first()
            if (token.isEmpty()) {
                _state.update {
                    it.copy(loading = false, targets = emptyList(), error = "尚未登录账号：请先到「账号」页登录")
                }
                return@launch
            }
            val deviceId = prefs.deviceId.first()
            val deviceToken = prefs.deviceToken.first()
            if (deviceId.isEmpty() || deviceToken.isEmpty()) {
                _state.update {
                    it.copy(loading = false, targets = emptyList(), error = "本机尚未登记：请到「账号」页刷新一次")
                }
                return@launch
            }
            try {
                // 关键：远控列表必须来自「已批准的控制配对」，而不是账号下的所有设备。
                // 只有控制配对才带 tenant_id，而 Relay 路由命令时必须要它。
                val pairings = accountClient.listControlPairings(token, deviceId, deviceToken)
                val myPairings = pairings.filter { it.isController(deviceId) }

                val deviceNames = accountClient.listDevices(token, deviceId, deviceToken)
                    .associate { it.id to it.deviceName }

                val targets = myPairings.map { p ->
                    RemoteTarget(
                        tenantId = p.tenantId,
                        targetDeviceId = p.peerDeviceId(deviceId),
                        deviceName = deviceNames[p.peerDeviceId(deviceId)] ?: "已配对设备",
                        permissions = p.permissions,
                    )
                }

                // 还没配对、但可以发起申请的 Windows 设备
                val pairedIds = myPairings.map { it.peerDeviceId(deviceId) }.toSet()
                val requestable = accountClient.listDevices(token, deviceId, deviceToken)
                    .filter { it.platform == "windows" && it.id !in pairedIds }
                    .map { RemoteTarget(tenantId = "", targetDeviceId = it.id, deviceName = it.deviceName) }

                // 已发出、等待对方确认的申请
                val pendingIds = try {
                    accountClient.listPairingRequests(token, deviceId, deviceToken)
                        .filter { it.status == "pending" && it.requesterDeviceId == deviceId }
                        .map { it.targetDeviceId }
                        .toSet()
                } catch (e: Exception) { emptySet() }

                _state.update {
                    it.copy(
                        targets = targets,
                        requestable = requestable,
                        pendingRequestDeviceIds = pendingIds,
                        loading = false,
                        error = if (targets.isEmpty()) {
                            "还没有可用于远控的电脑。请在电脑端账号页发起/批准一次控制授权配对。"
                        } else null,
                    )
                }
            } catch (e: Exception) {
                Logger.e(e)
                _state.update {
                    it.copy(loading = false, targets = emptyList(), error = "加载设备失败：${e.message}")
                }
            }
        }
    }

    /** 向目标电脑发起控制授权申请，需要对方在本机点"批准"。 */
    fun requestControl(targetDeviceId: String) {
        viewModelScope.launch {
            val token = prefs.accountAccessToken.first()
            val deviceId = prefs.deviceId.first()
            val deviceToken = prefs.deviceToken.first()
            if (token.isEmpty() || deviceId.isEmpty() || deviceToken.isEmpty()) {
                _state.update { it.copy(error = "请先登录账号并完成设备登记") }
                return@launch
            }
            _state.update { it.copy(loading = true, error = null) }
            try {
                accountClient.createPairingRequest(token, deviceId, deviceToken, targetDeviceId)
                _state.update {
                    it.copy(
                        loading = false,
                        pendingRequestDeviceIds = it.pendingRequestDeviceIds + targetDeviceId,
                        statusMessage = "已发送授权申请，请在电脑端 LocalMind「账号」页点「批准」",
                        statusType = "running",
                    )
                }
            } catch (e: Exception) {
                Logger.e(e)
                _state.update { it.copy(loading = false, error = "申请失败：${e.message}") }
            }
        }
    }

    fun selectTarget(deviceId: String) {
        _state.update { it.copy(selectedTargetId = deviceId, statusMessage = "", statusType = "", resultText = "") }
    }

    fun updateCommand(text: String) {
        _state.update { it.copy(commandText = text) }
    }

    fun sendCommand() {
        val currentState = _state.value
        val selectedId = currentState.selectedTargetId ?: return
        val intentText = currentState.commandText.trim()
        if (intentText.isEmpty()) return

        // 从选中的目标解析出 tenant_id（Relay 路由必需）与目标设备 id
        val target = currentState.targets.firstOrNull { it.targetDeviceId == selectedId }
        if (target == null) {
            _state.update { it.copy(isSending = false, statusType = "failed", statusMessage = "目标设备无效，请刷新后重试") }
            return
        }
        val targetDeviceId = target.targetDeviceId
        val tenantId = target.tenantId

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
                    tenantId = tenantId,
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
