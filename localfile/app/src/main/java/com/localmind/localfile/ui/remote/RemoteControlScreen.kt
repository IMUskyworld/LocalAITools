package com.localmind.localfile.ui.remote

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.Send
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel

@Composable
fun RemoteControlScreen(
    viewModel: RemoteControlViewModel = viewModel()
) {
    val state by viewModel.state.collectAsState()

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        item {
            Text(
                text = "远程控制",
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
            )
        }

        item {
            Text("已配对设备", style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Medium, modifier = Modifier.padding(horizontal = 16.dp))
        }

        if (state.devices.isEmpty()) {
            item {
                Card(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant), shape = RoundedCornerShape(16.dp)) {
                    Text("暂无已配对设备。请先在账号页登录并完成设备配对。", modifier = Modifier.padding(16.dp), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.outline)
                }
            }
        } else {
            items(state.devices) { device ->
                Card(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp), colors = CardDefaults.cardColors(containerColor = if (device.id == state.selectedDeviceId) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant), shape = RoundedCornerShape(16.dp), onClick = { viewModel.selectDevice(device.id) }) {
                    Row(modifier = Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Default.Computer, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                        Spacer(modifier = Modifier.width(12.dp))
                        Column {
                            Text(device.deviceName, fontWeight = FontWeight.Medium)
                            Text(device.platform, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
                        }
                    }
                }
            }
        }

        if (state.selectedDeviceId != null) {
            item {
                Card(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant), shape = RoundedCornerShape(16.dp)) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text("发送指令", style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
                        Spacer(modifier = Modifier.height(8.dp))
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            OutlinedTextField(value = state.commandText, onValueChange = { viewModel.updateCommand(it) }, modifier = Modifier.weight(1f), placeholder = { Text("例如：帮我整理桌面上的文件") }, singleLine = true, enabled = !state.isSending)
                            Spacer(modifier = Modifier.width(8.dp))
                            IconButton(onClick = { viewModel.sendCommand() }, enabled = state.commandText.isNotBlank() && !state.isSending) {
                                if (state.isSending) CircularProgressIndicator(modifier = Modifier.size(24.dp))
                                else Icon(Icons.Default.Send, contentDescription = "发送", tint = MaterialTheme.colorScheme.primary)
                            }
                        }
                    }
                }
            }
        }

        if (state.statusMessage.isNotEmpty()) {
            item {
                Card(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp), colors = CardDefaults.cardColors(containerColor = when (state.statusType) { "done" -> Color(0xFF4CAF50).copy(alpha = 0.1f); "failed" -> Color(0xFFE74C3C).copy(alpha = 0.1f); else -> MaterialTheme.colorScheme.surfaceVariant }), shape = RoundedCornerShape(16.dp)) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(text = when (state.statusType) { "running" -> "执行中..."; "done" -> "执行完成"; "failed" -> "执行失败"; else -> state.statusType }, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
                        if (state.resultText.isNotEmpty()) { Spacer(modifier = Modifier.height(8.dp)); Text(state.resultText, style = MaterialTheme.typography.bodySmall) }
                    }
                }
            }
        }
    }
}
