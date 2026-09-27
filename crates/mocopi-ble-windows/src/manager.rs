use crate::backend::{BackendConnection, BleBackend, MemoryBackend, WinRtBackend};
use crate::device::{ConnectionState, DeviceInfo, TrackerDevice, TrackerStatus};
use crate::error::{BleError, Result};
use crate::winrt;
use mocopi_protocol::{constants::*, DecodedPacket};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::time::interval;
use tracing::{info, warn};
use uuid::Uuid;

pub struct BleManager {
    devices: Arc<RwLock<HashMap<Uuid, Arc<TrackerDevice>>>>,
    address_to_id: Arc<RwLock<HashMap<String, Uuid>>>,
    connections: Arc<Mutex<HashMap<Uuid, BackendConnection>>>,
    backend: Arc<dyn BleBackend>,
    packet_tx: broadcast::Sender<(Uuid, DecodedPacket)>,
    state_tx: broadcast::Sender<(Uuid, ConnectionState)>,
    scan_active: Arc<Mutex<bool>>,
    config: Arc<Mutex<BleConfig>>,
    /// Genau ein GATT-Bring-up gleichzeitig (Auto-Connect + Scheduler +
    /// manuell teilen sich das Radio; parallel kollidieren sie).
    connect_lock: Arc<Mutex<()>>,
}

fn default_auto_scan() -> bool {
    true
}

