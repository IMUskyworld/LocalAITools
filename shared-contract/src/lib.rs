//! Shared protocol contracts for LocalMind, LocalFile and Relay.
//!
//! The versioned protocol files remain under `envelope/v1/` so the TypeScript
//! and Kotlin copies stay visible next to their Rust counterpart.

pub mod envelope {
    pub mod v1 {
        include!("../envelope/v1/envelope.rs");
    }
}

pub use envelope::v1::*;
