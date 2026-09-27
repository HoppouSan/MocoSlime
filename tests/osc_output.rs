//! OSC output vectors (Spec §§26–27, 31–32): addresses, bundles,
//! presets and input parsing, independent of BLE/GUI.

use osc_protocol::{
    address::TrackerNaming, bundle, input, message, presets::OscPreset, InputCommand, OscPacket,
};

#[test]
fn tracker_paths_cover_both_naming_modes() {
    let rot = message::rotation(TrackerNaming::Id, 0, 2, 0.0, 0.0, 0.0, 1.0);
    assert_eq!(rot.addr, "/moslime/tracker/0/rotation");
    let rot = message::rotation(TrackerNaming::Role, 0, 13, 0.0, 0.0, 0.0, 1.0);
    assert_eq!(rot.addr, "/moslime/tracker/left_foot/rotation");
}

#[test]
fn bundle_groups_frame_messages() {
    let msgs = vec![
        message::rotation(TrackerNaming::Role, 0, 2, 0.0, 0.0, 0.0, 1.0),
        message::acceleration(TrackerNaming::Role, 0, 2, 0.0, 0.0, 9.81),
        message::status(TrackerNaming::Role, 0, 2, "streaming"),
    ];
    let bytes = bundle::encode(&bundle::to_bundle(msgs)).unwrap();
    match bundle::decode_udp(&bytes).unwrap() {
        OscPacket::Bundle(b) => assert_eq!(b.content.len(), 3),
        other => panic!("expected bundle, got {other:?}"),
    }
}

#[test]
fn vrchat_preset_params() {
    assert_eq!(
        osc_protocol::vrchat_connected_param(13),
        "/avatar/parameters/MocopiLeftFootConnected"
    );
    assert!(OscPreset::all().contains(&OscPreset::VRChat));
    assert_eq!(OscPreset::VRChat.default_rate_hz(), Some(60));
}

#[test]
fn vrchat_tracking_addrs() {
    use osc_protocol::{position_addr, rotation_addr, VrcTracker};
    assert_eq!(
        position_addr(VrcTracker::Num(1)),
        "/tracking/trackers/1/position"
    );
    assert_eq!(
        rotation_addr(VrcTracker::Head),
        "/tracking/trackers/head/rotation"
    );
    assert_eq!(
        osc_protocol::vrc_tracker_for_sensor(2),
        Some(VrcTracker::Num(1))
    );
    assert_eq!(osc_protocol::vrc_tracker_for_sensor(7), None);
}

#[test]
fn test_hello_message_shape() {
    let msg = message::test_hello();
    assert_eq!(msg.addr, "/moslime/test");
    let bytes = bundle::encode(&OscPacket::Message(msg)).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn input_module_accepts_control_commands() {
    use osc_protocol::{OscMessage, OscPacket};
    for (addr, expected) in [
        ("/moslime/start", InputCommand::Start),
        ("/moslime/stop", InputCommand::Stop),
        ("/moslime/calibrate", InputCommand::CalibrateAll),
    ] {
        let packet = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![],
        });
        assert_eq!(input::parse_command(&packet).unwrap(), Some(expected));
    }
}