fn default_rssi_min() -> i16 {
    -85
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleConfig {
    #[serde(default = "default_auto_scan")]
    pub auto_scan: bool,
    pub auto_connect: bool,
    pub reconnect_delay_ms: u64,
    pub connection_timeout_ms: u64,
    pub max_trackers: usize,
    pub scan_duration_ms: u64,
    /// Schwächste noch angezeigte Signalstärke (dBm). `0` = unbekannt
    /// (WinRT liefert nicht immer RSSI) passiert immer den Filter.
    #[serde(default = "default_rssi_min")]
    pub rssi_min_dbm: i16,
}

impl Default for BleConfig {
    fn default() -> Self {
        Self {
            auto_scan: true,
            auto_connect: true,
            reconnect_delay_ms: 5000,
            connection_timeout_ms: 15000,
            max_trackers: 12,
            scan_duration_ms: 10000,
            rssi_min_dbm: -85,
        }
    }
}

impl BleConfig {
    /// Schwache Geräte (bekanntes RSSI unter Minimum) ausblenden?
    pub fn passes_rssi(&self, rssi: i16) -> bool {
        rssi == 0 || rssi >= self.rssi_min_dbm
    }
}

impl BleManager {
    pub fn new() -> Self {
        let (packet_tx, _) = broadcast::channel(256);
        let (state_tx, _) = broadcast::channel(256);
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            address_to_id: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
            backend: Arc::new(WinRtBackend),
            packet_tx,
            state_tx,
            scan_active: Arc::new(Mutex::new(false)),
            config: Arc::new(Mutex::new(BleConfig::default())),
            connect_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Test constructor with an explicit in-memory backend. Returns the
    /// manager plus the backend handle so tests can seed devices and
    /// inject packets without touching WinRT.
    pub fn new_in_memory() -> (Self, Arc<MemoryBackend>) {
        let (packet_tx, _) = broadcast::channel(256);
        let (state_tx, _) = broadcast::channel(256);
        let backend = MemoryBackend::new(packet_tx.clone());
        let manager = Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            address_to_id: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
            backend: backend.clone(),
            packet_tx,
            state_tx,
            scan_active: Arc::new(Mutex::new(false)),
            config: Arc::new(Mutex::new(BleConfig::default())),
            connect_lock: Arc::new(Mutex::new(())),
        };
        (manager, backend)
    }

    pub fn subscribe_packets(&self) -> broadcast::Receiver<(Uuid, DecodedPacket)> {
        self.packet_tx.subscribe()
    }

    pub fn subscribe_state(&self) -> broadcast::Receiver<(Uuid, ConnectionState)> {
        self.state_tx.subscribe()
    }

    pub async fn start_scan(&self) -> Result<Vec<DeviceInfo>> {
        {
            let mut guard = self.scan_active.lock().await;
            if *guard {
                return Err(BleError::InvalidState {
                    state: "Scan already in progress".to_string(),
                });
            }
            *guard = true;
        }
        info!("Starting BLE scan for Mocopi devices");
        let (timeout, rssi_min, max_trackers) = {
            let cfg = self.config.lock().await;
            (
                Duration::from_millis(cfg.scan_duration_ms),
                cfg.rssi_min_dbm,
                cfg.max_trackers,
            )
        };
        let devices = self.backend.scan(timeout).await;
        *self.scan_active.lock().await = false;
        let devices = devices?;
        // RSSI-Filter (0 = unbekannt passiert immer) + max_trackers-Cap.
        let mut filtered: Vec<DeviceInfo> = devices
            .into_iter()
            .filter(|d| d.rssi == 0 || d.rssi >= rssi_min)
            .collect();
        filtered.sort_by(|a, b| b.rssi.cmp(&a.rssi));
        // Apply the device cap after sorting so a noisy discovery order does
        // not hide stronger Mocopi trackers behind weaker ones.
        filtered.truncate(max_trackers.max(1));
        // Remember discovered devices so `connect_device` and the GUI can
        // resolve them by address without re-scanning.
        for info in &filtered {
            self.upsert_discovered(info.clone()).await;
        }
        info!("Discovered {} Mocopi device(s)", filtered.len());
        Ok(filtered)
    }

    pub async fn is_scanning(&self) -> bool {
        *self.scan_active.lock().await
    }

    async fn upsert_discovered(&self, info: DeviceInfo) {
        let mut addr_map = self.address_to_id.write().await;
        let mut devices = self.devices.write().await;
        for (id, existing) in devices.iter() {
            if existing
                .info
                .bluetooth_address
                .eq_ignore_ascii_case(&info.bluetooth_address)
            {
                addr_map.insert(info.bluetooth_address.clone(), *id);
                // Refresh the GUI name: a placeholder like "Mocopi (addr)"
                // from a direct connect is replaced by the live scan name.
                if !info.name.trim().is_empty() {
                    existing.set_display_name(info.name.clone()).await;
                }
                existing.set_rssi(info.rssi).await;
                return;
            }
        }
        let id = info.id;
        addr_map.insert(info.bluetooth_address.clone(), id);
        devices.insert(id, Arc::new(TrackerDevice::new(info)));
    }

    pub async fn connect_device(&self, address: &str) -> Result<Uuid> {
        // Validate early so typos fail fast instead of timing out in WinRT.
        winrt::address_to_u64(address).map(|_| ())?;
        let timeout = Duration::from_millis(self.config.lock().await.connection_timeout_ms);
        let device_id = self.find_or_create_device(address).await?;
        let device = self
            .get_device(&device_id)
            .await
            .ok_or(BleError::DeviceNotFound {
                identifier: address.to_string(),
            })?;

        // Doppelversuch-Guard (Auto-Connect + Ticker + manuell).
        if !device.try_begin_connect().await {
            return Err(BleError::InvalidState {
                state: "connect already in progress".to_string(),
            });
        }
        // Manueller Retry setzt den Backoff zurück.
        device.reset_reconnect_attempts().await;
        device.set_next_retry(None).await;

        device.set_state(ConnectionState::Connecting).await;
        self.emit_state_change(device_id, ConnectionState::Connecting)
            .await;
        // Manueller Connect hebt einen vorherigen manuellen Disconnect auf.
        device.set_manual_disconnect(false).await;

        let packet_tx = self.packet_tx.clone();
        let packet_baseline = device.get_packet_count().await;
        // Global serialisiert: nur ein Bring-up gleichzeitig aufs Radio.
        let _serial = self.connect_lock.lock().await;
        let result = tokio::time::timeout(
            timeout + Duration::from_secs(2),
            self.backend.connect(address, timeout, device_id, packet_tx),
        )
        .await;
        match result {
            Ok(Ok((conn, firmware, display_name))) => {
                device.set_state(ConnectionState::Initializing).await;
                self.emit_state_change(device_id, ConnectionState::Initializing)
                    .await;
                // Do not report Streaming just because GATT setup succeeded.
                // Wait for an actual valid IMU frame; the reconnect scheduler
                // will retry if the tracker never starts its data stream.
                if !Self::wait_for_tracking_packet(
                    &device,
                    packet_baseline,
                    conn.requires_tracking_packet(),
                    Duration::from_secs(5),
                )
                .await
                {
                    let err = BleError::ConnectionFailed {
                        reason: "BLE connected, but fewer than 3 fresh tracking packets arrived within 5 seconds"
                            .into(),
                    };
                    device.set_state(ConnectionState::Error).await;
                    device.set_error(Some(err.clone())).await;
                    self.emit_state_change(device_id, ConnectionState::Error)
                        .await;
                    device.end_connect().await;
                    return Err(err);
                }
                if let Some(fw) = firmware {
                    device.set_firmware_version(fw).await;
                }
                // Best-effort status poll; failure does not fail the connection
                // because streaming notifications are the source of truth.
                let _ = self
                    .backend
                    .write_command(address, CMD_GET_STATUS, Duration::from_secs(3))
                    .await;
                {
                    let mut conns = self.connections.lock().await;
                    if let Some(old) = conns.insert(device_id, conn) {
                        drop(old);
                    }
                }
                device.set_display_name(display_name).await;
                device.set_state(ConnectionState::Streaming).await;
                self.emit_state_change(device_id, ConnectionState::Streaming)
                    .await;
                device.reset_reconnect_attempts().await;
                device.set_next_retry(None).await;
                device.set_error(None).await;
                device.end_connect().await;
                Ok(device_id)
            }
            Ok(Err(e)) => {
                device.set_state(ConnectionState::Error).await;
                device.set_error(Some(e.clone())).await;
                self.emit_state_change(device_id, ConnectionState::Error)
                    .await;
                device.end_connect().await;
                Err(e)
            }
            Err(_) => {
                let err = BleError::ConnectionTimeout;
                device.set_state(ConnectionState::Error).await;
                device.set_error(Some(err.clone())).await;
                self.emit_state_change(device_id, ConnectionState::Error)
                    .await;
                device.end_connect().await;
                Err(err)
            }
        }
    }

    async fn wait_for_tracking_packet(
        device: &TrackerDevice,
        baseline: u64,
        required: bool,
        timeout: Duration,
    ) -> bool {
        if !required {
            return true;
        }
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            let received = device.get_packet_count().await.saturating_sub(baseline);
            if received >= 3
                && device
                    .tracking_data_age()
                    .await
                    .is_some_and(|age| age < Duration::from_secs(1))
            {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    async fn find_or_create_device(&self, address: &str) -> Result<Uuid> {
        let mut addr_map = self.address_to_id.write().await;
        if let Some(id) = addr_map.get(address) {
            return Ok(*id);
        }
        let mut devices = self.devices.write().await;
        for (id, device) in devices.iter() {
            if device.info.bluetooth_address.eq_ignore_ascii_case(address) {
                addr_map.insert(address.to_string(), *id);
                return Ok(*id);
            }
        }
        let info = DeviceInfo::new(address.to_string(), format!("Mocopi ({address})"), 0);
        let device = Arc::new(TrackerDevice::new(info));
        let id = device.info.id;
        devices.insert(id, device);
        addr_map.insert(address.to_string(), id);
        Ok(id)
    }

    pub async fn get_device(&self, id: &Uuid) -> Option<Arc<TrackerDevice>> {
        self.devices.read().await.get(id).cloned()
    }

    pub async fn disconnect_device(&self, id: &Uuid) -> Result<()> {
        if let Some(device) = self.get_device(id).await {
            if let Some(conn) = self.connections.lock().await.remove(id) {
                self.backend.disconnect(conn).await?;
            }
            // Manueller Disconnect: Auto-Reconnect bleibt aus.
            device.set_manual_disconnect(true).await;
            device.set_next_retry(None).await;
            device.set_state(ConnectionState::Disconnected).await;
            self.emit_state_change(*id, ConnectionState::Disconnected)
                .await;
        }
        Ok(())
    }

    /// Request a battery report. The value arrives asynchronously as a
    /// `DecodedPacket::Battery` notification; nothing is fabricated here.
    pub async fn request_battery(&self, id: &Uuid) -> Result<()> {
        let device = self.get_device(id).await.ok_or(BleError::DeviceNotFound {
            identifier: id.to_string(),
        })?;
        if device.get_state().await != ConnectionState::Streaming {
            return Err(BleError::InvalidState {
                state: "battery poll requires Streaming state".to_string(),
            });
        }
        let timeout = Duration::from_secs(3);
        self.backend
            .write_command(&device.info.bluetooth_address, CMD_GET_BATTERY, timeout)
            .await
    }

    /// Auto-Reconnect-Scheduler: tickt jede Sekunde, blockiert nie.
    /// Pro Gerät wird ein Retry-Zeitpunkt geplant (`next_retry`); fällige
    /// Versuche laufen als eigene Tasks. Unbegrenzt mit Backoff+Jitter,
    /// Stopp nur bei manuellem Disconnect / auto_connect=false / Stop.
    pub async fn start_reconnect_loop(&self) {
        let devices = self.devices.clone();
        let backend = self.backend.clone();
        let packet_tx = self.packet_tx.clone();
        let state_tx = self.state_tx.clone();
        let connections = self.connections.clone();
        let address_to_id = self.address_to_id.clone();
        let config = self.config.clone();
        let connect_lock = self.connect_lock.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(1));
            loop {
                ticker.tick().await;
                // Live-Config (kein stale Klon vom Loop-Start).
                let (auto_connect, base_delay_ms, timeout_ms) = {
                    let cfg = config.lock().await;
                    (
                        cfg.auto_connect,
                        cfg.reconnect_delay_ms,
                        cfg.connection_timeout_ms,
                    )
                };
                let snapshot: Vec<Arc<TrackerDevice>> =
                    { devices.read().await.values().cloned().collect() };
                for device in snapshot {
                    let state = device.get_state().await;
                    if state == ConnectionState::Streaming {
                        let (has_connection, link_alive) = {
                            let conns = connections.lock().await;
                            match conns.get(&device.info.id) {
                                Some(conn) => (true, conn.is_connected()),
                                None => (false, false),
                            }
                        };
                        if !has_connection || !link_alive {
                            // Data delays are surfaced to the UI but do not
                            // tear down a healthy GATT link. Reconnect only
                            // after the BLE connection itself is gone.
                            connections.lock().await.remove(&device.info.id);
                            device.set_manual_disconnect(false).await;
                            device.set_state(ConnectionState::Error).await;
                            device
                                .set_error(Some(BleError::ConnectionFailed {
                                    reason: "Bluetooth link disconnected".into(),
                                }))
                                .await;
                            let _ = state_tx.send((device.info.id, ConnectionState::Error));
                        }
                        continue;
                    }
                    if state != ConnectionState::Error && state != ConnectionState::Disconnected {
                        continue;
                    }
                    if !auto_connect {
                        continue;
                    }
                    // Nach manuellem Disconnect kein Auto-Reconnect.
                    if device.is_manual_disconnect().await {
                        continue;
                    }
                    // Geplanter Retry noch nicht fällig?
                    if device.retry_in_ms().await.unwrap_or(0) > 0 {
                        continue;
                    }
                    // Doppelversuch-Guard (läuft schon ein Connect?).
                    let attempts = device.increment_reconnect_attempts().await;
                    // Unbegrenzt: kein Cap mehr (Zähler läuft für die Anzeige).
                    let delay = Self::backoff_delay(
                        base_delay_ms,
                        attempts,
                        &device.info.bluetooth_address,
                    );
                    device
                        .set_next_retry(Some(
                            tokio::time::Instant::now() + Duration::from_millis(delay),
                        ))
                        .await;
                    device.set_state(ConnectionState::Reconnecting).await;
                    let _ = state_tx.send((device.info.id, ConnectionState::Reconnecting));

                    // Versuch als eigene Task (blockiert den Loop nie).
                    let dev = device.clone();
                    let be = backend.clone();
                    let ptx = packet_tx.clone();
                    let stx = state_tx.clone();
                    let conns = connections.clone();
                    let amap = address_to_id.clone();
                    let clock = connect_lock.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        // Global serialisiert (siehe connect_device).
                        let _serial = clock.lock().await;
                        // A manual reconnect clears the retry deadline and
                        // sets manual_disconnect before starting its own
                        // connect. Let that explicit request supersede this
                        // delayed automatic attempt.
                        if dev.is_manual_disconnect().await
                            || dev.retry_in_ms().await.is_none()
                            || !dev.try_begin_connect().await
                        {
                            return;
                        }
                        let packet_baseline = dev.get_packet_count().await;
                        let timeout = Duration::from_millis(timeout_ms);
                        let address = dev.info.bluetooth_address.clone();
                        let id = dev.info.id;
                        match be.connect(&address, timeout, id, ptx).await {
                            Ok((conn, fw, display_name)) => {
                                if Self::wait_for_tracking_packet(
                                    &dev,
                                    packet_baseline,
                                    conn.requires_tracking_packet(),
                                    Duration::from_secs(5),
                                )
                                .await
                                {
                                    if let Some(fw) = fw {
                                        dev.set_firmware_version(fw).await;
                                    }
                                    dev.set_display_name(display_name).await;
                                    conns.lock().await.insert(id, conn);
                                    let mut map = amap.write().await;
                                    map.insert(address.clone(), id);
                                    dev.set_state(ConnectionState::Streaming).await;
                                    dev.reset_reconnect_attempts().await;
                                    dev.set_next_retry(None).await;
                                    dev.set_error(None).await;
                                    let _ = stx.send((id, ConnectionState::Streaming));
                                } else {
                                    let e = BleError::ConnectionFailed {
                                        reason: "BLE connected, but fewer than 3 fresh tracking packets arrived within 5 seconds".into(),
                                    };
                                    warn!("Reconnect failed for {address}: {e}");
                                    dev.set_state(ConnectionState::Error).await;
                                    dev.set_error(Some(e)).await;
                                    let _ = stx.send((id, ConnectionState::Error));
                                }
                            }
                            Err(e) => {
                                warn!("Reconnect failed for {address}: {e}");
                                dev.set_state(ConnectionState::Error).await;
                                dev.set_error(Some(e)).await;
                                let _ = stx.send((id, ConnectionState::Error));
                            }
                        }
                        dev.end_connect().await;
                    });
                }
            }
        });
    }

    /// Backoff mit Jitter + Staffelung: Basis 5s, Verdopplung bis 60s Cap,
    /// ±20% deterministischer Jitter (MAC-Hash + Versuche), +0–2s Staffelung
    /// pro Gerät gegen Radio-K contention bei vielen Trackern.
    fn backoff_delay(base_ms: u64, attempts: u32, mac: &str) -> u64 {
        let shift = attempts.saturating_sub(1).min(4);
        let exp = (base_ms << shift).min(60_000);
        let mut hash: u64 = 0;
        for b in mac.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u64);
        }
        let jitter_pct = 80 + ((hash.wrapping_add(attempts as u64 * 2654435761)) % 41) as u64;
        let stagger = (hash % 2001) / (attempts.max(1) as u64);
        (exp * jitter_pct / 100 + stagger.min(2000)).min(62_000)
    }

    async fn emit_state_change(&self, id: Uuid, state: ConnectionState) {
        let _ = self.state_tx.send((id, state));
    }

    pub async fn get_all_devices(&self) -> Vec<TrackerStatus> {
        let devices = self.devices.read().await;
        let mut statuses = Vec::with_capacity(devices.len());
        for device in devices.values() {
            statuses.push(TrackerStatus::from_device(device.as_ref()).await);
        }
        statuses
    }

    pub async fn get_device_status(&self, id: &Uuid) -> Option<TrackerStatus> {
        if let Some(device) = self.get_device(id).await {
            Some(TrackerStatus::from_device(device.as_ref()).await)
        } else {
            None
        }
    }

    /// Record an IMU frame after the tracking core accepted and processed it.
    /// This is the readiness signal used by connect/reconnect; a successful
    /// GATT subscription by itself does not mean tracking is working.
    pub async fn record_tracking_packet(&self, id: Uuid, timestamp_ns: u64) {
        if let Some(device) = self.get_device(&id).await {
            device.increment_packet_count().await;
            device.update_last_packet(timestamp_ns).await;
        }
    }

    /// Restore a persisted role onto the live device matching `address`
    /// (case-insensitive). Returns the device id when found. Lock scopes
    /// are kept strictly sequential (never map+devices together) to
    /// avoid lock-order inversions with [`Self::upsert_discovered`].
    pub async fn apply_role_by_address(
        &self,
        address: &str,
        role: tracking_core::TrackerRole,
    ) -> Option<Uuid> {
        let id_opt = {
            if let Some(id) = self.address_to_id.read().await.get(address).copied() {
                Some(id)
            } else {
                self.devices
                    .read()
                    .await
                    .values()
                    .find(|d| d.info.bluetooth_address.eq_ignore_ascii_case(address))
                    .map(|d| d.info.id)
            }
        };
        let id = id_opt?;
        self.get_device(&id).await?;
        // Re-fetch under no map locks, then assign.
        if let Some(device) = self.get_device(&id).await {
            device.set_assigned_role(Some(role)).await;
            return Some(id);
        }
        None
    }

    pub async fn set_config(&self, config: BleConfig) {
        *self.config.lock().await = config;
    }

    pub async fn get_config(&self) -> BleConfig {
        self.config.lock().await.clone()
    }
}

