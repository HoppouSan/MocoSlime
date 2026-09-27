use crate::nalgebra_serde::{QuaternionSerde, Vector3Serde};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Body role. Serde names use the human-readable GUI labels
/// ("Left Foot", not "LeftFoot") so Flutter `allRoles` round-trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TrackerRole {
    Head = 0,
    Chest = 1,
    Waist = 2,
    #[serde(rename = "Left Upper Arm")]
    LeftUpperArm = 3,
    #[serde(rename = "Right Upper Arm")]
    RightUpperArm = 4,
    #[serde(rename = "Left Lower Arm")]
    LeftLowerArm = 5,
    #[serde(rename = "Right Lower Arm")]
    RightLowerArm = 6,
    #[serde(rename = "Left Hand")]
    LeftHand = 7,
    #[serde(rename = "Right Hand")]
    RightHand = 8,
    #[serde(rename = "Left Upper Leg")]
    LeftUpperLeg = 9,
    #[serde(rename = "Right Upper Leg")]
    RightUpperLeg = 10,
    #[serde(rename = "Left Lower Leg")]
    LeftLowerLeg = 11,
    #[serde(rename = "Right Lower Leg")]
    RightLowerLeg = 12,
    #[serde(rename = "Left Foot")]
    LeftFoot = 13,
    #[serde(rename = "Right Foot")]
    RightFoot = 14,
}

impl TrackerRole {
    pub fn all() -> &'static [TrackerRole] {
        &[
            TrackerRole::Head,
            TrackerRole::Chest,
            TrackerRole::Waist,
            TrackerRole::LeftUpperArm,
            TrackerRole::RightUpperArm,
            TrackerRole::LeftLowerArm,
            TrackerRole::RightLowerArm,
            TrackerRole::LeftHand,
            TrackerRole::RightHand,
            TrackerRole::LeftUpperLeg,
            TrackerRole::RightUpperLeg,
            TrackerRole::LeftLowerLeg,
            TrackerRole::RightLowerLeg,
            TrackerRole::LeftFoot,
            TrackerRole::RightFoot,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            TrackerRole::Head => "Head",
            TrackerRole::Chest => "Chest",
            TrackerRole::Waist => "Waist",
            TrackerRole::LeftUpperArm => "Left Upper Arm",
            TrackerRole::RightUpperArm => "Right Upper Arm",
            TrackerRole::LeftLowerArm => "Left Lower Arm",
            TrackerRole::RightLowerArm => "Right Lower Arm",
            TrackerRole::LeftHand => "Left Hand",
            TrackerRole::RightHand => "Right Hand",
            TrackerRole::LeftUpperLeg => "Left Upper Leg",
            TrackerRole::RightUpperLeg => "Right Upper Leg",
            TrackerRole::LeftLowerLeg => "Left Lower Leg",
            TrackerRole::RightLowerLeg => "Right Lower Leg",
            TrackerRole::LeftFoot => "Left Foot",
            TrackerRole::RightFoot => "Right Foot",
        }
    }

    pub fn slimevr_sensor_id(&self) -> u8 {
        *self as u8
    }

    pub fn is_left_side(&self) -> bool {
        matches!(
            self,
            TrackerRole::LeftUpperArm
                | TrackerRole::LeftLowerArm
                | TrackerRole::LeftHand
                | TrackerRole::LeftUpperLeg
                | TrackerRole::LeftLowerLeg
                | TrackerRole::LeftFoot
        )
    }

    pub fn is_right_side(&self) -> bool {
        matches!(
            self,
            TrackerRole::RightUpperArm
                | TrackerRole::RightLowerArm
                | TrackerRole::RightHand
                | TrackerRole::RightUpperLeg
                | TrackerRole::RightLowerLeg
                | TrackerRole::RightFoot
        )
    }

    pub fn mirror(&self) -> Option<TrackerRole> {
        match self {
            TrackerRole::LeftUpperArm => Some(TrackerRole::RightUpperArm),
            TrackerRole::RightUpperArm => Some(TrackerRole::LeftUpperArm),
            TrackerRole::LeftLowerArm => Some(TrackerRole::RightLowerArm),
            TrackerRole::RightLowerArm => Some(TrackerRole::LeftLowerArm),
            TrackerRole::LeftHand => Some(TrackerRole::RightHand),
            TrackerRole::RightHand => Some(TrackerRole::LeftHand),
            TrackerRole::LeftUpperLeg => Some(TrackerRole::RightUpperLeg),
            TrackerRole::RightUpperLeg => Some(TrackerRole::LeftUpperLeg),
            TrackerRole::LeftLowerLeg => Some(TrackerRole::RightLowerLeg),
            TrackerRole::RightLowerLeg => Some(TrackerRole::LeftLowerLeg),
            TrackerRole::LeftFoot => Some(TrackerRole::RightFoot),
            TrackerRole::RightFoot => Some(TrackerRole::LeftFoot),
            _ => None,
        }
    }
}

