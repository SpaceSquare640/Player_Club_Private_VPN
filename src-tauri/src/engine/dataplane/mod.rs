//! Data-plane bridge (Phase C5) — couples the blocking Wintun [`TunDevice`] to
//! the async pipeline driver via a pair of channels.
//!
//! The device API is poll-style and blocking (`read_frame` returns `Ok(None)`
//! when idle; `write_frame` blocks), and its methods take `&mut self`, so it is
//! owned by **one dedicated blocking thread** that interleaves both directions:
//!
//!   * **uplink** (host → peer): `read_frame` → an unbounded-in-spirit but
//!     bounded `tokio::mpsc` the async driver seals and sends.
//!   * **downlink** (peer → host): a bounded `std::mpsc::sync_channel` the driver
//!     fills after decrypting; the thread drains it into `write_frame`. When the
//!     device is idle the thread blocks on `recv_timeout` so writes stay
//!     low-latency instead of waiting out a poll sleep.
//!
//! Both channels are **bounded** and drop-newest on saturation — game traffic
//! prefers a fresh packet over a buffered one, and a stalled consumer must never
//! back-pressure the async driver or the device.

use std::io;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::engine::tun::{self, TunConfig, TunDevice};

/// Bound on each direction's queue. At 256 × MTU (~360 KB) this is generous
/// headroom; beyond it we drop rather than buffer (see module docs).
const CHANNEL_CAP: usize = 256;

/// How long the bridge blocks on the downlink queue when the device is idle,
/// before looping back to poll the device again.
const IDLE_POLL: Duration = Duration::from_millis(1);

/// Back-off after a device read error, so a persistently failing device does
/// not spin the CPU.
const ERR_BACKOFF: Duration = Duration::from_millis(5);

/// A live data-plane bridge: the driver reads outbound IP packets from
/// [`uplink_rx`] and pushes decrypted inbound packets to [`downlink_tx`].
/// Dropping both ends (or signalling the shared shutdown `watch`) stops the
/// bridge thread; [`handle`] joins it so the adapter is fully released.
///
/// [`uplink_rx`]: DataPlane::uplink_rx
/// [`downlink_tx`]: DataPlane::downlink_tx
/// [`handle`]: DataPlane::handle
pub struct DataPlane {
    /// Outbound IP packets captured off the virtual adapter (host → peer).
    pub uplink_rx: tokio_mpsc::Receiver<Vec<u8>>,
    /// Inbound IP packets to inject onto the virtual adapter (peer → host).
    pub downlink_tx: std_mpsc::SyncSender<Vec<u8>>,
    /// The blocking bridge thread; await to join on teardown.
    pub handle: JoinHandle<()>,
}

/// Open the virtual adapter for `cfg` and start the bridge. The (blocking) adapter
/// creation runs on a blocking thread so the caller's async task is not stalled;
/// an open failure (e.g. lost elevation) surfaces here so the caller can fall
/// back to a control-only link.
pub async fn open(cfg: TunConfig, shutdown: watch::Receiver<bool>) -> io::Result<DataPlane> {
    let mtu = cfg.mtu as usize;
    let dev = tokio::task::spawn_blocking(move || tun::open_device(&cfg))
        .await
        .map_err(|e| io::Error::other(format!("open task join: {e}")))??;
    Ok(spawn_bridge(dev, shutdown, mtu))
}

/// Start the bridge over an already-opened device. Split out from [`open`] so
/// tests can drive it with a mock device (no Wintun / elevation required).
pub fn spawn_bridge(
    dev: Box<dyn TunDevice>,
    shutdown: watch::Receiver<bool>,
    mtu: usize,
) -> DataPlane {
    let (uplink_tx, uplink_rx) = tokio_mpsc::channel(CHANNEL_CAP);
    let (downlink_tx, downlink_rx) = std_mpsc::sync_channel(CHANNEL_CAP);
    let handle =
        tokio::task::spawn_blocking(move || bridge(dev, uplink_tx, downlink_rx, shutdown, mtu));
    DataPlane {
        uplink_rx,
        downlink_tx,
        handle,
    }
}

