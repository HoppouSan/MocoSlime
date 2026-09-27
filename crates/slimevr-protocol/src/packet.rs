use crate::error::{Result, SlimeVRError};
use serde::{Deserialize, Serialize};

/// SlimeVR UDP packet types. Wire value is a big-endian u32 header,
/// matching the original Python `struct.pack('>I', ...)` / SlimeVR server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum PacketType {
    Ping = 0x01,
    Pong = 0x02,
    Handshake = 0x03,
    Acceleration = 0x04,
    Battery = 0x0C,
    Error = 0x0E,
    SensorInfo = 0x0F,
    Rotation = 0x11,
}

impl TryFrom<u32> for PacketType {
    type Error = SlimeVRError;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            0x01 => Ok(PacketType::Ping),
            0x02 => Ok(PacketType::Pong),
            0x03 => Ok(PacketType::Handshake),
            0x04 => Ok(PacketType::Acceleration),
            0x0C => Ok(PacketType::Battery),
            0x0E => Ok(PacketType::Error),
            0x0F => Ok(PacketType::SensorInfo),
            0x11 => Ok(PacketType::Rotation),
            _ => Err(SlimeVRError::UnknownPacketType {
                r#type: value as u8,
            }),
        }
    }
}

/// A decoded SlimeVR UDP datagram.
///
/// Wire format (all multi-byte fields big-endian, matching Python `>`):
/// `type u32 | counter u64 | payload ..`
/// There is deliberately NO length prefix and NO extra sensor-id header:
/// sensor ids live inside the payload exactly as the Python reference does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlimeVRPacket {
    pub packet_type: PacketType,
    pub counter: u64,
    pub payload: Vec<u8>,
}

impl SlimeVRPacket {
    pub fn new(packet_type: PacketType, counter: u64, payload: Vec<u8>) -> Self {
        Self {
            packet_type,
            counter,
            payload,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 8 + self.payload.len());
        buf.extend_from_slice(&(self.packet_type as u32).to_be_bytes());
        buf.extend_from_slice(&self.counter.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(SlimeVRError::PacketTooShort {
                expected: 12,
                actual: data.len(),
            });
        }
        let packet_type =
            PacketType::try_from(u32::from_be_bytes([data[0], data[1], data[2], data[3]]))?;
        let counter = u64::from_be_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ]);
        Ok(Self {
            packet_type,
            counter,
            payload: data[12..].to_vec(),
        })
    }
}

fn parse_mac(mac: &str) -> Result<[u8; 6]> {
    let clean: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() < 12 {
        return Err(SlimeVRError::SerializationError(format!(
            "invalid MAC address '{mac}': need 12 hex digits"
        )));
    }
    let tail = &clean[clean.len() - 12..];
    let bytes = hex::decode(tail)
        .map_err(|e| SlimeVRError::SerializationError(format!("invalid MAC '{mac}': {e}")))?;
    let mut out = [0u8; 6];
    out.copy_from_slice(&bytes[..6]);
    Ok(out)
}

/// Build a SlimeVR handshake (packet 3).
/// Byte-for-byte compatible with `mocoslime_common.build_handshake`.
pub fn build_handshake_packet(mac: &str, firmware: &str, counter: u64) -> Vec<u8> {
    // Keep the Python firmware-string shape so SlimeVR shows a familiar name.
    let fw_string = format!("Mocoslime - Tracker Version:{firmware}");
    let mut payload = Vec::new();

    payload.extend_from_slice(&15u32.to_be_bytes()); // board type
    payload.extend_from_slice(&8u32.to_be_bytes()); // IMU type (BMI160 compat; Mocopi uses BMI270)
    payload.extend_from_slice(&7u32.to_be_bytes()); // MCU type
    payload.extend_from_slice(&[0u8; 12]); // IMU info (3x u32 zero)
    let build = firmware.replace('.', "").parse::<u32>().unwrap_or(0);
    payload.extend_from_slice(&build.to_be_bytes());
    payload.push(fw_string.len().min(255) as u8);
    payload.extend_from_slice(fw_string.as_bytes());
    match parse_mac(mac) {
        Ok(bytes) => payload.extend_from_slice(&bytes),
        Err(_) => payload.extend_from_slice(&[0u8; 6]),
    }
    payload.push(255);

    SlimeVRPacket::new(PacketType::Handshake, counter, payload).serialize()
}

/// Build a sensor-info packet (15): tracker_id, sensor status, sensor type.
pub fn build_sensor_info_packet(counter: u64, sensor_id: u8) -> Vec<u8> {
    // Sensor status 1 means connected/OK; 0 makes SlimeVR create the sensor
    // as disconnected even though it is receiving rotation packets.
    let payload = vec![sensor_id, 1, 8];
    SlimeVRPacket::new(PacketType::SensorInfo, counter, payload).serialize()
}

/// Build a rotation packet (17). Quaternion remap `(-x, z, y, w)`
/// matches the Python `struct.pack('>ffff', -qx, qz, qy, qw)`.
pub fn build_rotation_packet(
    quat: nalgebra::UnitQuaternion<f32>,
    counter: u64,
    sensor_id: u8,
) -> Vec<u8> {
    let (x, y, z, w) = (quat.i, quat.j, quat.k, quat.w);
    let mut payload = Vec::with_capacity(2 + 16 + 1);
    payload.push(sensor_id);
    payload.push(1); // data type (kept from reference implementation)
    payload.extend_from_slice(&(-x).to_be_bytes());
    payload.extend_from_slice(&z.to_be_bytes());
    payload.extend_from_slice(&y.to_be_bytes());
    payload.extend_from_slice(&w.to_be_bytes());
    payload.push(0); // calibration info (unused by SlimeVR currently)
    SlimeVRPacket::new(PacketType::Rotation, counter, payload).serialize()
}

