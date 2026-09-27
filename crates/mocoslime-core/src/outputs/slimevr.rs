//! SlimeVR UDP output (`outputs/slimevr.rs`).
//!
//! Owns its socket, packet counter and per-tracker rate limiter. Sends at
//! most `packet_rate` frames per tracker per second (`0` = unlimited).

use super::{
    BatteryEvent, HandshakeCtx, OutputFrame, OutputStats, Stats, StatusEvent, TrackingOutput,
};
use crate::error::{CoreError, Result};
use async_trait::async_trait;
use slimevr_protocol::{
    build_acceleration_packet, build_battery_packet, build_error_packet, build_handshake_packet,
    build_rotation_packet, build_sensor_info_packet,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{net::UdpSocket, sync::Mutex};
use tracing::{debug, warn};
use uuid::Uuid;

pub struct SlimevrOutput {
    enabled: AtomicBool,
    packet_rate: Mutex<u32>,
    /// SlimeVR identifies a UDP tracker by its source socket address. Each
    /// physical Mocopi tracker therefore needs its own local UDP port.
    sockets: Mutex<HashMap<Uuid, Arc<UdpSocket>>>,
    addr: Mutex<Option<SocketAddr>>,
    auto_discovered: AtomicBool,
    counters: Mutex<HashMap<Uuid, u64>>,
    last_send: Mutex<HashMap<Uuid, Instant>>,
    stats: Stats,
}

/// Status-Snapshot für die GUI (P3): Ziel, Auto/Manuell, Zähler.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlimevrStatus {
    pub enabled: bool,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub auto_discovered: bool,
    pub packets_sent: u64,
    pub errors: u64,
}

impl SlimevrOutput {
    pub fn new(enabled: bool, packet_rate: u32) -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(enabled),
            packet_rate: Mutex::new(packet_rate),
            sockets: Mutex::new(HashMap::new()),
            addr: Mutex::new(None),
            auto_discovered: AtomicBool::new(false),
            counters: Mutex::new(HashMap::new()),
            last_send: Mutex::new(HashMap::new()),
            stats: Stats::default(),
        })
    }

    pub async fn set_config(&self, enabled: bool, packet_rate: u32) {
        self.enabled.store(enabled, Ordering::Relaxed);
        *self.packet_rate.lock().await = packet_rate;
    }

    /// Bind a local socket and connect it to the SlimeVR server.
    /// `auto` markiert Autodiscovery (GUI zeigt "Auto", sonst "Manuell").
    pub async fn set_target(&self, ip: &str, port: u16, auto: bool) -> Result<()> {
        let addr: SocketAddr =
            format!("{ip}:{port}")
                .parse()
                .map_err(
                    |e: std::net::AddrParseError| CoreError::ConfigurationError {
                        reason: e.to_string(),
                    },
                )?;
        // Validate the endpoint with a temporary socket before replacing the
        // target; each tracker gets its own source port when it sends.
        let probe =
            UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(|e| CoreError::SlimeVRConnectionFailed {
                    reason: e.to_string(),
                })?;
        probe
            .connect(addr)
            .await
            .map_err(|e| CoreError::SlimeVRConnectionFailed {
                reason: e.to_string(),
            })?;
        self.sockets.lock().await.clear();
        *self.addr.lock().await = Some(addr);
        self.auto_discovered.store(auto, Ordering::Relaxed);
        Ok(())
    }

    pub async fn clear_target(&self) {
        self.sockets.lock().await.clear();
        *self.addr.lock().await = None;
        self.auto_discovered.store(false, Ordering::Relaxed);
    }

    async fn next_counter(&self, device_id: Uuid, step: u64) -> u64 {
        let mut guard = self.counters.lock().await;
        let counter = guard.entry(device_id).or_default();
        *counter += step;
        *counter
    }

    /// Per-tracker rate gate. Returns false when the frame must be skipped.
    async fn rate_ok(&self, device_id: Uuid) -> bool {
        let rate = *self.packet_rate.lock().await;
        if rate == 0 {
            return true;
        }
        let min_interval = Duration::from_secs_f64(1.0 / f64::from(rate.max(1)));
        let mut guard = self.last_send.lock().await;
        let now = Instant::now();
        match guard.get(&device_id) {
            Some(last) if now.duration_since(*last) < min_interval => false,
            _ => {
                guard.insert(device_id, now);
                true
            }
        }
    }

    pub async fn has_target(&self) -> bool {
        self.addr.lock().await.is_some()
    }

    pub async fn status(&self) -> SlimevrStatus {
        let addr = *self.addr.lock().await;
        let stats = self.stats.snapshot();
        SlimevrStatus {
            enabled: self.enabled.load(Ordering::Relaxed),
            ip: addr.map(|a| a.ip().to_string()),
            port: addr.map(|a| a.port()),
            auto_discovered: self.auto_discovered.load(Ordering::Relaxed),
            packets_sent: stats.packets_sent,
            errors: stats.errors,
        }
    }

    async fn socket_for(&self, device_id: Uuid) -> Result<Option<Arc<UdpSocket>>> {
        let mut sockets = self.sockets.lock().await;
        if let Some(socket) = sockets.get(&device_id) {
            return Ok(Some(socket.clone()));
        }
        let Some(addr) = *self.addr.lock().await else {
            return Ok(None);
        };
        let socket =
            UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(|e| CoreError::SlimeVRConnectionFailed {
                    reason: e.to_string(),
                })?;
        socket
            .connect(addr)
            .await
            .map_err(|e| CoreError::SlimeVRConnectionFailed {
                reason: e.to_string(),
            })?;
        let socket = Arc::new(socket);
        sockets.insert(device_id, socket.clone());
        Ok(Some(socket))
    }

    async fn send_raw(&self, device_id: Uuid, data: &[u8]) {
        match self.socket_for(device_id).await {
            Ok(Some(socket)) => {
                // The socket is connected to this tracker's SlimeVR target.
                if let Err(e) = socket.send(data).await {
                    self.stats.errors.fetch_add(1, Ordering::Relaxed);
                    warn!("SlimeVR send failed: {e}");
                    return;
                }
                self.stats.packets_sent.fetch_add(1, Ordering::Relaxed);
            }
            Ok(None) => {}
            Err(e) => {
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
                warn!("SlimeVR socket setup failed for {device_id}: {e}");
            }
        }
    }
}

