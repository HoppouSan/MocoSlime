use crate::error::BleError;
use mocopi_protocol::BatteryInfo;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracking_core::TrackerRole;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ConnectionState {
    Disconnected = 0,
    Discovered = 1,
    Discovering = 2,
    Connecting = 3,
    Initializing = 4,
    Streaming = 5,
    Reconnecting = 6,
    Error = 7,
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Streaming)
    }

    pub fn is_connecting(&self) -> bool {
        matches!(
            self,
            ConnectionState::Connecting | ConnectionState::Initializing
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: Uuid,
    pub bluetooth_address: String,
    pub name: String,
    pub rssi: i16,
    pub paired: bool,
    pub connectable: bool,
}

impl DeviceInfo {
    pub fn new(bluetooth_address: String, name: String, rssi: i16) -> Self {
        Self {
            id: Uuid::new_v4(),
            bluetooth_address,
            name,
            rssi,
            paired: false,
            connectable: true,
        }
    }

    pub fn is_mocopi(&self) -> bool {
        self.name.contains("QM-SS1") || self.name.contains("Mocopi")
    }
}

#[derive(Debug, Clone)]
pub struct TrackerDevice {
    pub info: DeviceInfo,
    rssi: Arc<Mutex<i16>>,
    pub state: Arc<Mutex<ConnectionState>>,
    pub assigned_role: Arc<Mutex<Option<TrackerRole>>>,
    pub battery: Arc<Mutex<Option<BatteryInfo>>>,
    pub firmware_version: Arc<Mutex<Option<String>>>,
    pub last_packet_ns: Arc<Mutex<Option<u64>>>,
    pub packet_count: Arc<Mutex<u64>>,
    last_tracking_at: Arc<Mutex<Option<tokio::time::Instant>>>,
    pub reconnect_attempts: Arc<Mutex<u32>>,
    pub last_error: Arc<Mutex<Option<BleError>>>,
    /// True nach manuellem Disconnect durch den User: Auto-Reconnect
    /// bleibt aus, bis wieder manuell verbunden wird.
    pub manual_disconnect: Arc<Mutex<bool>>,
    /// Verbindungsversuch läuft (Guard gegen Doppelversuche aus
    /// Auto-Connect + Reconnect-Ticker + manuellem Connect).
    connecting: Arc<Mutex<bool>>,
    /// Nächster geplanter Auto-Retry (Scheduler, kein Sleep-in-Loop).
    next_retry: Arc<Mutex<Option<tokio::time::Instant>>>,
    /// Live display name from WinRT (scan or GATT). Falls back to
    /// `info.name` (placeholder `Mocopi (<addr>)` for direct connects).
    display_name: Arc<Mutex<Option<String>>>,
}
impl TrackerDevice {
    pub fn new(info: DeviceInfo) -> Self {
        Self {
            rssi: Arc::new(Mutex::new(info.rssi)),
            info,
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            assigned_role: Arc::new(Mutex::new(None)),
            battery: Arc::new(Mutex::new(None)),
            firmware_version: Arc::new(Mutex::new(None)),
            last_packet_ns: Arc::new(Mutex::new(None)),
            packet_count: Arc::new(Mutex::new(0)),
            last_tracking_at: Arc::new(Mutex::new(None)),
            reconnect_attempts: Arc::new(Mutex::new(0)),
            last_error: Arc::new(Mutex::new(None)),
            manual_disconnect: Arc::new(Mutex::new(false)),
            connecting: Arc::new(Mutex::new(false)),
            next_retry: Arc::new(Mutex::new(None)),
            display_name: Arc::new(Mutex::new(None)),
        }
    }

    /// Name to show in the GUI: live WinRT name when known, else the
    /// discovery placeholder from [`DeviceInfo`].
    pub async fn display_name(&self) -> String {
        if let Some(name) = self.display_name.lock().await.clone() {
            if !name.is_empty() {
                return name;
            }
        }
        self.info.name.clone()
    }

    /// Store the live WinRT device name. Empty names are ignored so a
    /// failed lookup can never blank out a previously known name.
    pub async fn set_display_name(&self, name: String) {
        if !name.trim().is_empty() {
            *self.display_name.lock().await = Some(name);
        }
    }

    pub async fn get_state(&self) -> ConnectionState {
        *self.state.lock().await
    }

    pub async fn set_state(&self, state: ConnectionState) {
        *self.state.lock().await = state;
    }

    pub async fn get_assigned_role(&self) -> Option<TrackerRole> {
        *self.assigned_role.lock().await
    }

    pub async fn set_assigned_role(&self, role: Option<TrackerRole>) {
        *self.assigned_role.lock().await = role;
    }

    pub async fn update_battery(&self, battery: BatteryInfo) {
        *self.battery.lock().await = Some(battery);
    }

    pub async fn get_battery(&self) -> Option<BatteryInfo> {
        *self.battery.lock().await
    }

    pub async fn increment_packet_count(&self) {
        *self.packet_count.lock().await += 1;
    }

    pub async fn get_packet_count(&self) -> u64 {
        *self.packet_count.lock().await
    }

    pub async fn update_last_packet(&self, timestamp_ns: u64) {
        *self.last_packet_ns.lock().await = Some(timestamp_ns);
        *self.last_tracking_at.lock().await = Some(tokio::time::Instant::now());
    }

    pub async fn tracking_data_age(&self) -> Option<Duration> {
        self.last_tracking_at.lock().await.map(|at| at.elapsed())
    }

    pub async fn get_last_packet_ns(&self) -> Option<u64> {
        *self.last_packet_ns.lock().await
    }

    /// Get the timestamp (ns) of the last received packet, if any.
    pub async fn last_packet_timestamp(&self) -> Option<u64> {
        *self.last_packet_ns.lock().await
    }

    /// Get the current RSSI value from the device info.
    pub async fn current_rssi(&self) -> i16 {
        *self.rssi.lock().await
    }

    /// Calculate the average RSSI (currently based on device info).
    pub async fn average_rssi(&self) -> f32 {
        self.current_rssi().await as f32
    }

    pub async fn set_rssi(&self, rssi: i16) {
        if rssi != 0 {
            *self.rssi.lock().await = rssi;
        }
    }

    pub async fn increment_reconnect_attempts(&self) -> u32 {
        let mut attempts = self.reconnect_attempts.lock().await;
        *attempts += 1;
        *attempts
    }

    pub async fn reset_reconnect_attempts(&self) {
        *self.reconnect_attempts.lock().await = 0;
    }

    pub async fn get_reconnect_attempts(&self) -> u32 {
        *self.reconnect_attempts.lock().await
    }

    pub async fn set_manual_disconnect(&self, v: bool) {
        *self.manual_disconnect.lock().await = v;
    }

    pub async fn is_manual_disconnect(&self) -> bool {
        *self.manual_disconnect.lock().await
    }

    /// Atomarer Doppelversuch-Guard: genau ein Caller gewinnt.
    pub async fn try_begin_connect(&self) -> bool {
        let mut guard = self.connecting.lock().await;
        if *guard {
            return false;
        }
        *guard = true;
        true
    }

    pub async fn end_connect(&self) {
        *self.connecting.lock().await = false;
    }

    pub async fn set_next_retry(&self, at: Option<tokio::time::Instant>) {
        *self.next_retry.lock().await = at;
    }

    /// Verbleibende ms bis zum geplanten Retry (None = keiner geplant).
    pub async fn retry_in_ms(&self) -> Option<u64> {
        let guard = self.next_retry.lock().await;
        let at = (*guard)?;
        let now = tokio::time::Instant::now();
        Some(at.saturating_duration_since(now).as_millis() as u64)
    }

    pub async fn set_error(&self, error: Option<BleError>) {
        *self.last_error.lock().await = error;
    }

    pub async fn get_error(&self) -> Option<BleError> {
        self.last_error.lock().await.clone()
    }

    pub async fn set_firmware_version(&self, version: String) {
        *self.firmware_version.lock().await = Some(version);
    }

    pub async fn get_firmware_version(&self) -> Option<String> {
        self.firmware_version.lock().await.clone()
    }

    /// Convenience async accessor for the device UUID.
    pub async fn device_id(&self) -> uuid::Uuid {
        self.info.id
    }

    /// Convenience async accessor for the Bluetooth address string.
    pub async fn bluetooth_address(&self) -> String {
        self.info.bluetooth_address.clone()
    }

    /// Clone the current connection state (requires async lock).
    pub async fn state_clone(&self) -> ConnectionState {
        *self.state.lock().await
    }

    /// Clone the currently assigned role, if any.
    pub async fn assigned_role_clone(&self) -> Option<TrackerRole> {
        *self.assigned_role.lock().await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerStatus {
    pub device_id: Uuid,
    pub bluetooth_address: String,
    pub name: String,
    pub state: ConnectionState,
    pub assigned_role: Option<TrackerRole>,
    pub battery: Option<BatteryInfo>,
    pub rssi: i16,
    pub firmware_version: Option<String>,
    pub packet_count: u64,
    pub last_packet_ns: Option<u64>,
    pub reconnect_attempts: u32,
    pub error: Option<String>,
    /// Countdown zum nächsten Auto-Retry in ms (None = keiner geplant).
    #[serde(default)]
    pub reconnect_in_ms: Option<u64>,
}

impl TrackerStatus {
    pub async fn from_device(device: &TrackerDevice) -> Self {
        Self {
            device_id: device.info.id,
            bluetooth_address: device.info.bluetooth_address.clone(),
            name: device.display_name().await,
            state: device.get_state().await,
            assigned_role: device.get_assigned_role().await,
            battery: device.get_battery().await,
            rssi: device.current_rssi().await,
            firmware_version: device.get_firmware_version().await,
            packet_count: device.get_packet_count().await,
            last_packet_ns: device.get_last_packet_ns().await,
            reconnect_attempts: device.get_reconnect_attempts().await,
            error: device.get_error().await.map(|e| e.to_string()),
            reconnect_in_ms: device.retry_in_ms().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mocopi_protocol::BatteryInfo;

    #[test]
    fn test_connection_state() {
        assert!(ConnectionState::Streaming.is_connected());
        assert!(!ConnectionState::Connecting.is_connected());
        assert!(ConnectionState::Connecting.is_connecting());
        assert!(ConnectionState::Initializing.is_connecting());
    }

    #[test]
    fn test_device_info_mocopi() {
        let device = DeviceInfo::new("3C:38:F4:12:34:56".to_string(), "QM-SS1".to_string(), -50);
        assert!(device.is_mocopi());

        let device2 = DeviceInfo::new(
            "AA:BB:CC:DD:EE:FF".to_string(),
            "Other Device".to_string(),
            -60,
        );
        assert!(!device2.is_mocopi());
    }

    #[tokio::test]
    async fn test_tracker_device() {
        let info = DeviceInfo::new("3C:38:F4:12:34:56".to_string(), "QM-SS1".to_string(), -50);
        let device = TrackerDevice::new(info);

        assert_eq!(device.get_state().await, ConnectionState::Disconnected);

        device.set_state(ConnectionState::Connecting).await;
        assert_eq!(device.get_state().await, ConnectionState::Connecting);

        device.set_assigned_role(Some(TrackerRole::LeftFoot)).await;
        assert_eq!(
            device.get_assigned_role().await,
            Some(TrackerRole::LeftFoot)
        );

        let battery = BatteryInfo {
            voltage: 3.7,
            percentage: 0.8,
        };
        device.update_battery(battery).await;
        assert_eq!(device.get_battery().await, Some(battery));
    }
}
