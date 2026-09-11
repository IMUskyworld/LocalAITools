//! Relay HTTP 通道。
//!
//! Relay 目前部署在纯公网 IP 上（暂无域名/ICP），使用 Caddy internal CA 签发证书。
//! WebView2 的 fetch 走 Windows 系统信任链，无法直接访问该证书，因此账号、设备
//! 注册等 Relay 请求统一在本模块发起：这里显式把仓库内置的 internal CA 加入信任
//! 锚，不再依赖操作系统证书存储。
//!
//! 安全边界：
//! - 仅允许 http/https，仅允许相对路径，禁止把 base_url 与 path 拼成任意主机；
//! - 不提供关闭证书校验的开关，禁止 `danger_accept_invalid_certs`。

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;

/// 仓库内置的 Caddy internal CA（编译期嵌入，随可执行文件一起分发）。
const RELAY_CA_PEM: &[u8] = include_bytes!("../certs/localmind-relay-ca.crt");

const RELAY_TIMEOUT_SECS: u64 = 25;
const RELAY_CONNECT_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Serialize)]
pub struct RelayHttpResponse {
    /// HTTP 状态码，4xx/5xx 也按原样返回，由前端决定错误语义。
    pub status: u16,
    /// 原始响应体（当前 Relay 只返回 JSON）。
    pub body: String,
}

fn build_client() -> Result<reqwest::Client, String> {
    let certificate = reqwest::Certificate::from_pem(RELAY_CA_PEM)
        .map_err(|e| format!("内置 Relay 根证书无法解析: {e}"))?;

    reqwest::Client::builder()
        .use_rustls_tls()
        .add_root_certificate(certificate)
        .timeout(Duration::from_secs(RELAY_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(RELAY_CONNECT_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("创建 Relay HTTP 客户端失败: {e}"))
}

fn normalize_base_url(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Relay 地址为空".to_string());
    }
    let parsed = url::Url::parse(trimmed).map_err(|e| format!("Relay 地址无效: {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("Relay 地址只支持 http/https，当前为 {other}")),
    }
    if parsed.host_str().unwrap_or_default().is_empty() {
        return Err("Relay 地址缺少主机名".to_string());
    }
    Ok(trimmed.to_string())
}

fn validate_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err("Relay 请求路径必须以 / 开头".to_string());
    }
    if path.contains(char::is_whitespace) {
        return Err("Relay 请求路径不能包含空白字符".to_string());
    }
    if path.contains("//") || path.contains("..") {
        return Err("Relay 请求路径包含非法片段".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn relay_http_request(
    base_url: String,
    method: String,
    path: String,
    body: Option<String>,
    access_token: Option<String>,
    headers: Option<HashMap<String, String>>,
) -> Result<RelayHttpResponse, String> {
    let base = normalize_base_url(&base_url)?;
    validate_path(&path)?;
    let url = format!("{base}{path}");

    let parsed_method = reqwest::Method::from_bytes(method.to_uppercase().as_bytes())
        .map_err(|_| format!("不支持的 HTTP 方法: {method}"))?;

    let client = build_client()?;
    let mut request = client
        .request(parsed_method, &url)
        .header(reqwest::header::ACCEPT, "application/json");

    if let Some(token) = access_token.filter(|value| !value.is_empty()) {
        request = request.bearer_auth(token);
    }

    if let Some(extra_headers) = headers {
        for (name, value) in extra_headers {
            let header_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| format!("请求头名称非法: {name}"))?;
            let header_value = reqwest::header::HeaderValue::from_str(&value)
                .map_err(|_| format!("请求头 {name} 的值包含非法字符"))?;
            request = request.header(header_name, header_value);
        }
    }

    if let Some(payload) = body {
        request = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("连接 Relay 失败: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 Relay 响应失败: {e}"))?;

    Ok(RelayHttpResponse { status, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_expected_base_urls() {
        assert!(normalize_base_url("https://39.107.53.230/").is_ok());
        assert!(normalize_base_url("http://127.0.0.1:8080").is_ok());
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(normalize_base_url("file:///etc/passwd").is_err());
        assert!(normalize_base_url("").is_err());
    }

    #[test]
    fn rejects_unsafe_paths() {
        assert!(validate_path("/v1/auth/login").is_ok());
        assert!(validate_path("v1/auth/login").is_err());
        assert!(validate_path("/v1/../secrets").is_err());
        assert!(validate_path("/v1/auth login").is_err());
    }

    #[test]
    fn embeds_relay_ca() {
        let pem = std::str::from_utf8(RELAY_CA_PEM).expect("CA 必须是有效的 UTF-8 PEM");
        assert!(pem.contains("BEGIN CERTIFICATE"), "内置 CA 不是 PEM 证书");
    }
}
