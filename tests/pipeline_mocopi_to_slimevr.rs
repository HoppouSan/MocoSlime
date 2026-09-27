//! End-to-end pipeline (platform-independent, Spec §§5–9):
//! raw Mocopi BLE bytes → protocol decoder → tracking core
//! (calibration + filter + role) → SlimeVR UDP bytes → re-parse.
//!
//! Deliberately avoids the Windows-only BLE crate so the pipeline stays
//! testable anywhere; BLE transport is covered by its own memory-backend
//! tests inside `mocopi-ble-windows`.

use mocopi_protocol::{
    decode_packet, DecodedPacket, EXPECTED_IMU_PACKET_SIZE, PACKET_IMU_DATA, QUATERNION_SCALE,
};
use slimevr_protocol::{
    build_acceleration_packet, build_rotation_packet, PacketType, SlimeVRPacket,
};
use tracking_core::{CalibrationManager, TrackerRole, TrackingFilter};

fn identity_imu_frame(counter: u64) -> Vec<u8> {
    let mut data = vec![0u8; EXPECTED_IMU_PACKET_SIZE];
    data[0] = PACKET_IMU_DATA;
    data[1..8].copy_from_slice(&counter.to_le_bytes()[..7]);
    let q = (1.0 / QUATERNION_SCALE) as i16;
    data[8..10].copy_from_slice(&q.to_le_bytes());
    for range in [[10, 12], [12, 14], [14, 16]] {
        data[range[0]..range[1]].copy_from_slice(&0i16.to_le_bytes());
    }
    let zero = half::f16::from_f32(0.0).to_le_bytes();
    data[24..26].copy_from_slice(&zero);
    data[26..28].copy_from_slice(&zero);
    data[28..30].copy_from_slice(&zero);
    data
}

#[test]
fn imu_bytes_flow_to_slimevr_rotation() {
    let raw = identity_imu_frame(78125);
    let decoded = decode_packet(&raw, None).unwrap();
    let DecodedPacket::Imu(packet) = decoded else {
        panic!("expected Imu, got {decoded:?}");
    };
    assert_eq!(packet.counter, 78125);

    let role = TrackerRole::LeftFoot;
    let calibrations = CalibrationManager::new();
    let calibrated = calibrations.apply_calibration(role, packet.quaternion);

    let filter = TrackingFilter::new_default();
    let (quat, accel) = filter
        .process(calibrated, packet.acceleration, packet.timestamp_ns)
        .unwrap();

    let sensor_id = role.slimevr_sensor_id();
    let wire = build_rotation_packet(quat, 7, sensor_id);
    let parsed = SlimeVRPacket::parse(&wire).unwrap();
    assert_eq!(parsed.packet_type, PacketType::Rotation);
    assert_eq!(parsed.counter, 7);
    assert_eq!(parsed.payload[0], sensor_id);
    // Quaternion remap (-x, z, y, w); identity stays identity.
    // Layout: [sensor_id, data_type, x, z, y, w].
    let w = f32::from_be_bytes(parsed.payload[14..18].try_into().unwrap());
    assert!((w - 1.0).abs() < 0.01, "unexpected w: {w}");
    let _ = accel;
}

#[test]
fn acceleration_flows_to_slimevr_packet() {
    let raw = identity_imu_frame(0);
    let DecodedPacket::Imu(packet) = decode_packet(&raw, None).unwrap() else {
        panic!("expected Imu");
    };
    let wire = build_acceleration_packet(packet.acceleration, 9, 13);
    let parsed = SlimeVRPacket::parse(&wire).unwrap();
    assert_eq!(parsed.packet_type, PacketType::Acceleration);
    assert_eq!(parsed.counter, 9);
    assert_eq!(*parsed.payload.last().unwrap(), 13);
}

#[test]
fn unknown_bytes_do_not_break_pipeline() {
    let raw = vec![0xAB; EXPECTED_IMU_PACKET_SIZE];
    match decode_packet(&raw, None).unwrap() {
        DecodedPacket::Unknown { packet_type, .. } => assert_eq!(packet_type, 0xAB),
        other => panic!("expected Unknown, got {other:?}"),
    }
}
