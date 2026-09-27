use crate::error::{CoreError, Result};
use crate::outputs::{
    BatteryEvent, HandshakeCtx, OutputFrame, OutputRouter, SlimevrOutput, SlimevrStatus,
    StatusEvent,
};
use configuration::{AppConfig, TrackerConfig};
use mocopi_ble_windows::{BleConfig, BleManager, ConnectionState, TrackerStatus};
use mocopi_protocol::{BatteryInfo, DecodedPacket, MocopiPacket};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use tracking_core::{
    CalibrationPose, FilterConfig, PoseDetector, PoseDetectorConfig, SkeletonConfig, TrackerRole,
    TrackingFilter,
};
use uuid::Uuid;

pub struct TrackerManager {
    ble_manager: Arc<BleManager>,
    config: Arc<RwLock<AppConfig>>,
    active_pose: Arc<RwLock<CalibrationPose>>,
    pose_detector: Arc<Mutex<PoseDetector>>,
    auto_pose_enabled: Arc<RwLock<bool>>,
    auto_pose_confidence: Arc<RwLock<f32>>,
    latest_orientations: Arc<Mutex<HashMap<TrackerRole, nalgebra::UnitQuaternion<f32>>>>,
    /// Geglättete Weltpositionen (T5): Tiefpass gegen FK-Jitter.
    smooth_positions: Arc<Mutex<HashMap<TrackerRole, nalgebra::Vector3<f32>>>>,
    /// Letzte zwei gefilterte Orientierungen je Gerät (T4-Prädiktion).
    motion: Arc<Mutex<HashMap<Uuid, (nalgebra::UnitQuaternion<f32>, u64)>>>,
    /// SlimeVR-Health: (last_packets, last_errors, unhealthy_streak, reported).
    slimevr_health: Arc<Mutex<(u64, u64, u8, bool)>>,
    /// CalibrateAll-Warteschlange (Geräte-UUIDs, aktive Pose).
    /// Bereits abgeschlossene Sessions der laufenden Queue.
    /// Gerät der laufenden Kalibrier-Session: nur dessen Pakete zählen
    /// (sonst mischen streamende Tracker den Mittelwert → Müll-Offset).
    filters: Arc<RwLock<HashMap<Uuid, TrackingFilter>>>,
    router: Arc<OutputRouter>,
    slimevr: Arc<SlimevrOutput>,
    running: Arc<RwLock<bool>>,
    event_tx: broadcast::Sender<CoreEvent>,
    command_rx: Arc<Mutex<Option<mpsc::Receiver<CoreCommand>>>>,
    logs: crate::logging::LogBuffer,
    /// Last GUI `TrackingData` emit per device. The output router always
    /// runs at full rate; only the FFI event channel is throttled so the
    /// GUI never slows the realtime loop (docs/tracking.md).
    last_gui_emit: Arc<Mutex<HashMap<Uuid, std::time::Instant>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoreEvent {
    DeviceDiscovered(TrackerStatus),
    DeviceConnected(Uuid),
    DeviceDisconnected(Uuid),
    DeviceUpdated(TrackerStatus),
    BatteryUpdated(Uuid, BatteryInfo),
    TrackingData(Uuid, MocopiPacket),
    TrackingPosition(Uuid, Option<[f32; 3]>),
    AutoPoseChanged(CalibrationPose, f32),
    ActivePoseChanged(CalibrationPose),
    ScanCompleted(usize),
    Error(String),
    /// SlimeVR-Ziel verbunden: (ip, port, auto_discovered).
    SlimeVRConnected(String, u16, bool),
    SlimeVRDisconnected,
    SlimeVRDiscovered(String, u16),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoreCommand {
    ScanDevices,
    ConnectDevice(Uuid),
    DisconnectDevice(Uuid),
    AssignTracker(Uuid, TrackerRole),
    SetActivePose(CalibrationPose),
    SetAutoPose(bool),
    SetSkeleton(Box<SkeletonConfig>),
    SetPoseDetectorConfig(Box<PoseDetectorConfig>),
    SetMountFlip(Uuid, bool),
    ResendHandshake,
    StartStreaming,
    StopStreaming,
    SetSlimeVRAddress(String, u16),
    SetBleConfig(Box<mocopi_ble_windows::BleConfig>),
    SetLanguage(String),
    SetConfig(Box<AppConfig>),
    Shutdown,
}

impl TrackerManager {
    pub fn new(
        ble_manager: Arc<BleManager>,
        config: AppConfig,
    ) -> (
        Self,
        broadcast::Receiver<CoreEvent>,
        mpsc::Sender<CoreCommand>,
    ) {
        Self::new_with_ble(ble_manager, config)
    }

    /// Konstruktor mit injizierbarem BLE-Manager (Memory-Backend für Tests).
    pub fn new_with_ble(
        ble_manager: Arc<BleManager>,
        config: AppConfig,
    ) -> (
        Self,
        broadcast::Receiver<CoreEvent>,
        mpsc::Sender<CoreCommand>,
    ) {
        let (event_tx, event_rx) = broadcast::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel(256);

        let slimevr = SlimevrOutput::new(config.slimevr.enabled, config.slimevr.packet_rate);
        let router = Arc::new(OutputRouter::new(vec![
            slimevr.clone() as Arc<dyn crate::outputs::TrackingOutput>
        ]));
        let initial_pose = config
            .tracking
            .active_pose
            .parse::<CalibrationPose>()
            .unwrap_or(CalibrationPose::Standing);
        let active_pose = Arc::new(RwLock::new(initial_pose));
        let manager = Self {
            ble_manager,
            config: Arc::new(RwLock::new(config.clone())),
            active_pose,
            pose_detector: Arc::new(Mutex::new(PoseDetector::new(
                config.tracking.pose_detector.clone(),
            ))),
            auto_pose_enabled: Arc::new(RwLock::new(config.tracking.enable_auto_pose)),
            auto_pose_confidence: Arc::new(RwLock::new(1.0)),
            latest_orientations: Arc::new(Mutex::new(HashMap::new())),
            smooth_positions: Arc::new(Mutex::new(HashMap::new())),
            motion: Arc::new(Mutex::new(HashMap::new())),
            slimevr_health: Arc::new(Mutex::new((0, 0, 0, false))),
            filters: Arc::new(RwLock::new(HashMap::new())),
            router,
            slimevr,
            running: Arc::new(RwLock::new(false)),
            event_tx,
            command_rx: Arc::new(Mutex::new(Some(cmd_rx))),
            logs: crate::logging::LogBuffer::new(),
            last_gui_emit: Arc::new(Mutex::new(HashMap::new())),
        };

        (manager, event_rx, cmd_tx)
    }

    pub async fn run(&self) -> Result<()> {
        *self.running.write().await = true;

        let packet_rx = self.ble_manager.subscribe_packets();
        let state_rx = self.ble_manager.subscribe_state();

        self.start_packet_processor(packet_rx).await;
        self.start_state_processor(state_rx).await;
        self.start_command_processor().await;
        self.start_slimevr_handler().await;
        self.start_status_poller().await;
        // Auto-Reconnect-Scheduler (war nie gestartet → keine Retries).
        self.ble_manager.start_reconnect_loop().await;

        info!("TrackerManager started");
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        *self.running.write().await = false;
        self.stop_streaming().await?;
        info!("TrackerManager stopped");
        Ok(())
    }

    /// Watchdog gegen hängende Kalibrier-Sessions (keine Samples in 30s,
    /// z.B. Tracker nicht streaming): bricht ab und meldet es der GUI,
    /// damit der Spinner nicht ewig läuft und andere Sessions blockiert.
    /// Watchdog gegen hängende Kalibrier-Sessions (keine Samples in 30s,
    /// z.B. Tracker nicht streaming): bricht ab und meldet es der GUI,
    /// damit der Spinner nicht ewig läuft und andere Sessions blockiert.
    async fn start_packet_processor(&self, mut rx: broadcast::Receiver<(Uuid, DecodedPacket)>) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok((device_id, packet)) => {
                        // Debug, not error: per-packet failures (corrupt frame,
                        // backwards timestamp) are routine at 50-100 Hz per
                        // tracker and must never flood the logs.
                        if let Err(e) = manager.handle_packet(device_id, packet).await {
                            debug!("Packet handling error for {device_id}: {e}");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        // Continue from the newest packet after a temporary
                        // consumer backlog. Exiting here permanently disables
                        // tracking and leaves the UI stuck at “waiting”.
                        warn!("BLE packet consumer lagged; skipped {skipped} stale packet(s)");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    async fn start_state_processor(&self, mut rx: broadcast::Receiver<(Uuid, ConnectionState)>) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok((device_id, state)) => {
                        if let Err(e) = manager.handle_state_change(device_id, state).await {
                            error!("State handling error: {}", e);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("BLE state consumer lagged; skipped {skipped} stale update(s)");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    async fn start_command_processor(&self) {
        let rx = self.command_rx.lock().await.take();
        let Some(mut rx) = rx else {
            error!("Command processor already running; refusing second start");
            return;
        };
        let manager = self.clone();
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                if let Err(e) = manager.handle_command(cmd).await {
                    error!("Command handling error: {}", e);
                    // Surface failures to the GUI: without this, scan /
                    // connect errors are only visible in logs and the UI
                    // just shows an empty list ("findet nichts").
                    manager.emit_event(CoreEvent::Error(e.to_string()));
                }
            }
        });
    }

    async fn start_slimevr_handler(&self) {
        // Echter Health-Monitor: Pakete laufen, Fehler nicht.
        // 2 aufeinanderfolgende 5s-Fenster mit Fehler-Zuwachs bei
        // Stillstand der Paket-Zähler → SlimeVRDisconnected (einmalig bis
        // zur Erholung). So merkt die GUI einen toten Server/Port.
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if !*manager.running.read().await {
                    debug!("SlimeVR health monitor stopping");
                    break;
                }
                let status = manager.slimevr.status().await;
                let mut health = manager.slimevr_health.lock().await;
                let (last_packets, last_errors, streak, reported) = *health;
                if !status.enabled || status.ip.is_none() {
                    *health = (status.packets_sent, status.errors, 0, false);
                    continue;
                }
                let packets_delta = status.packets_sent.saturating_sub(last_packets);
                let errors_delta = status.errors.saturating_sub(last_errors);
                if packets_delta > 0 {
                    *health = (status.packets_sent, status.errors, 0, false);
                } else if errors_delta > 0 {
                    let next_streak = streak.saturating_add(1);
                    if next_streak >= 2 && !reported {
                        warn!(
                            "SlimeVR target unhealthy ({} errors, no packets) – reporting disconnect",
                            status.errors
                        );
                        manager.emit_event(CoreEvent::SlimeVRDisconnected);
                        *health = (status.packets_sent, status.errors, next_streak, true);
                    } else {
                        *health = (status.packets_sent, status.errors, next_streak, reported);
                    }
                } else {
                    // Idle (kein Streaming): Baseline nachziehen, kein Alarm.
                    *health = (status.packets_sent, status.errors, 0, reported);
                }
            }
        });
    }

