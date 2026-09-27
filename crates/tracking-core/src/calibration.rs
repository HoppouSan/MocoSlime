use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Body posture used for pose detection and position estimation.
/// Actual tracker/body calibration is owned by SlimeVR Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationPose {
    Standing,
    Sitting,
    Lying,
}

impl CalibrationPose {
    pub fn all() -> &'static [CalibrationPose] {
        &[Self::Standing, Self::Sitting, Self::Lying]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Standing => "standing",
            Self::Sitting => "sitting",
            Self::Lying => "lying",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Standing => "Stehen",
            Self::Sitting => "Sitzen",
            Self::Lying => "Liegen",
        }
    }
}

impl FromStr for CalibrationPose {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "standing" | "stehen" => Ok(Self::Standing),
            "sitting" | "sitzen" => Ok(Self::Sitting),
            "lying" | "liegen" => Ok(Self::Lying),
            _ => Err(format!("Unknown body pose: {value}")),
        }
    }
}

impl std::fmt::Display for CalibrationPose {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}
