//! Engine error type, serialized to the UI as a readable string.

use serde::{Serialize, Serializer};

/// Errors surfaced across the IPC boundary. They serialize to a plain string so
/// the frontend receives a human-readable message from a rejected `invoke`.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine is already running")]
    AlreadyRunning,
    #[error("engine is not running")]
    NotRunning,
    #[error("engine controller lock was poisoned")]
    LockPoisoned,
}

impl Serialize for EngineError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Convenience alias used throughout the engine.
pub type Result<T> = std::result::Result<T, EngineError>;