    async fn start_status_poller(&self) {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if !*manager.running.read().await {
                    break;
                }
                if let Err(e) = manager.poll_all_battery().await {
                    warn!("Battery poll error: {}", e);
                }
            }
        });
    }

    async fn handle_packet(&self, device_id: Uuid, packet: DecodedPacket) -> Result<()> {
        match packet {
            DecodedPacket::Imu(imu_packet) => {
                self.process_imu_packet(device_id, imu_packet).await?;
            }
            DecodedPacket::Battery(battery) => {
                self.handle_battery_update(device_id, battery).await?;
            }
            DecodedPacket::Status(_) => {
                debug!("Status response from {}", device_id);
            }
            DecodedPacket::Unknown { packet_type, data } => {
                debug!(
                    "Unknown packet type 0x{:02x} from {}: {} bytes",
                    packet_type,
                    device_id,
                    data.len()
                );
            }
        }
        Ok(())
    }

    async fn process_imu_packet(&self, device_id: Uuid, packet: MocopiPacket) -> Result<()> {
        let role = self.get_tracker_role(device_id).await;

        // T3 Mount-Flip: vor Kalibrierung/Filter korrigieren.
        let packet = if self.is_mount_flip(device_id).await {
            let flip = tracking_core::mount_flip_correction();
            MocopiPacket {
                quaternion: flip * packet.quaternion,
                acceleration: flip * packet.acceleration,
                ..packet
            }
        } else {
            packet
        };

        // Koordinatensystem aus der Config (T3-Fix: vorher nie angewendet).
        // Reihenfolge: Flip → Kalibrierung (Sensor-Frame) → Konversion → Filter.
        let target_system = {
            let cfg = self.config.read().await;
            tracking_core::CoordinateSystem::from_name(&cfg.tracking.coordinate_system)
        };

        // SlimeVR owns body calibration and tracker alignment. Forward the
        // tracker orientation after only hardware mount and coordinate-system
        // conversion; local pose offsets would be applied twice by SlimeVR.
        let input_pose = if let Some(role) = role {
            tracking_core::TrackerPose::new(
                role,
                packet.quaternion,
                packet.acceleration,
                packet.timestamp_ns,
            )
        } else {
            tracking_core::TrackerPose::new(
                tracking_core::TrackerRole::Waist,
                packet.quaternion,
                packet.acceleration,
                packet.timestamp_ns,
            )
        };
        let converted = input_pose.to_coordinate_system(target_system);
        let converted_packet = MocopiPacket {
            quaternion: converted.rotation,
            acceleration: converted.acceleration,
            ..packet
        };
        let filtered_packet = self.apply_filter(device_id, converted_packet).await?;

        // Auto-Pose: kalibrierte Orientierung pro Rolle cachen und Detektor füttern.
        if let Some(role) = role {
            let switched = {
                let mut latest = self.latest_orientations.lock().await;
                latest.insert(role, filtered_packet.quaternion);
                if *self.auto_pose_enabled.read().await {
                    let snapshot = latest.clone();
                    drop(latest);
                    let mut detector = self.pose_detector.lock().await;
                    detector.update(&snapshot)
                } else {
                    None
                }
            };
            if let Some((pose, conf)) = switched {
                *self.active_pose.write().await = pose;
                // Position smoothing must not blend coordinates from two
                // different skeleton poses (e.g. standing into lying).
                self.smooth_positions.lock().await.clear();
                *self.auto_pose_confidence.write().await = conf;
                {
                    let mut cfg = self.config.write().await;
                    cfg.tracking.active_pose = pose.name().to_string();
                }
                self.emit_event(CoreEvent::AutoPoseChanged(pose, conf));
                self.emit_event(CoreEvent::ActivePoseChanged(pose));
                info!("Auto-pose switched to {pose} (conf {conf:.2})");
            }
        }

        // Estimated tracker position for the UI; IMU position is only an estimate.
        // T5: Tiefpass (Alpha 0.3) gegen FK-Jitter + Boden-Klemmung (Y>=0),
        // damit Füße in VRC nicht unter den Boden rutschen.
        let position = match role {
            Some(r) => {
                let snapshot = self.latest_orientations.lock().await.clone();
                let skeleton = self.config.read().await.tracking.skeleton.clone();
                let pose = *self.active_pose.read().await;
                let estimated = skeleton.estimate_positions(&snapshot, pose);
                let raw = estimated.get(&r).cloned();
                match raw {
                    Some(p) => {
                        let mut cache = self.smooth_positions.lock().await;
                        let smoothed = match cache.get(&r) {
                            Some(last) => *last + (p - *last) * 0.3,
                            None => p,
                        };
                        cache.insert(r, smoothed);
                        Some(smoothed)
                    }
                    None => None,
                }
            }
            None => None,
        };

        // Throttle GUI events to ~4 Hz per tracker; SlimeVR output below
        // always uses the full-rate filtered packet.
        let gui_due = {
            let mut guard = self.last_gui_emit.lock().await;
            let now = std::time::Instant::now();
            match guard.get(&device_id) {
                Some(last) if now.duration_since(*last) < Duration::from_millis(250) => false,
                _ => {
                    guard.insert(device_id, now);
                    true
                }
            }
        };
        if gui_due {
            self.emit_event(CoreEvent::TrackingData(device_id, filtered_packet));
            self.emit_event(CoreEvent::TrackingPosition(
                device_id,
                position.map(|p| [p.x, p.y, p.z]),
            ));
        }
        // Count only packets which made it through the tracking pipeline.
        // BLE/GATT being connected is not enough to mark a tracker Streaming.
        self.ble_manager
            .record_tracking_packet(device_id, filtered_packet.timestamp_ns)
            .await;

        let (sensor_id, assigned) = match role {
            Some(r) => (r.slimevr_sensor_id(), true),
            None => (0, false),
        };
        // T4-Prädiktion (~8ms Horizont, max 15ms): nur für Streaming-Output,
        // GUI + Kalibrierung nutzen das gefilterte Signal.
        let output_quat = {
            let mut motion = self.motion.lock().await;
            let (prev, prev_ts) = motion
                .get(&device_id)
                .cloned()
                .unwrap_or((filtered_packet.quaternion, filtered_packet.timestamp_ns));
            let dt = filtered_packet.timestamp_ns.saturating_sub(prev_ts);
            let predicted =
                tracking_core::predict_orientation(prev, filtered_packet.quaternion, dt, 8_000_000);
            motion.insert(
                device_id,
                (filtered_packet.quaternion, filtered_packet.timestamp_ns),
            );
            predicted
        };
        self.router
            .send_frame(&OutputFrame {
                device_id,
                sensor_id,
                assigned,
                quaternion: output_quat,
                acceleration: filtered_packet.acceleration,
                counter: filtered_packet.counter,
            })
            .await;

        Ok(())
    }

    async fn is_mount_flip(&self, device_id: Uuid) -> bool {
        let mac = match self.ble_manager.get_device(&device_id).await {
            Some(d) => d.info.bluetooth_address.clone(),
            None => return false,
        };
        self.config
            .read()
            .await
            .trackers
            .get(&mac)
            .map(|t| t.mount_flip)
            .unwrap_or(false)
    }

    pub async fn set_mount_flip(&self, device_id: Uuid, flip: bool) -> Result<()> {
        let mac = match self.ble_manager.get_device(&device_id).await {
            Some(d) => d.info.bluetooth_address.clone(),
            None => {
                return Err(CoreError::TrackerNotFound {
                    id: device_id.to_string(),
                })
            }
        };
        {
            let mut cfg = self.config.write().await;
            let entry = cfg
                .trackers
                .entry(mac.clone())
                .or_insert_with(|| TrackerConfig {
                    mac_address: mac.clone(),
                    assigned_role: None,
                    mount_flip: false,
                });
            entry.mount_flip = flip;
        }
        self.persist_config().await;
        Ok(())
    }

    fn status_str(state: ConnectionState) -> &'static str {
        match state {
            ConnectionState::Disconnected | ConnectionState::Discovered => "disconnected",
            ConnectionState::Discovering
            | ConnectionState::Connecting
            | ConnectionState::Initializing => "connecting",
            ConnectionState::Streaming => "streaming",
            ConnectionState::Reconnecting => "reconnecting",
            ConnectionState::Error => "error",
        }
    }

    async fn apply_filter(&self, device_id: Uuid, packet: MocopiPacket) -> Result<MocopiPacket> {
        use std::collections::hash_map::Entry;
        // GUI-Slider sind die Quelle (T1-Fix: vorher immer Default).
        let (enabled, base_slerp, base_alpha, role, pose) = {
            let cfg = self.config.read().await;
            (
                cfg.tracking.enable_filtering,
                cfg.tracking.filter_slerp_factor,
                cfg.tracking.filter_accel_alpha,
                self.get_tracker_role(device_id).await,
                *self.active_pose.read().await,
            )
        };
        if !enabled {
            return Ok(packet);
        }
        // Lower slerp alpha damps IMU jitter; fast motion remains adaptive.
        let (role_slerp, role_alpha) = Self::filter_scales(role, pose);
        let wanted = FilterConfig {
            enable_quaternion_slerp: true,
            slerp_factor: (base_slerp * role_slerp).clamp(0.02, 0.6),
            enable_accel_lowpass: true,
            accel_lowpass_alpha: (base_alpha * role_alpha).clamp(0.05, 0.9),
            max_timestamp_jump_ms: 100,
            min_packet_interval_ms: 5,
            adaptive: true,
            spike_max_deg: 35.0,
            compensate_gravity: true,
        };
        let mut filters = self.filters.write().await;
        let filter = match filters.entry(device_id) {
            Entry::Occupied(entry) => {
                let f = entry.into_mut();
                // Laufend nachziehen (Slider-/Pose-Wechsel ohne Reconnect).
                let _ = f.set_config(wanted);
                f
            }
            Entry::Vacant(entry) => match TrackingFilter::new(wanted) {
                Ok(filter) => entry.insert(filter),
                Err(e) => {
                    warn!("Filter config invalid ({e}); forwarding unfiltered");
                    return Ok(packet);
                }
            },
        };

        let (filtered_quat, filtered_accel) =
            filter.process(packet.quaternion, packet.acceleration, packet.timestamp_ns)?;

        Ok(MocopiPacket {
            quaternion: filtered_quat,
            acceleration: filtered_accel,
            counter: packet.counter,
            timestamp_ns: packet.timestamp_ns,
        })
    }

    /// Skalierung je Rolle/Pose (T1): >1 = ruhiger, <1 = agiler.
    fn filter_scales(role: Option<TrackerRole>, pose: CalibrationPose) -> (f32, f32) {
        let base = match role {
            Some(TrackerRole::Waist) | Some(TrackerRole::Chest) => (0.8, 0.85),
            Some(TrackerRole::LeftUpperLeg)
            | Some(TrackerRole::RightUpperLeg)
            | Some(TrackerRole::LeftLowerLeg)
            | Some(TrackerRole::RightLowerLeg)
            | Some(TrackerRole::LeftFoot)
            | Some(TrackerRole::RightFoot) => (0.85, 0.9),
            Some(TrackerRole::Head) => (0.6, 0.75),
            Some(_) => (0.65, 0.8),
            None => (1.0, 1.0),
        };
        let pose_scale = match pose {
            CalibrationPose::Standing => 1.0,
            CalibrationPose::Sitting => 0.85,
            CalibrationPose::Lying => 0.8,
        };
        (base.0 * pose_scale, base.1)
    }

    async fn handle_battery_update(&self, device_id: Uuid, battery: BatteryInfo) -> Result<()> {
        if let Some(device) = self.ble_manager.get_device(&device_id).await {
            device.update_battery(battery).await;
        }

        self.emit_event(CoreEvent::BatteryUpdated(device_id, battery));

        let role = self.get_tracker_role(device_id).await;
        let (sensor_id, assigned) = match role {
            Some(r) => (r.slimevr_sensor_id(), true),
            None => (0, false),
        };
        self.router
            .send_battery(&BatteryEvent {
                device_id,
                sensor_id,
                assigned,
                battery,
            })
            .await;

        Ok(())
    }

    /// Handshake für ein Gerät (neu verbinden lassen am Server).
    async fn send_handshake_for(&self, device_id: Uuid) {
        if let Some(device) = self.ble_manager.get_device(&device_id).await {
            let fw = device
                .get_firmware_version()
                .await
                .unwrap_or_else(|| "Unknown".to_string());
            let sensor_id = device
                .get_assigned_role()
                .await
                .map(|r| r.slimevr_sensor_id())
                .unwrap_or(0);
            debug!(%device_id, fw = %fw, "sending handshake via outputs");
            self.router
                .send_handshake(&HandshakeCtx {
                    device_id,
                    mac: device.info.bluetooth_address.clone(),
                    firmware: fw,
                    sensor_id,
                })
                .await;
        }
    }

    /// Handshakes für alle gerade streamenden Tracker (z.B. nach Start oder
    /// Server-Neustart – der Server legt Sensoren nur per Handshake an).
    /// Gibt die Anzahl zurück.
    async fn resend_handshakes(&self) -> usize {
        let devices = self.ble_manager.get_all_devices().await;
        let mut n = 0;
        for device in devices {
            if device.state.is_connected() {
                self.send_handshake_for(device.device_id).await;
                n += 1;
            }
        }
        if n > 0 {
            info!("Resent {n} SlimeVR handshake(s)");
        }
        n
    }

    async fn handle_state_change(&self, device_id: Uuid, state: ConnectionState) -> Result<()> {
        match state {
            ConnectionState::Streaming => {
                self.emit_event(CoreEvent::DeviceConnected(device_id));
                self.send_handshake_for(device_id).await;
            }
            ConnectionState::Disconnected => {
                self.emit_event(CoreEvent::DeviceDisconnected(device_id));
            }
            _ => {}
        }

        if let Some(status) = self.ble_manager.get_device_status(&device_id).await {
            let (sensor_id, assigned) = match status.assigned_role {
                Some(r) => (r.slimevr_sensor_id(), true),
                None => (0, false),
            };
            self.router
                .send_status(&StatusEvent {
                    device_id,
                    sensor_id,
                    assigned,
                    status: Self::status_str(state),
                    battery: status.battery,
                    rssi: Some(status.rssi),
                })
                .await;
            self.emit_event(CoreEvent::DeviceUpdated(status));
        }

        Ok(())
    }

    async fn handle_command(&self, cmd: CoreCommand) -> Result<()> {
        match cmd {
            CoreCommand::ScanDevices => {
                self.scan_devices().await?;
            }
            CoreCommand::ConnectDevice(id) => {
                self.connect_device(id).await?;
            }
            CoreCommand::DisconnectDevice(id) => {
                self.disconnect_device(id).await?;
            }
            CoreCommand::AssignTracker(id, role) => {
                self.assign_tracker(id, role).await?;
            }
            CoreCommand::SetActivePose(pose) => {
                self.set_active_pose(pose).await;
            }
            CoreCommand::SetAutoPose(enabled) => {
                self.set_auto_pose(enabled).await;
            }
            CoreCommand::SetSkeleton(skeleton) => {
                self.set_skeleton(*skeleton).await;
            }
            CoreCommand::SetPoseDetectorConfig(cfg) => {
                self.pose_detector.lock().await.set_config(*cfg.clone());
                {
                    let mut config = self.config.write().await;
                    config.tracking.pose_detector = *cfg;
                }
                self.persist_config().await;
            }
            CoreCommand::SetMountFlip(id, flip) => {
                self.set_mount_flip(id, flip).await?;
            }
            CoreCommand::ResendHandshake => {
                self.resend_handshakes().await;
            }
            CoreCommand::StartStreaming => {
                self.start_streaming().await?;
            }
            CoreCommand::StopStreaming => {
                self.stop_streaming().await?;
            }
            CoreCommand::SetSlimeVRAddress(ip, port) => {
                self.set_slimevr_address(ip, port, false).await?;
            }
            CoreCommand::SetBleConfig(config) => {
                let ble_config = (*config).clone();
                let _ = self.ble_manager.set_config(ble_config.clone()).await;
                {
                    let mut cfg = self.config.write().await;
                    cfg.bluetooth.auto_scan = ble_config.auto_scan;
                    cfg.bluetooth.auto_connect = ble_config.auto_connect;
                    cfg.bluetooth.reconnect_delay_ms = ble_config.reconnect_delay_ms;
                    cfg.bluetooth.connection_timeout_ms = ble_config.connection_timeout_ms;
                    cfg.bluetooth.max_trackers = ble_config.max_trackers;
                    cfg.bluetooth.scan_duration_ms = ble_config.scan_duration_ms;
                    cfg.bluetooth.rssi_min_dbm = ble_config.rssi_min_dbm;
                }
                self.persist_config().await;
            }
            CoreCommand::SetConfig(config) => {
                self.update_config(*config).await?;
            }
            CoreCommand::SetLanguage(language) => {
                self.config.write().await.general.language = language;
                self.persist_config().await;
            }
            CoreCommand::Shutdown => {
                self.shutdown().await?;
            }
        }
        Ok(())
    }

    async fn scan_devices(&self) -> Result<()> {
        tracing::info!("TrackerManager: Starting scan");
        let devices = self
            .ble_manager
            .start_scan()
            .await
            .map_err(|e| CoreError::BleError(e.to_string()))?;

        tracing::info!("TrackerManager: Scan found {} devices", devices.len());
        for device in &devices {
            tracing::info!(
                "  Found: {} ({}) RSSI: {}",
                device.name,
                device.bluetooth_address,
                device.rssi
            );
        }

        let count = devices.len();
        // Persistierte Rollen auf wiedergefundene Geräte anwenden, damit
        // nach Neustart kein manuelles Re-Assign nötig ist.
        let restored: Vec<(String, TrackerRole)> = {
            let cfg = self.config.read().await;
            devices
                .iter()
                .filter_map(|d| {
                    cfg.trackers
                        .get(&d.bluetooth_address)
                        .and_then(|t| t.assigned_role)
                        .map(|role| (d.bluetooth_address.clone(), role))
                })
                .collect()
        };
        for (addr, role) in restored {
            if self
                .ble_manager
                .apply_role_by_address(&addr, role)
                .await
                .is_some()
            {
                debug!("Restored role {role} for {addr}");
            }
        }
        for device in devices {
            // Status nach Restore lesen (mit Rolle statt None).
            let status = self
                .ble_manager
                .get_device_status(&device.id)
                .await
                .unwrap_or(TrackerStatus {
                    device_id: device.id,
                    bluetooth_address: device.bluetooth_address,
                    name: device.name,
                    state: ConnectionState::Discovered,
                    assigned_role: None,
                    battery: None,
                    rssi: device.rssi,
                    firmware_version: None,
                    packet_count: 0,
                    last_packet_ns: None,
                    reconnect_attempts: 0,
                    error: None,
                    reconnect_in_ms: None,
                });
            self.emit_event(CoreEvent::DeviceDiscovered(status));
        }
        // Explicit completion signal so the GUI stops "Scanning..."
        // even if some DeviceDiscovered events were dropped.
        self.emit_event(CoreEvent::ScanCompleted(count));
        Ok(())
    }

    async fn connect_device(&self, device_id: Uuid) -> Result<()> {
        let status = self.ble_manager.get_device_status(&device_id).await.ok_or(
            CoreError::TrackerNotFound {
                id: device_id.to_string(),
            },
        )?;
        let connected_id = self
            .ble_manager
            .connect_device(&status.bluetooth_address)
            .await
            .map_err(|e| CoreError::BleError(e.to_string()))?;
        if connected_id != device_id {
            return Err(CoreError::TrackerNotFound {
                id: format!(
                    "address {} resolved to {connected_id}, expected {device_id}",
                    status.bluetooth_address
                ),
            });
        }
        Ok(())
    }

    async fn disconnect_device(&self, device_id: Uuid) -> Result<()> {
        self.ble_manager
            .disconnect_device(&device_id)
            .await
            .map_err(|e| CoreError::BleError(e.to_string()))?;
        Ok(())
    }

    async fn assign_tracker(&self, device_id: Uuid, role: TrackerRole) -> Result<()> {
        let device =
            self.ble_manager
                .get_device(&device_id)
                .await
                .ok_or(CoreError::TrackerNotFound {
                    id: device_id.to_string(),
                })?;

        device.set_assigned_role(Some(role)).await;

        {
            let mut config = self.config.write().await;
            config
                .trackers
                .entry(device.info.bluetooth_address.clone())
                .and_modify(|tracker_config| {
                    tracker_config.assigned_role = Some(role);
                })
                .or_insert_with(|| TrackerConfig {
                    mac_address: device.info.bluetooth_address.clone(),
                    assigned_role: Some(role),
                    mount_flip: false,
                });
        }

        if let Some(status) = self.ble_manager.get_device_status(&device_id).await {
            self.emit_event(CoreEvent::DeviceUpdated(status));
        }

        self.persist_config().await;
        Ok(())
    }
    /// Alle verbundenen + zugewiesenen Tracker für die aktive Pose.
    /// Füllt die Warteschlange und startet die erste Session; der Rest
    /// Importierte Kalibrierungs-Profile anwenden: in-memory Manager plus
    /// persistente Offsets aller Tracker mit passender Rolle.
    async fn set_active_pose(&self, pose: CalibrationPose) {
        *self.active_pose.write().await = pose;
        // Manueller Eingriff pausiert Auto für ~30s (bei ~50Hz ≈ 1500 Frames).
        self.pose_detector.lock().await.set_manual_hold(1500);
        {
            let mut cfg = self.config.write().await;
            cfg.tracking.active_pose = pose.name().to_string();
        }
        self.emit_event(CoreEvent::ActivePoseChanged(pose));
        self.persist_config().await;
    }
    async fn set_auto_pose(&self, enabled: bool) {
        *self.auto_pose_enabled.write().await = enabled;
        {
            let mut cfg = self.config.write().await;
            cfg.tracking.enable_auto_pose = enabled;
        }
        self.persist_config().await;
        info!("Auto-pose {}", if enabled { "enabled" } else { "disabled" });
    }

    async fn set_skeleton(&self, skeleton: SkeletonConfig) {
        if skeleton.validate().is_err() {
            warn!("Rejected invalid skeleton config");
            return;
        }
        {
            let mut cfg = self.config.write().await;
            cfg.tracking.skeleton = skeleton;
        }
        self.persist_config().await;
    }

    /// Offset nach erfolgreicher Kalibrierung in Config spiegeln (pro Pose).
    async fn start_streaming(&self) -> Result<()> {
        // SlimeVR autodiscovery: if enabled and no explicit address has been
        // set yet, listen briefly for the server broadcast ("Hey OVR =D").
        // This mirrors the Python `find_slime()` behaviour without blocking
        // the command loop: failures fall back to the configured address.
        let (slime_enabled, slime_ip, slime_port, auto_connect) = {
            let cfg = self.config.read().await;
            (
                cfg.slimevr.enabled,
                cfg.slimevr.server_ip.clone(),
                cfg.slimevr.server_port,
                cfg.bluetooth.auto_connect,
            )
        };
        if slime_enabled {
            let slime_result = if self.config.read().await.slimevr.auto_discover
                && !self.slimevr.has_target().await
            {
                match self.discover_slimevr().await {
                    Some((ip, port)) => {
                        self.emit_event(CoreEvent::SlimeVRDiscovered(ip.clone(), port));
                        self.connect_slimevr(&ip, port, true).await
                    }
                    None => self.connect_slimevr(&slime_ip, slime_port, false).await,
                }
            } else {
                self.connect_slimevr(&slime_ip, slime_port, false).await
            };
            if let Err(error) = slime_result {
                return Err(error);
            }
        }
        let devices = self.ble_manager.get_all_devices().await;
        // Gestaffelt verbinden (2s Abstand, global serialisiert): schont das
        // BLE-Radio bei vielen Trackern; der Guard in connect_device
        // verhindert zusätzlich Doppelversuche mit dem Reconnect-Ticker.
        let mut first = true;
        for device in devices {
            if device.state == ConnectionState::Disconnected && auto_connect {
                if !first {
                    tokio::time::sleep(Duration::from_millis(2000)).await;
                }
                first = false;
                match self.connect_device(device.device_id).await {
                    Ok(_) => info!("Auto-connect ok for {}", device.bluetooth_address),
                    Err(e) => warn!("Auto-connect failed for {}: {e}", device.bluetooth_address),
                }
            }
        }
        // Bereits streamende Tracker beim Server (neu) anmelden: ohne
        // Handshake legt SlimeVR keine Sensoren an.
        self.resend_handshakes().await;
        Ok(())
    }

    /// Hört auf SlimeVR-Broadcasts ("Hey OVR =D"). Bindet bevorzugt :6969,
    /// fällt bei belegtem Port (z.B. lokaler Server) auf Ephemeral zurück
    /// und gibt dann `None` (Caller nutzt Config) statt zu crashen.
    /// Timeout ~3s (3x1s) für schnellen Start.
    async fn discover_slimevr(&self) -> Option<(String, u16)> {
        use tokio::time::timeout;
        let sock = match tokio::net::UdpSocket::bind("0.0.0.0:6969").await {
            Ok(s) => s,
            Err(e) => {
                debug!("SlimeVR discovery: :6969 busy ({e}), skip auto");
                return None;
            }
        };
        let mut buf = [0u8; 1024];
        for _ in 0..3 {
            let recv = timeout(Duration::from_secs(1), sock.recv_from(&mut buf)).await;
            if let Ok(Ok((len, src))) = recv {
                if buf[..len].windows(8).any(|w| w == b"Hey OVR ") {
                    info!("SlimeVR autodiscovered at {src}");
                    return Some((src.ip().to_string(), src.port()));
                }
            }
        }
        None
    }

    async fn stop_streaming(&self) -> Result<()> {
        let devices = self.ble_manager.get_all_devices().await;
        for device in devices {
            if device.state.is_connected() {
                // One failing tracker must not abort the shutdown of the rest.
                if let Err(e) = self.disconnect_device(device.device_id).await {
                    warn!(
                        "Disconnect failed for {} during stop: {e}",
                        device.bluetooth_address
                    );
                }
            }
        }

        self.slimevr.clear_target().await;
        self.emit_event(CoreEvent::SlimeVRDisconnected);
        Ok(())
    }

    async fn set_slimevr_address(&self, ip: String, port: u16, auto: bool) -> Result<()> {
        // Validate early for a clean ConfigurationError instead of a UDP error.
        let _: std::net::SocketAddr =
            format!("{ip}:{port}")
                .parse()
                .map_err(
                    |e: std::net::AddrParseError| CoreError::ConfigurationError {
                        reason: e.to_string(),
                    },
                )?;
        {
            let mut cfg = self.config.write().await;
            cfg.slimevr.server_ip = ip.clone();
            cfg.slimevr.server_port = port;
            cfg.slimevr.auto_discover = auto;
        }
        self.persist_config().await;
        self.slimevr
            .set_target(&ip, port, auto)
            .await
            .map_err(|e| match e {
                CoreError::ConfigurationError { .. } => e,
                other => CoreError::SlimeVRConnectionFailed {
                    reason: other.to_string(),
                },
            })?;
        self.emit_event(CoreEvent::SlimeVRConnected(ip, port, auto));
        Ok(())
    }

    async fn connect_slimevr(&self, ip: &str, port: u16, auto: bool) -> Result<()> {
        self.set_slimevr_address(ip.to_string(), port, auto).await
    }
    /// Best-effort disk persistence for every settings mutation (roles,
    /// sliders, log level). Never fails the caller: a broken config
    /// dir must not break tracking.
    async fn persist_config(&self) {
        let snapshot = self.config.read().await.clone();
        let path = configuration::AppConfig::default_config_path();
        if let Err(e) = snapshot.save(&path) {
            warn!("config save failed ({}): {e}", path.display());
        }
    }
    pub async fn get_slimevr_status(&self) -> SlimevrStatus {
        self.slimevr.status().await
    }

    pub async fn is_scanning(&self) -> bool {
        self.ble_manager.is_scanning().await
    }

    pub async fn ble_config(&self) -> mocopi_ble_windows::BleConfig {
        self.ble_manager.get_config().await
    }

    /// Alle Kalibrierungs-Profile (für Diagnostics/Backup).
    async fn update_config(&self, config: AppConfig) -> Result<()> {
        let previous_slimevr = self.config.read().await.slimevr.clone();
        self.slimevr
            .set_config(config.slimevr.enabled, config.slimevr.packet_rate)
            .await;
        let slimevr_target_changed = previous_slimevr.server_ip != config.slimevr.server_ip
            || previous_slimevr.server_port != config.slimevr.server_port
            || previous_slimevr.auto_discover != config.slimevr.auto_discover;
        if config.slimevr.enabled && slimevr_target_changed {
            if let Err(e) = self
                .slimevr
                .set_target(
                    &config.slimevr.server_ip,
                    config.slimevr.server_port,
                    config.slimevr.auto_discover,
                )
                .await
            {
                warn!("SlimeVR target update failed: {e}");
            } else {
                self.emit_event(CoreEvent::SlimeVRConnected(
                    config.slimevr.server_ip.clone(),
                    config.slimevr.server_port,
                    config.slimevr.auto_discover,
                ));
            }
        }
        // Bluetooth settings (scan duration, timeouts, auto-connect)
        // must reach the BLE layer, otherwise the GUI edits them
        // without any effect on scanning.
        self.ble_manager
            .set_config(BleConfig {
                auto_scan: config.bluetooth.auto_scan,
                auto_connect: config.bluetooth.auto_connect,
                reconnect_delay_ms: config.bluetooth.reconnect_delay_ms,
                connection_timeout_ms: config.bluetooth.connection_timeout_ms,
                max_trackers: config.bluetooth.max_trackers,
                scan_duration_ms: config.bluetooth.scan_duration_ms,
                rssi_min_dbm: config.bluetooth.rssi_min_dbm,
            })
            .await;
        // Pose/Skeleton in die laufenden Komponenten übernehmen.
        if let Ok(pose) = config.tracking.active_pose.parse::<CalibrationPose>() {
            *self.active_pose.write().await = pose;
        }
        *self.auto_pose_enabled.write().await = config.tracking.enable_auto_pose;
        self.pose_detector
            .lock()
            .await
            .set_config(config.tracking.pose_detector.clone());
        *self.config.write().await = config;
        self.persist_config().await;
        Ok(())
    }

    async fn get_tracker_role(&self, device_id: Uuid) -> Option<TrackerRole> {
        if let Some(device) = self.ble_manager.get_device(&device_id).await {
            device.get_assigned_role().await
        } else {
            None
        }
    }

    async fn poll_all_battery(&self) -> Result<()> {
        let devices = self.ble_manager.get_all_devices().await;
        for device in devices {
            if device.state.is_connected() {
                // Best effort: a single failed poll must not fail the round.
                if let Err(e) = self.ble_manager.request_battery(&device.device_id).await {
                    debug!("Battery poll failed for {}: {e}", device.bluetooth_address);
                }
            }
        }
        Ok(())
    }

    fn emit_event(&self, event: CoreEvent) {
        let _ = self.event_tx.send(event);
    }

    pub async fn get_all_trackers(&self) -> Vec<TrackerStatus> {
        self.ble_manager.get_all_devices().await
    }

    pub async fn get_tracker_status(&self, device_id: &Uuid) -> Option<TrackerStatus> {
        self.ble_manager.get_device_status(device_id).await
    }

    pub async fn get_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    pub async fn active_pose(&self) -> CalibrationPose {
        *self.active_pose.read().await
    }

    pub async fn auto_pose_enabled(&self) -> bool {
        *self.auto_pose_enabled.read().await
    }

    pub async fn auto_pose_confidence(&self) -> f32 {
        *self.auto_pose_confidence.read().await
    }

    /// Pose-Status für die GUI: pro Rolle je Pose (calibrated, at_ns, variance).
    /// Public config replace (used by FFI `set_log_level` persistence).
    pub async fn update_config_public(&self, config: AppConfig) -> Result<()> {
        self.update_config(config).await
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<CoreEvent> {
        self.event_tx.subscribe()
    }

    pub fn logs(&self) -> crate::logging::LogBuffer {
        self.logs.clone()
    }
}

