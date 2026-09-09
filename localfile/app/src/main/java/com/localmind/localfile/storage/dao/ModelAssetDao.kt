package com.localmind.localfile.storage.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.localmind.localfile.storage.ModelAssetEntity
import kotlinx.coroutines.flow.Flow

@Dao
interface ModelAssetDao {
    @Query("SELECT * FROM model_assets ORDER BY downloadedAt DESC")
    fun getAllAssets(): Flow<List<ModelAssetEntity>>

    @Query("SELECT * FROM model_assets WHERE id = :id")
    suspend fun getAssetById(id: String): ModelAssetEntity?

    @Query("SELECT * FROM model_assets WHERE isDownloaded = 1")
    suspend fun getDownloadedAssets(): List<ModelAssetEntity>

    @Query("SELECT * FROM model_assets WHERE isActive = 1 LIMIT 1")
    suspend fun getActiveAsset(): ModelAssetEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAsset(asset: ModelAssetEntity)

    @Update
    suspend fun updateAsset(asset: ModelAssetEntity)

    @Delete
    suspend fun deleteAsset(asset: ModelAssetEntity)

    @Query("DELETE FROM model_assets WHERE id = :id")
    suspend fun deleteAssetById(id: String)
}
