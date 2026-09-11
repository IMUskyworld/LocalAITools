//! LocalMind Relay library.

pub mod account;
mod account_db;
pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod hub;
pub mod protocol;

pub use account_db::{AccountDevice, AuthTokens, DevicePairing, PairingRequest, User};
pub use api::{build_router, AppState};
pub use config::Config;
pub use db::Db;
