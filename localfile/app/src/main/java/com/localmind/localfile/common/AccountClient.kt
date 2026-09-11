package com.localmind.localfile.common

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject

data class RelayDeviceRegistration(
    val deviceId: String,
    val deviceToken: String,
    val createdAt: Long
)

data class RelayAccountUser(
    val id: String,
    val email: String,
    val displayName: String
)

data class RelayAuthSession(
    val user: RelayAccountUser,
    val accessToken: String,
    val refreshToken: String,
    val accessExpiresAt: Long,
    val refreshExpiresAt: Long
)

data class RelayAccountDevice(
    val id: String,
    val deviceName: String,
    val platform: String,
    val model: String,
    val osVersion: String,
    val lastSeen: Long
)

class AccountApiException(
    message: String,
    val code: String,
    val statusCode: Int,
    cause: Throwable? = null
) : Exception(message, cause)

class AccountClient(
    context: Context,
    private val baseUrl: String = DEFAULT_RELAY_URL
) {
    companion object {
        const val DEFAULT_RELAY_URL = "https://39.107.53.230"
        private val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()
    }

    // Relay 使用 Caddy internal CA（纯 IP 部署，暂无域名/ICP），必须显式信任内置 CA。
    private val client = RelayTls.relayHttpClient(context)

    suspend fun registerDevice(
        deviceName: String,
        model: String,
        osVersion: String
    ): RelayDeviceRegistration = withContext(Dispatchers.IO) {
        val body = JSONObject().apply {
            put("device_name", deviceName)
            put("platform", "android")
            put("model", model)
            put("os_version", osVersion)
        }
        val json = execute(
            Request.Builder()
                .url("$baseUrl/v1/devices/register")
                .post(body.toString().toRequestBody(JSON_MEDIA_TYPE))
                .build()
        )
        RelayDeviceRegistration(
            deviceId = json.getString("device_id"),
            deviceToken = json.getString("device_token"),
            createdAt = json.optLong("created_at")
        )
    }

    suspend fun registerAccount(email: String, displayName: String, password: String): RelayAuthSession =
        authRequest(
            path = "/v1/auth/register",
            body = JSONObject().apply {
                put("email", email)
                put("display_name", displayName)
                put("password", password)
            }
        )

    suspend fun loginAccount(email: String, password: String): RelayAuthSession =
        authRequest(
            path = "/v1/auth/login",
            body = JSONObject().apply {
                put("email", email)
                put("password", password)
            }
        )

    suspend fun refreshSession(refreshToken: String): RelayAuthSession = withContext(Dispatchers.IO) {
        val body = JSONObject().put("refresh_token", refreshToken)
        val request = Request.Builder()
            .url("$baseUrl/v1/auth/refresh")
            .post(body.toString().toRequestBody(JSON_MEDIA_TYPE))
            .build()
        parseAuthSession(execute(request))
    }

    suspend fun logout(refreshToken: String): Unit = withContext(Dispatchers.IO) {
        val body = JSONObject().put("refresh_token", refreshToken)
        execute(
            Request.Builder()
                .url("$baseUrl/v1/auth/logout")
                .post(body.toString().toRequestBody(JSON_MEDIA_TYPE))
                .build(),
            expectJson = false
        )
    }

    suspend fun currentUser(accessToken: String): RelayAccountUser = withContext(Dispatchers.IO) {
        val request = Request.Builder()
            .url("$baseUrl/v1/auth/me")
            .header("Authorization", "Bearer $accessToken")
            .get()
            .build()
        parseUser(execute(request))
    }

    suspend fun claimDevice(
        accessToken: String,
        deviceId: String,
        deviceToken: String
    ): RelayAccountDevice = withContext(Dispatchers.IO) {
        val request = Request.Builder()
            .url("$baseUrl/v1/account/devices/claim")
            .header("Authorization", "Bearer $accessToken")
            .header("X-Device-Id", deviceId)
            .header("X-Device-Token", deviceToken)
            .post(EMPTY_BODY)
            .build()
        parseDevice(execute(request))
    }

    suspend fun listDevices(accessToken: String, deviceId: String, deviceToken: String): List<RelayAccountDevice> =
        withContext(Dispatchers.IO) {
            val request = Request.Builder()
                .url("$baseUrl/v1/account/devices")
                .header("Authorization", "Bearer $accessToken")
                .header("X-Device-Id", deviceId)
                .header("X-Device-Token", deviceToken)
                .get()
                .build()
            val array = JSONArray(executeRaw(request))
            buildList {
                for (index in 0 until array.length()) add(parseDevice(array.getJSONObject(index)))
            }
        }

    suspend fun removeDevice(
        accessToken: String,
        deviceId: String,
        deviceToken: String,
        targetDeviceId: String
    ): Unit = withContext(Dispatchers.IO) {
        val request = Request.Builder()
            .url("$baseUrl/v1/account/devices/${encodePath(targetDeviceId)}")
            .header("Authorization", "Bearer $accessToken")
            .header("X-Device-Id", deviceId)
            .header("X-Device-Token", deviceToken)
            .delete()
            .build()
        execute(request, expectJson = false)
    }

    private suspend fun authRequest(path: String, body: JSONObject): RelayAuthSession =
        withContext(Dispatchers.IO) {
            val request = Request.Builder()
                .url("$baseUrl$path")
                .post(body.toString().toRequestBody(JSON_MEDIA_TYPE))
                .build()
            parseAuthSession(execute(request))
        }

    private fun execute(request: Request, expectJson: Boolean = true): JSONObject {
        val body = executeRaw(request)
        if (!expectJson || body.isBlank()) return JSONObject()
        return JSONObject(body)
    }

    private fun executeRaw(request: Request): String {
        client.newCall(request).execute().use { response ->
            val body = response.body?.string().orEmpty()
            if (!response.isSuccessful) {
                val error = runCatching { JSONObject(body).getJSONObject("error") }.getOrNull()
                throw AccountApiException(
                    message = error?.optString("message").orEmpty().ifEmpty { "账号服务请求失败 (${response.code})" },
                    code = error?.optString("code").orEmpty().ifEmpty { response.code.toString() },
                    statusCode = response.code
                )
            }
            return body
        }
    }

    private fun parseAuthSession(json: JSONObject): RelayAuthSession {
        val tokens = json.getJSONObject("tokens")
        return RelayAuthSession(
            user = parseUser(json.getJSONObject("user")),
            accessToken = tokens.getString("access_token"),
            refreshToken = tokens.getString("refresh_token"),
            accessExpiresAt = tokens.optLong("access_expires_at"),
            refreshExpiresAt = tokens.optLong("refresh_expires_at")
        )
    }

    private fun parseUser(json: JSONObject): RelayAccountUser = RelayAccountUser(
        id = json.getString("id"),
        email = json.getString("email"),
        displayName = json.optString("display_name")
    )

    private fun parseDevice(json: JSONObject): RelayAccountDevice = RelayAccountDevice(
        id = json.getString("id"),
        deviceName = json.optString("device_name"),
        platform = json.optString("platform"),
        model = json.optString("model"),
        osVersion = json.optString("os_version"),
        lastSeen = json.optLong("last_seen")
    )

    private fun encodePath(value: String): String = java.net.URLEncoder.encode(value, Charsets.UTF_8.name())

    private val EMPTY_BODY = ByteArray(0).toRequestBody(null)
}
