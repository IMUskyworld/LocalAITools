pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // pdfbox-android（PDF 解析）在 JitPack 上
        maven(url = "https://jitpack.io")
    }
}

rootProject.name = "LocalFile"

include(":app")
