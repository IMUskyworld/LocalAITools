// 模型管理模块 — M2
// 模型下载/校验、推理服务启停、设备检测
// 对齐：系统设计 §3.2.M2

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::State;

use crate::common::{AppConfig, AppResponse};
use crate::storage::StorageManager;

/// 模型资产状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAsset {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub license: String,
    pub platform: String,
    pub status: String,
    pub downloaded_bytes: u64,
    pub updated_at: i64,
}

/// 下载任务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub asset_id: String,
    pub state: String,
    pub percent: f32,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: u64,
}

/// 推理服务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceState {
    pub state: String,
    pub port: u16,
    pub memory_mb: u64,
}

/// 设备检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCheck {
    pub ram_mb: u64,
    pub soc_model: String,
    pub free_storage_mb: u64,
    pub recommended_tier: String,
    pub check_passed: bool,
    pub reason: String,
}

/// 模型管理器
pub struct ModelManager {
    config: AppConfig,
    storage: Arc<RwLock<StorageManager>>,
    inference_running: RwLock<bool>,
    inference_port: RwLock<u16>,
}

impl ModelManager {
    pub fn new(storage: Arc<RwLock<StorageManager>>) -> Self {
        Self {
            config: AppConfig::load(),
            storage,
            inference_running: RwLock::new(false),
            inference_port: RwLock::new(8081),
        }
    }

    pub async fn get_state(&self) -> String {
        if *self.inference_running.read().await {
            "running".to_string()
        } else {
            "stopped".to_string()
        }
    }
}

// ========== IPC Commands ==========

#[tauri::command]
pub async fn get_models(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<Vec<ModelAsset>>, String> {
    let config = AppConfig::load();
    let model = config.model.default_windows;

    let assets = vec![ModelAsset {
        id: "default-windows".to_string(),
        name: model.name,
        size_bytes: model.size_mb * 1024 * 1024,
        license: model.license,
        platform: "windows".to_string(),
        status: "available".to_string(),
        downloaded_bytes: 0,
        updated_at: chrono::Utc::now().timestamp_millis(),
    }];

    Ok(AppResponse::ok(assets))
}

#[tauri::command]
pub async fn start_download(
    asset_id: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<DownloadTask>, String> {
    // 模拟下载任务
    Ok(AppResponse::ok(DownloadTask {
        asset_id,
        state: "running".to_string(),
        percent: 0.0,
        speed_bytes_per_sec: 5_242_880, // 5MB/s
        eta_seconds: 1000,
    }))
}

#[tauri::command]
pub async fn pause_download(
    asset_id: String,
) -> Result<AppResponse<bool>, String> {
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn cancel_download(
    asset_id: String,
) -> Result<AppResponse<bool>, String> {
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn verify_model(
    asset_id: String,
) -> Result<AppResponse<bool>, String> {
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn delete_model(
    asset_id: String,
) -> Result<AppResponse<bool>, String> {
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn start_inference(
    _threads: Option<u32>,
    _context_length: Option<u32>,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<InferenceState>, String> {
    let mgr = state.model_manager.read().await;
    let mut running = mgr.inference_running.write().await;
    *running = true;
    let port = *mgr.inference_port.read().await;

    Ok(AppResponse::ok(InferenceState {
        state: "running".to_string(),
        port,
        memory_mb: 5120,
    }))
}

#[tauri::command]
pub async fn stop_inference(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<InferenceState>, String> {
    let mgr = state.model_manager.read().await;
    let mut running = mgr.inference_running.write().await;
    *running = false;
    let port = *mgr.inference_port.read().await;

    Ok(AppResponse::ok(InferenceState {
        state: "stopped".to_string(),
        port,
        memory_mb: 0,
    }))
}

#[tauri::command]
pub async fn get_inference_state(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<InferenceState>, String> {
    let mgr = state.model_manager.read().await;
    let running = *mgr.inference_running.read().await;
    let port = *mgr.inference_port.read().await;

    Ok(AppResponse::ok(InferenceState {
        state: if running { "running".to_string() } else { "stopped".to_string() },
        port,
        memory_mb: if running { 5120 } else { 0 },
    }))
}

#[tauri::command]
pub async fn check_device(
) -> Result<AppResponse<DeviceCheck>, String> {
    // 模拟设备检测
    Ok(AppResponse::ok(DeviceCheck {
        ram_mb: 16384,
        soc_model: "Intel Core i7-12700H".to_string(),
        free_storage_mb: 102400,
        recommended_tier: "8B".to_string(),
        check_passed: true,
        reason: "推荐使用 8B 模型档位".to_string(),
    }))
}
