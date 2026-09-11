package com.localmind.localfile.common

import android.content.Context
import com.localmind.localfile.R
import okhttp3.OkHttpClient
import java.security.KeyStore
import java.security.cert.CertificateException
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManagerFactory
import javax.net.ssl.X509TrustManager

/**
 * Relay 目前部署在纯公网 IP 上（暂无域名/ICP），使用 Caddy internal CA 签发证书。
 * Android 系统信任链不包含该 CA，因此 Relay 请求统一使用这里构造的 OkHttpClient：
 * 在系统信任之外，额外信任随 APK 一起分发的 internal CA。
 *
 * 注意：只信任仓库内置的这一个 CA，不提供「忽略证书错误」的开关。
 */
object RelayTls {

    fun relayHttpClient(
        context: Context,
        connectTimeoutSeconds: Long = 15,
        readTimeoutSeconds: Long = 30,
        writeTimeoutSeconds: Long = 20
    ): OkHttpClient {
        val trustManager = combinedTrustManager(context)
        val sslContext = SSLContext.getInstance("TLS").apply {
            init(null, arrayOf<X509TrustManager>(trustManager), null)
        }
        return OkHttpClient.Builder()
            .sslSocketFactory(sslContext.socketFactory, trustManager)
            .connectTimeout(connectTimeoutSeconds, TimeUnit.SECONDS)
            .readTimeout(readTimeoutSeconds, TimeUnit.SECONDS)
            .writeTimeout(writeTimeoutSeconds, TimeUnit.SECONDS)
            .build()
    }

    private fun combinedTrustManager(context: Context): X509TrustManager =
        CombinedTrustManager(
            primary = systemTrustManager(),
            additional = pinnedTrustManager(context)
        )

    private fun systemTrustManager(): X509TrustManager {
        val factory = TrustManagerFactory
            .getInstance(TrustManagerFactory.getDefaultAlgorithm())
            .apply { init(null as KeyStore?) }
        return factory.trustManagers.filterIsInstance<X509TrustManager>().firstOrNull()
            ?: error("系统 TrustManager 不可用")
    }

    private fun pinnedTrustManager(context: Context): X509TrustManager {
        val certificate = context.resources
            .openRawResource(R.raw.localmind_relay_ca)
            .use { stream -> CertificateFactory.getInstance("X.509").generateCertificate(stream) }

        val keyStore = KeyStore.getInstance(KeyStore.getDefaultType()).apply {
            load(null)
            setCertificateEntry("localmind-relay-ca", certificate)
        }
        val factory = TrustManagerFactory
            .getInstance(TrustManagerFactory.getDefaultAlgorithm())
            .apply { init(keyStore) }
        return factory.trustManagers.filterIsInstance<X509TrustManager>().firstOrNull()
            ?: error("无法从内置 Relay CA 构建 TrustManager")
    }

    /** 先走系统信任链，失败后再尝试内置 Relay CA。 */
    private class CombinedTrustManager(
        private val primary: X509TrustManager,
        private val additional: X509TrustManager
    ) : X509TrustManager {

        override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {
            try {
                primary.checkClientTrusted(chain, authType)
            } catch (error: CertificateException) {
                additional.checkClientTrusted(chain, authType)
            }
        }

        override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {
            try {
                primary.checkServerTrusted(chain, authType)
            } catch (error: CertificateException) {
                additional.checkServerTrusted(chain, authType)
            }
        }

        override fun getAcceptedIssuers(): Array<X509Certificate> =
            primary.acceptedIssuers + additional.acceptedIssuers
    }
}
