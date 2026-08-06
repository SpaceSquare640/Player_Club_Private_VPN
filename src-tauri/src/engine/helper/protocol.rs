//! Wire protocol for the elevation helper (Phase E.3, step 1). Deliberately a
//! small, closed set of privileged operations — everything the helper can be
//! asked to do is enumerated here, nothing more. The main app stays
//! unelevated and never sends arbitrary commands; the helper is not a
//! general-purpose remote shell.
//!
//! Framing: newline-delimited JSON, one message per line — the same
//! convention the signaling server (`signaling::protocol`) already uses for
//! a very similar problem (typed request/response messages over a
//! long-lived stream), reused here rather than inventing a second framing.
//! A pipe transport can therefore read with a plain line reader and no
//! length prefix: a JSON string escapes any embedded control character
//! (`serde_json` renders a literal `\n` inside a string as the two-character
//! sequence `\` `n`), so an adapter name containing a real newline can never
//! produce a serialized line that itself contains one — line framing is
//! safe by construction, not by a separate check on our part. See
//! `encode_never_produces_an_embedded_newline_even_when_the_input_does`.
//!
//! Not yet wired into any pipe transport or Tauri command (that's step 2 —
//! see the module doc comment on `engine::helper`), hence the explicit
//! allow rather than papering over each item individually.
#![allow(dead_code)]

use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

/// Current helper protocol version. Bumped on any breaking change to
/// [`HelperRequest`]/[`HelperResponse`]; the helper rejects a request whose
/// `v` it does not understand rather than guessing at a partial match.
pub const HELPER_PROTOCOL_VERSION: u32 = 1;

/// A network to route into the adapter, mirroring
/// `split_tunnel::Ipv4Cidr`/`TunConfig::extra_routes`'s `(network, prefix)`
/// shape. A plain tuple-of-fields struct (not imported from `split_tunnel`
/// or `tun::device`) so this protocol module has no dependency on either —
/// callers on both ends convert at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteSpec {
    pub network: Ipv4Addr,
    pub prefix: u8,
}

/// One privileged operation the main app can ask the helper to perform.
/// Every variant corresponds 1:1 to an existing function in
/// `tun::windows` — the helper's job is to run that same function with
/// these arguments, not to expose any broader capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HelperRequest {
    /// Create the Wintun adapter and assign its IPv4 address —
    /// `tun::windows::WintunDevice::open`'s adapter-creation and
    /// `assign_ip` steps.
    CreateAdapter { v: u32, name: String, virtual_ip: Ipv4Addr, prefix_len: u8 },
    /// `tun::windows::configure_network_integration`.
    ConfigureNetworkIntegration { v: u32, adapter_name: String },
    /// `tun::windows::remove_network_integration` (adapter teardown).
    RemoveNetworkIntegration { v: u32, adapter_name: String },
    /// `tun::windows::add_extra_routes` (Phase E.2).
    AddExtraRoutes { v: u32, adapter_name: String, routes: Vec<RouteSpec> },
    /// `tun::windows::remove_extra_routes` (Phase E.2, adapter teardown).
    RemoveExtraRoutes { v: u32, adapter_name: String, routes: Vec<RouteSpec> },
    /// Ask the helper to exit. Sent once the main app no longer needs any
    /// privileged operation for the remainder of the session.
    Shutdown { v: u32 },
}

impl HelperRequest {
    /// The protocol version this request was constructed with.
    pub fn version(&self) -> u32 {
        match self {
            HelperRequest::CreateAdapter { v, .. }
            | HelperRequest::ConfigureNetworkIntegration { v, .. }
            | HelperRequest::RemoveNetworkIntegration { v, .. }
            | HelperRequest::AddExtraRoutes { v, .. }
            | HelperRequest::RemoveExtraRoutes { v, .. }
            | HelperRequest::Shutdown { v } => *v,
        }
    }
}

