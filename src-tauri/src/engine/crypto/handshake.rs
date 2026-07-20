//! Noise IK handshake over the shared UDP transport.
//!
//! IK = initiator knows the responder's static key (exchanged via signaling in
//! C3), giving a 2-message, 1-RTT, mutually authenticated handshake. In C4 the
//! initiator message doubles as the NAT hole-punch (sent to all candidates);
//! here it targets a single address.
// The fan-out entry points (`initiate_fanout` / `respond_punch`) are driven by
// the C4 connect flow; the single-target `initiate` / `respond` primitives are
// retained as the directly-tested building blocks (dead outside tests).
#![allow(dead_code)]

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use snow::{Builder, HandshakeState};
use tokio::sync::watch;
use tokio::time;

use crate::engine::transport::frame::{self, FrameKind, Inbound};
use crate::engine::transport::UdpTransport;

use super::identity::Identity;
use super::session::CryptoSession;
use super::NOISE_PARAMS;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_NOISE_MSG: usize = 1024;

/// How often the fan-out resends message-1 (initiator) or punch probes
/// (responder). Each resend also re-punches every candidate NAT path, so this
/// doubles as the hole-punch keep-alive during the handshake window.
const RETRANSMIT: Duration = Duration::from_millis(250);

/// The product of a successful fan-out handshake.
///
/// `peer` is the source address of the winning datagram — the nominated
/// endpoint the data plane pins to. `session` is the single promoted transport
/// session (the [`HandshakeState`] was consumed by `into_transport_mode`, so a
/// second session for this attempt is structurally impossible). `responder_m2`
/// carries the responder's message-2 bytes so the driver can re-answer a
/// duplicate message-1 if the original reply was lost; it is `None` on the
/// initiator.
pub struct Handshaken {
    pub peer: SocketAddr,
    pub session: CryptoSession,
    pub responder_m2: Option<Vec<u8>>,
}

/// Initiator: send message 1 to `target`, await message 2, return the session.
pub async fn initiate(
    transport: &UdpTransport,
    target: SocketAddr,
    identity: &Identity,
    peer_public: &[u8],
) -> io::Result<CryptoSession> {
    let params = NOISE_PARAMS.parse().map_err(noise_err)?;
    let mut hs = Builder::new(params)
        .local_private_key(identity.private())
        .remote_public_key(peer_public)
        .build_initiator()
        .map_err(noise_err)?;

    let mut msg = [0u8; MAX_NOISE_MSG];
    let n = hs.write_message(&[], &mut msg).map_err(noise_err)?;
    transport
        .send_to(&frame::encode_handshake(&msg[..n]), target)
        .await?;

    let mut buf = vec![0u8; 2048];
    let (payload, _from) = recv_handshake(transport, &mut buf).await?;
    let mut tmp = [0u8; MAX_NOISE_MSG];
    hs.read_message(&payload, &mut tmp).map_err(noise_err)?;

    let transport_state = hs.into_transport_mode().map_err(noise_err)?;
    Ok(CryptoSession::new(transport_state))
}

/// Responder: await message 1, reply message 2, return `(peer_addr, session)`.
///
/// `expected_peer`, when `Some`, is the initiator's static public key from
/// signaling; the handshake is rejected if the connecting peer's static key
/// does not match (IK transmits but does not by itself authorize the
/// initiator). `None` accepts any initiator.
pub async fn respond(
    transport: &UdpTransport,
    identity: &Identity,
    expected_peer: Option<&[u8]>,
) -> io::Result<(SocketAddr, CryptoSession)> {
    let params = NOISE_PARAMS.parse().map_err(noise_err)?;
    let mut hs = Builder::new(params)
        .local_private_key(identity.private())
        .build_responder()
        .map_err(noise_err)?;

    let mut buf = vec![0u8; 2048];
    let (payload, from) = recv_handshake(transport, &mut buf).await?;
    let mut tmp = [0u8; MAX_NOISE_MSG];
    hs.read_message(&payload, &mut tmp).map_err(noise_err)?;

    // Authorize the initiator before responding.
    if let Some(expected) = expected_peer {
        let matches = hs.get_remote_static().map(|s| s == expected).unwrap_or(false);
        if !matches {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "initiator static key does not match the expected peer",
            ));
        }
    }

    let mut msg = [0u8; MAX_NOISE_MSG];
    let n = hs.write_message(&[], &mut msg).map_err(noise_err)?;
    transport
        .send_to(&frame::encode_handshake(&msg[..n]), from)
        .await?;

    let transport_state = hs.into_transport_mode().map_err(noise_err)?;
    Ok((from, CryptoSession::new(transport_state)))
}