#[async_trait]
impl TrackingOutput for SlimevrOutput {
    fn name(&self) -> &'static str {
        "slimevr"
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    async fn send_handshake(&self, ctx: &HandshakeCtx) {
        let counter = self.next_counter(ctx.device_id, 1).await;
        debug!(device_id = %ctx.device_id, sensor_id = ctx.sensor_id, counter, "slimevr handshake");
        let handshake = build_handshake_packet(&ctx.mac, &ctx.firmware, counter);
        self.send_raw(ctx.device_id, &handshake).await;
        let sensor_info = build_sensor_info_packet(counter + 1, ctx.sensor_id);
        self.send_raw(ctx.device_id, &sensor_info).await;
    }

    async fn send_frame(&self, frame: &OutputFrame) {
        if !frame.assigned {
            return;
        }
        if !self.rate_ok(frame.device_id).await {
            return;
        }
        let counter = self.next_counter(frame.device_id, 2).await;
        let rot = build_rotation_packet(frame.quaternion, counter, frame.sensor_id);
        self.send_raw(frame.device_id, &rot).await;
        let accel = build_acceleration_packet(frame.acceleration, counter + 1, frame.sensor_id);
        self.send_raw(frame.device_id, &accel).await;
    }

    async fn send_battery(&self, event: &BatteryEvent) {
        if !event.assigned {
            return;
        }
        let counter = self.next_counter(event.device_id, 1).await;
        let packet = build_battery_packet(
            event.battery.voltage,
            event.battery.percentage,
            counter,
            event.sensor_id,
        );
        self.send_raw(event.device_id, &packet).await;
    }

    async fn send_status(&self, event: &StatusEvent) {
        // SlimeVR has no generic status packet; surface hard errors so the
        // server can mark the sensor faulty.
        if event.status == "error" {
            let counter = self.next_counter(event.device_id, 1).await;
            let packet = build_error_packet(counter, event.sensor_id, 1);
            self.send_raw(event.device_id, &packet).await;
        }
    }

    fn stats(&self) -> OutputStats {
        self.stats.snapshot()
    }
}
