//! Post-handshake session driver — the steady-state peer link (Phase C4/C5).
//!
//! [`run`] is the whole lifecycle of one peer connection:
//!
//!   1. **Connecting** — drives the hole-punch handshake ([`crate::engine::crypto::handshake`])
//!      for our negotiated [`Role`], with an overall [`HANDSHAKE_DEADLINE`].
//!   2. **Connected** — once a single [`CryptoSession`] is nominated, runs an
//!      *encrypted* keepalive (ping/pong inside `Data` frames) to the nominated
//!      endpoint for real RTT/jitter/loss, and — in **C5** — bridges the virtual
//!      TUN adapter over the same session: outbound IP packets are sealed and
//!      sent to the peer (uplink); inbound `Data` frames are decrypted and
//!      injected onto the adapter (downlink).
//!
//! Payloads carried inside the encrypted `Data` frame are tagged with a 1-byte
//! inner kind so keepalive control and tunnelled IP packets share one channel:
//! [`INNER_PING`]/[`INNER_PONG`] carry a sequence number; [`INNER_DATA`] carries
//! a raw IP packet. An IPv4 packet's first byte (`0x45`) never collides with the
//! control tags.
//!
//! Telemetry is emitted through the [`TelemetrySink`] seam (the same
//! `engine://state` / `telemetry://stats` / `telemetry://packet` channels the UI
//! already consumes). A `watch` channel cancels the whole flow promptly on
//! disconnect.

use std::io;
use std::net::SocketAddr;
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::watch;
use tokio::time;

use crate::engine::connection::{LinkState, Role};
use crate::engine::crypto::handshake::{self, Handshaken};
use crate::engine::crypto::identity::{fingerprint_of, Identity};
use crate::engine::crypto::session::CryptoSession;
use crate::engine::dataplane;
use crate::engine::fec::{RsDecoder, RsEncoder, RsParity};
use crate::engine::notice::EngineNotice;
use crate::engine::split_tunnel::SplitPolicy;
use crate::engine::state::EngineState;
use crate::engine::telemetry::metrics::TelemetrySnapshot;
use crate::engine::telemetry::packet_log::{Direction, PacketLogEntry};
use crate::engine::telemetry::sink::TelemetrySink;
use crate::engine::transport::frame::{self, FrameKind, Inbound};
use crate::engine::transport::keepalive::RttTracker;
use crate::engine::transport::UdpTransport;
use crate::engine::tun::{packet, TunConfig};

/// Overall budget for the hole-punch handshake before we report failure.
pub const HANDSHAKE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(8);

/// Cadence of the encrypted keepalive once connected.
const KEEPALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// Inner (encrypted) payload tags, carried inside a `Data` frame.
const INNER_PING: u8 = 1; // keepalive request:  `[1][seq:8 BE]`
const INNER_PONG: u8 = 2; // keepalive reply:    `[2][seq:8 BE]`
const INNER_DATA: u8 = 3; // tunnelled packet:   `[3][ip bytes…]`
const INNER_FEC_DATA: u8 = 4; // FEC data packet: `[4][group:4][index:1][ip…]`
const INNER_FEC_PARITY: u8 = 5; // FEC parity: `[5][group:4][k:1][r:1][idx:1][shard…]`

/// FEC geometry: `FEC_GROUP_SIZE` data packets protected by `FEC_PARITY_SHARDS`
/// parity shards, so any `FEC_PARITY_SHARDS` losses in a group are recoverable
/// (Phase D.2). `r = 1` reproduces the D.1 XOR behaviour at 1/k overhead.
const FEC_GROUP_SIZE: u8 = 8;
const FEC_PARITY_SHARDS: u8 = 1;
const FEC_FLUSH: std::time::Duration = std::time::Duration::from_millis(30);
/// Recent groups the decoder retains for reordering / late parity.
const FEC_GROUP_CAP: usize = 64;

/// Maximum packet-log entries buffered between telemetry flushes. The batch is
/// drained once per [`KEEPALIVE_INTERVAL`], so without a cap a high packet rate
/// would both grow the buffer unboundedly within a tick and hand the IPC layer
/// one enormous payload to serialize. Past the cap we count instead of collect.
const PACKET_LOG_BATCH_CAP: usize = 256;

/// A bounded batch of packet-log entries for one telemetry tick.
struct LogBatch {
    entries: Vec<PacketLogEntry>,
    suppressed: u32,
}

impl LogBatch {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            suppressed: 0,
        }
    }

    /// Record an entry, or count it as suppressed once the cap is reached.
    fn push(&mut self, entry: PacketLogEntry) {
        if self.entries.len() < PACKET_LOG_BATCH_CAP {
            self.entries.push(entry);
        } else {
            self.suppressed += 1;
        }
    }

    fn has_content(&self) -> bool {
        !self.entries.is_empty() || self.suppressed > 0
    }

    /// Close the batch, appending a summary line if anything was suppressed so
    /// the gap is visible rather than silent.
    fn take(&mut self, started: Instant) -> Vec<PacketLogEntry> {
        if self.suppressed > 0 {
            let note = format!("{} further entries suppressed (log rate cap)", self.suppressed);
            self.entries
                .push(pkt(started, Direction::Rx, "LOG", 0, note));
            self.suppressed = 0;
        }
        std::mem::take(&mut self.entries)
    }
}