/// Wait for the next Handshake frame, returning its noise payload and sender.
async fn recv_handshake(
    transport: &UdpTransport,
    buf: &mut [u8],
) -> io::Result<(Vec<u8>, SocketAddr)> {
    loop {
        let (n, from) = tokio::time::timeout(HANDSHAKE_TIMEOUT, transport.recv_from(buf))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "handshake timeout"))??;
        if let Inbound::Frame(FrameKind::Handshake, p) = frame::classify(&buf[..n]) {
            return Ok((p.to_vec(), from));
        }
        // Ignore non-handshake datagrams and keep waiting.
    }
}

fn noise_err<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("noise: {e}"))
}

/// Build a fresh Noise IK responder handshake state.
fn build_responder(identity: &Identity) -> io::Result<HandshakeState> {
    let params = NOISE_PARAMS.parse().map_err(noise_err)?;
    Builder::new(params)
        .local_private_key(identity.private())
        .build_responder()
        .map_err(noise_err)
}

/// Initiator fan-out (hole-punch-as-handshake).
///
/// Builds **one** Noise IK message-1 and broadcasts the identical bytes to every
/// peer candidate, retransmitting on [`RETRANSMIT`] until `deadline`. Each send
/// punches that candidate's NAT path. Inbound `Handshake` frames are fed, one at
/// a time, into the **single** [`HandshakeState`]:
///
///   * `read_message` is transactional in `snow` (it checkpoints and restores
///     the symmetric state on failure), so a spoofed/garbage message-2 simply
///     rolls back and we keep waiting — it cannot poison the real handshake.
///   * the **first** message-2 that validates consumes the handshake state via
///     `into_transport_mode` (which takes `self` by value). After that there is
///     no handshake object left to turn a later/duplicate response into a second
///     session — the single-session guarantee is structural, not a check.
///
/// The source address of the winning datagram is nominated as the peer endpoint.
/// `cancel` (a `watch` channel) cancels the attempt promptly on disconnect.
pub async fn initiate_fanout(
    transport: &UdpTransport,
    candidates: &[SocketAddr],
    identity: &Identity,
    peer_public: &[u8],
    deadline: Duration,
    cancel: &mut watch::Receiver<bool>,
) -> io::Result<Handshaken> {
    let params = NOISE_PARAMS.parse().map_err(noise_err)?;
    let mut hs = Builder::new(params)
        .local_private_key(identity.private())
        .remote_public_key(peer_public)
        .build_initiator()
        .map_err(noise_err)?;

    // Build message-1 ONCE — the same bytes punch every candidate path.
    let mut m1 = [0u8; MAX_NOISE_MSG];
    let n = hs.write_message(&[], &mut m1).map_err(noise_err)?;
    let m1_frame = frame::encode_handshake(&m1[..n]);

    let outcome = time::timeout(deadline, async move {
        let mut retransmit = time::interval(RETRANSMIT);
        let mut buf = vec![0u8; 2048];
        loop {
            tokio::select! {
                _ = retransmit.tick() => {
                    for c in candidates {
                        let _ = transport.send_to(&m1_frame, *c).await;
                    }
                }
                r = transport.recv_from(&mut buf) => {
                    if let Ok((len, from)) = r {
                        if let Inbound::Frame(FrameKind::Handshake, payload) = frame::classify(&buf[..len]) {
                            let mut tmp = [0u8; MAX_NOISE_MSG];
                            // First VALID message-2 wins and consumes the state.
                            if hs.read_message(payload, &mut tmp).is_ok() {
                                let ts = hs.into_transport_mode().map_err(noise_err)?;
                                return Ok(Handshaken {
                                    peer: from,
                                    session: CryptoSession::new(ts),
                                    responder_m2: None,
                                });
                            }
                            // Invalid bytes: snow rolled the state back; keep waiting.
                        }
                    }
                }
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow() {
                        return Err(io::Error::new(io::ErrorKind::Interrupted, "handshake cancelled"));
                    }
                }
            }
        }
    })
    .await;

    match outcome {
        Ok(res) => res,
        Err(_elapsed) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "handshake fan-out timed out (no candidate path opened?)",
        )),
    }
}

