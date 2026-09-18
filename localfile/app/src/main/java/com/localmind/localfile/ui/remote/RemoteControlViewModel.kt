package com.localmind.localfile.ui.remote

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.localmind.localfile.common.AccountApiException
import com.localmind.localfile.common.AccountClient
import com.localmind.localfile.common.ErrorCodes
import com.localmind.localfile.common.Logger
import com.localmind.localfile.common.RelayWssClient
import com.localmind.localfile.storage.PreferencesManager
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
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

        // 申请发出后自动轮询：电脑端一旦批准，手机这边 5 秒内就能看到，不必手动点刷新。
        // 2026-09-18：此前只在进入页面时刷新一次，电脑端批准后手机一直停在
        // 「等待电脑端确认…」，容易被误认为没生效。
        viewModelScope.launch {
            while (isActive) {
                delay(5_000)
                if (_state.value.pendingRequestDeviceIds.isNotEmpty()) {
                    refresh(silent = true)
                }
            }
        }
    }

    /**
     * 拉取可用于远控的 Windows 设备。
     *
     * 注意：以前只在 init 里调用一次，如果用户「先打开远控页、后登录账号」，
     * 页面会永远停在"暂无已配对设备"，且错误信息不显示。现在改为可重复调用，
     * 页面每次进入都会刷新，用户也能手动点刷新。
     */
    fun refresh(silent: Boolean = false) {
        viewModelScope.launch {
            if (!silent) {
                _state.update { it.copy(loading = true, error = null) }
            }

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
                val pairings = withFreshAccessToken { access ->
                    accountClient.listControlPairings(access, deviceId, deviceToken)
                }
                val myPairings = pairings.filter { it.isController(deviceId) }

                // 设备列表一次查询同时用于「显示名映射」和「可申请设备」，避免重复请求 Relay
                val deviceList = withFreshAccessToken { access ->
                    accountClient.listDevices(access, deviceId, deviceToken)
                }
                val deviceNames = deviceList.associate { it.id to it.deviceName }

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
                val requestable = deviceList
                    .filter { it.platform == "windows" && it.id !in pairedIds }
                    .map { RemoteTarget(tenantId = "", targetDeviceId = it.id, deviceName = it.deviceName) }

                // 已发出、等待对方确认的申请
                val pendingIds = try {
                    withFreshAccessToken { access ->
                        accountClient.listPairingRequests(access, deviceId, deviceToken)
                    }
                        .filter { it.status == "pending" && it.requesterDeviceId == deviceId }
                        .map { it.targetDeviceId }
                        .toSet()
                } catch (e: Exception) { emptySet() }

                _state.update { current ->
                    // 选中项必须跟着最新配对列表走：切换账号 / 撤销配对 / 换机器后，
                    // 旧的 selectedTargetId 会指向已不存在的设备，此时点发送只会得到
                    // "目标设备无效"，而界面上还留着上一轮的旧结果 —— 很容易被误读成
                    // 链路故障。这里失效就重置，没人选中时自动选第一台。
                    val keepSelection = current.selectedTargetId?.let { id ->
                        targets.any { it.targetDeviceId == id }
                    } == true
                    current.copy(
                        targets = targets,
                        requestable = requestable,
                        pendingRequestDeviceIds = pendingIds,
                        selectedTargetId = if (keepSelection) current.selectedTargetId
                                           else targets.firstOrNull()?.targetDeviceId,
                        statusMessage = if (keepSelection) current.statusMessage else "",
                        statusType = if (keepSelection) current.statusType else "",
                        resultText = if (keepSelection) current.resultText else "",
                        loading = false,
                        error = if (targets.isEmpty()) {
                            "还没有可用于远控的电脑。请在电脑端账号页发起/批准一次控制授权配对。"
                        } else null,
                    )
                }
            } catch (e: Exception) {
                Logger.e(e)
                if (!silent) {
                    _state.update {
                        it.copy(loading = false, targets = emptyList(), error = "加载设备失败：${friendlyError(e)}")
                    }
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
                withFreshAccessToken { access ->
                    accountClient.createPairingRequest(access, deviceId, deviceToken, targetDeviceId)
                }
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
                _state.update { it.copy(loading = false, error = "申请失败：${friendlyError(e)}") }
            }
        }
    }

    /**
     * 用账号 access token 调 Relay；token 过期（403005）时先用 refresh token 换新并持久化，再重试一次。
     *
     * access token 只有 30 分钟有效期，而远控页不会走账号页的 restoreSession。
     * 没有这层兜底时，App 连续开着超过 30 分钟再进远控页就会一直报「加载设备失败」，
     * 用户只能去账号页刷新或重启 App 才能恢复。
     */
    /**
     * 把 Relay 的错误码翻成用户看得懂的中文。
     *
     * 2026-09-18：此前直接拼 e.message，界面上会出现
     * 「加载设备失败：device is not enrolled in this account」这类英文原文。
     */
    private fun friendlyError(e: Exception): String {
        if (e is AccountApiException) {
            return when (e.code) {
                "403007" -> "本机已被移出该账号，请到「账号」页重新登录后再试"
                else -> ErrorCodes.getMessage(e.code)
            }
        }
        return e.message ?: "未知错误"
    }

    private suspend fun <T> withFreshAccessToken(block: suspend (String) -> T): T {
        val access = prefs.accountAccessToken.first()
        return try {
            block(access)
        } catch (error: AccountApiException) {
            if (error.code != "403005") throw error
            val refresh = prefs.accountRefreshToken.first()
            if (refresh.isBlank()) throw error
            val session = accountClient.refreshSession(refresh)
            prefs.setAccountSession(
                session.accessToken,
                session.refreshToken,
                session.user.email,
                session.user.displayName,
            )
            block(session.accessToken)
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
            // 把原因写进 resultText：结果卡片只渲染 statusType + resultText，
            // 不写的话用户只会看到一个没有解释的"执行失败"。
            _state.update {
                it.copy(
                    isSending = false,
                    statusType = "failed",
                    statusMessage = "目标设备无效",
                    resultText = "未选中可用的电脑：请在上方「已配对设备」里点一下目标电脑，再发送指令。",
                )
            }
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
            // 每次等待 60 秒、最多 6 次 ≈ 6 分钟。
            // 旧值是 3×30s = 90 秒 —— 生成 PPT/文档这类任务要 1~3 分钟，
            // 手机必然在电脑完成前就放弃（重连时会收到 Relay 缓存的 done 状态）。
            val maxAttempts = 6
            val perAttemptTimeoutMs = 60_000L
            var attempt = 0
            // 终态标志：收到 done/failed 后置位。OKHttp 回调在其自己的线程上，
            // 且命令完成后连接关闭会补一次 onFailure，因此用原子类型跨线程共享。
            val finished = java.util.concurrent.atomic.AtomicBoolean(false)

            while (attempt < maxAttempts && !finished.get()) {
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
                        _state.update { it.copy(statusMessage = "\u5df2\u53d1\u9001\uff0c\u7b49\u5f85\u7535\u8111\u6267\u884c\uff08\u590d\u6742\u4efb\u52a1\u53ef\u80fd\u9700\u8981\u51e0\u5206\u949f\uff09...") }
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
                                finished.set(true)
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
                                finished.set(true)
                                doneSignal.complete(true)
                            }
                        }
                    },
                    onError = { error ->
                        // \u4e0d\u7acb\u5373\u62a5\u9519\uff0c\u4ea4\u7ed9\u5916\u5c42\u91cd\u8bd5\u5faa\u73af\u5904\u7406
                        if (!finished.get()) {
                            _state.update { it.copy(resultText = error) }
                        }
                        doneSignal.complete(false)
                    }
                )

                // 等待本次尝试结束（成功、失败或超时；超时后重连会收到 Relay 缓存的最终状态）
                val settled = withTimeoutOrNull(perAttemptTimeoutMs) { doneSignal.await() } ?: false
                if (settled || finished.get()) break
            }

            if (!finished.get()) {
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
