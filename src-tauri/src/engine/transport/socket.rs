//! The shared UDP socket — a deliberately "dumb pipe".
//!
//! It only moves opaque datagrams in and out of one bound port. It never parses
//! payloads: STUN, Ping/Pong, and (later) Handshake/Data all share this socket,
//! and the demux in [`super::frame`] decides what each datagram means. Using a
//! single bound port for STUN *and* data is what makes the NAT mapping observed
//! by STUN the same one peers send to.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

/// Cheap-to-clone handle around one bound UDP socket.
#[derive(Clone)]
pub struct UdpTransport {
    inner: Arc<UdpSocket>,
}

impl UdpTransport {
    /// Bind an IPv4 UDP socket on `port` (0 = ephemeral) with `SO_REUSEADDR`.
    pub async fn bind(port: u16) -> io::Result<Self> {
        let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;
        socket.bind(&addr.into())?;
        let std_sock: StdUdpSocket = socket.into();
        let tokio_sock = UdpSocket::from_std(std_sock)?;
        Ok(Self {
            inner: Arc::new(tokio_sock),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    pub async fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        self.inner.send_to(buf, target).await
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.recv_from(buf).await
    }
}
