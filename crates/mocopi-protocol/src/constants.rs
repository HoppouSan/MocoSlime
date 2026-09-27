pub const CMD_SERVICE_UUID: &str = "0000ff00-0000-1000-8000-00805f9b34fb";
pub const IMU_SERVICE_UUID: &str = "0000fff0-0000-1000-8000-00805f9b34fb";

pub const CMD_CHARACTERISTIC_UUID: &str = "0000ff01-0000-1000-8000-00805f9b34fb";
pub const IMU_CHARACTERISTIC_UUID: &str = "0000fff1-0000-1000-8000-00805f9b34fb";
/// IMU stream on current firmware: notify-capable characteristic inside
/// the CMD service (verified on-device: service `FF00` exposes
/// `FF01` read|write, `FF02` read, `FF03` read|notify; no `FFF0`
/// service exists). Used when the spec-layout `FFF0`/`FFF1` pair is
/// absent.
pub const IMU_FALLBACK_CHARACTERISTIC_UUID: &str = "0000ff03-0000-1000-8000-00805f9b34fb";

pub const CMD_START_STREAMING: &[u8] = &[0x7e, 0x03, 0x18, 0xd6, 0x01, 0x00, 0x00];
pub const CMD_GET_STATUS: &[u8] = &[0x7e, 0x02, 0x09, 0x02, 0x9a, 0xda];
pub const CMD_GET_BATTERY: &[u8] = &[0x7e, 0x07, 0x09, 0x02, 0x9a, 0xda];

pub const IMU_NOTIFICATION_HANDLE: u16 = 73;
pub const CMD_RESPONSE_HANDLE: u16 = 34;

pub const PACKET_IMU_DATA: u8 = 0x49;
pub const PACKET_BATTERY_RESPONSE: u8 = 0x07;
pub const PACKET_STATUS_RESPONSE: u8 = 0x02;
/// IMU stream marker variant (community reverse-engineering:
/// "sometimes changes to 0x30 under unknown circumstances").
/// Same 36-byte layout as `PACKET_IMU_DATA`.
pub const PACKET_IMU_DATA_V2: u8 = 0x30;

pub const EXPECTED_IMU_PACKET_SIZE: usize = 30;
pub const EXPECTED_BATTERY_PACKET_SIZE: usize = 12;

pub const QUATERNION_SCALE: f32 = 1.0 / 8192.0;
pub const ACCEL_CORRECTION_FACTOR: f32 = 0.12;
pub const COUNTER_INCREMENT: u64 = 78125;
