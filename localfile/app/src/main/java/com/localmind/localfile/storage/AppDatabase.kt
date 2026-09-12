package com.localmind.localfile.storage

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.localmind.localfile.storage.dao.AuditLogDao
import com.localmind.localfile.storage.dao.FileRecordDao
import com.localmind.localfile.storage.dao.MessageDao
import com.localmind.localfile.storage.dao.ModelAssetDao
import com.localmind.localfile.storage.dao.SessionDao

@Database(
    entities = [
        SessionEntity::class,
        MessageEntity::class,
        ModelAssetEntity::class,
        FileRecordEntity::class,
        AuditLogEntity::class
    ],
    version = 3,
    exportSchema = false
)
abstract class AppDatabase : RoomDatabase() {

    abstract fun sessionDao(): SessionDao
    abstract fun messageDao(): MessageDao
    abstract fun modelAssetDao(): ModelAssetDao
    abstract fun fileRecordDao(): FileRecordDao
    abstract fun auditLogDao(): AuditLogDao

    companion object {
        @Volatile
        private var INSTANCE: AppDatabase? = null

        /** v2 -> v3：新增 audit_log 表（工具调用审计）。 */
        private val MIGRATION_2_3 = object : Migration(2, 3) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL(
                    "CREATE TABLE IF NOT EXISTS audit_log (" +
                        "id TEXT NOT NULL PRIMARY KEY, " +
                        "sessionId TEXT NOT NULL DEFAULT '', " +
                        "action TEXT NOT NULL, " +
                        "target TEXT NOT NULL DEFAULT '', " +
                        "riskLevel TEXT NOT NULL DEFAULT 'L2', " +
                        "result TEXT NOT NULL, " +
                        "detail TEXT NOT NULL DEFAULT '', " +
                        "createdAt INTEGER NOT NULL DEFAULT 0)"
                )
            }
        }

        /** v1 -> v2：messages 表新增 tokenCount（token 计量）。 */
        private val MIGRATION_1_2 = object : Migration(1, 2) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL("ALTER TABLE messages ADD COLUMN tokenCount INTEGER NOT NULL DEFAULT 0")
            }
        }

        fun getInstance(context: Context): AppDatabase {
            return INSTANCE ?: synchronized(this) {
                INSTANCE ?: Room.databaseBuilder(
                    context.applicationContext,
                    AppDatabase::class.java,
                    "localfile.db"
                )
                    .addMigrations(MIGRATION_1_2, MIGRATION_2_3)
                    .fallbackToDestructiveMigration()
                    .build()
                    .also { INSTANCE = it }
            }
        }
    }
}