/// The blocking bridge loop. Runs until shutdown is signalled or either channel
/// end is dropped by the async driver.
fn bridge(
    mut dev: Box<dyn TunDevice>,
    uplink_tx: tokio_mpsc::Sender<Vec<u8>>,
    downlink_rx: std_mpsc::Receiver<Vec<u8>>,
    shutdown: watch::Receiver<bool>,
    mtu: usize,
) {
    let mut buf = vec![0u8; mtu + 128];
    loop {
        if *shutdown.borrow() {
            break;
        }

        // Flush any ready inbound packets first (non-blocking).
        loop {
            match downlink_rx.try_recv() {
                Ok(pkt) => {
                    let _ = dev.write_frame(&pkt);
                }
                Err(std_mpsc::TryRecvError::Empty) => break,
                Err(std_mpsc::TryRecvError::Disconnected) => return,
            }
        }

        match dev.read_frame(&mut buf) {
            Ok(Some(n)) => {
                // Drop-newest on a full queue: never block the device.
                let _ = uplink_tx.try_send(buf[..n].to_vec());
            }
            Ok(None) => {
                // Device idle: block briefly on the downlink so a newly-arrived
                // inbound packet is written promptly instead of after a sleep.
                match downlink_rx.recv_timeout(IDLE_POLL) {
                    Ok(pkt) => {
                        let _ = dev.write_frame(&pkt);
                    }
                    Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            Err(_) => std::thread::sleep(ERR_BACKOFF),
        }
    }

    drop(dev); // release the adapter promptly on teardown
}

/// A fake virtual adapter, shared by the data-plane and pipeline tests.
///
/// Standing in for Wintun lets the tests drive the *whole* path — a packet
/// pushed in with [`MockTun::send_from_host`] travels through the real encoder,
/// crypto session and socket, and is observed with [`MockTun::injected`] on the
/// far side — without needing Windows, elevation or a second machine.
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct MockTun {
    /// Packets the host stack "hands" to the adapter (i.e. the uplink source).
    outbound: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<Vec<u8>>>>,
    /// Packets the engine injected onto the adapter (i.e. the downlink sink).
    inbound: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

#[cfg(test)]
impl MockTun {
    /// A device handle backed by this mock. The handle can be moved into the
    /// bridge while the `MockTun` stays here for observation.
    pub(crate) fn device(&self) -> Box<dyn TunDevice> {
        Box::new(MockTunDevice(self.clone()))
    }

    /// Queue a packet as though the host stack routed it to the adapter.
    pub(crate) fn send_from_host(&self, packet: Vec<u8>) {
        self.outbound.lock().unwrap().push_back(packet);
    }

    /// Everything the engine has injected onto the adapter so far.
    pub(crate) fn injected(&self) -> Vec<Vec<u8>> {
        self.inbound.lock().unwrap().clone()
    }
}

#[cfg(test)]
struct MockTunDevice(MockTun);

#[cfg(test)]
impl TunDevice for MockTunDevice {
    fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        match self.0.outbound.lock().unwrap().pop_front() {
            Some(f) => {
                let n = f.len().min(buf.len());
                buf[..n].copy_from_slice(&f[..n]);
                Ok(Some(n))
            }
            None => Ok(None),
        }
    }
    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
        self.0.inbound.lock().unwrap().push(frame.to_vec());
        Ok(frame.len())
    }
    fn info(&self) -> crate::engine::tun::device::DeviceInfo {
        crate::engine::tun::device::DeviceInfo {
            name: "mock".to_string(),
            mtu: 1420,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames read off the device surface on the uplink; packets pushed to the
    /// downlink are written back onto the device.
    #[tokio::test]
    async fn bridge_pumps_both_directions() {
        let mock = MockTun::default();
        mock.send_from_host(vec![1, 2, 3]);
        mock.send_from_host(vec![4, 5, 6, 7]);

        let (cancel, rx) = watch::channel(false);
        let mut dp = spawn_bridge(mock.device(), rx, 1420);

        // Uplink: the two queued frames emerge in order.
        assert_eq!(dp.uplink_rx.recv().await.unwrap(), vec![1, 2, 3]);
        assert_eq!(dp.uplink_rx.recv().await.unwrap(), vec![4, 5, 6, 7]);

        // Downlink: a pushed packet gets written to the device.
        dp.downlink_tx.send(vec![9, 9, 9]).unwrap();
        for _ in 0..200 {
            if !mock.injected().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert_eq!(mock.injected().as_slice(), &[vec![9, 9, 9]]);

        cancel.send(true).unwrap();
        let _ = dp.handle.await;
    }
}