impl Default for BleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mocopi_protocol::BatteryInfo;

    #[tokio::test]
    async fn test_ble_manager_new() {
        let (manager, _backend) = BleManager::new_in_memory();
        assert!(manager.get_all_devices().await.is_empty());
    }

    #[tokio::test]
    async fn test_connect_device_memory() {
        let (manager, backend) = BleManager::new_in_memory();
        backend
            .add_device(DeviceInfo::new(
                "3C:38:F4:12:34:56".to_string(),
                "QM-SS1".to_string(),
                -50,
            ))
            .await;
        let id = manager.connect_device("3C:38:F4:12:34:56").await.unwrap();
        let status = manager.get_device_status(&id).await.unwrap();
        assert_eq!(status.state, ConnectionState::Streaming);
    }

    #[tokio::test]
    async fn test_battery_roundtrip_memory() {
        let (manager, backend) = BleManager::new_in_memory();
        backend
            .add_device(DeviceInfo::new(
                "3C:38:F4:12:34:56".to_string(),
                "QM-SS1".to_string(),
                -50,
            ))
            .await;
        let id = manager.connect_device("3C:38:F4:12:34:56").await.unwrap();
        let mut rx = manager.subscribe_packets();
        backend
            .inject_battery(
                id,
                BatteryInfo {
                    voltage: 3.7,
                    percentage: 0.5,
                },
            )
            .await
            .unwrap();
        let (got_id, packet) = rx.recv().await.unwrap();
        assert_eq!(got_id, id);
        assert!(matches!(packet, DecodedPacket::Battery(_)));
    }

    #[test]
    fn test_rssi_filter() {
        let cfg = BleConfig {
            rssi_min_dbm: -70,
            ..Default::default()
        };
        assert!(cfg.passes_rssi(0)); // unbekannt passiert immer
        assert!(cfg.passes_rssi(-50));
        assert!(!cfg.passes_rssi(-90));
    }

    #[tokio::test]
    async fn test_scan_filters_weak_and_caps() {
        let (manager, backend) = BleManager::new_in_memory();
        backend
            .add_device(DeviceInfo::new(
                "AA:1".to_string(),
                "QM-SS1-A".to_string(),
                -40,
            ))
            .await;
        backend
            .add_device(DeviceInfo::new(
                "AA:2".to_string(),
                "QM-SS1-B".to_string(),
                -90,
            ))
            .await;
        manager
            .set_config(BleConfig {
                rssi_min_dbm: -70,
                max_trackers: 12,
                ..Default::default()
            })
            .await;
        let found = manager.start_scan().await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].bluetooth_address, "AA:1");
    }

    #[tokio::test]
    async fn test_manual_disconnect_blocks_reconnect_flag() {
        let (manager, backend) = BleManager::new_in_memory();
        backend
            .add_device(DeviceInfo::new(
                "3C:38:F4:12:34:56".to_string(),
                "QM-SS1".to_string(),
                -50,
            ))
            .await;
        let id = manager.connect_device("3C:38:F4:12:34:56").await.unwrap();
        let dev = manager.get_device(&id).await.unwrap();
        assert!(!dev.is_manual_disconnect().await);
        manager.disconnect_device(&id).await.unwrap();
        assert!(dev.is_manual_disconnect().await);
        // Erneuter manueller Connect hebt die Sperre auf.
        manager.connect_device("3C:38:F4:12:34:56").await.unwrap();
        assert!(!dev.is_manual_disconnect().await);
    }

    #[test]
    fn test_backoff_grows_and_caps() {
        let mac = "3C:38:F4:12:34:56";
        let d1 = BleManager::backoff_delay(5000, 1, mac);
        let d2 = BleManager::backoff_delay(5000, 2, mac);
        let d5 = BleManager::backoff_delay(5000, 5, mac);
        let d20 = BleManager::backoff_delay(5000, 20, mac);
        assert!(d1 >= 4000 && d1 <= 8000, "d1={d1}");
        assert!(d2 > d1, "d1={d1} d2={d2}");
        assert!(d20 <= 62000, "d20={d20}");
        assert!(d5 <= 62000, "d5={d5}");
    }

    #[tokio::test]
    async fn test_connect_guard_blocks_duplicates() {
        use crate::device::{DeviceInfo, TrackerDevice};
        let dev = TrackerDevice::new(DeviceInfo::new(
            "3C:38:F4:12:34:56".to_string(),
            "QM-SS1".to_string(),
            -50,
        ));
        assert!(dev.try_begin_connect().await);
        assert!(!dev.try_begin_connect().await);
        dev.end_connect().await;
        assert!(dev.try_begin_connect().await);
        dev.end_connect().await;
    }
}