/// Mount-Flip-Korrektur (T3): 180° Yaw für verdreht montierte Sensoren.
/// Wird vor Kalibrierung/Filter auf die Roh-Rotation angewendet.
pub fn mount_flip_correction() -> nalgebra::UnitQuaternion<f32> {
    nalgebra::UnitQuaternion::from_euler_angles(0.0, std::f32::consts::PI, 0.0)
}

impl fmt::Display for TrackerRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::str::FromStr for TrackerRole {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        for role in TrackerRole::all() {
            if role.name().eq_ignore_ascii_case(s) {
                return Ok(*role);
            }
        }
        Err(format!("Unknown tracker role: {}", s))
    }
}

impl TryFrom<u8> for TrackerRole {
    type Error = ();

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        for role in TrackerRole::all() {
            if *role as u8 == value {
                return Ok(*role);
            }
        }
        Err(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinateSystem {
    Mocopi,
    SlimeVR,
    OpenVR,
    Unity,
}

impl CoordinateSystem {
    /// Feste Basis-Rotationen ab Mocopi (per Referenz vermessen, T3-Golden-Tests).
    fn from_mocopi(&self) -> nalgebra::UnitQuaternion<f32> {
        use nalgebra::UnitQuaternion;
        match self {
            CoordinateSystem::Mocopi => UnitQuaternion::identity(),
            // Aktuell identity (Referenz-Messung steht aus – Golden-Test pinnt das).
            CoordinateSystem::SlimeVR => UnitQuaternion::identity(),
            CoordinateSystem::OpenVR => {
                UnitQuaternion::from_euler_angles(0.0, std::f32::consts::FRAC_PI_2, 0.0)
            }
            CoordinateSystem::Unity => {
                UnitQuaternion::from_euler_angles(-std::f32::consts::FRAC_PI_2, 0.0, 0.0)
            }
        }
    }

    pub fn conversion_to(&self, target: CoordinateSystem) -> nalgebra::UnitQuaternion<f32> {
        // A→B = (Mocopi→B) * (Mocopi→A)⁻¹. Keine Rekursion, immer terminierend.
        target.from_mocopi() * self.from_mocopi().inverse()
    }

    /// GUI-/Config-Name → System ("SlimeVR", "Mocopi", "OpenVR", "Unity").
    /// Unbekannt fällt auf SlimeVR zurück (Default-Verhalten, nie Fehler).
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "mocopi" => CoordinateSystem::Mocopi,
            "openvr" => CoordinateSystem::OpenVR,
            "unity" => CoordinateSystem::Unity,
            _ => CoordinateSystem::SlimeVR,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackerPose {
    pub role: TrackerRole,
    pub rotation: nalgebra::UnitQuaternion<f32>,
    pub acceleration: nalgebra::Vector3<f32>,
    pub timestamp_ns: u64,
    pub confidence: f32,
}

impl TrackerPose {
    pub fn new(
        role: TrackerRole,
        rotation: nalgebra::UnitQuaternion<f32>,
        acceleration: nalgebra::Vector3<f32>,
        timestamp_ns: u64,
    ) -> Self {
        Self {
            role,
            rotation,
            acceleration,
            timestamp_ns,
            confidence: 1.0,
        }
    }

    pub fn to_coordinate_system(&self, target: CoordinateSystem) -> Self {
        let conversion = CoordinateSystem::Mocopi.conversion_to(target);
        Self {
            rotation: conversion * self.rotation,
            acceleration: conversion * self.acceleration,
            ..*self
        }
    }
}

impl Serialize for TrackerPose {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("TrackerPose", 5)?;
        state.serialize_field("role", &self.role)?;
        state.serialize_field("rotation", &QuaternionSerde::from(self.rotation))?;
        state.serialize_field("acceleration", &Vector3Serde::from(self.acceleration))?;
        state.serialize_field("timestamp_ns", &self.timestamp_ns)?;
        state.serialize_field("confidence", &self.confidence)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for TrackerPose {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            role: TrackerRole,
            rotation: QuaternionSerde,
            acceleration: Vector3Serde,
            timestamp_ns: u64,
            confidence: f32,
        }
        let helper = Helper::deserialize(deserializer)?;
        Ok(TrackerPose {
            role: helper.role,
            rotation: helper.rotation.into(),
            acceleration: helper.acceleration.into(),
            timestamp_ns: helper.timestamp_ns,
            confidence: helper.confidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::UnitQuaternion;

    #[test]
    fn test_tracker_role_all() {
        assert_eq!(TrackerRole::all().len(), 15);
    }

    #[test]
    fn test_tracker_role_mirror() {
        assert_eq!(TrackerRole::LeftHand.mirror(), Some(TrackerRole::RightHand));
        assert_eq!(TrackerRole::RightFoot.mirror(), Some(TrackerRole::LeftFoot));
        assert_eq!(TrackerRole::Head.mirror(), None);
    }

    #[test]
    fn test_tracker_role_from_str() {
        assert_eq!("Head".parse::<TrackerRole>().unwrap(), TrackerRole::Head);
        assert_eq!(
            "left foot".parse::<TrackerRole>().unwrap(),
            TrackerRole::LeftFoot
        );
        assert!("Invalid".parse::<TrackerRole>().is_err());
    }

    #[test]
    fn test_coordinate_system_conversion() {
        let quat = UnitQuaternion::identity();
        let converted = CoordinateSystem::Mocopi.conversion_to(CoordinateSystem::SlimeVR) * quat;
        assert_eq!(converted, UnitQuaternion::identity());
    }

    #[test]
    fn test_coordinate_roundtrip() {
        // T3-Golden: A→B→A muss identity sein (keine Rekursion, kein Drift).
        use nalgebra::UnitQuaternion;
        let q = UnitQuaternion::from_euler_angles(0.3, -0.5, 0.7);
        for (a, b) in [
            (CoordinateSystem::Mocopi, CoordinateSystem::SlimeVR),
            (CoordinateSystem::Mocopi, CoordinateSystem::OpenVR),
            (CoordinateSystem::Mocopi, CoordinateSystem::Unity),
            (CoordinateSystem::SlimeVR, CoordinateSystem::Mocopi),
            (CoordinateSystem::OpenVR, CoordinateSystem::Unity),
            (CoordinateSystem::SlimeVR, CoordinateSystem::SlimeVR),
        ] {
            let there = a.conversion_to(b) * q;
            let back = b.conversion_to(a) * there;
            assert!(
                back.angle_to(&q) < 1e-4,
                "{a:?}->{b:?} roundtrip driftete: {}",
                back.angle_to(&q)
            );
        }
    }

    #[test]
    fn test_mount_flip_is_180_yaw() {
        let flip = mount_flip_correction();
        let yaw = flip.angle_to(&UnitQuaternion::identity());
        assert!((yaw - std::f32::consts::PI).abs() < 1e-4, "yaw={yaw}");
        // Doppel-Flip = identity.
        let twice = flip * flip;
        assert!(twice.angle_to(&UnitQuaternion::identity()) < 1e-4);
    }

    #[test]
    fn test_tracker_pose_conversion() {
        let pose = TrackerPose::new(
            TrackerRole::Head,
            UnitQuaternion::identity(),
            nalgebra::Vector3::zeros(),
            0,
        );
        let converted = pose.to_coordinate_system(CoordinateSystem::SlimeVR);
        assert_eq!(converted.rotation, UnitQuaternion::identity());
    }
}
