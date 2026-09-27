//! Pose-Profile + Auto-Pose (P4): Offsets sind pro Pose isoliert,
//! der Detektor schaltet Stehen<->Sitzen anhand der Gelenkwinkel um.

use std::collections::HashMap;
use tracking_core::{
    CalibrationManager, CalibrationPose, PoseDetector, PoseDetectorConfig, TrackerRole,
};

#[test]
fn pose_offsets_are_isolated() {
    let mgr = CalibrationManager::new();
    let stand = nalgebra::UnitQuaternion::from_euler_angles(0.1, 0.0, 0.0);
    let sit = nalgebra::UnitQuaternion::from_euler_angles(0.0, 0.6, 0.0);
    mgr.set_calibration_for(TrackerRole::Waist, CalibrationPose::Standing, stand, 60);
    mgr.set_calibration_for(TrackerRole::Waist, CalibrationPose::Sitting, sit, 60);

    mgr.set_active_pose(CalibrationPose::Standing);
    assert_eq!(mgr.get_offset(TrackerRole::Waist), stand);
    mgr.set_active_pose(CalibrationPose::Sitting);
    assert_eq!(mgr.get_offset(TrackerRole::Waist), sit);
    // Liegen wurde nie kalibriert -> identity.
    assert_eq!(
        mgr.get_offset_for(TrackerRole::Waist, CalibrationPose::Lying),
        nalgebra::UnitQuaternion::identity()
    );
}

#[test]
fn auto_pose_switches_standing_to_sitting() {
    let mut det = PoseDetector::new(PoseDetectorConfig {
        confirm_frames: 3,
        lie_confirm_frames: 10,
        ..Default::default()
    });
    assert_eq!(det.current().0, CalibrationPose::Standing);
    let mut frame = HashMap::new();
    frame.insert(TrackerRole::Waist, nalgebra::UnitQuaternion::identity());
    frame.insert(
        TrackerRole::LeftUpperLeg,
        nalgebra::UnitQuaternion::from_euler_angles(90f32.to_radians(), 0.0, 0.0),
    );
    frame.insert(
        TrackerRole::RightUpperLeg,
        nalgebra::UnitQuaternion::from_euler_angles(90f32.to_radians(), 0.0, 0.0),
    );
    let mut switched = None;
    for _ in 0..5 {
        switched = det.update(&frame).or(switched);
    }
    assert_eq!(switched.map(|(p, _)| p), Some(CalibrationPose::Sitting));
}

#[test]
fn skeleton_defaults_are_valid() {
    let s = tracking_core::SkeletonConfig::default();
    assert!(s.validate().is_ok());
    assert!(s.upper_leg_cm > 20.0);
}
