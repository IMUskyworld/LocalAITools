pluginManagement {
    repositories {
        // China mirrors first; official repositories remain as fallback.
        maven(url = "https://maven.aliyun.com/repository/gradle-plugin")
        maven(url = "https://maven.aliyun.com/repository/google")
        maven(url = "https://maven.aliyun.com/repository/public")
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // China mirrors first; official repositories remain as fallback.
        maven(url = "https://maven.aliyun.com/repository/google")
        maven(url = "https://maven.aliyun.com/repository/public")
        google()
        mavenCentral()
        // pdfbox-android（PDF 解析）在 JitPack 上
        maven(url = "https://jitpack.io")
    }
}

rootProject.name = "LocalFile"

include(":app")
