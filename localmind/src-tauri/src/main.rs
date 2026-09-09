#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent_process;
mod chat_api;
mod chat;
mod common;
mod files;
mod inference;
mod model;
mod storage;
mod tools;

use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub chat_engine: Arc<tokio::sync::RwLock<chat::ChatEngine>>,
    pub model_manager: Arc<tokio::sync::RwLock<model::ModelManager>>,
    pub storage: Arc<tokio::sync::RwLock<storage::StorageManager>>,
    pub agent_manager: Arc<agent_process::AgentManager>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let storage = Arc::new(tokio::sync::RwLock::new(storage::StorageManager::new()));
    let chat_engine = Arc::new(tokio::sync::RwLock::new(chat::ChatEngine::new(storage.clone())));
    let model_manager = Arc::new(tokio::sync::RwLock::new(model::ModelManager::new(storage.clone())));
    let agent_manager = Arc::new(agent_process::AgentManager::new());

    let app_state = AppState { chat_engine, model_manager, storage, agent_manager };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            chat::get_sessions,
            chat_api::list_sessions,
            chat_api::create_chat_session,
            chat_api::delete_chat_session,
            chat_api::rename_chat_session,
            chat_api::list_messages,
            chat_api::append_message,
            chat::create_session,
            chat::delete_session,
            chat::rename_session,
            chat::send_message,
            chat::stop_generation,
            chat::get_messages,
            chat::switch_mode,
            chat::get_mode,
            model::get_models,
            model::start_download,
            model::pause_download,
            model::cancel_download,
            model::verify_model,
            model::delete_model,
            model::start_inference,
            model::stop_inference,
            model::get_inference_state,
            model::check_device,
            storage::get_config,
            storage::save_config,
            storage::get_app_info,
            files::read_text_file,
            tools::write_file,
            tools::append_file,
            tools::read_file,
            tools::list_dir,
            tools::delete_path,
            tools::rename_path,
            tools::run_command,
            tools::open_app,
            tools::get_common_paths,
            tools::check_ollama,
            tools::ollama_chat,
            tools::create_doc,
            agent_process::get_agent_config,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // 应用退出时杀掉 Python Agent 子进程，避免孤儿进程占用端口
                let state = app.state::<AppState>();
                state.agent_manager.kill();
            }
        });
}