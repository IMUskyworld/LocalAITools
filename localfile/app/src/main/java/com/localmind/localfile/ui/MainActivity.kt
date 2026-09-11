package com.localmind.localfile.ui

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import com.localmind.localfile.storage.PreferencesManager
import com.localmind.localfile.common.DeepSeekConfig
import com.localmind.localfile.ui.components.LocalFileTheme
import com.localmind.localfile.ui.components.ThemeMode
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // 启动时从 DataStore 加载用户填写的 API key
        val prefs = PreferencesManager(applicationContext)
        runBlocking {
            DeepSeekConfig.API_KEY = prefs.gatewayToken.first()
        }
        setContent {
            val prefs = remember { PreferencesManager(applicationContext) }
            val savedTheme by prefs.themeMode.collectAsState(initial = "system")
            val themeMode = when (savedTheme) {
                "light" -> ThemeMode.LIGHT
                "dark" -> ThemeMode.DARK
                else -> ThemeMode.SYSTEM
            }
            LocalFileTheme(themeMode = themeMode, dynamicColor = true) {
                LocalFileApp()
            }
        }
    }
}
