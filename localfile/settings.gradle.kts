pluginManagement {
    repositories {
        // 阿里云镜像只在本机（中国大陆网络）启用：拉取快。
        // CI 必须绕开它 —— GitHub runner 访问镜像曾返回 502 Bad Gateway，
        // 导致「Build debug APK」直接失败（镜像被禁用后官方源也没能兜住）。
        // GitHub Actions 会自动注入 CI=true。
        if (System.getenv("CI").isNullOrBlank()) {
            maven(url = "https://maven.aliyun.com/repository/gradle-plugin")
            maven(url = "https://maven.aliyun.com/repository/google")
            maven(url = "https://maven.aliyun.com/repository/public")
        }
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // 同上：镜像仅本机使用，CI 走官方源
        if (System.getenv("CI").isNullOrBlank()) {
            maven(url = "https://maven.aliyun.com/repository/google")
            maven(url = "https://maven.aliyun.com/repository/public")
        }
        google()
        mavenCentral()
        // pdfbox-android（PDF 解析）在 JitPack 上
        maven(url = "https://jitpack.io")
    }
}

rootProject.name = "LocalFile"

include(":app")