/// The helper's reply to one [`HelperRequest`]. `CreateAdapter` is the only
/// request whose success carries data back (the actual bound port isn't
/// relevant here the way it is for `SignalingServer::local_addr` — Wintun
/// sessions are opened by the *main* app against the adapter the helper
/// created, so success is enough; failure carries a message either way).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HelperResponse {
    Ok,
    /// The request's `v` is not one this helper build understands.
    UnsupportedVersion { supported: u32 },
    /// The operation was attempted and failed. `message` is
    /// human-readable-only — never parsed by the caller — mirroring every
    /// other `io::Error`-to-`String` boundary already in this codebase
    /// (`connect_peer`, `create_network`, …).
    Error { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum HelperCodecError {
    #[error("failed to serialize helper message: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("failed to parse helper message: {0}")]
    Parse(#[source] serde_json::Error),
}

fn encode<T: Serialize>(message: &T) -> Result<String, HelperCodecError> {
    serde_json::to_string(message).map_err(HelperCodecError::Serialize)
}

/// Encode one request as a single line (no trailing newline — the caller's
/// transport, e.g. a pipe writer, appends its own line terminator).
pub fn encode_request(request: &HelperRequest) -> Result<String, HelperCodecError> {
    encode(request)
}

/// Encode one response as a single line.
pub fn encode_response(response: &HelperResponse) -> Result<String, HelperCodecError> {
    encode(response)
}

/// Decode one line (leading/trailing whitespace, e.g. a trailing `\r` from a
/// CRLF-terminated pipe, is trimmed first) into a request.
pub fn decode_request(line: &str) -> Result<HelperRequest, HelperCodecError> {
    serde_json::from_str(line.trim()).map_err(HelperCodecError::Parse)
}

/// Decode one line into a response.
pub fn decode_response(line: &str) -> Result<HelperResponse, HelperCodecError> {
    serde_json::from_str(line.trim()).map_err(HelperCodecError::Parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_requests() -> Vec<HelperRequest> {
        vec![
            HelperRequest::CreateAdapter {
                v: HELPER_PROTOCOL_VERSION,
                name: "PlayerClubVPN".to_string(),
                virtual_ip: Ipv4Addr::new(10, 77, 0, 1),
                prefix_len: 24,
            },
            HelperRequest::ConfigureNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            },
            HelperRequest::RemoveNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            },
            HelperRequest::AddExtraRoutes {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
                routes: vec![
                    RouteSpec { network: Ipv4Addr::new(192, 168, 50, 0), prefix: 24 },
                    RouteSpec { network: Ipv4Addr::new(10, 0, 5, 0), prefix: 24 },
                ],
            },
            HelperRequest::RemoveExtraRoutes {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
                routes: vec![RouteSpec { network: Ipv4Addr::new(192, 168, 50, 0), prefix: 24 }],
            },
            HelperRequest::Shutdown { v: HELPER_PROTOCOL_VERSION },
        ]
    }

    #[test]
    fn every_request_variant_round_trips() {
        for req in sample_requests() {
            let line = encode_request(&req).unwrap();
            assert!(!line.contains('\n'), "encoded line must not embed a newline");
            assert_eq!(decode_request(&line).unwrap(), req);
        }
    }

    #[test]
    fn every_response_variant_round_trips() {
        let responses = [
            HelperResponse::Ok,
            HelperResponse::UnsupportedVersion { supported: HELPER_PROTOCOL_VERSION },
            HelperResponse::Error { message: "adapter creation failed: access denied".to_string() },
        ];
        for resp in responses {
            let line = encode_response(&resp).unwrap();
            assert_eq!(decode_response(&line).unwrap(), resp);
        }
    }

    #[test]
    fn decode_trims_a_trailing_carriage_return() {
        // A pipe opened in text mode, or a naive CRLF writer on the other
        // end, can leave a trailing \r on the line a reader splits on \n.
        let req = HelperRequest::Shutdown { v: HELPER_PROTOCOL_VERSION };
        let line = encode_request(&req).unwrap();
        assert_eq!(decode_request(&format!("{line}\r")).unwrap(), req);
    }

    #[test]
    fn decode_rejects_malformed_json() {
        assert!(matches!(decode_request("{not json"), Err(HelperCodecError::Parse(_))));
        assert!(matches!(decode_response("{not json"), Err(HelperCodecError::Parse(_))));
    }

    #[test]
    fn decode_rejects_a_response_shaped_line_as_a_request_and_vice_versa() {
        let response_line = encode_response(&HelperResponse::Ok).unwrap();
        assert!(decode_request(&response_line).is_err());

        let request_line = encode_request(&HelperRequest::Shutdown { v: HELPER_PROTOCOL_VERSION }).unwrap();
        assert!(decode_response(&request_line).is_err());
    }

    #[test]
    fn request_version_reads_every_variant() {
        for req in sample_requests() {
            assert_eq!(req.version(), HELPER_PROTOCOL_VERSION);
        }
    }

    /// A request the helper doesn't understand the version of is a
    /// `HelperResponse`, not a codec-level failure — the helper can parse
    /// the envelope enough to know `v`, it just declines to act on it. This
    /// guards against a future client accidentally treating a version
    /// mismatch as a transport error instead of a protocol-level one.
    #[test]
    fn unsupported_version_response_round_trips_with_its_supported_value() {
        let resp = HelperResponse::UnsupportedVersion { supported: HELPER_PROTOCOL_VERSION };
        let line = encode_response(&resp).unwrap();
        assert_eq!(decode_response(&line).unwrap(), resp);
    }

    /// Line framing depends on no encoded message ever containing a raw
    /// newline. `serde_json` guarantees this by escaping control characters
    /// inside strings, but that's an assumption about a dependency's
    /// behavior worth pinning down with a test, not just a doc comment —
    /// this is the case that would break framing if it ever stopped holding.
    #[test]
    fn encode_never_produces_an_embedded_newline_even_when_the_input_does() {
        let req = HelperRequest::ConfigureNetworkIntegration {
            v: HELPER_PROTOCOL_VERSION,
            adapter_name: "evil\nadapter\r\nnamed\tthing".to_string(),
        };
        let line = encode_request(&req).unwrap();
        assert!(!line.contains('\n'));
        assert!(!line.contains('\r'));
        assert_eq!(decode_request(&line).unwrap(), req);
    }
}
