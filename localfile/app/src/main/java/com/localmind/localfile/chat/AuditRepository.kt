package com.localmind.localfile.chat

import android.content.Context
import com.localmind.localfile.storage.AppDatabase
import com.localmind.localfile.storage.AuditLogEntity
import java.util.UUID

/** 工具调用审计日志。只增不改，用户可在"审计日志"页查看。 */
class AuditRepository(context: Context) {
    private val dao = AppDatabase.getInstance(context).auditLogDao()

    suspend fun record(
        sessionId: String,
        action: String,
        target: String,
        riskLevel: String,
        success: Boolean,
        detail: String
    ) {
        dao.insert(
            AuditLogEntity(
                id = UUID.randomUUID().toString(),
                sessionId = sessionId,
                action = action,
                target = target.take(300),
                riskLevel = riskLevel,
                result = if (success) "success" else "failed",
                detail = detail.take(1000),
            )
        )
    }

    suspend fun recent(limit: Int = 100): List<AuditLogEntity> = dao.recent(limit)
    suspend fun clearAll() = dao.clearAll()
}
