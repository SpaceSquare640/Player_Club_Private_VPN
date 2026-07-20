//! Identity command — exposes this node's Peer Address + public key to the UI.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::engine::crypto::Identity;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    /// Short human-readable fingerprint, e.g. `PC-3F8A-9C2D-71B0-44EE`.
    pub peer_address: String,
    /// Base64 of the public key — paste this into a peer to connect (C3).
    pub public_key_b64: String,
}

#[tauri::command]
pub fn get_identity(identity: State<'_, Arc<Identity>>) -> IdentityInfo {
    IdentityInfo {
        peer_address: identity.peer_address(),
        public_key_b64: identity.public_b64(),
    }
}
