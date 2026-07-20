//! Structured notices surfaced to the UI (remediation hints, errors).

use serde::Serialize;

/// A notice emitted on `engine://notice`. `code` is machine-readable, `message`
/// is for display, and `remediation` (when present) names a UI action.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineNotice {
    pub code: String,
    pub message: String,
    pub remediation: Option<String>,
}

impl EngineNotice {
    pub fn needs_elevation() -> Self {
        Self {
            code: "needs_elevation".into(),
            message:
                "Administrator privileges are required to create the virtual network adapter."
                    .into(),
            remediation: Some("relaunch_elevated".into()),
        }
    }

    pub fn unsupported_platform() -> Self {
        Self {
            code: "unsupported_platform".into(),
            message: "The real network adapter is only available on Windows in this build.".into(),
            remediation: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            code: "engine_error".into(),
            message: message.into(),
            remediation: None,
        }
    }

    /// A non-error informational notice (e.g. discovered candidates).
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            remediation: None,
        }
    }
}