/// Where this link's data plane comes from.
///
/// The adapter is only brought up *after* the handshake succeeds, so a failed
/// connection never creates (and destroys) one needlessly.
pub enum DataPlaneSource {
    /// No data plane — the link runs control-only (encrypted keepalive), as in C4.
    None,
    /// Create a real virtual adapter with this configuration (the production path).
    Adapter(TunConfig),
    /// Drive an already-built device. This is the seam that lets the integration
    /// tests exercise the entire path — handshake, crypto, FEC, policy and the
    /// bridge — against a mock adapter, with no Windows, elevation or second
    /// machine required. Compiled out of release builds entirely.
    #[cfg(test)]
    Device(Box<dyn crate::engine::tun::TunDevice>, TunConfig),
}

/// Drive one peer connection from handshake through steady state. Owns the recv
/// loop on the shared socket for the lifetime of the link.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    transport: UdpTransport,
    role: Role,
    identity: Arc<Identity>,
    peer_public: Vec<u8>,
    peer_candidates: Vec<SocketAddr>,
    dataplane_src: DataPlaneSource,
    sink: Box<dyn TelemetrySink>,
    link: Arc<Mutex<LinkState>>,
    mut cancel: watch::Receiver<bool>,
) {
    sink.state(EngineState::Connecting);

    // Phase 1 — hole-punch-as-handshake.
    let result = match role {
        Role::Initiator => {
            handshake::initiate_fanout(
                &transport,
                &peer_candidates,
                &identity,
                &peer_public,
                HANDSHAKE_DEADLINE,
                &mut cancel,
            )
            .await
        }
        Role::Responder => {
            handshake::respond_punch(
                &transport,
                &peer_candidates,
                &identity,
                &peer_public,
                HANDSHAKE_DEADLINE,
                &mut cancel,
            )
            .await
        }
        Role::Idle => Err(io::Error::new(
            io::ErrorKind::Other,
            "no role negotiated — create or accept an offer first",
        )),
    };

    let Handshaken {
        peer,
        session,
        responder_m2,
    } = match result {
        Ok(h) => h,
        Err(e) => {
            // A cancellation is an orderly disconnect, not a failure.
            let cancelled = e.kind() == io::ErrorKind::Interrupted;
            *link.lock().unwrap() = if cancelled {
                LinkState::Idle
            } else {
                LinkState::Failed
            };
            sink.state(if cancelled {
                EngineState::Idle
            } else {
                EngineState::Error
            });
            if !cancelled {
                sink.notice(&EngineNotice::error(format!("Handshake failed: {e}")));
            }
            return;
        }
    };

    // Phase 2 — connected. Nominate the winning endpoint and announce the peer.
    let peer_fp = session
        .remote_static()
        .map(|s| fingerprint_of(&s))
        .unwrap_or_else(|| "unknown".to_string());
    *link.lock().unwrap() = LinkState::Connected;
    sink.state(EngineState::Connected);
    sink.notice(&EngineNotice::info(
        "peer_connected",
        format!("Connected to {peer_fp} via {peer}"),
    ));

    // Bring up the C5 data plane if requested; degrade gracefully on failure.
    // The split-tunnel policy (E.1) is derived from the same config.
    let (dataplane, split_policy) = match dataplane_src {
        DataPlaneSource::Adapter(cfg) => {
            let vip = cfg.virtual_ip;
            let policy = SplitPolicy::from_tun(&cfg);
            match dataplane::open(cfg, cancel.clone()).await {
                Ok(dp) => {
                    sink.notice(&EngineNotice::info(
                        "data_plane",
                        format!(
                            "Data plane active on {vip} — tunnelling to {peer_fp} · FEC on (RS k={FEC_GROUP_SIZE}/r={FEC_PARITY_SHARDS})"
                        ),
                    ));
                    (Some(dp), Some(policy))
                }
                Err(e) => {
                    sink.notice(&EngineNotice::info(
                        "data_plane_off",
                        format!("Connected (control link only) — virtual adapter unavailable: {e}"),
                    ));
                    (None, None)
                }
            }
        }
        #[cfg(test)]
        DataPlaneSource::Device(dev, cfg) => {
            let policy = SplitPolicy::from_tun(&cfg);
            let dp = dataplane::spawn_bridge(dev, cancel.clone(), cfg.mtu as usize);
            (Some(dp), Some(policy))
        }
        DataPlaneSource::None => (None, None),
    };

    // Destructure so the driver owns the channel ends (dropping them signals the
    // bridge thread to stop); the join handle stays here to await teardown.
    let (mut uplink_rx, downlink_tx, dp_handle) = match dataplane {
        Some(dp) => (Some(dp.uplink_rx), Some(dp.downlink_tx), Some(dp.handle)),
        None => (None, None, None),
    };

    // FEC protects data-plane traffic only; a control-only link has none.
    let has_dataplane = uplink_rx.is_some();
    let fec_enc = has_dataplane.then(|| RsEncoder::new(FEC_GROUP_SIZE, FEC_PARITY_SHARDS));
    let fec_dec = has_dataplane.then(|| RsDecoder::new(FEC_GROUP_CAP));

    drive(
        &transport,
        peer,
        session,
        responder_m2,
        uplink_rx.as_mut(),
        downlink_tx.as_ref(),
        fec_enc,
        fec_dec,
        split_policy,
        sink.as_ref(),
        &mut cancel,
    )
    .await;

    // Drop the downlink sender so the bridge sees a disconnect, then join it —
    // this guarantees the Wintun adapter is released before we report Idle.
    drop(downlink_tx);
    if let Some(h) = dp_handle {
        let _ = h.await;
    }

    // Loop exited only via cancellation (disconnect).
    *link.lock().unwrap() = LinkState::Idle;
    sink.state(EngineState::Idle);
}

