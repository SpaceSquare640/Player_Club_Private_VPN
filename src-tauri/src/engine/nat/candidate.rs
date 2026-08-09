//! Candidate gathering: host (local interface) + reflexive (via STUN).

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use crate::engine::transport::UdpTransport;

use super::stun;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    Host,
    Reflexive,
}

impl CandidateKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CandidateKind::Host => "host",
            CandidateKind::Reflexive => "reflexive",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub addr: SocketAddr,
    pub kind: CandidateKind,
}

/// Gather host + reflexive candidates using the shared transport's bound port.
/// Reflexive discovery is best-effort — STUN failure simply omits it.
pub async fn gather(transport: &UdpTransport, stun_server: &str) -> Vec<Candidate> {
    let port = transport.local_addr().map(|a| a.port()).unwrap_or(0);
    let mut candidates = Vec::new();

    if let Some(ip) = primary_local_ipv4() {
        candidates.push(Candidate {
            addr: SocketAddr::new(IpAddr::V4(ip), port),
            kind: CandidateKind::Host,
        });
    }

    if let Ok(addr) = stun::reflexive_addr(transport, stun_server, Duration::from_secs(2)).await {
        candidates.push(Candidate {
            addr,
            kind: CandidateKind::Reflexive,
        });
    }

    candidates
}

/// Best-effort primary local IPv4 via the standard "connect a UDP socket to a
/// public address and read back the chosen source IP" trick (sends no packets).
///
/// `pub(crate)` rather than private: `mesh::MeshSession::create` reuses this
/// to resolve the host address it displays/returns when the signaling
/// server was bound to `0.0.0.0` (the UI's default) — `TcpListener::
/// local_addr()` echoes back the unspecified address verbatim rather than
/// resolving it to a real interface IP, which would otherwise show the user
/// an address nobody (not even a second instance on the same machine) can
/// actually connect to.
pub(crate) fn primary_local_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    sock.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(ip) => Some(ip),
        IpAddr::V6(_) => None,
    }
}
