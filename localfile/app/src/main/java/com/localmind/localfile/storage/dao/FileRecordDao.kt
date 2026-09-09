package com.localmind.localfile.storage.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.localmind.localfile.storage.FileRecordEntity
import kotlinx.coroutines.flow.Flow

@Dao
interface FileRecordDao {
    @Query("SELECT * FROM file_records ORDER BY processedAt DESC")
    fun getAllRecords(): Flow<List<FileRecordEntity>>

    @Query("SELECT * FROM file_records WHERE id = :id")
    suspend fun getRecordById(id: String): FileRecordEntity?

    @Query("SELECT * FROM file_records ORDER BY processedAt DESC LIMIT :limit")
    fun getRecentRecords(limit: Int = 20): Flow<List<FileRecordEntity>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertRecord(record: FileRecordEntity)

    @Delete
    suspend fun deleteRecord(record: FileRecordEntity)

    @Query("DELETE FROM file_records WHERE id = :id")
    suspend fun deleteRecordById(id: String)

    @Query("DELETE FROM file_records")
    suspend fun deleteAllRecords()
}