/// Steady-state loop: encrypted keepalive RTT, TUN uplink/downlink pumps, and
/// inbound frame routing.
#[allow(clippy::too_many_arguments)]
async fn drive(
    transport: &UdpTransport,
    peer: SocketAddr,
    mut session: CryptoSession,
    responder_m2: Option<Vec<u8>>,
    mut uplink_rx: Option<&mut tokio_mpsc::Receiver<Vec<u8>>>,
    downlink_tx: Option<&std_mpsc::SyncSender<Vec<u8>>>,
    mut fec_enc: Option<RsEncoder>,
    mut fec_dec: Option<RsDecoder>,
    split_policy: Option<SplitPolicy>,
    sink: &dyn TelemetrySink,
    cancel: &mut watch::Receiver<bool>,
) {
    let started = Instant::now();
    let mut tracker = RttTracker::new();
    let mut ticker = time::interval(KEEPALIVE_INTERVAL);
    let mut fec_flush = time::interval(FEC_FLUSH);
    let mut buf = vec![0u8; 2048];
    let mut pending = LogBatch::new();

    // Byte counters since the last stats emit, for live throughput.
    let mut tx_bytes = 0u64;
    let mut rx_bytes = 0u64;
    let mut last_emit = Instant::now();

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Encrypted keepalive ping to the nominated endpoint.
                let seq = tracker.next_ping();
                if let Ok((counter, ct)) = session.seal(&ctrl_encode(INNER_PING, seq)) {
                    let datagram = frame::encode_data(counter, &ct);
                    let _ = transport.send_to(&datagram, peer).await;
                    pending.push(pkt(started, Direction::Tx, "KA-PING", datagram.len(), format!("seq={seq} → {peer}")));
                }

                let secs = last_emit.elapsed().as_secs_f32().max(0.001);
                let (rtt, jitter, loss) = tracker.stats();
                let snapshot = TelemetrySnapshot {
                    state: EngineState::Connected,
                    rtt_ms: rtt,
                    jitter_ms: jitter,
                    loss_pct: loss,
                    tx_kbps: round2(tx_bytes as f32 * 8.0 / 1000.0 / secs),
                    rx_kbps: round2(rx_bytes as f32 * 8.0 / 1000.0 / secs),
                    peers: 1,
                };
                sink.stats(&snapshot);
                tx_bytes = 0;
                rx_bytes = 0;
                last_emit = Instant::now();
                if pending.has_content() {
                    sink.packets(&pending.take(started));
                }
            }
            // Uplink: an outbound IP packet captured off the virtual adapter.
            up = async {
                match uplink_rx.as_deref_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending::<Option<Vec<u8>>>().await,
                }
            } => {
                if let Some(ip) = up {
                    // Split-tunnel policy (E.1): only tunnel-destined packets go
                    // to the peer. Everything the OS put on the adapter that we do
                    // not admit is dropped here — native traffic never reaches us.
                    if !admits_packet(split_policy.as_ref(), &ip) {
                        pending.push(pkt(started, Direction::Tx, "DROP", ip.len(), "split-tunnel blocked".to_string()));
                    } else if let Some(enc) = fec_enc.as_mut() {
                        // FEC path: send the packet as FEC_DATA, then any parity
                        // that closing the group produced.
                        let (group, index, parity) = enc.push(&ip);
                        send_sealed(transport, &mut session, peer, &fec_data_encode(group, index, &ip)).await;
                        tx_bytes += ip.len() as u64;
                        pending.push(pkt(started, Direction::Tx, &proto_label(&ip), ip.len(), format!("g{group}/{index} → {peer}")));
                        for p in &parity {
                            send_sealed(transport, &mut session, peer, &fec_parity_encode(p)).await;
                            pending.push(pkt(started, Direction::Tx, "FEC-PAR", p.shard.len(), format!("g{} k{}/r{} #{} → {peer}", p.group, p.k, p.r, p.index)));
                        }
                    } else if let Ok((counter, ct)) = session.seal(&dp_encode(&ip)) {
                        let datagram = frame::encode_data(counter, &ct);
                        let _ = transport.send_to(&datagram, peer).await;
                        tx_bytes += ip.len() as u64;
                        pending.push(pkt(started, Direction::Tx, &proto_label(&ip), ip.len(), format!("→ {peer}")));
                    }
                }
                // `None` means the bridge ended; the cancel branch handles teardown.
            }
            // FEC idle flush: close a partial group so its parity is not delayed.
            _ = fec_flush.tick() => {
                let flushed = fec_enc.as_mut().map(|enc| enc.flush()).unwrap_or_default();
                for p in &flushed {
                    send_sealed(transport, &mut session, peer, &fec_parity_encode(p)).await;
                    pending.push(pkt(started, Direction::Tx, "FEC-PAR", p.shard.len(), format!("g{} k{}/r{} #{} flush → {peer}", p.group, p.k, p.r, p.index)));
                }
            }
            r = transport.recv_from(&mut buf) => {
                let Ok((n, from)) = r else { continue };
                match frame::classify(&buf[..n]) {
                    Inbound::Frame(FrameKind::Data, payload) => {
                        let Some((counter, ct)) = frame::decode_data(payload) else { continue };
                        let Ok(plaintext) = session.open(counter, ct) else { continue };
                        match decode_inner(&plaintext) {
                            Some(Inner::Ctrl(INNER_PING, seq)) => {
                                if let Ok((c2, ct2)) = session.seal(&ctrl_encode(INNER_PONG, seq)) {
                                    let datagram = frame::encode_data(c2, &ct2);
                                    let _ = transport.send_to(&datagram, from).await;
                                    pending.push(pkt(started, Direction::Rx, "KA-PING", n, format!("seq={seq} ← {from}")));
                                    pending.push(pkt(started, Direction::Tx, "KA-PONG", datagram.len(), format!("seq={seq} → {from}")));
                                }
                            }
                            Some(Inner::Ctrl(INNER_PONG, seq)) => {
                                tracker.on_pong(seq);
                                pending.push(pkt(started, Direction::Rx, "KA-PONG", n, format!("seq={seq} ← {from}")));
                            }
                            // Downlink: a tunnelled IP packet — inject onto the adapter.
                            Some(Inner::Data(ip)) => {
                                rx_bytes += ip.len() as u64;
                                pending.push(inbound_entry(started, inject_downlink(ip, split_policy.as_ref(), downlink_tx), ip, format!("← {from}")));
                            }
                            // FEC-protected data: inject immediately, then feed the
                            // decoder (which may reconstruct a *different* missing one).
                            Some(Inner::FecData { group, index, ip }) => {
                                rx_bytes += ip.len() as u64;
                                pending.push(inbound_entry(started, inject_downlink(ip, split_policy.as_ref(), downlink_tx), ip, format!("g{group}/{index} ← {from}")));
                                // Feed the decoder regardless: FEC bookkeeping tracks the
                                // transport stream, so a legitimate loss stays recoverable.
                                let recovered = fec_dec.as_mut().map(|d| d.on_data(group, index, ip)).unwrap_or_default();
                                for rec in &recovered {
                                    forward_recovered(rec, split_policy.as_ref(), downlink_tx, started, &mut pending);
                                }
                            }
                            // Parity: never injected; only feeds recovery. A single
                            // parity can complete a group that lost several packets.
                            Some(Inner::FecParity { group, k, r, index, q }) => {
                                let recovered = fec_dec.as_mut().map(|d| d.on_parity(group, k, r, index, q)).unwrap_or_default();
                                for rec in &recovered {
                                    forward_recovered(rec, split_policy.as_ref(), downlink_tx, started, &mut pending);
                                }
                            }
                            _ => {}
                        }
                    }
                    Inbound::Frame(FrameKind::Handshake, _) => {
                        // Responder only: a duplicate message-1 from the peer means
                        // our message-2 was lost. Re-send the *cached* M2 — never a
                        // new handshake, so still exactly one session.
                        if let Some(m2) = responder_m2.as_ref() {
                            if from == peer {
                                let _ = transport.send_to(m2, from).await;
                            }
                        }
                    }
                    // STUN replies and leftover punch Pings/Pongs: ignore.
                    _ => {}
                }
            }
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow() {
                    break;
                }
            }
        }
    }
}

