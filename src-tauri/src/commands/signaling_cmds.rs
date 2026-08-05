//! Manual-signaling commands (UI ↔ engine).

use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rand::RngCore;
use tauri::State;

use crate::engine::connection::{ConnectionManager, ConnectionSnapshot, NegotiatedPeer};
use crate::engine::crypto::Identity;
use crate::engine::nat::candidate::Candidate;
use crate::engine::signaling::blob;
use crate::engine::signaling::message::{SignalEnvelope, SignalKind, WireCandidate, PROTOCOL_VERSION};

// STUN server for candidate gathering (matches the C1 default).
const STUN_SERVER: &str = "stun.l.google.com:19302";

fn wire_candidates(cands: &[Candidate]) -> Vec<WireCandidate> {
    cands
        .iter()
        .map(|c| WireCandidate {
            a: c.addr.to_string(),
            k: c.kind.as_str().to_string(),
        })
        .collect()
}

fn to_peer(envelope: &SignalEnvelope) -> Result<NegotiatedPeer, String> {
    // `blob::decode` already validated the key length and candidate addresses.
    let public_key = STANDARD
        .decode(envelope.pk.trim())
        .map_err(|e| format!("public key: {e}"))?;
    let mut candidates = Vec::with_capacity(envelope.cands.len());
    for c in &envelope.cands {
        candidates.push(
            c.a.parse()
                .map_err(|_| format!("candidate address '{}'", c.a))?,
        );
    }
    Ok(NegotiatedPeer {
        public_key,
        candidates,
    })
}

/// Create an Offer blob (binds the socket + gathers candidates).
#[tauri::command]
pub async fn create_offer(
    identity: State<'_, Arc<Identity>>,
    manager: State<'_, Arc<ConnectionManager>>,
) -> Result<String, String> {
    manager
        .ensure_socket(STUN_SERVER)
        .await
        .map_err(|e| e.to_string())?;

    let mut sid_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut sid_bytes);
    let sid = STANDARD.encode(sid_bytes);
    manager.begin_offer(sid.clone());

    let envelope = SignalEnvelope {
        v: PROTOCOL_VERSION,
        kind: SignalKind::Offer,
        sid,
        pk: identity.public_b64(),
        cands: wire_candidates(&manager.local_candidates()),
    };
    Ok(blob::encode(&envelope))
}

/// Accept a peer's Offer and return our Answer blob (responder side).
#[tauri::command]
pub async fn accept_offer(
    blob_str: String,
    identity: State<'_, Arc<Identity>>,
    manager: State<'_, Arc<ConnectionManager>>,
) -> Result<String, String> {
    let offer = blob::decode(&blob_str).map_err(|e| e.to_string())?;
    if offer.kind != SignalKind::Offer {
        return Err("expected an offer blob".into());
    }
    manager
        .ensure_socket(STUN_SERVER)
        .await
        .map_err(|e| e.to_string())?;

    manager.begin_answer(offer.sid.clone());
    manager.set_peer(to_peer(&offer)?);

    let envelope = SignalEnvelope {
        v: PROTOCOL_VERSION,
        kind: SignalKind::Answer,
        sid: offer.sid,
        pk: identity.public_b64(),
        cands: wire_candidates(&manager.local_candidates()),
    };
    Ok(blob::encode(&envelope))
}

/// Accept a peer's Answer to our Offer (initiator side).
#[tauri::command]
pub fn accept_answer(
    blob_str: String,
    manager: State<'_, Arc<ConnectionManager>>,
) -> Result<(), String> {
    let answer = blob::decode(&blob_str).map_err(|e| e.to_string())?;
    if answer.kind != SignalKind::Answer {
        return Err("expected an answer blob".into());
    }
    match manager.session_id() {
        Some(sid) if sid == answer.sid => {}
        Some(_) => return Err("answer session id does not match the pending offer".into()),
        None => return Err("no pending offer — create an offer first".into()),
    }
    manager.set_peer(to_peer(&answer)?);
    Ok(())
}

/// Current connection/negotiation status for the UI.
#[tauri::command]
pub fn get_connection(manager: State<'_, Arc<ConnectionManager>>) -> ConnectionSnapshot {
    manager.snapshot()
}
