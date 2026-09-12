package com.localmind.localfile.ui.remote

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Send
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel

@Composable
fun RemoteControlScreen(
    viewModel: RemoteControlViewModel = viewModel()
) {
    val state by viewModel.state.collectAsState()
    val context = LocalContext.current

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        contentPadding = PaddingValues(bottom = 32.dp)
    ) {
        item {
            Text("远程控制", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
        }

        item { Text("已配对设备", style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Medium, modifier = Modifier.padding(horizontal = 16.dp)) }

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
                        Column { Text(device.deviceName, fontWeight = FontWeight.Medium); Text(device.platform, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline) }
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
            item { ResultCard(command = state.commandText, status = state.statusType, result = state.resultText, context = context, compact = false) }
        }

        if (state.history.isNotEmpty()) {
            item {
                Row(modifier = Modifier.padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Default.History, contentDescription = null, modifier = Modifier.size(18.dp), tint = MaterialTheme.colorScheme.outline)
                    Spacer(modifier = Modifier.width(6.dp))
                    Text("历史命令", style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Medium, color = MaterialTheme.colorScheme.outline)
                }
            }
            items(state.history.reversed().take(10)) { entry ->
                ResultCard(command = entry.command, status = entry.status, result = entry.result, context = context, compact = true)
            }
        }
    }
}

@Composable
private fun ResultCard(command: String, status: String, result: String, context: Context, compact: Boolean) {
    var expanded by remember { mutableStateOf(!compact) }
    val maxPreview = 200

    Card(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        colors = CardDefaults.cardColors(containerColor = when (status) { "done" -> Color(0xFF4CAF50).copy(alpha = 0.08f); "failed" -> Color(0xFFE74C3C).copy(alpha = 0.08f); else -> MaterialTheme.colorScheme.surfaceVariant }),
        shape = RoundedCornerShape(16.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = when (status) { "done" -> Icons.Default.CheckCircle; "failed" -> Icons.Default.Error; else -> Icons.Default.Computer },
                    contentDescription = null,
                    tint = when (status) { "done" -> Color(0xFF4CAF50); "failed" -> Color(0xFFE74C3C); else -> MaterialTheme.colorScheme.primary },
                    modifier = Modifier.size(20.dp)
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text(text = when (status) { "running" -> "执行中..."; "done" -> "执行完成"; "failed" -> "执行失败"; else -> status }, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
            }

            if (command.isNotBlank()) {
                Spacer(modifier = Modifier.height(4.dp))
                Text(text = command, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }

            if (result.isNotEmpty()) {
                Spacer(modifier = Modifier.height(8.dp))
                if (compact && !expanded && result.length > maxPreview) {
                    Text(text = result.take(maxPreview) + "...", style = MaterialTheme.typography.bodySmall)
                    TextButton(onClick = { expanded = true }, modifier = Modifier.align(Alignment.End)) { Text("展开全部", style = MaterialTheme.typography.labelSmall) }
                } else {
                    Box(modifier = Modifier.heightIn(max = if (compact) 300.dp else 600.dp).verticalScroll(rememberScrollState())) {
                        Text(text = result, style = MaterialTheme.typography.bodySmall)
                    }
                    if (compact && result.length > maxPreview) {
                        TextButton(onClick = { expanded = false }, modifier = Modifier.align(Alignment.End)) { Text("收起", style = MaterialTheme.typography.labelSmall) }
                    }
                }

                Row(modifier = Modifier.align(Alignment.End)) {
                    TextButton(onClick = {
                        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                        clipboard.setPrimaryClip(ClipData.newPlainText("remote_result", result))
                    }) {
                        Icon(Icons.Default.ContentCopy, contentDescription = null, modifier = Modifier.size(14.dp))
                        Spacer(modifier = Modifier.width(4.dp))
                        Text("复制", style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
        }
    }
}
