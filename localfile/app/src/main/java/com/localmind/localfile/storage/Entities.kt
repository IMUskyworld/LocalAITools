package com.localmind.localfile.storage

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "sessions")
data class SessionEntity(
    @PrimaryKey val id: String,
    val title: String,
    val mode: String = "online", // "online" or "offline"
    val modelName: String = "DeepSeek",
    val messageCount: Int = 0,
    val createdAt: Long = System.currentTimeMillis(),
    val updatedAt: Long = System.currentTimeMillis()
)

@Entity(tableName = "messages")
data class MessageEntity(
    @PrimaryKey val id: String,
    val sessionId: String,
    val role: String, // "user" or "assistant"
    val content: String,
    val modelName: String = "",
    val speedText: String = "",
    val createdAt: Long = System.currentTimeMillis()
)

@Entity(tableName = "model_assets")
data class ModelAssetEntity(
    @PrimaryKey val id: String,
    val name: String,
    val displayName: String,
    val url: String,
    val fileSize: Long = 0,
    val downloadedBytes: Long = 0,
    val isDownloaded: Boolean = false,
    val isActive: Boolean = false,
    val checksum: String = "",
    val downloadPath: String = "",
    val downloadedAt: Long = 0
)

@Entity(tableName = "file_records")
data class FileRecordEntity(
    @PrimaryKey val id: String,
    val uri: String,
    val fileName: String,
    val fileSize: Long = 0,
    val mimeType: String = "",
    val processingMode: String = "", // summarize, translate, rename, extract
    val resultText: String = "",
    val originalName: String = "", // for rename mode — original name before rename
    val renamedName: String = "", // for rename mode — suggested new name
    val isProcessed: Boolean = false,
    val errorCode: String = "",
    val processedAt: Long = System.currentTimeMillis()
)
