//! UDP transport: the shared socket, the frame demux, RTT keepalive, and the
//! Phase-C1 probe task that surfaces real latency before the data plane lands.

pub mod frame;
pub mod keepalive;
pub mod probe;
pub mod socket;

pub use socket::UdpTransport;
