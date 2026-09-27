pub mod error;
pub mod packet;

pub use error::{Result, SlimeVRError};
pub use packet::{
    build_acceleration_packet, build_battery_packet, build_error_packet, build_handshake_packet,
    build_ping_packet, build_rotation_packet, build_sensor_info_packet, PacketType, SlimeVRPacket,
};