/// Build an acceleration packet (4): `x,y,z f32 BE + sensor id`.
pub fn build_acceleration_packet(
    accel: nalgebra::Vector3<f32>,
    counter: u64,
    sensor_id: u8,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(12 + 1);
    payload.extend_from_slice(&accel.x.to_be_bytes());
    payload.extend_from_slice(&accel.y.to_be_bytes());
    payload.extend_from_slice(&accel.z.to_be_bytes());
    payload.push(sensor_id);
    SlimeVRPacket::new(PacketType::Acceleration, counter, payload).serialize()
}

/// Build a battery packet (12): `voltage f32 BE + percentage f32 BE`.
pub fn build_battery_packet(
    voltage: f32,
    percentage: f32,
    counter: u64,
    _sensor_id: u8,
) -> Vec<u8> {
    // Reference Python battery packets carry no sensor id; keep the
    // parameter for API symmetry but do not emit it on the wire.
    let mut payload = Vec::with_capacity(8);
    payload.extend_from_slice(&voltage.to_be_bytes());
    payload.extend_from_slice(&percentage.clamp(0.0, 1.0).to_be_bytes());
    SlimeVRPacket::new(PacketType::Battery, counter, payload).serialize()
}

pub fn build_error_packet(counter: u64, sensor_id: u8, error_code: u8) -> Vec<u8> {
    let payload = vec![sensor_id, error_code];
    SlimeVRPacket::new(PacketType::Error, counter, payload).serialize()
}

pub fn build_ping_packet(counter: u64) -> Vec<u8> {
    SlimeVRPacket::new(PacketType::Ping, counter, Vec::new()).serialize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{UnitQuaternion, Vector3};

    #[test]
    fn test_handshake_packet() {
        let packet = build_handshake_packet("3C:38:F4:12:34:56", "1.2.3", 1);
        assert!(!packet.is_empty());
        assert_eq!(packet[0..4], [0x00, 0x00, 0x00, 0x03]);
        assert_eq!(u64::from_be_bytes(packet[4..12].try_into().unwrap()), 1);
        // MAC must appear verbatim at payload end (6 bytes + 0xFF).
        assert_eq!(
            &packet[packet.len() - 7..packet.len() - 1],
            &[0x3C, 0x38, 0xF4, 0x12, 0x34, 0x56]
        );
        assert_eq!(packet[packet.len() - 1], 255);
    }

    #[test]
    fn test_sensor_info_packet() {
        let packet = build_sensor_info_packet(2, 0);
        assert_eq!(packet[0..4], [0x00, 0x00, 0x00, 0x0F]);
        assert_eq!(u64::from_be_bytes(packet[4..12].try_into().unwrap()), 2);
        assert_eq!(&packet[12..], &[0, 0, 8]);
    }

    #[test]
    fn test_rotation_packet() {
        let quat = UnitQuaternion::from_euler_angles(0.1, 0.2, 0.3);
        let packet = build_rotation_packet(quat, 3, 0);
        assert_eq!(packet[0..4], [0x00, 0x00, 0x00, 0x11]);
        assert_eq!(u64::from_be_bytes(packet[4..12].try_into().unwrap()), 3);
        // sensor id + data type + 4xf32 + calib
        assert_eq!(packet.len(), 12 + 2 + 16 + 1);
        assert_eq!(packet[12], 0);
        assert_eq!(packet[13], 1);
    }

    #[test]
    fn test_acceleration_packet() {
        let accel = Vector3::new(1.0, 2.0, 3.0);
        let packet = build_acceleration_packet(accel, 4, 0);
        assert_eq!(packet[0..4], [0x00, 0x00, 0x00, 0x04]);
        assert_eq!(u64::from_be_bytes(packet[4..12].try_into().unwrap()), 4);
        assert_eq!(packet.len(), 12 + 12 + 1);
    }

    #[test]
    fn test_battery_packet() {
        let packet = build_battery_packet(3.7, 0.85, 5, 0);
        assert_eq!(packet[0..4], [0x00, 0x00, 0x00, 0x0C]);
        assert_eq!(u64::from_be_bytes(packet[4..12].try_into().unwrap()), 5);
        assert_eq!(packet.len(), 12 + 8);
    }

    #[test]
    fn test_packet_roundtrip() {
        let quat = UnitQuaternion::from_euler_angles(0.1, 0.2, 0.3);
        let packet = build_rotation_packet(quat, 42, 1);
        let parsed = SlimeVRPacket::parse(&packet).unwrap();
        assert_eq!(parsed.packet_type, PacketType::Rotation);
        assert_eq!(parsed.counter, 42);
        assert_eq!(parsed.payload[0], 1);
    }

    #[test]
    fn test_parse_invalid_packet() {
        let data = [0xFF; 10];
        assert!(SlimeVRPacket::parse(&data).is_err());
    }

    #[test]
    fn test_parse_mac() {
        assert_eq!(
            parse_mac("3C:38:F4:12:34:56").unwrap(),
            [0x3C, 0x38, 0xF4, 0x12, 0x34, 0x56]
        );
        assert!(parse_mac("not-a-mac").is_err());
    }
}
