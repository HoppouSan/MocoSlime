//! Pluggable BLE backend: real WinRT on Windows, in-memory for tests.
//!
//! The previous `BleManager` fabricated scan results, firmware strings and
//! battery levels with `sleep()` calls. That test-only behaviour now lives
//! explicitly in [`MemoryBackend`]; production uses [`WinRtBackend`].

use crate::device::DeviceInfo;
use crate::error::{BleError, Result};
use async_trait::async_trait;
use mocopi_protocol::{BatteryInfo, DecodedPacket, MocopiPacket};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

/// A live BLE connection. The WinRT variant owns notification tokens;
/// the memory variant is a no-op handle used by tests.
pub enum BackendConnection {
    WinRt(crate::winrt::WinRtConnection),
    Memory,
}

impl BackendConnection {
    /// Best-effort link check. Memory connections are always "connected".
    pub fn is_connected(&self) -> bool {
        match self {
            BackendConnection::WinRt(c) => c.is_connected(),
            BackendConnection::Memory => true,
        }
    }

    pub fn requires_tracking_packet(&self) -> bool {
        matches!(self, BackendConnection::WinRt(_))
    }
}

/// Abstraction over the OS Bluetooth stack so unit tests never touch WinRT.
#[async_trait]
pub trait BleBackend: Send + Sync + 'static {
    async fn scan(&self, timeout: Duration) -> Result<Vec<DeviceInfo>>;
    async fn connect(
        &self,
        address: &str,
        timeout: Duration,
        device_id: Uuid,
        packet_tx: broadcast::Sender<(Uuid, DecodedPacket)>,
    ) -> Result<(BackendConnection, Option<String>, String)>;
    async fn disconnect(&self, conn: BackendConnection) -> Result<()>;
    async fn write_command(&self, address: &str, cmd: &[u8], timeout: Duration) -> Result<()>;
}

/// Real Windows implementation.
pub struct WinRtBackend;

#[async_trait]
impl BleBackend for WinRtBackend {
    async fn scan(&self, timeout: Duration) -> Result<Vec<DeviceInfo>> {
        crate::winrt::scan(timeout).await
    }

    async fn connect(
        &self,
        address: &str,
        timeout: Duration,
        device_id: Uuid,
        packet_tx: broadcast::Sender<(Uuid, DecodedPacket)>,
    ) -> Result<(BackendConnection, Option<String>, String)> {
        let (conn, fw) = crate::winrt::connect(address, timeout, device_id, packet_tx).await?;
        let name = conn.device_name();
        let display = if name.is_empty() {
            format!("Mocopi ({address})")
        } else {
            name
        };
        Ok((BackendConnection::WinRt(conn), fw, display))
    }

    async fn disconnect(&self, conn: BackendConnection) -> Result<()> {
        match conn {
            BackendConnection::WinRt(c) => {
                c.disconnect();
                Ok(())
            }
            BackendConnection::Memory => Ok(()),
        }
    }

    async fn write_command(&self, address: &str, cmd: &[u8], timeout: Duration) -> Result<()> {
        let addr = crate::winrt::address_to_u64(address)?;
        crate::winrt::write_command(addr, cmd.to_vec(), timeout).await
    }
}

/// In-memory backend for unit/integration tests. Never touches WinRT and
/// never fabricates battery or firmware data: injected packets are the
/// only data source.
pub struct MemoryBackend {
    devices: RwLock<HashMap<String, DeviceInfo>>,
    packets: broadcast::Sender<(Uuid, DecodedPacket)>,
}

impl MemoryBackend {
    pub fn new(packet_tx: broadcast::Sender<(Uuid, DecodedPacket)>) -> Arc<Self> {
        Arc::new(Self {
            devices: RwLock::new(HashMap::new()),
            packets: packet_tx,
        })
    }

    pub async fn add_device(&self, info: DeviceInfo) {
        self.devices
            .write()
            .await
            .insert(info.bluetooth_address.clone(), info);
    }

    /// Inject a fully-formed IMU packet for tests. The caller supplies
    /// complete parsed data; only available on the memory backend.
    pub async fn inject_imu(&self, device_id: Uuid, packet: MocopiPacket) -> Result<()> {
        self.packets
            .send((device_id, DecodedPacket::Imu(packet)))
            .map(|_| ())
            .map_err(|_| BleError::Disconnected)
    }

    pub async fn inject_battery(&self, device_id: Uuid, battery: BatteryInfo) -> Result<()> {
        self.packets
            .send((device_id, DecodedPacket::Battery(battery)))
            .map(|_| ())
            .map_err(|_| BleError::Disconnected)
    }
}

#[async_trait]
impl BleBackend for MemoryBackend {
    async fn scan(&self, _timeout: Duration) -> Result<Vec<DeviceInfo>> {
        Ok(self.devices.read().await.values().cloned().collect())
    }

    async fn connect(
        &self,
        address: &str,
        _timeout: Duration,
        _device_id: Uuid,
        _packet_tx: broadcast::Sender<(Uuid, DecodedPacket)>,
    ) -> Result<(BackendConnection, Option<String>, String)> {
        // Memory backend has no firmware source, so it reports None.
        let name = self
            .devices
            .read()
            .await
            .get(address)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| format!("Mocopi ({address})"));
        Ok((BackendConnection::Memory, None, name))
    }

    async fn disconnect(&self, _conn: BackendConnection) -> Result<()> {
        Ok(())
    }

    async fn write_command(&self, _address: &str, _cmd: &[u8], _timeout: Duration) -> Result<()> {
        // Memory backend: commands are accepted but produce no fabricated
        // responses. Tests inject responses explicitly via inject_*.
        Ok(())
    }
}