/// Responder fan-out: punch every initiator candidate with `Ping` probes (to
/// open our own NAT mappings) while awaiting message-1.
///
/// On a message-1 that both validates **and** carries the `expected_peer` static
/// key, we reply message-2, promote the single handshake state, and return the
/// message-2 bytes for retransmit. A message-1 whose static key does *not* match
/// is an impostor (anyone with our public key can craft a valid IK message-1) —
/// we discard it and rebuild a fresh handshake state so we keep waiting for the
/// genuine peer rather than aborting the whole attempt. Garbage that fails to
/// decrypt rolls back in `snow` and leaves the state reusable as-is.
pub async fn respond_punch(
    transport: &UdpTransport,
    candidates: &[SocketAddr],
    identity: &Identity,
    expected_peer: &[u8],
    deadline: Duration,
    cancel: &mut watch::Receiver<bool>,
) -> io::Result<Handshaken> {
    let mut hs = build_responder(identity)?;

    let outcome = time::timeout(deadline, async move {
        let mut punch = time::interval(RETRANSMIT);
        let mut buf = vec![0u8; 2048];
        let mut punch_seq = 0u64;
        loop {
            tokio::select! {
                _ = punch.tick() => {
                    let ping = frame::encode_ping(punch_seq);
                    punch_seq = punch_seq.wrapping_add(1);
                    for c in candidates {
                        let _ = transport.send_to(&ping, *c).await;
                    }
                }
                r = transport.recv_from(&mut buf) => {
                    let (len, from) = match r {
                        Ok(x) => x,
                        Err(_) => continue,
                    };
                    let Inbound::Frame(FrameKind::Handshake, payload) = frame::classify(&buf[..len]) else {
                        continue; // Pong / STUN / other punch traffic: ignore.
                    };
                    let mut tmp = [0u8; MAX_NOISE_MSG];
                    if hs.read_message(payload, &mut tmp).is_err() {
                        continue; // Garbage message-1: state rolled back, reuse it.
                    }
                    // Authorize the initiator's static key before responding.
                    let authorized = hs
                        .get_remote_static()
                        .map(|s| s == expected_peer)
                        .unwrap_or(false);
                    if !authorized {
                        // Impostor with our public key: discard and keep waiting
                        // with a fresh state (this message-1 consumed `hs`).
                        hs = build_responder(identity)?;
                        continue;
                    }
                    let mut m2 = [0u8; MAX_NOISE_MSG];
                    let n = hs.write_message(&[], &mut m2).map_err(noise_err)?;
                    let m2_frame = frame::encode_handshake(&m2[..n]);
                    transport.send_to(&m2_frame, from).await?;
                    let ts = hs.into_transport_mode().map_err(noise_err)?;
                    return Ok(Handshaken {
                        peer: from,
                        session: CryptoSession::new(ts),
                        responder_m2: Some(m2_frame),
                    });
                }
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow() {
                        return Err(io::Error::new(io::ErrorKind::Interrupted, "handshake cancelled"));
                    }
                }
            }
        }
    })
    .await;

    match outcome {
        Ok(res) => res,
        Err(_elapsed) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "handshake punch timed out (peer never reached us?)",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn ik_handshake_echo_replay_and_tamper() {
        let a_id = Identity::generate().unwrap();
        let b_id = Identity::generate().unwrap();
        let a_pub = STANDARD.decode(a_id.public_b64()).unwrap();
        let b_pub = STANDARD.decode(b_id.public_b64()).unwrap();

        let a_tx = UdpTransport::bind(0).await.unwrap();
        let b_tx = UdpTransport::bind(0).await.unwrap();
        let b_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, b_tx.local_addr().unwrap().port()));

        // Responder authorizes only the expected initiator (a_pub).
        let responder = tokio::spawn(async move {
            let (_from, session) = respond(&b_tx, &b_id, Some(&a_pub)).await.unwrap();
            (b_tx, session)
        });

        let mut a_session = initiate(&a_tx, b_addr, &a_id, &b_pub).await.unwrap();
        let (b_tx, mut b_session) = responder.await.unwrap();

        // A → B encrypted echo.
        let (counter, ct) = a_session.seal(b"hello peer").unwrap();
        a_tx.send_to(&frame::encode_data(counter, &ct), b_addr)
            .await
            .unwrap();
        let mut buf = vec![0u8; 2048];
        let (n, _from) = b_tx.recv_from(&mut buf).await.unwrap();
        let (rc, ciphertext) = match frame::classify(&buf[..n]) {
            Inbound::Frame(FrameKind::Data, payload) => frame::decode_data(payload).unwrap(),
            other => panic!("expected data frame, got {other:?}"),
        };
        assert_eq!(b_session.open(rc, ciphertext).unwrap(), b"hello peer");

        // Replay of the same counter is rejected.
        assert!(b_session.open(rc, ciphertext).is_err());

        // Tampered ciphertext (fresh counter) fails authentication.
        let (c2, mut ct2) = a_session.seal(b"world").unwrap();
        ct2[0] ^= 0xff;
        assert!(b_session.open(c2, &ct2).is_err());
    }

    #[tokio::test]
    async fn responder_rejects_unexpected_initiator() {
        let a_id = Identity::generate().unwrap();
        let b_id = Identity::generate().unwrap();
        let c_id = Identity::generate().unwrap(); // a different, unexpected key
        let b_pub = STANDARD.decode(b_id.public_b64()).unwrap();
        let c_pub = STANDARD.decode(c_id.public_b64()).unwrap();

        let a_tx = UdpTransport::bind(0).await.unwrap();
        let b_tx = UdpTransport::bind(0).await.unwrap();
        let b_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, b_tx.local_addr().unwrap().port()));

        // Responder expects `c`, but `a` connects → must be rejected.
        let responder = tokio::spawn(async move { respond(&b_tx, &b_id, Some(&c_pub)).await });
        let initiator = tokio::spawn(async move { initiate(&a_tx, b_addr, &a_id, &b_pub).await });

        assert!(responder.await.unwrap().is_err());
        initiator.abort(); // initiator would otherwise wait out the handshake timeout
    }

    /// Fan-out nominates the one live candidate (ignoring a dead one) and yields
    /// a single working encrypted session.
    #[tokio::test]
    async fn fanout_nominates_live_candidate_and_ignores_dead() {
        let a_id = Identity::generate().unwrap();
        let b_id = Identity::generate().unwrap();
        let a_pub = STANDARD.decode(a_id.public_b64()).unwrap();
        let b_pub = STANDARD.decode(b_id.public_b64()).unwrap();

        let a_tx = UdpTransport::bind(0).await.unwrap();
        let b_tx = UdpTransport::bind(0).await.unwrap();
        let a_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, a_tx.local_addr().unwrap().port()));
        let b_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, b_tx.local_addr().unwrap().port()));
        // RFC 5737 TEST-NET-1: off-machine and unreachable, so it neither answers
        // nor triggers a local ICMP reset (which would flake on Windows).
        let dead = SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 9));

        let (_b_cancel, mut b_rx) = watch::channel(false);
        let responder = tokio::spawn(async move {
            let h = respond_punch(&b_tx, &[a_addr], &b_id, &a_pub, Duration::from_secs(3), &mut b_rx)
                .await
                .unwrap();
            (b_tx, h)
        });

        let (_a_cancel, mut a_rx) = watch::channel(false);
        let a = initiate_fanout(&a_tx, &[dead, b_addr], &a_id, &b_pub, Duration::from_secs(3), &mut a_rx)
            .await
            .unwrap();
        let (b_tx, b) = responder.await.unwrap();

        // Nomination: A picked the live responder's address; only B caches M2.
        assert_eq!(a.peer, b_addr);
        assert!(a.responder_m2.is_none());
        assert!(b.responder_m2.is_some());

        // Encrypted echo over the nominated path proves a single live session.
        let mut a_session = a.session;
        let mut b_session = b.session;
        let (counter, ct) = a_session.seal(b"punch through").unwrap();
        a_tx.send_to(&frame::encode_data(counter, &ct), b_addr)
            .await
            .unwrap();
        let mut buf = vec![0u8; 2048];
        let (n, _from) = b_tx.recv_from(&mut buf).await.unwrap();
        let (rc, ciphertext) = match frame::classify(&buf[..n]) {
            Inbound::Frame(FrameKind::Data, payload) => frame::decode_data(payload).unwrap(),
            other => panic!("expected data frame, got {other:?}"),
        };
        assert_eq!(b_session.open(rc, ciphertext).unwrap(), b"punch through");
    }

    /// A valid IK message-1 from an unexpected static key (an impostor who has
    /// our public key) is discarded — the responder rebuilds and keeps waiting,
    /// then times out rather than aborting early or accepting. No session is ever
    /// formed for the wrong peer.
    #[tokio::test]
    async fn responder_waits_out_unexpected_initiator() {
        let a_id = Identity::generate().unwrap(); // the peer that actually connects
        let b_id = Identity::generate().unwrap(); // responder
        let c_id = Identity::generate().unwrap(); // the static key B expects
        let b_pub = STANDARD.decode(b_id.public_b64()).unwrap();
        let c_pub = STANDARD.decode(c_id.public_b64()).unwrap();

        let a_tx = UdpTransport::bind(0).await.unwrap();
        let b_tx = UdpTransport::bind(0).await.unwrap();
        let a_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, a_tx.local_addr().unwrap().port()));
        let b_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, b_tx.local_addr().unwrap().port()));

        let (_b_cancel, mut b_rx) = watch::channel(false);
        let responder = tokio::spawn(async move {
            respond_punch(&b_tx, &[a_addr], &b_id, &c_pub, Duration::from_millis(500), &mut b_rx).await
        });
        let (_a_cancel, mut a_rx) = watch::channel(false);
        let initiator = tokio::spawn(async move {
            initiate_fanout(&a_tx, &[b_addr], &a_id, &b_pub, Duration::from_secs(5), &mut a_rx).await
        });

        // `Handshaken` holds a `CryptoSession` (not `Debug`), so match rather
        // than `unwrap_err`.
        match responder.await.unwrap() {
            Err(e) => assert_eq!(e.kind(), io::ErrorKind::TimedOut),
            Ok(_) => panic!("responder must not accept an unexpected initiator"),
        }
        initiator.abort();
    }

    /// `cancel` aborts an in-flight handshake promptly (well before `deadline`).
    #[tokio::test]
    async fn cancel_aborts_handshake() {
        let a_id = Identity::generate().unwrap();
        let b_id = Identity::generate().unwrap();
        let b_pub = STANDARD.decode(b_id.public_b64()).unwrap();
        let a_tx = UdpTransport::bind(0).await.unwrap();
        // A lone dead candidate: nobody will ever answer.
        let dead = SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 9));

        let (cancel, mut rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            initiate_fanout(&a_tx, &[dead], &a_id, &b_pub, Duration::from_secs(30), &mut rx).await
        });
        cancel.send(true).unwrap();
        match task.await.unwrap() {
            Err(e) => assert_eq!(e.kind(), io::ErrorKind::Interrupted),
            Ok(_) => panic!("a cancelled handshake must not succeed"),
        }
    }
}
