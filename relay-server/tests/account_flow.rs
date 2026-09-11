use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpListener;

#[derive(Debug, Clone, Deserialize)]
struct RegisterResponse {
    device_id: String,
    device_token: String,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    user: UserResponse,
    tokens: TokensResponse,
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    id: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct TokensResponse {
    access_token: String,
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
struct AccountDeviceResponse {
    platform: String,
}

#[derive(Debug, Deserialize)]
struct PairingRequestResponse {
    id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct DevicePairingResponse {
    tenant_id: String,
    permissions: Vec<String>,
}

#[tokio::test]
async fn account_auth_enrollment_and_control_approval_flow() {
    let temp = tempfile::tempdir().unwrap();
    let config = localmind_relay::Config {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        db_path: temp.path().join("relay.db"),
        max_message_bytes: 64 * 1024,
        max_pending_messages_per_device: 500,
        pairing_code_ttl_seconds: 300,
        pairing_request_ttl_seconds: 600,
        command_ttl_seconds: 7 * 24 * 60 * 60,
        access_token_ttl_seconds: 30 * 60,
        refresh_token_ttl_seconds: 30 * 24 * 60 * 60,
    };
    let db = localmind_relay::Db::open(&config.db_path).unwrap();
    let app = localmind_relay::build_router(localmind_relay::AppState::new(db.clone(), config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let base_url = format!("http://{address}");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let http = reqwest::Client::new();
    let windows = register_device(&http, &base_url, "desktop", "windows").await;
    let android = register_device(&http, &base_url, "phone", "android").await;

    let auth: AuthResponse = http
        .post(format!("{base_url}/v1/auth/register"))
        .json(&json!({
            "email": "student@example.com",
            "display_name": "Student",
            "password": "correct-horse-battery-staple"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(auth.user.email, "student@example.com");

    let access = auth.tokens.access_token.clone();
    let windows_enrollment: AccountDeviceResponse = claim_device(
        &http,
        &base_url,
        &access,
        &windows.device_id,
        &windows.device_token,
    )
    .await;
    let android_enrollment: AccountDeviceResponse = claim_device(
        &http,
        &base_url,
        &access,
        &android.device_id,
        &android.device_token,
    )
    .await;
    assert_eq!(windows_enrollment.platform, "windows");
    assert_eq!(android_enrollment.platform, "android");

    let devices: Vec<AccountDeviceResponse> = http
        .get(format!("{base_url}/v1/account/devices"))
        .header("authorization", format!("Bearer {access}"))
        .header("x-device-id", &windows.device_id)
        .header("x-device-token", &windows.device_token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(devices.len(), 2);

    let request: PairingRequestResponse = http
        .post(format!("{base_url}/v1/control/pairing-requests"))
        .header("authorization", format!("Bearer {access}"))
        .header("x-device-id", &android.device_id)
        .header("x-device-token", &android.device_token)
        .json(&json!({
            "target_device_id": windows.device_id,
            "permissions": ["device.ping", "device.status"]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(request.status, "pending");

    let before_approval: Vec<DevicePairingResponse> = http
        .get(format!("{base_url}/v1/control/pairings"))
        .header("authorization", format!("Bearer {access}"))
        .header("x-device-id", &windows.device_id)
        .header("x-device-token", &windows.device_token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        before_approval.is_empty(),
        "same account must not imply control authorization"
    );

    let pairing: DevicePairingResponse = http
        .post(format!(
            "{base_url}/v1/control/pairing-requests/{}/approve",
            request.id
        ))
        .header("authorization", format!("Bearer {access}"))
        .header("x-device-id", &windows.device_id)
        .header("x-device-token", &windows.device_token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(pairing.permissions.contains(&"device.ping".to_string()));

    let route = db
        .route_between(&pairing.tenant_id, &android.device_id, &windows.device_id)
        .unwrap()
        .expect("approved control pairing must create a route");
    assert_eq!(route.permissions.unwrap(), pairing.permissions);

    let refreshed: AuthResponse = http
        .post(format!("{base_url}/v1/auth/refresh"))
        .json(&json!({ "refresh_token": auth.tokens.refresh_token }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(refreshed.user.id, auth.user.id);

    let me: UserResponse = http
        .get(format!("{base_url}/v1/auth/me"))
        .header(
            "authorization",
            format!("Bearer {}", refreshed.tokens.access_token),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me.id, auth.user.id);

    let old_refresh = http
        .post(format!("{base_url}/v1/auth/refresh"))
        .json(&json!({ "refresh_token": auth.tokens.refresh_token }))
        .send()
        .await
        .unwrap();
    assert_eq!(old_refresh.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(old_refresh).await, "403005");

    let remove = http
        .delete(format!(
            "{base_url}/v1/account/devices/{}",
            android.device_id
        ))
        .header(
            "authorization",
            format!("Bearer {}", refreshed.tokens.access_token),
        )
        .header("x-device-id", &windows.device_id)
        .header("x-device-token", &windows.device_token)
        .send()
        .await
        .unwrap();
    assert_eq!(remove.status(), reqwest::StatusCode::NO_CONTENT);
    assert!(
        db.route_between(&pairing.tenant_id, &android.device_id, &windows.device_id)
            .unwrap()
            .is_none(),
        "removing an account device must revoke its control routes"
    );

    let logout = http
        .post(format!("{base_url}/v1/auth/logout"))
        .json(&json!({ "refresh_token": refreshed.tokens.refresh_token }))
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), reqwest::StatusCode::NO_CONTENT);

    let after_logout = http
        .post(format!("{base_url}/v1/auth/refresh"))
        .json(&json!({ "refresh_token": refreshed.tokens.refresh_token }))
        .send()
        .await
        .unwrap();
    assert_eq!(after_logout.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(after_logout).await, "403005");
}

#[tokio::test]
async fn invalid_credentials_and_duplicate_email_have_stable_error_codes() {
    let temp = tempfile::tempdir().unwrap();
    let config = localmind_relay::Config {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        db_path: temp.path().join("relay.db"),
        max_message_bytes: 64 * 1024,
        max_pending_messages_per_device: 500,
        pairing_code_ttl_seconds: 300,
        pairing_request_ttl_seconds: 600,
        command_ttl_seconds: 7 * 24 * 60 * 60,
        access_token_ttl_seconds: 30 * 60,
        refresh_token_ttl_seconds: 30 * 24 * 60 * 60,
    };
    let db = localmind_relay::Db::open(&config.db_path).unwrap();
    let app = localmind_relay::build_router(localmind_relay::AppState::new(db, config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let base_url = format!("http://{address}");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let http = reqwest::Client::new();
    let payload = json!({
        "email": "duplicate@example.com",
        "display_name": "Duplicate",
        "password": "valid-password-123"
    });

    http.post(format!("{base_url}/v1/auth/register"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let duplicate = http
        .post(format!("{base_url}/v1/auth/register"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(error_code(duplicate).await, "403003");

    let invalid = http
        .post(format!("{base_url}/v1/auth/login"))
        .json(&json!({
            "email": "duplicate@example.com",
            "password": "wrong-password"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(invalid).await, "403004");
}

async fn register_device(
    http: &reqwest::Client,
    base_url: &str,
    device_name: &str,
    platform: &str,
) -> RegisterResponse {
    http.post(format!("{base_url}/v1/devices/register"))
        .json(&json!({
            "device_name": device_name,
            "platform": platform,
            "model": "test",
            "os_version": "test"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn claim_device(
    http: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    device_id: &str,
    device_token: &str,
) -> AccountDeviceResponse {
    http.post(format!("{base_url}/v1/account/devices/claim"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-device-id", device_id)
        .header("x-device-token", device_token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn error_code(response: reqwest::Response) -> String {
    let value: serde_json::Value = response.json().await.unwrap();
    value["error"]["code"]
        .as_str()
        .expect("error response must contain error.code")
        .to_string()
}
