//! Telemetry: stats/packet DTOs, the bounded packet log, the `TelemetrySink`
//! seam, the Phase-A simulator, and the async emission loop.

pub mod capture;
pub mod metrics;
pub mod packet_log;
pub mod simulator;
pub mod sink;
pub mod task;

pub use metrics::TelemetrySnapshot;
pub use packet_log::{PacketLogEntry, RingBuffer};
pub use sink::TelemetrySink;
