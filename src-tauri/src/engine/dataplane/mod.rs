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
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("open task join: {e}")))??;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tun::device::DeviceInfo;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// A fake layer-3 device: yields queued frames on read, records writes.
    struct MockTun {
        to_read: VecDeque<Vec<u8>>,
        written: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl TunDevice for MockTun {
        fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
            match self.to_read.pop_front() {
                Some(f) => {
                    let n = f.len().min(buf.len());
                    buf[..n].copy_from_slice(&f[..n]);
                    Ok(Some(n))
                }
                None => Ok(None),
            }
        }
        fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
            self.written.lock().unwrap().push(frame.to_vec());
            Ok(frame.len())
        }
        fn info(&self) -> DeviceInfo {
            DeviceInfo {
                name: "mock".to_string(),
                mtu: 1420,
            }
        }
    }

    /// Frames read off the device surface on the uplink; packets pushed to the
    /// downlink are written back onto the device.
    #[tokio::test]
    async fn bridge_pumps_both_directions() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let mock = MockTun {
            to_read: VecDeque::from(vec![vec![1, 2, 3], vec![4, 5, 6, 7]]),
            written: written.clone(),
        };
        let (cancel, rx) = watch::channel(false);
        let mut dp = spawn_bridge(Box::new(mock), rx, 1420);

        // Uplink: the two queued frames emerge in order.
        assert_eq!(dp.uplink_rx.recv().await.unwrap(), vec![1, 2, 3]);
        assert_eq!(dp.uplink_rx.recv().await.unwrap(), vec![4, 5, 6, 7]);

        // Downlink: a pushed packet gets written to the device.
        dp.downlink_tx.send(vec![9, 9, 9]).unwrap();
        for _ in 0..200 {
            if !written.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert_eq!(written.lock().unwrap().as_slice(), &[vec![9, 9, 9]]);

        cancel.send(true).unwrap();
        let _ = dp.handle.await;
    }
}
