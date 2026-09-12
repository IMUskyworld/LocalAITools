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

# pdfbox-android: JPX filter references optional JP2 classes not bundled on Android
-dontwarn com.gemalto.jp2.JP2Decoder
-dontwarn com.gemalto.jp2.JP2Encoder

# AndroidX Security / Tink (EncryptedSharedPreferences dependency)
-dontwarn com.google.errorprone.annotations.CanIgnoreReturnValue
-dontwarn com.google.errorprone.annotations.CheckReturnValue
-dontwarn com.google.errorprone.annotations.Immutable
-dontwarn com.google.errorprone.annotations.RestrictedApi
-keep class com.google.crypto.tink.** { *; }
-keep class com.google.crypto.tink.proto.** { *; }

# Tink / Google HTTP client (EncryptedSharedPreferences transitive dependency)
-dontwarn com.google.api.client.**
-dontwarn com.google.crypto.tink.**
-keep class com.google.crypto.tink.** { *; }
-keep class com.google.crypto.tink.proto.** { *; }
-keep class com.google.protobuf.** { *; }
-dontwarn com.google.protobuf.**