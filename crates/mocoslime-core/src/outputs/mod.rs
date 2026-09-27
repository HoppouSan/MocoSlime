//! SlimeVR tracking output router.
//!
//! ```text
//! Tracking Core ─┬─► SlimeVR UDP
//! ```
//!
//! The tracking core only knows [`TrackingOutput`]; concrete protocols live

pub mod slimevr;

pub use slimevr::{SlimevrOutput, SlimevrStatus};

use async_trait::async_trait;
use mocopi_protocol::BatteryInfo;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use uuid::Uuid;

/// One filtered tracking frame, protocol-agnostic.
#[derive(Debug, Clone)]
pub struct OutputFrame {
    pub device_id: Uuid,
    /// SlimeVR sensor id (= role discriminant, 0–14). Only meaningful
    /// when `assigned` is true.
    pub sensor_id: u8,
    /// Whether the tracker has a body role assigned. SlimeVR skips
    /// unassigned frames.
    pub assigned: bool,
    pub quaternion: nalgebra::UnitQuaternion<f32>,
    pub acceleration: nalgebra::Vector3<f32>,
    pub counter: u64,
}

/// Handshake context for stateful outputs (SlimeVR).
#[derive(Debug, Clone)]
pub struct HandshakeCtx {
    pub device_id: Uuid,
    pub mac: String,
    pub firmware: String,
    pub sensor_id: u8,
}

/// Battery event for outputs.
#[derive(Debug, Clone)]
pub struct BatteryEvent {
    pub device_id: Uuid,
    pub sensor_id: u8,
    pub assigned: bool,
    pub battery: BatteryInfo,
}

/// Status event for outputs.
#[derive(Debug, Clone)]
pub struct StatusEvent {
    pub device_id: Uuid,
    pub sensor_id: u8,
    pub assigned: bool,
    /// One of `disconnected|connecting|connected|streaming|reconnecting|error`.
    pub status: &'static str,
    pub battery: Option<BatteryInfo>,
    pub rssi: Option<i16>,
}

/// Counters per output (shown in the GUI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OutputStats {
    pub packets_sent: u64,
    pub errors: u64,
}

/// SlimeVR output abstraction.
///
/// Async methods use `async_trait`; implementations must never block the
/// tracking core (bounded channels, drop stale frames).
#[async_trait]
pub trait TrackingOutput: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_enabled(&self) -> bool;
    fn set_enabled(&self, enabled: bool);
    async fn send_handshake(&self, ctx: &HandshakeCtx);
    async fn send_frame(&self, frame: &OutputFrame);
    async fn send_battery(&self, event: &BatteryEvent);
    async fn send_status(&self, event: &StatusEvent);
    /// Pose-Wechsel (VRC-Avatar-Params). Default No-op (SlimeVR ignoriert).
    fn stats(&self) -> OutputStats;
}

/// Shared atomic counters.
#[derive(Debug, Default)]
pub struct Stats {
    pub packets_sent: AtomicU64,
    pub errors: AtomicU64,
}

impl Stats {
    pub fn snapshot(&self) -> OutputStats {
        OutputStats {
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

/// Fan-out router: every enabled output receives every event.
pub struct OutputRouter {
    outputs: Vec<Arc<dyn TrackingOutput>>,
}

impl OutputRouter {
    pub fn new(outputs: Vec<Arc<dyn TrackingOutput>>) -> Self {
        Self { outputs }
    }

    pub async fn send_handshake(&self, ctx: &HandshakeCtx) {
        for out in &self.outputs {
            if out.is_enabled() {
                out.send_handshake(ctx).await;
            }
        }
    }

    pub async fn send_frame(&self, frame: &OutputFrame) {
        for out in &self.outputs {
            if out.is_enabled() {
                out.send_frame(frame).await;
            }
        }
    }

    pub async fn send_battery(&self, event: &BatteryEvent) {
        for out in &self.outputs {
            if out.is_enabled() {
                out.send_battery(event).await;
            }
        }
    }

    pub async fn send_status(&self, event: &StatusEvent) {
        for out in &self.outputs {
            if out.is_enabled() {
                out.send_status(event).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64};

    struct StubOutput {
        enabled: AtomicBool,
        frames: AtomicU64,
    }

    impl StubOutput {
        fn new(enabled: bool) -> Arc<Self> {
            Arc::new(Self {
                enabled: AtomicBool::new(enabled),
                frames: AtomicU64::new(0),
            })
        }
    }

    #[async_trait]
    impl TrackingOutput for StubOutput {
        fn name(&self) -> &'static str {
            "stub"
        }
        fn is_enabled(&self) -> bool {
            self.enabled.load(Ordering::Relaxed)
        }
        fn set_enabled(&self, enabled: bool) {
            self.enabled.store(enabled, Ordering::Relaxed);
        }
        async fn send_handshake(&self, _ctx: &HandshakeCtx) {}
        async fn send_frame(&self, _frame: &OutputFrame) {
            self.frames.fetch_add(1, Ordering::Relaxed);
        }
        async fn send_battery(&self, _event: &BatteryEvent) {}
        async fn send_status(&self, _event: &StatusEvent) {}
        fn stats(&self) -> OutputStats {
            OutputStats::default()
        }
    }

    fn test_frame() -> OutputFrame {
        OutputFrame {
            device_id: Uuid::new_v4(),
            sensor_id: 2,
            assigned: true,
            quaternion: nalgebra::UnitQuaternion::identity(),
            acceleration: nalgebra::Vector3::zeros(),
            counter: 0,
        }
    }

    #[tokio::test]
    async fn router_fans_out_only_to_enabled_outputs() {
        let on = StubOutput::new(true);
        let off = StubOutput::new(false);
        let router = OutputRouter::new(vec![on.clone(), off.clone()]);
        router.send_frame(&test_frame()).await;
        assert_eq!(on.frames.load(Ordering::Relaxed), 1);
        assert_eq!(off.frames.load(Ordering::Relaxed), 0);
    }
}
