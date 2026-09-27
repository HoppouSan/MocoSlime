use crate::constants::*;
use crate::error::{ProtocolError, Result};
use nalgebra::{UnitQuaternion, Vector3};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MocopiPacket {
    pub quaternion: UnitQuaternion<f32>,
    pub acceleration: Vector3<f32>,
    pub counter: u64,
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BatteryInfo {
    pub voltage: f32,
    pub percentage: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct QuaternionSerde {
    w: f32,
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Vector3Serde {
    x: f32,
    y: f32,
    z: f32,
}

impl Serialize for MocopiPacket {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("MocopiPacket", 4)?;
        state.serialize_field(
            "quaternion",
            &QuaternionSerde {
                w: self.quaternion.w,
                x: self.quaternion.i,
                y: self.quaternion.j,
                z: self.quaternion.k,
            },
        )?;
        state.serialize_field(
            "acceleration",
            &Vector3Serde {
                x: self.acceleration.x,
                y: self.acceleration.y,
                z: self.acceleration.z,
            },
        )?;
        state.serialize_field("counter", &self.counter)?;
        state.serialize_field("timestamp_ns", &self.timestamp_ns)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for MocopiPacket {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            quaternion: QuaternionSerde,
            acceleration: Vector3Serde,
            counter: u64,
            timestamp_ns: u64,
        }

        let helper = Helper::deserialize(deserializer)?;
        Ok(MocopiPacket {
            quaternion: UnitQuaternion::new_normalize(nalgebra::Quaternion::new(
                helper.quaternion.w,
                helper.quaternion.x,
                helper.quaternion.y,
                helper.quaternion.z,
            )),
            acceleration: Vector3::new(
                helper.acceleration.x,
                helper.acceleration.y,
                helper.acceleration.z,
            ),
            counter: helper.counter,
            timestamp_ns: helper.timestamp_ns,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PacketType {
    ImuData = PACKET_IMU_DATA,
    ImuDataV2 = PACKET_IMU_DATA_V2,
    BatteryResponse = PACKET_BATTERY_RESPONSE,
    StatusResponse = PACKET_STATUS_RESPONSE,
    Unknown = 0xFF,
}

impl From<u8> for PacketType {
    fn from(value: u8) -> Self {
        match value {
            PACKET_IMU_DATA => PacketType::ImuData,
            PACKET_IMU_DATA_V2 => PacketType::ImuDataV2,
            PACKET_BATTERY_RESPONSE => PacketType::BatteryResponse,
            PACKET_STATUS_RESPONSE => PacketType::StatusResponse,
            _ => PacketType::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecodedPacket {
    Imu(MocopiPacket),
    Battery(BatteryInfo),
    Status(Vec<u8>),
    Unknown { packet_type: u8, data: Vec<u8> },
}

impl MocopiPacket {
    pub fn new(quaternion: UnitQuaternion<f32>, acceleration: Vector3<f32>, counter: u64) -> Self {
        Self {
            quaternion,
            acceleration,
            counter,
            timestamp_ns: current_timestamp_ns(),
        }
    }
}

fn current_timestamp_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Squared-norm bounds for a plausible (possibly unnormalized) firmware
/// quaternion. Live captures show ~0.95–1.0; anything far outside is BLE
/// corruption (the protocol has no checksum).
const QUAT_NORM_SQ_MIN: f32 = 0.36;
const QUAT_NORM_SQ_MAX: f32 = 2.25;

/// Decode one quaternion window (4× i16 LE, scale 1/8192) at `offset`.
fn decode_quat_window(data: &[u8], offset: usize) -> Result<UnitQuaternion<f32>> {
    if data.len() < offset + 8 {
        return Err(ProtocolError::InvalidPacketLength {
            expected: offset + 8,
            actual: data.len(),
        });
    }
    let qw = i16::from_le_bytes([data[offset], data[offset + 1]]) as f32 * QUATERNION_SCALE;
    let qx = i16::from_le_bytes([data[offset + 2], data[offset + 3]]) as f32 * QUATERNION_SCALE;
    let qy = i16::from_le_bytes([data[offset + 4], data[offset + 5]]) as f32 * QUATERNION_SCALE;
    let qz = i16::from_le_bytes([data[offset + 6], data[offset + 7]]) as f32 * QUATERNION_SCALE;
    let norm_sq = qw * qw + qx * qx + qy * qy + qz * qz;
    if !(QUAT_NORM_SQ_MIN..=QUAT_NORM_SQ_MAX).contains(&norm_sq) {
        return Err(ProtocolError::Corrupted {
            reason: format!("quaternion norm out of range: {norm_sq:.3}"),
        });
    }
    Ok(UnitQuaternion::new_normalize(nalgebra::Quaternion::new(
        qw, qx, qy, qz,
    )))
}

pub fn parse_imu_packet(
    data: &[u8],
    offset_quat: Option<UnitQuaternion<f32>>,
) -> Result<MocopiPacket> {
    if data.len() < EXPECTED_IMU_PACKET_SIZE {
        return Err(ProtocolError::InvalidPacketLength {
            expected: EXPECTED_IMU_PACKET_SIZE,
            actual: data.len(),
        });
    }

    if data[0] != PACKET_IMU_DATA && data[0] != PACKET_IMU_DATA_V2 {
        return Err(ProtocolError::UnknownPacketType { r#type: data[0] });
    }

    let quat = decode_quat_window(data, 8).or_else(|_| {
        // The stream carries the previous packet's quaternion at bytes
        // 16..24 for loss concealment (verified on-device: a corrupted
        // current window with a valid previous window). Fall back to it
        // instead of dropping the frame.
        decode_quat_window(data, 16)
    })?;

    let ax = half::f16::from_le_bytes([data[24], data[25]]).to_f32() * ACCEL_CORRECTION_FACTOR;
    let az = half::f16::from_le_bytes([data[26], data[27]]).to_f32() * ACCEL_CORRECTION_FACTOR;
    let ay = half::f16::from_le_bytes([data[28], data[29]]).to_f32() * ACCEL_CORRECTION_FACTOR;

    let acceleration = Vector3::new(ax, ay, az);

    let counter = u64::from_le_bytes([
        data[1], data[2], data[3], data[4], data[5], data[6], data[7], 0,
    ]);

    let mut packet = MocopiPacket::new(quat, acceleration, counter);

    if let Some(offset) = offset_quat {
        packet.quaternion = offset * packet.quaternion;
    }

    Ok(packet)
}

/// Parse a battery response frame from the command characteristic.
///
/// Reference Python (`mocoslime_common.calc_batt` + `mocoslime.handleNotification`):
/// the frame arrives on the command-response handle (34) and looks like
/// `7e 07 09 .. .. .. .. <u32 BE raw> ..`, with the raw ADC value at
/// bytes 7..11 big-endian. `EXPECTED_BATTERY_PACKET_SIZE` (12) is the
/// minimum length; longer frames are accepted and extra tail bytes ignored.
pub fn parse_battery_packet(data: &[u8]) -> Result<BatteryInfo> {
    if data.len() < EXPECTED_BATTERY_PACKET_SIZE {
        return Err(ProtocolError::InvalidPacketLength {
            expected: EXPECTED_BATTERY_PACKET_SIZE,
            actual: data.len(),
        });
    }

    // Accept both the raw `0x07` marker and the full `7e 07 09` frame.
    let is_framed = data[0] == 0x7E && data.len() > 2 && data[1] == PACKET_BATTERY_RESPONSE;
    let is_raw = data[0] == PACKET_BATTERY_RESPONSE;
    if !is_framed && !is_raw {
        return Err(ProtocolError::UnknownPacketType { r#type: data[0] });
    }

    let raw_value = u32::from_be_bytes([data[7], data[8], data[9], data[10]]);
    let percentage = (raw_value as f32 - 67410.0) / 6501390.0;
    let voltage = percentage + 3.2;

    Ok(BatteryInfo {
        voltage: voltage.clamp(3.0, 4.2),
        percentage: percentage.clamp(0.0, 1.0),
    })
}

pub fn decode_packet(
    data: &[u8],
    offset_quat: Option<UnitQuaternion<f32>>,
) -> Result<DecodedPacket> {
    if data.is_empty() {
        return Err(ProtocolError::InsufficientData { need: 1, have: 0 });
    }

    // Command-response frames are `7e <type> ...`. Battery frames are
    // `7e 07 09 ...` (see parse_battery_packet).
    if data[0] == 0x7E && data.len() > 2 {
        match data[1] {
            PACKET_BATTERY_RESPONSE => {
                let battery = parse_battery_packet(data)?;
                return Ok(DecodedPacket::Battery(battery));
            }
            PACKET_STATUS_RESPONSE => return Ok(DecodedPacket::Status(data.to_vec())),
            _ => {}
        }
    }

    let packet_type = PacketType::from(data[0]);

    match packet_type {
        PacketType::ImuData | PacketType::ImuDataV2 => {
            let packet = parse_imu_packet(data, offset_quat)?;
            Ok(DecodedPacket::Imu(packet))
        }
        PacketType::BatteryResponse => {
            let battery = parse_battery_packet(data)?;
            Ok(DecodedPacket::Battery(battery))
        }
        PacketType::StatusResponse => Ok(DecodedPacket::Status(data.to_vec())),
        PacketType::Unknown => Ok(DecodedPacket::Unknown {
            packet_type: data[0],
            data: data.to_vec(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_imu_packet_valid() {
        let mut data = vec![0u8; EXPECTED_IMU_PACKET_SIZE];
        data[0] = PACKET_IMU_DATA;
        data[1..8].copy_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        let quat_val = (1.0 / QUATERNION_SCALE) as i16;
        data[8..10].copy_from_slice(&quat_val.to_le_bytes());
        data[10..12].copy_from_slice(&0i16.to_le_bytes());
        data[12..14].copy_from_slice(&0i16.to_le_bytes());
        data[14..16].copy_from_slice(&0i16.to_le_bytes());

        let accel_val = half::f16::from_f32(0.0).to_le_bytes();
        data[24..26].copy_from_slice(&accel_val);
        data[26..28].copy_from_slice(&accel_val);
        data[28..30].copy_from_slice(&accel_val);

        let result = parse_imu_packet(&data, None);
        assert!(result.is_ok());
        let packet = result.unwrap();
        assert_eq!(packet.counter, 0);
        assert!((packet.quaternion.w - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_imu_data_v2_marker() {
        // 36-byte 0x30 frame (community RE layout): counter [1..8],
        // quat [8..16], prev quat [16..24], accel f16 [24..30].
        let mut data = vec![0u8; 36];
        data[0] = PACKET_IMU_DATA_V2;
        data[1..8].copy_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let w = (0.99 / QUATERNION_SCALE) as i16;
        data[8..10].copy_from_slice(&w.to_le_bytes());
        data[10..12].copy_from_slice(&0i16.to_le_bytes());
        data[12..14].copy_from_slice(&0i16.to_le_bytes());
        data[14..16].copy_from_slice(&0i16.to_le_bytes());
        let accel = half::f16::from_f32(1.0).to_le_bytes();
        data[24..26].copy_from_slice(&accel);
        data[26..28].copy_from_slice(&accel);
        data[28..30].copy_from_slice(&accel);

        let packet = parse_imu_packet(&data, None).unwrap();
        assert_eq!(packet.counter, 1);
        assert!((packet.quaternion.w - 0.99).abs() < 0.01);
        assert!(matches!(
            decode_packet(&data, None).unwrap(),
            DecodedPacket::Imu(_)
        ));
    }

    #[test]
    fn test_imu_falls_back_to_previous_quat() {
        // Corrupt current window (way off unit norm), valid previous.
        let mut data = vec![0u8; 36];
        data[0] = PACKET_IMU_DATA_V2;
        data[8..10].copy_from_slice(&30000i16.to_le_bytes());
        let w = (1.0 / QUATERNION_SCALE) as i16;
        data[16..18].copy_from_slice(&w.to_le_bytes());

        let packet = parse_imu_packet(&data, None).unwrap();
        assert!((packet.quaternion.w - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_imu_v2_live_capture_decodes() {
        // Real 36-byte 0x30 frame captured from a QM-SS1 tracker.
        let data: Vec<u8> = vec![
            0x30, 0xb9, 0x9d, 0x7a, 0x81, 0x4d, 0x23, 0x0d, 0x35, 0x14, 0x76, 0xe7, 0xfd, 0xff,
            0x27, 0x03, 0x4f, 0x14, 0x8b, 0xe7, 0xff, 0xff, 0x23, 0x03, 0x22, 0x17, 0x24, 0xb7,
            0x9f, 0xb3, 0x45, 0xac, 0x04, 0xb3, 0x7c, 0xb8,
        ];
        let packet = parse_imu_packet(&data, None).unwrap();
        assert_eq!(packet.counter, 0x0d234d817a9db9);
        let norm = (packet.quaternion.w.powi(2)
            + packet.quaternion.i.powi(2)
            + packet.quaternion.j.powi(2)
            + packet.quaternion.k.powi(2))
        .sqrt();
        assert!((norm - 1.0).abs() < 0.001, "quaternion not unit: {norm}");
        // Raw (0.6316, -0.7668, ~0, 0.0985), normalized.
        assert!((packet.quaternion.w - 0.633).abs() < 0.01);
        assert!(packet.quaternion.i < 0.0);
        assert!(matches!(
            decode_packet(&data, None).unwrap(),
            DecodedPacket::Imu(_)
        ));
    }

    #[test]
    fn test_imu_both_quats_corrupt_is_error() {
        let mut data = vec![0u8; 36];
        data[0] = PACKET_IMU_DATA_V2;
        // Zero quats in both windows: norm 0 -> invalid.
        let result = parse_imu_packet(&data, None);
        assert!(matches!(result, Err(ProtocolError::Corrupted { .. })));
    }

    #[test]
    fn test_parse_imu_packet_too_short() {
        let data = vec![PACKET_IMU_DATA; 10];
        let result = parse_imu_packet(&data, None);
        assert!(matches!(
            result,
            Err(ProtocolError::InvalidPacketLength { .. })
        ));
    }

    #[test]
    fn test_parse_imu_packet_wrong_type() {
        let data = vec![0xFF; EXPECTED_IMU_PACKET_SIZE];
        let result = parse_imu_packet(&data, None);
        assert!(matches!(
            result,
            Err(ProtocolError::UnknownPacketType { .. })
        ));
    }

    #[test]
    fn test_parse_battery_packet_valid() {
        let mut data = vec![0u8; EXPECTED_BATTERY_PACKET_SIZE];
        data[0] = PACKET_BATTERY_RESPONSE;
        let raw_val = (67410.0 + 6501390.0 * 0.5) as u32;
        data[7..11].copy_from_slice(&raw_val.to_be_bytes());

        let result = parse_battery_packet(&data);
        assert!(result.is_ok());
        let battery = result.unwrap();
        assert!((battery.percentage - 0.5).abs() < 0.01);
        assert!((battery.voltage - 3.7).abs() < 0.01);
    }

    #[test]
    fn test_parse_battery_packet_framed() {
        // Real on-wire frame: 7e 07 09 ... + u32 BE at [7..11].
        let mut data = vec![0u8; EXPECTED_BATTERY_PACKET_SIZE];
        data[0] = 0x7E;
        data[1] = PACKET_BATTERY_RESPONSE;
        data[2] = 0x09;
        let raw_val = (67410.0 + 6501390.0 * 0.5) as u32;
        data[7..11].copy_from_slice(&raw_val.to_be_bytes());
        let battery = parse_battery_packet(&data).unwrap();
        assert!((battery.percentage - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_battery_packet_clamping() {
        let mut data = vec![0u8; EXPECTED_BATTERY_PACKET_SIZE];
        data[0] = PACKET_BATTERY_RESPONSE;
        let raw_val = (67410.0 + 6501390.0 * 1.5) as u32;
        data[7..11].copy_from_slice(&raw_val.to_be_bytes());

        let result = parse_battery_packet(&data);
        assert!(result.is_ok());
        let battery = result.unwrap();
        assert_eq!(battery.percentage, 1.0);
        assert_eq!(battery.voltage, 4.2);
    }

    #[test]
    fn test_decode_packet_imu() {
        let mut data = vec![0u8; EXPECTED_IMU_PACKET_SIZE];
        data[0] = PACKET_IMU_DATA;
        data[1..8].copy_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        let quat_val = (1.0 / QUATERNION_SCALE) as i16;
        data[8..10].copy_from_slice(&quat_val.to_le_bytes());
        data[10..12].copy_from_slice(&0i16.to_le_bytes());
        data[12..14].copy_from_slice(&0i16.to_le_bytes());
        data[14..16].copy_from_slice(&0i16.to_le_bytes());

        let accel_val = half::f16::from_f32(0.0).to_le_bytes();
        data[24..26].copy_from_slice(&accel_val);
        data[26..28].copy_from_slice(&accel_val);
        data[28..30].copy_from_slice(&accel_val);

        let result = decode_packet(&data, None);
        assert!(matches!(result, Ok(DecodedPacket::Imu(_))));
    }

    #[test]
    fn test_decode_packet_unknown_type() {
        let data = vec![0xAB; EXPECTED_IMU_PACKET_SIZE];
        let result = decode_packet(&data, None).unwrap();
        match result {
            DecodedPacket::Unknown {
                packet_type,
                data: bytes,
            } => {
                assert_eq!(packet_type, 0xAB);
                assert_eq!(bytes.len(), EXPECTED_IMU_PACKET_SIZE);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn test_decode_packet_empty_is_error() {
        let result = decode_packet(&[], None);
        assert!(matches!(
            result,
            Err(ProtocolError::InsufficientData { .. })
        ));
    }

    #[test]
    fn test_corrupted_quaternion_still_normalized() {
        // Saturated i16 on all components (implausible sensor dump,
        // e.g. BLE corruption — the protocol has no checksum); the
        // decoder falls back to the previous packet's quaternion and
        // must still yield a unit quaternion, never NaN.
        let mut data = vec![0u8; EXPECTED_IMU_PACKET_SIZE];
        data[0] = PACKET_IMU_DATA;
        data[1] = 1;
        for range in [[8, 10], [10, 12], [12, 14], [14, 16]] {
            data[range[0]..range[1]].copy_from_slice(&i16::MAX.to_le_bytes());
        }
        let w = (1.0 / QUATERNION_SCALE) as i16;
        data[16..18].copy_from_slice(&w.to_le_bytes());
        let accel_val = half::f16::from_f32(1.0).to_le_bytes();
        data[24..26].copy_from_slice(&accel_val);
        data[26..28].copy_from_slice(&accel_val);
        data[28..30].copy_from_slice(&accel_val);
        let packet = parse_imu_packet(&data, None).unwrap();
        for v in [
            packet.quaternion.w,
            packet.quaternion.i,
            packet.quaternion.j,
            packet.quaternion.k,
        ] {
            assert!(v.is_finite(), "non-finite quaternion component: {v}");
        }
        let norm = (packet.quaternion.w.powi(2)
            + packet.quaternion.i.powi(2)
            + packet.quaternion.j.powi(2)
            + packet.quaternion.k.powi(2))
        .sqrt();
        assert!(
            (norm - 1.0).abs() < 0.01,
            "quaternion not normalized: {norm}"
        );
    }

    #[test]
    fn test_corrupted_battery_too_short() {
        let data = vec![0x7E, 0x07, 0x09];
        assert!(matches!(
            parse_battery_packet(&data),
            Err(ProtocolError::InvalidPacketLength { .. })
        ));
    }
}
