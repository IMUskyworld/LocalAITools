package com.localmind.localfile.storage

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import com.localmind.localfile.storage.dao.FileRecordDao
import com.localmind.localfile.storage.dao.MessageDao
import com.localmind.localfile.storage.dao.ModelAssetDao
import com.localmind.localfile.storage.dao.SessionDao

@Database(
    entities = [
        SessionEntity::class,
        MessageEntity::class,
        ModelAssetEntity::class,
        FileRecordEntity::class
    ],
    version = 1,
    exportSchema = false
)
abstract class AppDatabase : RoomDatabase() {

    abstract fun sessionDao(): SessionDao
    abstract fun messageDao(): MessageDao
    abstract fun modelAssetDao(): ModelAssetDao
    abstract fun fileRecordDao(): FileRecordDao

    companion object {
        @Volatile
        private var INSTANCE: AppDatabase? = null

        fun getInstance(context: Context): AppDatabase {
            return INSTANCE ?: synchronized(this) {
                INSTANCE ?: Room.databaseBuilder(
                    context.applicationContext,
                    AppDatabase::class.java,
                    "localfile.db"
                )
                    .fallbackToDestructiveMigration()
                    .build()
                    .also { INSTANCE = it }
            }
        }
    }
}
