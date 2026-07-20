//! Minimal STUN binding client (RFC 5389) — just enough to discover our
//! reflexive (public) address. Borrows the shared transport; it does not own a
//! socket of its own. Because STUN runs once at startup before the long-lived
//! recv loop, the request/response is handled inline here.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use rand::RngCore;
use tokio::net::lookup_host;

use crate::engine::transport::frame::STUN_MAGIC_COOKIE;
use crate::engine::transport::UdpTransport;

const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
const ATTR_MAPPED_ADDRESS: u16 = 0x0001;

/// Discover the reflexive address by querying `server` (host:port) over the
/// shared transport. Retransmits a few times before giving up.
pub async fn reflexive_addr(
    transport: &UdpTransport,
    server: &str,
    timeout: Duration,
) -> io::Result<SocketAddr> {
    let server_addr = resolve_v4(server).await?;

    let mut txid = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut txid);
    let request = build_request(&txid);

    let mut buf = [0u8; 512];
    for _ in 0..3 {
        transport.send_to(&request, server_addr).await?;
        match tokio::time::timeout(timeout, transport.recv_from(&mut buf)).await {
            Ok(Ok((n, _from))) => {
                if let Some(addr) = parse_response(&buf[..n], &txid) {
                    return Ok(addr);
                }
                // Not our response (or unparsable) — fall through and retry.
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => continue, // timeout → retransmit
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "STUN: no binding response",
    ))
}

async fn resolve_v4(server: &str) -> io::Result<SocketAddr> {
    for addr in lookup_host(server).await? {
        if addr.is_ipv4() {
            return Ok(addr);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "STUN: no IPv4 address for server",
    ))
}

fn build_request(txid: &[u8; 12]) -> [u8; 20] {
    let mut b = [0u8; 20];
    b[0..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    b[2..4].copy_from_slice(&0u16.to_be_bytes()); // message length (no attributes)
    b[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    b[8..20].copy_from_slice(txid);
    b
}

/// Parse a binding success response and extract the mapped address. Returns
/// `None` if the message isn't a matching success response.
fn parse_response(msg: &[u8], txid: &[u8; 12]) -> Option<SocketAddr> {
    if msg.len() < 20 {
        return None;
    }
    let msg_type = u16::from_be_bytes([msg[0], msg[1]]);
    let cookie = u32::from_be_bytes([msg[4], msg[5], msg[6], msg[7]]);
    if msg_type != BINDING_SUCCESS || cookie != STUN_MAGIC_COOKIE || &msg[8..20] != txid {
        return None;
    }
    let length = u16::from_be_bytes([msg[2], msg[3]]) as usize;
    let attrs = msg.get(20..20 + length)?;

    let mut i = 0;
    while i + 4 <= attrs.len() {
        let atype = u16::from_be_bytes([attrs[i], attrs[i + 1]]);
        let alen = u16::from_be_bytes([attrs[i + 2], attrs[i + 3]]) as usize;
        let vstart = i + 4;
        let value = attrs.get(vstart..vstart + alen)?;
        match atype {
            ATTR_XOR_MAPPED_ADDRESS => return parse_xor_mapped(value),
            ATTR_MAPPED_ADDRESS => return parse_mapped(value),
            _ => {}
        }
        // Attributes are padded to a 4-byte boundary.
        i = vstart + alen.div_ceil(4) * 4;
    }
    None
}

fn parse_xor_mapped(value: &[u8]) -> Option<SocketAddr> {
    // [reserved:1][family:1][x-port:2][x-address:4 (IPv4)]
    if value.len() < 8 || value[1] != 0x01 {
        return None;
    }
    let xport = u16::from_be_bytes([value[2], value[3]]);
    let port = xport ^ (STUN_MAGIC_COOKIE >> 16) as u16;
    let xaddr = u32::from_be_bytes([value[4], value[5], value[6], value[7]]);
    let addr = Ipv4Addr::from(xaddr ^ STUN_MAGIC_COOKIE);
    Some(SocketAddr::V4(SocketAddrV4::new(addr, port)))
}

fn parse_mapped(value: &[u8]) -> Option<SocketAddr> {
    if value.len() < 8 || value[1] != 0x01 {
        return None;
    }
    let port = u16::from_be_bytes([value[2], value[3]]);
    let addr = Ipv4Addr::new(value[4], value[5], value[6], value[7]);
    Some(SocketAddr::V4(SocketAddrV4::new(addr, port)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_xor_mapped_address() {
        // Build a synthetic success response for 203.0.113.5:50000 by applying
        // the same XOR the parser reverses (self-consistent round trip).
        let txid = [9u8; 12];
        let ip = Ipv4Addr::new(203, 0, 113, 5);
        let port: u16 = 50000;
        let xport = port ^ (STUN_MAGIC_COOKIE >> 16) as u16;
        let xaddr = u32::from(ip) ^ STUN_MAGIC_COOKIE;

        let mut msg = Vec::new();
        msg.extend_from_slice(&BINDING_SUCCESS.to_be_bytes());
        msg.extend_from_slice(&12u16.to_be_bytes()); // attr total length
        msg.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        msg.extend_from_slice(&txid);
        // XOR-MAPPED-ADDRESS attribute
        msg.extend_from_slice(&ATTR_XOR_MAPPED_ADDRESS.to_be_bytes());
        msg.extend_from_slice(&8u16.to_be_bytes());
        msg.push(0x00); // reserved
        msg.push(0x01); // family IPv4
        msg.extend_from_slice(&xport.to_be_bytes());
        msg.extend_from_slice(&xaddr.to_be_bytes());

        let got = parse_response(&msg, &txid).unwrap();
        assert_eq!(got, SocketAddr::V4(SocketAddrV4::new(ip, port)));
    }

    #[test]
    fn rejects_wrong_txid() {
        let txid = [1u8; 12];
        let mut msg = Vec::new();
        msg.extend_from_slice(&BINDING_SUCCESS.to_be_bytes());
        msg.extend_from_slice(&0u16.to_be_bytes());
        msg.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        msg.extend_from_slice(&[2u8; 12]); // different txid
        assert!(parse_response(&msg, &txid).is_none());
    }
}
