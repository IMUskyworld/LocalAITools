package com.localmind.localfile.model

import android.app.ActivityManager
import android.content.Context
import android.os.Build
import com.localmind.localfile.common.Logger

data class DeviceSpecs(
    val totalRamMb: Long,
    val availableRamMb: Long,
    val socModel: String,
    val cores: Int,
    val hasNPU: Boolean,
    val androidVersion: Int,
    val deviceModel: String
)

data class DeviceTier(
    val tier: String, // "high", "medium", "low", "unsupported"
    val recommendedModelSize: String, // "7B", "4B", "1.5B", ""
    val canRunOffline: Boolean,
    val reason: String
)

class DeviceDetector(private val context: Context) {

    /**
     * Detect device specifications.
     */
    fun detectSpecs(): DeviceSpecs {
        val activityManager = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        val memInfo = ActivityManager.MemoryInfo()
        activityManager?.getMemoryInfo(memInfo)

        val totalRamMb = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.JELLY_BEAN) {
            memInfo.totalMem / (1024 * 1024)
        } else {
            // Fallback estimate
            (memInfo.availMem * 2) / (1024 * 1024)
        }

        val availableRamMb = memInfo.availMem / (1024 * 1024)

        return DeviceSpecs(
            totalRamMb = totalRamMb,
            availableRamMb = availableRamMb,
            socModel = getSocName(),
            cores = Runtime.getRuntime().availableProcessors(),
            hasNPU = detectNPU(),
            androidVersion = Build.VERSION.SDK_INT,
            deviceModel = "${Build.MANUFACTURER} ${Build.MODEL}"
        )
    }

    /**
     * Determine device tier and recommend model size.
     */
    fun determineTier(specs: DeviceSpecs): DeviceTier {
        return when {
            specs.totalRamMb >= 12_000 -> {
                DeviceTier(
                    tier = "high",
                    recommendedModelSize = "7B",
                    canRunOffline = true,
                    reason = "内存充足，可运行大模型"
                )
            }
            specs.totalRamMb >= 8_000 -> {
                DeviceTier(
                    tier = "medium",
                    recommendedModelSize = "4B",
                    canRunOffline = true,
                    reason = "可运行中等规模模型"
                )
            }
            specs.totalRamMb >= 6_000 -> {
                DeviceTier(
                    tier = "low",
                    recommendedModelSize = "1.5B",
                    canRunOffline = true,
                    reason = "内存有限，建议使用小模型或在线模式"
                )
            }
            else -> {
                DeviceTier(
                    tier = "unsupported",
                    recommendedModelSize = "",
                    canRunOffline = false,
                    reason = "内存不足（${specs.totalRamMb}MB），请使用在线模式"
                )
            }
        }
    }

    /**
     * Check if device meets minimum requirements for offline model.
     */
    fun canDownloadModel(specs: DeviceSpecs, modelSize: String): Pair<Boolean, String> {
        val tier = determineTier(specs)
        if (!tier.canRunOffline) {
            return false to "设备不满足离线模式的最低要求"
        }

        return when (modelSize) {
            "7B" -> (specs.totalRamMb >= 12_000) to "至少需要 12GB 内存"
            "4B" -> (specs.totalRamMb >= 8_000) to "至少需要 8GB 内存"
            "1.5B" -> (specs.totalRamMb >= 6_000) to "至少需要 6GB 内存"
            else -> false to "未知的模型大小"
        }
    }

    /**
     * Format specifications for display.
     */
    fun formatSpecs(specs: DeviceSpecs): String {
        val soc = specs.socModel.ifEmpty { specs.deviceModel }
        return "$soc · ${specs.totalRamMb}MB 内存"
    }

    /**
     * Format conclusion text based on tier.
     */
    fun formatConclusion(specs: DeviceSpecs, tier: DeviceTier): String {
        val socInfo = specs.socModel.ifEmpty { specs.deviceModel }
        return when (tier.tier) {
            "high" -> "$socInfo · ${specs.totalRamMb}MB 内存"
            "medium" -> "$socInfo · ${specs.totalRamMb}MB 内存"
            "low" -> "${specs.totalRamMb}MB 内存"
            else -> "${specs.totalRamMb}MB 内存"
        }
    }

    private fun getSocName(): String {
        return when {
            Build.HARDWARE.contains("qcom", ignoreCase = true) ||
                Build.SOC_MANUFACTURER == "Qualcomm" ->
                "Snapdragon ${Build.SOC_MODEL}"
            Build.HARDWARE.contains("exynos", ignoreCase = true) ->
                "Exynos ${Build.SOC_MODEL}"
            Build.HARDWARE.contains("mt", ignoreCase = true) ||
                Build.SOC_MANUFACTURER == "MediaTek" ->
                "MediaTek ${Build.SOC_MODEL}"
            Build.HARDWARE.contains("kirin", ignoreCase = true) ->
                "Kirin ${Build.SOC_MODEL}"
            Build.SOC_MANUFACTURER.isNotEmpty() ->
                "${Build.SOC_MANUFACTURER} ${Build.SOC_MODEL}"
            else -> "${Build.MANUFACTURER} ${Build.MODEL}"
        }
    }

    private fun detectNPU(): Boolean {
        // Heuristic: newer flagship SoCs have NPU
        val socLower = getSocName().lowercase()
        return socLower.contains("snapdragon") && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ||
            socLower.contains("exynos") && Build.VERSION.SDK_INT >= 30 ||
            socLower.contains("tensor") ||
            socLower.contains("dimensity") ||
            socLower.contains("kirin") && Build.VERSION.SDK_INT >= 30
    }
}