/// A decoded inner (decrypted) payload.
enum Inner<'a> {
    /// Keepalive control: `(kind, seq)` where kind is [`INNER_PING`]/[`INNER_PONG`].
    Ctrl(u8, u64),
    /// A tunnelled IP packet (no FEC).
    Data(&'a [u8]),
    /// A FEC-protected IP packet with its group id and index.
    FecData { group: u32, index: u8, ip: &'a [u8] },
    /// A FEC parity packet: its group id, member count `k`, and `Q` bytes.
    FecParity { group: u32, k: u8, r: u8, index: u8, q: &'a [u8] },
}

/// Encode a keepalive control payload: `[kind:1][seq:8 BE]` (sealed before send).
fn ctrl_encode(kind: u8, seq: u64) -> [u8; 9] {
    let mut b = [0u8; 9];
    b[0] = kind;
    b[1..].copy_from_slice(&seq.to_be_bytes());
    b
}

/// The split-tunnel gate (E.1), applied to every packet crossing the tunnel in
/// either direction.
///
/// **Fail-closed:** a missing policy admits nothing. Today a policy is always
/// present whenever a data plane exists (both are produced by the same match in
/// [`run`]), but a leak-prevention filter must not depend on that coupling
/// holding — an absent policy must never mean "send everything".
fn admits_packet(policy: Option<&SplitPolicy>, frame: &[u8]) -> bool {
    policy.map_or(false, |p| p.admits(frame))
}

/// What actually happened to an inbound packet — distinguished so the packet log
/// never claims a packet reached the adapter when it did not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Injected {
    /// Queued for the adapter.
    Delivered,
    /// Rejected by the ingress policy.
    Blocked,
    /// Admitted, but the adapter queue was full (or absent) — dropped-newest.
    Queueless,
}

/// Inject an inbound tunnel packet onto the adapter, subject to the ingress
/// policy ([`SplitPolicy::admits_inbound`], which also checks the source so a
/// hostile peer cannot forge one). Fail-closed: no policy admits nothing.
fn inject_downlink(
    frame: &[u8],
    policy: Option<&SplitPolicy>,
    downlink_tx: Option<&std_mpsc::SyncSender<Vec<u8>>>,
) -> Injected {
    if !policy.map_or(false, |p| p.admits_inbound(frame)) {
        return Injected::Blocked;
    }
    match downlink_tx {
        Some(tx) if tx.try_send(frame.to_vec()).is_ok() => Injected::Delivered,
        _ => Injected::Queueless,
    }
}

/// Encode a tunnelled IP packet payload: `[INNER_DATA:1][ip bytes…]`.
fn dp_encode(ip: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(1 + ip.len());
    v.push(INNER_DATA);
    v.extend_from_slice(ip);
    v
}

/// Encode a FEC data payload: `[INNER_FEC_DATA][group:4 BE][index:1][ip…]`.
fn fec_data_encode(group: u32, index: u8, ip: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(6 + ip.len());
    v.push(INNER_FEC_DATA);
    v.extend_from_slice(&group.to_be_bytes());
    v.push(index);
    v.extend_from_slice(ip);
    v
}

/// Encode a FEC parity payload:
/// `[INNER_FEC_PARITY][group:4 BE][k:1][r:1][parity index:1][shard…]`.
fn fec_parity_encode(p: &RsParity) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + p.shard.len());
    v.push(INNER_FEC_PARITY);
    v.extend_from_slice(&p.group.to_be_bytes());
    v.push(p.k);
    v.push(p.r);
    v.push(p.index);
    v.extend_from_slice(&p.shard);
    v
}

