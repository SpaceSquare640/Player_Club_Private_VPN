//! Tauri-facing lifecycle wrapper around [`RelayServer`] — lets this app
//! itself run a relay (Settings/Relay page), rather than requiring the
//! separate `relay` binary. At most one locally-hosted relay at a time: a
//! second machine's relay is a shared resource other people register
//! against by address, not something meaningful to have several of on one
//! machine simultaneously.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Mutex;

use serde::Serialize;

use super::server::RelayServer;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelayHostStatus {
    /// What to give other people so they can reach this relay — the bound
    /// port on every interface (`0.0.0.0`), since this machine's actual
    /// internet-facing address is whatever its own network/port-forwarding
    /// makes it, not something discoverable here.
    pub port: u16,
    /// Network names currently registered — see
    /// `RelayServer::registered_network_names`.
    pub registered_networks: Vec<String>,
}

/// Owns zero-or-one locally-hosted [`RelayServer`], the same "lifecycle
/// commands drive it, status reads it live" shape as `mesh::MeshSession`.
#[derive(Default)]
pub struct RelayHost {
    active: Mutex<Option<RelayServer>>,
}

impl RelayHost {
    /// Starts a relay listening on `port` (every interface). Rejected if
    /// already hosting one — `stop` first.
    pub async fn start(&self, port: u16) -> Result<u16, String> {
        if self.active.lock().expect("relay host lock poisoned").is_some() {
            return Err("already hosting a relay — stop it first".into());
        }
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
        let server = RelayServer::start(bind_addr).await.map_err(|e| e.to_string())?;
        let bound_port = server.local_addr().port();
        *self.active.lock().expect("relay host lock poisoned") = Some(server);
        Ok(bound_port)
    }

    /// Stops the locally-hosted relay (idempotent: a no-op if not hosting
    /// one). Every connection it was splicing closes along with it.
    pub async fn stop(&self) {
        let server = self.active.lock().expect("relay host lock poisoned").take();
        if let Some(server) = server {
            server.shutdown().await;
        }
    }

    /// `None` if not currently hosting a relay.
    pub fn status(&self) -> Option<RelayHostStatus> {
        let active = self.active.lock().expect("relay host lock poisoned");
        let server = active.as_ref()?;
        Some(RelayHostStatus { port: server.local_addr().port(), registered_networks: server.registered_network_names() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn status_is_none_before_starting() {
        let host = RelayHost::default();
        assert!(host.status().is_none());
    }

    #[tokio::test]
    async fn start_then_status_reports_the_bound_port() {
        let host = RelayHost::default();
        let port = host.start(0).await.unwrap();
        assert_ne!(port, 0); // an ephemeral port was actually assigned

        let status = host.status().unwrap();
        assert_eq!(status.port, port);
        assert_eq!(status.registered_networks, Vec::<String>::new());

        host.stop().await;
        assert!(host.status().is_none());
    }

    #[tokio::test]
    async fn starting_twice_is_rejected() {
        let host = RelayHost::default();
        host.start(0).await.unwrap();

        let err = host.start(0).await.unwrap_err();
        assert!(err.contains("already hosting"));

        host.stop().await;
    }

    #[tokio::test]
    async fn stop_is_idempotent_when_not_hosting() {
        let host = RelayHost::default();
        host.stop().await; // must not panic
        assert!(host.status().is_none());
    }

    #[tokio::test]
    async fn status_lists_a_network_actually_registered_against_the_hosted_relay() {
        use crate::engine::signaling::server::SignalingServer;

        let host = RelayHost::default();
        let port = host.start(0).await.unwrap();
        let relay_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);

        let signaling = SignalingServer::start_via_relay(relay_addr, "party", "secret").await.unwrap();
        assert_eq!(host.status().unwrap().registered_networks, vec!["party".to_string()]);

        signaling.shutdown().await;
        host.stop().await;
    }
}
