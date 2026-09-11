package com.localmind.localfile.common

import com.localmind.localfile.BuildConfig

/**
 * DeepSeek API 配置。
 * key 从 BuildConfig 读取（本地构建时用 gradle 属性/环境变量 LOCAL_FILE_API_KEY 注入），
 * 源码不存明文（公开 GitHub 安全）。未注入时用占位符（APK 功能不可用）。
 */
object DeepSeekConfig {
    // 用户在设置页填写的 key；启动时从 DataStore 加载，保存时同步更新。
    // 不再从 BuildConfig 烘焙（用户自填模式）。
    var API_KEY: String = ""
    const val BASE_URL = "https://api.deepseek.com/v1"
    const val MODEL = "deepseek-flash"

    /** 文件内容发送上限（字符），防止超大文件撑爆上下文 */
    const val MAX_FILE_CHARS = 30_000
}