/// Seal `inner` into a `Data` frame and send it to `peer` (best-effort).
async fn send_sealed(
    transport: &UdpTransport,
    session: &mut CryptoSession,
    peer: SocketAddr,
    inner: &[u8],
) {
    if let Ok((counter, ct)) = session.seal(inner) {
        let _ = transport.send_to(&frame::encode_data(counter, &ct), peer).await;
    }
}

/// Build the packet-log entry for an inbound packet, reporting what *actually*
/// happened to it rather than assuming it reached the adapter.
fn inbound_entry(started: Instant, outcome: Injected, ip: &[u8], note: String) -> PacketLogEntry {
    match outcome {
        Injected::Delivered => pkt(started, Direction::Rx, &proto_label(ip), ip.len(), note),
        Injected::Blocked => pkt(
            started,
            Direction::Rx,
            "DROP",
            ip.len(),
            "split-tunnel blocked (inbound)".to_string(),
        ),
        Injected::Queueless => pkt(
            started,
            Direction::Rx,
            "DROP",
            ip.len(),
            "adapter queue full — packet dropped".to_string(),
        ),
    }
}

/// Inject a FEC-recovered IP packet onto the adapter and log it. Subject to the
/// same ingress policy as a directly-received packet.
fn forward_recovered(
    rec: &[u8],
    policy: Option<&SplitPolicy>,
    downlink_tx: Option<&std_mpsc::SyncSender<Vec<u8>>>,
    started: Instant,
    pending: &mut LogBatch,
) {
    let outcome = inject_downlink(rec, policy, downlink_tx);
    pending.push(inbound_entry(
        started,
        outcome,
        rec,
        "recovered via FEC".to_string(),
    ));
}

/// Decode a decrypted inner payload into control or data.
fn decode_inner(pt: &[u8]) -> Option<Inner<'_>> {
    match pt.first()? {
        &INNER_PING | &INNER_PONG => {
            if pt.len() < 9 {
                return None;
            }
            let seq = u64::from_be_bytes(pt[1..9].try_into().ok()?);
            Some(Inner::Ctrl(pt[0], seq))
        }
        &INNER_DATA => Some(Inner::Data(&pt[1..])),
        &INNER_FEC_DATA => {
            if pt.len() < 6 {
                return None; // tag(1) + group(4) + index(1)
            }
            let group = u32::from_be_bytes(pt[1..5].try_into().ok()?);
            Some(Inner::FecData {
                group,
                index: pt[5],
                ip: &pt[6..],
            })
        }
        &INNER_FEC_PARITY => {
            if pt.len() < 8 {
                return None; // tag(1) + group(4) + k(1) + r(1) + index(1)
            }
            let group = u32::from_be_bytes(pt[1..5].try_into().ok()?);
            Some(Inner::FecParity {
                group,
                k: pt[5],
                r: pt[6],
                index: pt[7],
                q: &pt[8..],
            })
        }
        _ => None,
    }
}

