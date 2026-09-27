use crate::calibration::CalibrationPose;
use crate::error::{Result, TrackingError};
use crate::tracker::TrackerRole;
use nalgebra::{UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Körpermaße in cm. Wird für Plausibilitätschecks und die
/// Sitz/Steh-Heuristik (erwartete Gelenkwinkel) genutzt.
/// SlimeVR selbst rechnet die Forward-Kinematik, wir liefern nur stabile
/// Offsets + erkannte Pose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkeletonConfig {
    pub height_cm: f32,
    pub torso_cm: f32,
    pub upper_leg_cm: f32,
    pub lower_leg_cm: f32,
    pub upper_arm_cm: f32,
    pub lower_arm_cm: f32,
    pub shoulder_cm: f32,
    #[serde(default)]
    pub auto_estimate: bool,
}

impl Default for SkeletonConfig {
    fn default() -> Self {
        Self::from_height(175.0)
    }
}

impl SkeletonConfig {
    /// Grobe Aufteilung aus der Körpergröße (anthropometrische Mittelwerte).
    /// Bein ≈ 48% der Größe, Torso ≈ 30%, Rest Kopf/Hals.
    pub fn from_height(height_cm: f32) -> Self {
        let leg_total = height_cm * 0.48;
        Self {
            height_cm,
            torso_cm: height_cm * 0.30,
            upper_leg_cm: leg_total * 0.52,
            lower_leg_cm: leg_total * 0.48,
            upper_arm_cm: height_cm * 0.17,
            lower_arm_cm: height_cm * 0.16,
            shoulder_cm: height_cm * 0.24,
            auto_estimate: true,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let err = |reason: &str| TrackingError::FilterConfigError {
            reason: reason.to_string(),
        };
        if !(120.0..=230.0).contains(&self.height_cm) {
            return Err(err("height_cm must be 120..230"));
        }
        for (name, v, lo, hi) in [
            ("torso_cm", self.torso_cm, 30.0, 80.0),
            ("upper_leg_cm", self.upper_leg_cm, 25.0, 70.0),
            ("lower_leg_cm", self.lower_leg_cm, 25.0, 70.0),
            ("upper_arm_cm", self.upper_arm_cm, 15.0, 50.0),
            ("lower_arm_cm", self.lower_arm_cm, 15.0, 50.0),
            ("shoulder_cm", self.shoulder_cm, 25.0, 65.0),
        ] {
            if !(lo..=hi).contains(&v) {
                return Err(err(&format!("{name} must be {lo}..{hi} (got {v})")));
            }
        }
        let leg = self.upper_leg_cm + self.lower_leg_cm;
        // Beinlänge muss plausibel zur Größe sein (35..60%).
        if leg < self.height_cm * 0.35 || leg > self.height_cm * 0.60 {
            return Err(err("leg length implausible for height"));
        }
        Ok(())
    }

    /// Nach einer Stehen-Kalibrierung aufrufen: behält manuelle Werte,
    /// füllt nur bei `auto_estimate` aus der Größe nach.
    pub fn refresh_estimates(&mut self) {
        if self.auto_estimate {
            let fresh = Self::from_height(self.height_cm);
            self.torso_cm = fresh.torso_cm;
            self.upper_leg_cm = fresh.upper_leg_cm;
            self.lower_leg_cm = fresh.lower_leg_cm;
            self.upper_arm_cm = fresh.upper_arm_cm;
            self.lower_arm_cm = fresh.lower_arm_cm;
            self.shoulder_cm = fresh.shoulder_cm;
        }
    }

    fn m(&self, cm: f32) -> f32 {
        cm / 100.0
    }

    /// Hüft-Höhe in Metern je Pose (geschätzt, für VRC-Tracking).
    pub fn hip_height_m(&self, pose: CalibrationPose) -> f32 {
        match pose {
            CalibrationPose::Standing => self.m(self.upper_leg_cm + self.lower_leg_cm),
            CalibrationPose::Sitting => (self.m(self.height_cm) * 0.30).clamp(0.40, 0.60),
            CalibrationPose::Lying => 0.20,
        }
    }

    /// Grobe Weltpositionen (Meter) aus kalibrierten Orientierungen.
    /// Rein geschätzt (IMU hat keine Position) – für SlimeVR-Positionsanzeige,
    /// explizit als `estimated` zu labeln. Fehlende Rollen werden aus
    /// Nachbarn approximiert; unbekannt bleibt am Hüft-Ursprung.
    pub fn estimate_positions(
        &self,
        orientations: &HashMap<TrackerRole, UnitQuaternion<f32>>,
        pose: CalibrationPose,
    ) -> HashMap<TrackerRole, Vector3<f32>> {
        let down = Vector3::y_axis();
        let dir = |role: TrackerRole| -> Vector3<f32> {
            orientations
                .get(&role)
                .map(|q| (q * -down).into_inner())
                .unwrap_or_else(|| -down.into_inner())
        };
        let up_of = |role: TrackerRole| -> Vector3<f32> {
            orientations
                .get(&role)
                .map(|q| (q * Vector3::y_axis()).into_inner())
                .unwrap_or_else(|| Vector3::y_axis().into_inner())
        };
        // Rotate left/right joint offsets with the user's heading. This keeps
        // the estimated skeleton aligned when turning in place.
        let lateral_raw = orientations
            .get(&TrackerRole::Waist)
            .map(|q| (q * Vector3::x_axis()).into_inner())
            .unwrap_or_else(|| Vector3::x_axis().into_inner());
        let lateral_flat = Vector3::new(lateral_raw.x, 0.0, lateral_raw.z);
        let lateral = if lateral_flat.norm_squared() > 1e-6 {
            lateral_flat.normalize()
        } else {
            Vector3::x()
        };

        let hip_y = match pose {
            CalibrationPose::Standing => self.hip_height_m(pose),
            CalibrationPose::Sitting => (self.m(self.height_cm) * 0.30).clamp(0.40, 0.60),
            CalibrationPose::Lying => 0.20,
        };
        let hip = Vector3::new(0.0, hip_y, 0.0);
        let waist_up = up_of(TrackerRole::Waist);
        let chest = hip + waist_up * self.m(self.torso_cm * 0.6);
        let head = hip + waist_up * self.m(self.torso_cm + 12.0);

        let mut out = HashMap::new();
        out.insert(TrackerRole::Waist, hip);
        out.insert(TrackerRole::Chest, chest);
        out.insert(TrackerRole::Head, head);

        // Beine: Hüftgelenke ±10cm, Knie = Hüfte + Oberschenkel-Richtung,
        // Fuß = Knie + Unterschenkel-Richtung.
        for (side_upper, side_lower, side_foot, x) in [
            (
                TrackerRole::LeftUpperLeg,
                TrackerRole::LeftLowerLeg,
                TrackerRole::LeftFoot,
                -0.10,
            ),
            (
                TrackerRole::RightUpperLeg,
                TrackerRole::RightLowerLeg,
                TrackerRole::RightFoot,
                0.10,
            ),
        ] {
            let joint = hip + lateral * x;
            out.insert(side_upper, joint);
            let knee = joint + dir(side_upper) * self.m(self.upper_leg_cm);
            let lower_dir: Vector3<f32> = orientations
                .get(&side_lower)
                .map(|q| (q * -down).into_inner())
                .unwrap_or_else(|| dir(side_upper));
            let foot = knee + lower_dir * self.m(self.lower_leg_cm);
            out.insert(side_lower, knee);
            out.insert(side_foot, foot);
        }

        // Arme: Schulter = Chest ± Breite/2, Ellenbogen/Hand per FK.
        let half_shoulder = self.m(self.shoulder_cm) * 0.5;
        for (up_arm, lo_arm, hand, x) in [
            (
                TrackerRole::LeftUpperArm,
                TrackerRole::LeftLowerArm,
                TrackerRole::LeftHand,
                -half_shoulder,
            ),
            (
                TrackerRole::RightUpperArm,
                TrackerRole::RightLowerArm,
                TrackerRole::RightHand,
                half_shoulder,
            ),
        ] {
            let shoulder = chest + lateral * x;
            out.insert(up_arm, shoulder);
            let elbow = shoulder + dir(up_arm) * self.m(self.upper_arm_cm);
            let lower_dir: Vector3<f32> = orientations
                .get(&lo_arm)
                .map(|q| (q * -down).into_inner())
                .unwrap_or_else(|| dir(up_arm));
            let hand_pos = elbow + lower_dir * self.m(self.lower_arm_cm);
            out.insert(lo_arm, elbow);
            out.insert(hand, hand_pos);
        }
        // Keep the whole estimated body above one shared floor plane. Clamping
        // each tracker separately flattens lying poses and breaks limb lengths.
        if let Some(min_y) = out.values().map(|p| p.y).reduce(f32::min) {
            let floor_shift = -min_y.min(0.0);
            if floor_shift > 0.0 {
                for position in out.values_mut() {
                    position.y += floor_shift;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_valid() {
        assert!(SkeletonConfig::default().validate().is_ok());
    }

    #[test]
    fn rejects_absurd_height() {
        let mut s = SkeletonConfig::default();
        s.height_cm = 90.0;
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_implausible_leg() {
        let mut s = SkeletonConfig::default();
        s.upper_leg_cm = 70.0;
        s.lower_leg_cm = 70.0;
        assert!(s.validate().is_err());
    }

    #[test]
    fn from_height_scales() {
        let s = SkeletonConfig::from_height(200.0);
        assert!(s.validate().is_ok());
        assert!(s.upper_leg_cm > SkeletonConfig::from_height(160.0).upper_leg_cm);
    }

    #[test]
    fn estimate_positions_standing_hip_height() {
        use crate::tracker::TrackerRole;
        let s = SkeletonConfig::from_height(175.0);
        let mut o = std::collections::HashMap::new();
        o.insert(TrackerRole::Waist, nalgebra::UnitQuaternion::identity());
        let pos = s.estimate_positions(&o, crate::calibration::CalibrationPose::Standing);
        let hip = pos[&TrackerRole::Waist];
        // Beinlänge ~84cm bei 175cm.
        assert!((hip.y - 0.84).abs() < 0.05, "hip.y={}", hip.y);
        // Sitzen ist niedriger, Liegen am niedrigsten.
        let sit = s.estimate_positions(&o, crate::calibration::CalibrationPose::Sitting);
        let lie = s.estimate_positions(&o, crate::calibration::CalibrationPose::Lying);
        assert!(sit[&TrackerRole::Waist].y < hip.y);
        assert!(lie[&TrackerRole::Waist].y < sit[&TrackerRole::Waist].y);
    }
}
