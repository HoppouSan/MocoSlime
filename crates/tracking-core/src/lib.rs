pub mod calibration;
pub mod error;
pub mod filter;
pub mod nalgebra_serde;
pub mod pose_detector;
pub mod skeleton;
pub mod tracker;

pub use calibration::CalibrationPose;
pub use error::{Result, TrackingError};
pub use filter::{predict_orientation, FilterConfig, FilterStats, TrackingFilter};
pub use nalgebra_serde::QuaternionSerde;
pub use pose_detector::{PoseDetector, PoseDetectorConfig, PoseDetectorState};
pub use skeleton::SkeletonConfig;
pub use tracker::{mount_flip_correction, CoordinateSystem, TrackerPose, TrackerRole};
