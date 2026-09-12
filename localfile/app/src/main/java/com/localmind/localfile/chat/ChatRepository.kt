package com.localmind.localfile.chat

import android.content.Context
import com.localmind.localfile.storage.AppDatabase
import com.localmind.localfile.storage.MessageEntity
import com.localmind.localfile.storage.SessionEntity
import java.util.UUID

class ChatRepository(context: Context) {
    data class Session(val id: String, val title: String, val createdAt: Long, val updatedAt: Long)
    data class Message(val id: String, val sessionId: String, val role: String, val content: String, val modelLabel: String, val tokenCount: Int = 0, val createdAt: Long)

    private val db = AppDatabase.getInstance(context)
    private val sessionDao = db.sessionDao()
    private val messageDao = db.messageDao()

    suspend fun listSessions(): List<Session> {
        return sessionDao.getAllSessionsOnce().map { it.toSession() }
    }

    suspend fun createSession(title: String): Session {
        val now = System.currentTimeMillis()
        val entity = SessionEntity(
            id = UUID.randomUUID().toString(),
            title = title.ifEmpty { "新对话" },
            createdAt = now,
            updatedAt = now
        )
        sessionDao.insertSession(entity)
        return entity.toSession()
    }

    suspend fun deleteSession(sessionId: String) {
        messageDao.deleteMessagesBySession(sessionId)
        sessionDao.deleteSessionById(sessionId)
    }

    suspend fun listMessages(sessionId: String): List<Message> {
        return messageDao.getMessagesBySessionOnce(sessionId).map { it.toMessage() }
    }

    suspend fun appendMessage(
        sessionId: String,
        role: String,
        content: String,
        modelLabel: String,
        tokenCount: Int = 0
    ): Message {
        val now = System.currentTimeMillis()
        val entity = MessageEntity(
            id = UUID.randomUUID().toString(),
            sessionId = sessionId,
            role = role,
            content = content,
            modelName = modelLabel,
            tokenCount = tokenCount,
            createdAt = now
        )
        messageDao.insertMessage(entity)
        // Update session timestamp and count
        sessionDao.updateSessionTime(sessionId, now)
        return entity.toMessage()
    }

    private fun SessionEntity.toSession() = Session(id, title, createdAt, updatedAt)
    private fun MessageEntity.toMessage() = Message(id, sessionId, role, content, modelName, tokenCount, createdAt)
}