impl Clone for TrackerManager {
    fn clone(&self) -> Self {
        Self {
            ble_manager: self.ble_manager.clone(),
            config: self.config.clone(),
            active_pose: self.active_pose.clone(),
            pose_detector: self.pose_detector.clone(),
            auto_pose_enabled: self.auto_pose_enabled.clone(),
            auto_pose_confidence: self.auto_pose_confidence.clone(),
            latest_orientations: self.latest_orientations.clone(),
            smooth_positions: self.smooth_positions.clone(),
            motion: self.motion.clone(),
            slimevr_health: self.slimevr_health.clone(),
            filters: self.filters.clone(),
            router: self.router.clone(),
            slimevr: self.slimevr.clone(),
            running: self.running.clone(),
            event_tx: self.event_tx.clone(),
            command_rx: self.command_rx.clone(),
            logs: self.logs.clone(),
            last_gui_emit: self.last_gui_emit.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mocopi_ble_windows::BleManager;

    #[tokio::test]
    async fn test_tracker_manager_new() {
        let ble = Arc::new(BleManager::new());
        let config = AppConfig::default();
        let (manager, _events, _cmds) = TrackerManager::new(ble, config);
        assert!(!*manager.running.read().await);
    }

    #[tokio::test]
    async fn test_slimevr_status_initially_empty() {
        let ble = Arc::new(BleManager::new());
        let (manager, _events, _cmds) = TrackerManager::new(ble, AppConfig::default());
        let status = manager.get_slimevr_status().await;
        assert!(status.enabled);
        assert!(status.ip.is_none());
        assert!(!status.auto_discovered);
    }

    #[tokio::test]
    async fn test_slimevr_target_roundtrip() {
        let ble = Arc::new(BleManager::new());
        let (manager, _events, _cmds) = TrackerManager::new(ble, AppConfig::default());
        manager
            .set_slimevr_address("127.0.0.1".to_string(), 6969, false)
            .await
            .unwrap();
        let status = manager.get_slimevr_status().await;
        assert_eq!(status.ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(status.port, Some(6969));
        assert!(!status.auto_discovered);
    }
}