/// Best-effort protocol label for the packet log (falls back to "IP").
fn proto_label(ip: &[u8]) -> String {
    packet::classify(ip)
        .map(|m| m.proto.to_string())
        .unwrap_or_else(|| "IP".to_string())
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

fn pkt(started: Instant, dir: Direction, proto: &str, len: usize, note: String) -> PacketLogEntry {
    PacketLogEntry {
        t_ms: started.elapsed().as_millis() as u64,
        dir,
        proto: proto.to_string(),
        len: len as u16,
        note,
    }
}

/// Full-path integration harness (Phase F.0).
///
/// Two complete [`run`] tasks are connected over loopback with a mock adapter on
/// each side, so a packet handed to one virtual adapter must come back out of
/// the other having travelled the **real** path: hole-punch handshake, Noise
/// session, FEC, split-tunnel policy, UDP transport and the data-plane bridge.
///
/// What this deliberately does **not** cover: real NAT traversal (there is no
/// NAT on loopback), the real Wintun driver, and the Windows firewall/routing
/// stack. Those remain verifiable only on two real machines.
#[cfg(test)]
mod integration {
    use super::*;
    use crate::engine::crypto::identity::Identity;
    use crate::engine::dataplane::MockTun;
    use crate::engine::tun::TunConfig;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use std::net::Ipv4Addr;
    use std::time::Duration;

    /// A sink that ignores everything — these tests assert on observable link
    /// state and adapter traffic, not on telemetry.
    struct NullSink;
    impl TelemetrySink for NullSink {
        fn stats(&self, _: &TelemetrySnapshot) {}
        fn packets(&self, _: &[PacketLogEntry]) {}
        fn state(&self, _: EngineState) {}
        fn notice(&self, _: &EngineNotice) {}
    }

    fn tun_cfg(last_octet: u8) -> TunConfig {
        TunConfig {
            virtual_ip: Ipv4Addr::new(10, 77, 0, last_octet),
            ..TunConfig::default()
        }
    }

    /// A minimal well-formed IPv4 packet with the given addresses and body.
    fn ipv4(src: [u8; 4], dst: [u8; 4], body: &[u8]) -> Vec<u8> {
        let total = 20 + body.len();
        let mut p = vec![0u8; total];
        p[0] = 0x45; // IPv4, IHL 5
        p[2..4].copy_from_slice(&(total as u16).to_be_bytes());
        p[9] = 17; // UDP, so the packet log labels it
        p[12..16].copy_from_slice(&src);
        p[16..20].copy_from_slice(&dst);
        p[20..].copy_from_slice(body);
        p
    }

    /// Poll `cond` until true, or fail after `secs`.
    async fn until(secs: u64, label: &str, mut cond: impl FnMut() -> bool) {
        for _ in 0..(secs * 100) {
            if cond() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for: {label}");
    }

    struct Pair {
        a_tun: MockTun,
        b_tun: MockTun,
        a_link: Arc<Mutex<LinkState>>,
        b_link: Arc<Mutex<LinkState>>,
        a_cancel: watch::Sender<bool>,
        b_cancel: watch::Sender<bool>,
        a_task: tokio::task::JoinHandle<()>,
        b_task: tokio::task::JoinHandle<()>,
    }

    /// Bring up a connected pair over loopback, each with a mock adapter.
    async fn connect_pair() -> Pair {
        let a_id = Arc::new(Identity::generate().unwrap());
        let b_id = Arc::new(Identity::generate().unwrap());
        let a_pub = STANDARD.decode(a_id.public_b64()).unwrap();
        let b_pub = STANDARD.decode(b_id.public_b64()).unwrap();

        let a_tx = UdpTransport::bind(0).await.unwrap();
        let b_tx = UdpTransport::bind(0).await.unwrap();
        let a_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, a_tx.local_addr().unwrap().port()));
        let b_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, b_tx.local_addr().unwrap().port()));
        // RFC 5737 TEST-NET-1: unreachable, so the fan-out must ignore it.
        let dead = SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 9));

        let (a_tun, b_tun) = (MockTun::default(), MockTun::default());
        let a_link = Arc::new(Mutex::new(LinkState::Connecting));
        let b_link = Arc::new(Mutex::new(LinkState::Connecting));
        let (a_cancel, a_rx) = watch::channel(false);
        let (b_cancel, b_rx) = watch::channel(false);

        let a_task = tokio::spawn(run(
            a_tx,
            Role::Initiator,
            a_id,
            b_pub,
            vec![dead, b_addr],
            DataPlaneSource::Device(a_tun.device(), tun_cfg(1)),
            Box::new(NullSink),
            a_link.clone(),
            a_rx,
        ));
        let b_task = tokio::spawn(run(
            b_tx,
            Role::Responder,
            b_id,
            a_pub,
            vec![a_addr],
            DataPlaneSource::Device(b_tun.device(), tun_cfg(2)),
            Box::new(NullSink),
            b_link.clone(),
            b_rx,
        ));

        let (al, bl) = (a_link.clone(), b_link.clone());
        until(15, "both sides Connected", move || {
            *al.lock().unwrap() == LinkState::Connected
                && *bl.lock().unwrap() == LinkState::Connected
        })
        .await;

        Pair { a_tun, b_tun, a_link, b_link, a_cancel, b_cancel, a_task, b_task }
    }

    /// The handshake completes through the real fan-out, ignoring a dead
    /// candidate, and both sides reach Connected.
    #[tokio::test]
    async fn two_pipelines_complete_the_handshake() {
        let p = connect_pair().await;
        assert_eq!(*p.a_link.lock().unwrap(), LinkState::Connected);
        assert_eq!(*p.b_link.lock().unwrap(), LinkState::Connected);
        let _ = p.a_cancel.send(true);
        let _ = p.b_cancel.send(true);
        let _ = p.a_task.await;
        let _ = p.b_task.await;
    }

    /// ⭐ The end-to-end proof: a packet handed to A's virtual adapter comes out
    /// of B's virtual adapter byte-identical, having traversed the entire real
    /// stack in both processes.
    #[tokio::test]
    async fn a_packet_crosses_the_tunnel_intact() {
        let p = connect_pair().await;

        let packet = ipv4([10, 77, 0, 1], [10, 77, 0, 2], b"hello over the tunnel");
        p.a_tun.send_from_host(packet.clone());

        let b = p.b_tun.clone();
        until(10, "the packet to arrive on B's adapter", move || {
            !b.injected().is_empty()
        })
        .await;

        assert_eq!(p.b_tun.injected()[0], packet, "bytes must survive intact");
        // Nothing should have leaked back onto the sender's own adapter.
        assert!(p.a_tun.injected().is_empty());

        let _ = p.a_cancel.send(true);
        let _ = p.b_cancel.send(true);
        let _ = p.a_task.await;
        let _ = p.b_task.await;
    }

    /// The split-tunnel policy holds on the real path: a packet addressed
    /// outside the virtual subnet never reaches the peer.
    #[tokio::test]
    async fn policy_blocks_a_packet_bound_outside_the_virtual_lan() {
        let p = connect_pair().await;

        p.a_tun
            .send_from_host(ipv4([10, 77, 0, 1], [8, 8, 8, 8], b"should never leave"));
        // Then a legitimate one, so we can prove the first was dropped rather
        // than merely slower — the tunnel is ordered.
        let allowed = ipv4([10, 77, 0, 1], [10, 77, 0, 2], b"this one is fine");
        p.a_tun.send_from_host(allowed.clone());

        let b = p.b_tun.clone();
        until(10, "the allowed packet to arrive", move || {
            !b.injected().is_empty()
        })
        .await;

        let got = p.b_tun.injected();
        assert_eq!(got.len(), 1, "only the in-subnet packet may cross");
        assert_eq!(got[0], allowed);

        let _ = p.a_cancel.send(true);
        let _ = p.b_cancel.send(true);
        let _ = p.a_task.await;
        let _ = p.b_task.await;
    }

    /// Disconnecting tears the link down cleanly on both sides.
    #[tokio::test]
    async fn cancel_returns_both_sides_to_idle() {
        let p = connect_pair().await;
        let _ = p.a_cancel.send(true);
        let _ = p.b_cancel.send(true);
        let _ = p.a_task.await;
        let _ = p.b_task.await;
        assert_eq!(*p.a_link.lock().unwrap(), LinkState::Idle);
        assert_eq!(*p.b_link.lock().unwrap(), LinkState::Idle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_payload_roundtrip() {
        let b = ctrl_encode(INNER_PING, 0x0102_0304_0506_0708);
        match decode_inner(&b) {
            Some(Inner::Ctrl(INNER_PING, seq)) => assert_eq!(seq, 0x0102_0304_0506_0708),
            _ => panic!("expected a ping control payload"),
        }
        assert!(matches!(decode_inner(&[INNER_PONG]), None)); // too short
    }

    #[test]
    fn data_payload_roundtrip() {
        let ip = [0x45u8, 0x00, 0x00, 0x1c, 0xde, 0xad];
        let enc = dp_encode(&ip);
        match decode_inner(&enc) {
            Some(Inner::Data(body)) => assert_eq!(body, &ip),
            _ => panic!("expected a data payload"),
        }
    }

    /// The split-tunnel gate used by both the uplink and the downlink. Pins the
    /// *polarity* (admitted → true) and the fail-closed default, which the
    /// `SplitPolicy` unit tests cannot cover on their own.
    #[test]
    fn admits_packet_gate_polarity_and_fail_closed() {
        use crate::engine::tun::TunConfig;
        use std::net::Ipv4Addr;

        let policy = SplitPolicy::from_tun(&TunConfig {
            virtual_ip: Ipv4Addr::new(10, 77, 0, 1),
            prefix_len: 24,
            ..TunConfig::default()
        });
        let mut frame = [0u8; 20];
        frame[0] = 0x45;
        frame[16] = 10;
        frame[17] = 77;
        frame[18] = 0;
        frame[19] = 2;

        // In-subnet unicast is admitted…
        assert!(admits_packet(Some(&policy), &frame));
        // …an out-of-subnet destination is not…
        frame[16] = 8;
        frame[17] = 8;
        frame[18] = 8;
        frame[19] = 8;
        assert!(!admits_packet(Some(&policy), &frame));
        // …and no policy admits nothing (fail-closed, never "send everything").
        frame[16] = 10;
        frame[17] = 77;
        frame[18] = 0;
        frame[19] = 2;
        assert!(!admits_packet(None, &frame));
    }

    /// The batch must stay bounded no matter the packet rate, and must make the
    /// gap visible rather than silently losing entries.
    #[test]
    fn log_batch_is_bounded_and_reports_suppression() {
        let started = Instant::now();
        let mut b = LogBatch::new();
        for i in 0..(PACKET_LOG_BATCH_CAP + 500) {
            b.push(pkt(started, Direction::Rx, "X", i, String::new()));
        }
        let out = b.take(started);
        // cap entries + one summary line
        assert_eq!(out.len(), PACKET_LOG_BATCH_CAP + 1);
        assert_eq!(out.last().unwrap().proto, "LOG");
        assert!(out.last().unwrap().note.contains("500 further entries suppressed"));
        // Taking resets the batch.
        assert!(!b.has_content());
        assert!(b.take(started).is_empty());
    }

    #[test]
    fn unknown_inner_kind_is_none() {
        assert!(matches!(decode_inner(&[0x00, 1, 2]), None));
        assert!(matches!(decode_inner(&[]), None));
    }

    /// End-to-end (headless): a tunnelled IP packet survives the full data-plane
    /// path — `dp_encode` → `seal` → `Data` frame → transport → `open` →
    /// `decode_inner` — across two real handshake-established sessions. This is
    /// the crypto/frame core of the uplink→downlink pumps, with no TUN adapter.
    #[tokio::test]
    async fn data_plane_ip_packet_survives_seal_transport_open() {
        use crate::engine::crypto::handshake;
        use crate::engine::crypto::identity::Identity;
        use crate::engine::transport::UdpTransport;
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        use std::net::Ipv4Addr;

        let a_id = Identity::generate().unwrap();
        let b_id = Identity::generate().unwrap();
        let a_pub = STANDARD.decode(a_id.public_b64()).unwrap();
        let b_pub = STANDARD.decode(b_id.public_b64()).unwrap();

        let a_tx = UdpTransport::bind(0).await.unwrap();
        let b_tx = UdpTransport::bind(0).await.unwrap();
        let b_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, b_tx.local_addr().unwrap().port()));

        let responder = tokio::spawn(async move {
            let (_from, session) = handshake::respond(&b_tx, &b_id, Some(&a_pub)).await.unwrap();
            (b_tx, session)
        });
        let mut a_session = handshake::initiate(&a_tx, b_addr, &a_id, &b_pub).await.unwrap();
        let (b_tx, mut b_session) = responder.await.unwrap();

        // A tunnels an IPv4-like packet to B.
        let ip = [0x45u8, 0x00, 0x00, 0x1c, 1, 2, 3, 4];
        let (counter, ct) = a_session.seal(&dp_encode(&ip)).unwrap();
        a_tx.send_to(&frame::encode_data(counter, &ct), b_addr)
            .await
            .unwrap();

        let mut buf = vec![0u8; 2048];
        let (n, _from) = b_tx.recv_from(&mut buf).await.unwrap();
        let (rc, ciphertext) = match frame::classify(&buf[..n]) {
            Inbound::Frame(FrameKind::Data, payload) => frame::decode_data(payload).unwrap(),
            other => panic!("expected data frame, got {other:?}"),
        };
        let plaintext = b_session.open(rc, ciphertext).unwrap();
        match decode_inner(&plaintext) {
            Some(Inner::Data(body)) => assert_eq!(body, &ip),
            _ => panic!("expected the tunnelled IP packet to survive intact"),
        }
    }

    /// End-to-end (headless): with FEC on, a data packet dropped in transit is
    /// reconstructed at the receiver from the group's parity — through the real
    /// seal → transport → open path across two handshake-established sessions.
    #[tokio::test]
    async fn fec_recovers_a_dropped_data_packet_end_to_end() {
        use crate::engine::crypto::handshake;
        use crate::engine::crypto::identity::Identity;
        use crate::engine::transport::UdpTransport;
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        use std::net::Ipv4Addr;

        let a_id = Identity::generate().unwrap();
        let b_id = Identity::generate().unwrap();
        let a_pub = STANDARD.decode(a_id.public_b64()).unwrap();
        let b_pub = STANDARD.decode(b_id.public_b64()).unwrap();
        let a_tx = UdpTransport::bind(0).await.unwrap();
        let b_tx = UdpTransport::bind(0).await.unwrap();
        let b_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, b_tx.local_addr().unwrap().port()));

        let responder = tokio::spawn(async move {
            let (_from, s) = handshake::respond(&b_tx, &b_id, Some(&a_pub)).await.unwrap();
            (b_tx, s)
        });
        let mut a_session = handshake::initiate(&a_tx, b_addr, &a_id, &b_pub).await.unwrap();
        let (b_tx, mut b_session) = responder.await.unwrap();

        // Sender: 6 packets with r=2 parity; drop TWO data packets in transit
        // (never sealed/sent) — beyond what D.1's single XOR parity could rebuild.
        let mut enc = RsEncoder::new(6, 2);
        let packets: Vec<Vec<u8>> = (0..6).map(|i| vec![0x10 + i as u8; 20 + i * 4]).collect();
        let dropped = [1u8, 4u8];
        let mut sent = 0usize;
        for p in &packets {
            let (g, idx, parity) = enc.push(p);
            if !dropped.contains(&idx) {
                let (c, ct) = a_session.seal(&fec_data_encode(g, idx, p)).unwrap();
                a_tx.send_to(&frame::encode_data(c, &ct), b_addr).await.unwrap();
                sent += 1;
            }
            for par in &parity {
                let (c, ct) = a_session.seal(&fec_parity_encode(par)).unwrap();
                a_tx.send_to(&frame::encode_data(c, &ct), b_addr).await.unwrap();
                sent += 1;
            }
        }

        // Receiver: decrypt each datagram and drive the decoder (4 data + 2 parity).
        let mut dec = RsDecoder::new(64);
        let mut recovered: Vec<Vec<u8>> = Vec::new();
        let mut buf = vec![0u8; 2048];
        for _ in 0..sent {
            let (n, _from) = b_tx.recv_from(&mut buf).await.unwrap();
            let (c, ct) = match frame::classify(&buf[..n]) {
                Inbound::Frame(FrameKind::Data, payload) => frame::decode_data(payload).unwrap(),
                other => panic!("expected data frame, got {other:?}"),
            };
            let pt = b_session.open(c, ct).unwrap();
            match decode_inner(&pt) {
                Some(Inner::FecData { group, index, ip }) => {
                    recovered.extend(dec.on_data(group, index, ip));
                }
                Some(Inner::FecParity { group, k, r, index, q }) => {
                    recovered.extend(dec.on_parity(group, k, r, index, q));
                }
                _ => panic!("unexpected inner payload on the FEC stream"),
            }
        }
        assert_eq!(recovered.len(), 2, "both dropped packets must be rebuilt");
        assert!(recovered.contains(&packets[1]));
        assert!(recovered.contains(&packets[4]));
    }
}
