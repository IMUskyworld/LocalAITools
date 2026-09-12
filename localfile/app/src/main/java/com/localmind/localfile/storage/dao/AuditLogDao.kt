package com.localmind.localfile.storage.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.localmind.localfile.storage.AuditLogEntity

@Dao
interface AuditLogDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(entry: AuditLogEntity)

    @Query("SELECT * FROM audit_log ORDER BY createdAt DESC LIMIT :limit")
    suspend fun recent(limit: Int = 100): List<AuditLogEntity>

    @Query("SELECT COUNT(*) FROM audit_log")
    suspend fun count(): Int

    @Query("DELETE FROM audit_log")
    suspend fun clearAll()
}
