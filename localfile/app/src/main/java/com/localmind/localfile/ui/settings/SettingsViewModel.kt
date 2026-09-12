package com.localmind.localfile.ui.settings

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.localmind.localfile.model.DeviceDetector
import com.localmind.localfile.model.DeviceSpecs
import com.localmind.localfile.model.DeviceTier
import com.localmind.localfile.common.DeepSeekConfig
import com.localmind.localfile.storage.PreferencesManager
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SettingsUiState(
    val deviceSpecs: DeviceSpecs? = null,
    val deviceTier: DeviceTier? = null,
    val specsFormatted: String = "",
    val conclusionText: String = "",
    val themeMode: String = "system", // "light", "dark", "system"
    val gatewayStatus: ConnectionStatusUI = ConnectionStatusUI.UNKNOWN,
    val gatewayError: String? = null,
    val apiKey: String = "",
    val apiKeySaved: Boolean = false,
    val isChecking: Boolean = true,
    val error: String? = null
)

enum class ConnectionStatusUI {
    UNKNOWN, CONNECTED, DISCONNECTED, ERROR
}

class SettingsViewModel(application: Application) : AndroidViewModel(application) {

    private val detector = DeviceDetector(application)
    private val prefsManager = PreferencesManager(application)

    private val _state = MutableStateFlow(SettingsUiState())
    val state: StateFlow<SettingsUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val savedTheme = prefsManager.themeMode.first()
            _state.update { it.copy(themeMode = savedTheme) }
        }
        viewModelScope.launch {
            val savedKey = prefsManager.gatewayToken.first()
            _state.update { it.copy(apiKey = savedKey) }
            com.localmind.localfile.common.DeepSeekConfig.API_KEY = savedKey
        }
        detectDevice()
    }

    fun detectDevice() {
        viewModelScope.launch {
            _state.update { it.copy(isChecking = true) }
            try {
                val specs = detector.detectSpecs()
                val tier = detector.determineTier(specs)

                _state.update {
                    it.copy(
                        deviceSpecs = specs,
                        deviceTier = tier,
                        specsFormatted = detector.formatSpecs(specs),
                        conclusionText = detector.formatConclusion(specs, tier),
                        isChecking = false
                    )
                }
            } catch (e: Exception) {
                _state.update {
                    it.copy(
                        isChecking = false,
                        error = "设备检测失败：${e.message}"
                    )
                }
            }
        }
    }

    fun setThemeMode(mode: String) {
        viewModelScope.launch {
            prefsManager.setThemeMode(mode)
            _state.update { it.copy(themeMode = mode) }
        }
    }

    /**
     * 真实检测 DeepSeek 连接：用当前 key 发一个 1-token 的请求。
     * 之前这里是 delay(1500) 后固定显示"已连接"的假实现。
     */
    fun checkGatewayConnection() {
        viewModelScope.launch {
            _state.update { it.copy(gatewayStatus = ConnectionStatusUI.UNKNOWN, gatewayError = null) }
            val key = DeepSeekConfig.API_KEY
            if (key.isBlank()) {
                _state.update {
                    it.copy(
                        gatewayStatus = ConnectionStatusUI.ERROR,
                        gatewayError = "未填写 API Key",
                    )
                }
                return@launch
            }
            try {
                val result = com.localmind.localfile.common.GatewayClient().ping(key)
                if (result.ok) {
                    _state.update { it.copy(gatewayStatus = ConnectionStatusUI.CONNECTED, gatewayError = null) }
                } else {
                    _state.update {
                        it.copy(gatewayStatus = ConnectionStatusUI.ERROR, gatewayError = result.message)
                    }
                }
            } catch (e: Exception) {
                _state.update {
                    it.copy(gatewayStatus = ConnectionStatusUI.ERROR, gatewayError = e.message ?: "连接失败")
                }
            }
        }
    }

    fun setApiKey(key: String) {
        viewModelScope.launch {
            prefsManager.setGatewayToken(key)
            com.localmind.localfile.common.DeepSeekConfig.API_KEY = key
            _state.update { it.copy(apiKey = key, apiKeySaved = true) }
        }
    }

    fun clearError() {
        _state.update { it.copy(error = null) }
    }
}
