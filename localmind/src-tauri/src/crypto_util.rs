//! 本地加密工具 — 用 AES-256-GCM 加密敏感数据后存入 SQLite。
//!
//! 密钥派生：SHA256(device_id + 固定盐)，device_id 首次生成后持久化不变。
//! 这样即使攻击者拿到 SQLite 文件，也无法直接读取明文 key。
//! 注意：这不是操作系统级 DPAPI，但对"防止明文泄露"的需求足够。

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};

const SALT: &[u8] = b"localmind-encryption-salt-v1";

/// 从 device_id 派生 AES-256 密钥（32 字节）。
fn derive_key(device_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SALT);
    hasher.update(device_id.as_bytes());
    hasher.update(SALT);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// 加密明文。返回 nonce(12字节) + ciphertext。
pub fn encrypt(plaintext: &[u8], device_id: &str) -> Vec<u8> {
    let key = derive_key(device_id);
    let cipher = Aes256Gcm::new_from_slice(&key).expect("key length must be 32");
    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext).expect("encryption failed");
    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    output
}

/// 解密密文。输入为 nonce(12) + ciphertext。
pub fn decrypt(data: &[u8], device_id: &str) -> Result<Vec<u8>, String> {
    if data.len() < 12 {
        return Err("encrypted data too short".to_string());
    }
    let key = derive_key(device_id);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("key error: {e}"))?;
    let nonce = Nonce::from_slice(&data[..12]);
    cipher
        .decrypt(nonce, &data[12..])
        .map_err(|_| "decryption failed (wrong device_id or corrupted data)".to_string())
}

/// 加密字符串，返回 base64 编码。
pub fn encrypt_string(plaintext: &str, device_id: &str) -> String {
    base64_encode(&encrypt(plaintext.as_bytes(), device_id))
}

/// 解密 base64 编码的密文，返回明文字符串。
pub fn decrypt_string(encoded: &str, device_id: &str) -> Result<String, String> {
    let data = base64_decode(encoded)?;
    let plaintext = decrypt(&data, device_id)?;
    String::from_utf8(plaintext).map_err(|_| "decrypted data is not valid UTF-8".to_string())
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("base64 decode error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let device_id = "test-device-id-123";
        let secret = "sk-abc123def456";
        let encrypted = encrypt_string(secret, device_id);
        assert_ne!(encrypted, secret);
        let decrypted = decrypt_string(&encrypted, device_id).unwrap();
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn wrong_device_fails() {
        let encrypted = encrypt_string("secret", "device-a");
        assert!(decrypt_string(&encrypted, "device-b").is_err());
    }
}
