//! A dumb TCP rendezvous/byte-splicer, one layer *below* `engine::signaling`.
//!
//! Direct-bind virtual networking (the default) has `SignalingServer` bind a
//! TCP listener and advertise its address for joiners to dial straight into
//! — which only works when that address is actually reachable, i.e. same
//! LAN, or the host has port-forwarded. Across the open internet, neither is
//! usually true, and a signaling connection that can never be established
//! means the whole virtual-network flow never gets off the ground.
//!
//! This module fixes reachability, not protocol: both the host and every
//! joiner make ordinary *outbound* connections to a relay with a public IP,
//! and the relay pairs two of those connections together and splices their
//! bytes verbatim. `SignalingServer`/`SignalingClient` then run their
//! existing WebSocket accept/connect handshake completely unmodified over
//! that spliced pipe — this module has zero knowledge of WebSocket, Noise,
//! or the mesh protocol, and can't regress anything above it.
//!
//! Handshake, since one host may have many joiners over the network's
//! lifetime (not just one):
//! 1. **Register** — the host opens one long-lived control connection, sends
//!    `REGISTER <network_name>`. Stays open for as long as the network is up.
//! 2. **Connect** — a joiner opens a connection, sends `CONNECT <network_name>`.
//!    Rejected immediately (`ERR no such network`) if nothing is registered
//!    under that name.
//! 3. **Notify** — the relay sends `NEW_MEMBER <session_id>` down the host's
//!    control connection.
//! 4. **Accept** — the host opens a *second*, fresh connection, sends
//!    `ACCEPT <session_id>`. The relay now has both ends of this session and
//!    splices them bidirectionally until either side closes.
//!
//! From here the host treats its `ACCEPT` socket exactly like a freshly
//! accepted `TcpListener` connection, and the joiner treats its `CONNECT`
//! socket exactly like a freshly dialed one — see
//! `SignalingServer::start_via_relay` / `SignalingClient::join_via_relay`.

pub mod host;
pub mod protocol;
pub mod server;

pub use host::{RelayHost, RelayHostStatus};
pub use server::RelayServer;
