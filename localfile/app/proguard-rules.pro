# Keep Room entities
-keep class com.localmind.localfile.storage.** { *; }

# Keep JNI bridge
-keep class com.localmind.localfile.inference.LlamaBridge { *; }

# Keep envelope types for JSON serialization
-keep class com.localmind.localfile.common.CommandEnvelope { *; }
-keep class com.localmind.localfile.common.DeviceInfo { *; }

# OkHttp
-dontwarn okhttp3.**
-dontwarn okio.**

# ZXing
-keep class com.google.zxing.** { *; }
