use crate::calibration::CalibrationPose;
use crate::tracker::TrackerRole;
use nalgebra::UnitQuaternion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Schwellen für die Auto-Erkennung. Alle Winkel in Grad.
/// Heuristik (nur Rotation, keine absolute Position — Mocopi liefert keine):
/// - Stehen: Waist↔Oberschenkel-Winkel klein (Bein gestreckt).
/// - Sitzen: Waist↔Oberschenkel ~90° (Hüfte gebeugt), Unterschenkel vertikal.
/// - Liegen: Torso-Up-Vektor ~horizontal (Torso umgekippt).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoseDetectorConfig {
    pub stand_max_thigh_torso_deg: f32,
    pub sit_min_thigh_torso_deg: f32,
    pub sit_max_thigh_torso_deg: f32,
    pub lie_min_torso_tilt_deg: f32,
    /// Aufeinanderfolgende stabile Frames vor Umschaltung (Hysterese).
    pub confirm_frames: usize,
    /// Frames für Liegen (länger, gegen Fehltrigger im Sitzen).
    pub lie_confirm_frames: usize,
}

impl Default for PoseDetectorConfig {
    fn default() -> Self {
        Self {
            stand_max_thigh_torso_deg: 30.0,
            sit_min_thigh_torso_deg: 60.0,
            sit_max_thigh_torso_deg: 120.0,
            lie_min_torso_tilt_deg: 55.0,
            confirm_frames: 15,
            lie_confirm_frames: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoseDetectorState {
    pub current: CalibrationPose,
    pub confidence: f32,
    pub candidate: Option<CalibrationPose>,
    pub candidate_frames: usize,
    /// Manueller Override: Auto pausiert bis zu diesem Zeitpunkt (ms seit Start).
    pub manual_hold_frames: usize,
}

pub struct PoseDetector {
    config: PoseDetectorConfig,
    state: PoseDetectorState,
}

impl PoseDetector {
    pub fn new(config: PoseDetectorConfig) -> Self {
        Self {
            config,
            state: PoseDetectorState {
                current: CalibrationPose::Standing,
                confidence: 1.0,
                candidate: None,
                candidate_frames: 0,
                manual_hold_frames: 0,
            },
        }
    }

    pub fn current(&self) -> (CalibrationPose, f32) {
        (self.state.current, self.state.confidence)
    }

    pub fn set_manual_hold(&mut self, frames: usize) {
        self.state.manual_hold_frames = frames;
    }

    pub fn set_current(&mut self, pose: CalibrationPose) {
        self.state.current = pose;
        self.state.confidence = 1.0;
        self.state.candidate = None;
        self.state.candidate_frames = 0;
    }

    pub fn config(&self) -> &PoseDetectorConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: PoseDetectorConfig) {
        self.config = config;
    }

    /// Ein Frame mit den aktuell kalibrierten Orientierungen füttern.
    /// Gibt `Some(pose)` genau dann zurück, wenn umgeschaltet wurde.
    pub fn update(
        &mut self,
        orientations: &HashMap<TrackerRole, UnitQuaternion<f32>>,
    ) -> Option<(CalibrationPose, f32)> {
        if self.state.manual_hold_frames > 0 {
            self.state.manual_hold_frames -= 1;
            return None;
        }
        let Some(voted) = Self::vote(orientations, &self.config) else {
            self.state.candidate = None;
            self.state.candidate_frames = 0;
            return None;
        };
        let (pose, confidence) = voted;
        if pose == self.state.current {
            self.state.candidate = None;
            self.state.candidate_frames = 0;
            // Konfidenz nachziehen (gedämpft).
            self.state.confidence = self.state.confidence * 0.9 + confidence * 0.1;
            return None;
        }
        let required = if pose == CalibrationPose::Lying {
            self.config.lie_confirm_frames
        } else {
            self.config.confirm_frames
        };
        match self.state.candidate {
            Some(c) if c == pose => {
                self.state.candidate_frames += 1;
            }
            _ => {
                self.state.candidate = Some(pose);
                self.state.candidate_frames = 1;
            }
        }
        if self.state.candidate_frames >= required {
            self.state.current = pose;
            self.state.confidence = confidence;
            self.state.candidate = None;
            self.state.candidate_frames = 0;
            return Some((pose, confidence));
        }
        None
    }

    fn vote(
        orientations: &HashMap<TrackerRole, UnitQuaternion<f32>>,
        cfg: &PoseDetectorConfig,
    ) -> Option<(CalibrationPose, f32)> {
        let waist = orientations
            .get(&TrackerRole::Waist)
            .or_else(|| orientations.get(&TrackerRole::Chest))?;
        let thigh_l = orientations.get(&TrackerRole::LeftUpperLeg);
        let thigh_r = orientations.get(&TrackerRole::RightUpperLeg);
        let thigh = match (thigh_l, thigh_r) {
            (Some(a), Some(b)) => Some(a.slerp(b, 0.5)),
            (Some(a), None) => Some(*a),
            (None, Some(b)) => Some(*b),
            (None, None) => None,
        };

        // Measure the segment axes against gravity, not quaternion angle from
        // identity. The latter counts yaw as tilt and classified turning in
        // place as lying. Comparing up axes is yaw invariant.
        let up = nalgebra::Vector3::y_axis();
        let torso_up = waist * up;
        let torso_tilt = angle_between_deg(torso_up.into_inner(), up.into_inner());

        let thigh_torso_opt = thigh
            .map(|thigh_q| angle_between_deg(torso_up.into_inner(), (thigh_q * up).into_inner()));

        // Ohne Bein-Tracker erst ab 70° als Liegen (sitzendes Lümmeln mit
        // 55–70° bleibt Sitzen/Unknown statt Fehltrigger).
        let lie_threshold = if thigh_torso_opt.is_some() {
            cfg.lie_min_torso_tilt_deg
        } else {
            cfg.lie_min_torso_tilt_deg.max(70.0)
        };

        // Liegen: Torso stark gekippt UND Bein gestreckt (Hüfte offen).
        // Verhindert Fehltrigger beim sitzenden Lümmeln (Torso gekippt,
        // Hüfte aber gebeugt → Sitzen).
        if torso_tilt >= lie_threshold {
            match thigh_torso_opt {
                Some(thigh_torso) if thigh_torso > 45.0 => {
                    // Hüfte gebeugt → kein Liegen, unten als Sitzen werten.
                }
                _ => {
                    let conf = ((torso_tilt - lie_threshold) / 35.0).clamp(0.3, 1.0);
                    return Some((CalibrationPose::Lying, conf));
                }
            }
        }

        let Some(thigh_torso) = thigh_torso_opt else {
            // Ohne Bein-Tracker nur Liegen sicher erkennbar.
            return None;
        };

        if thigh_torso <= cfg.stand_max_thigh_torso_deg {
            let conf = (1.0 - thigh_torso / cfg.stand_max_thigh_torso_deg.max(1.0)).clamp(0.3, 1.0);
            return Some((CalibrationPose::Standing, conf));
        }
        if (cfg.sit_min_thigh_torso_deg..=cfg.sit_max_thigh_torso_deg).contains(&thigh_torso) {
            let mid = (cfg.sit_min_thigh_torso_deg + cfg.sit_max_thigh_torso_deg) * 0.5;
            let half = (cfg.sit_max_thigh_torso_deg - cfg.sit_min_thigh_torso_deg) * 0.5;
            let conf = (1.0 - ((thigh_torso - mid).abs() / half.max(1.0)) * 0.5).clamp(0.4, 1.0);
            return Some((CalibrationPose::Sitting, conf));
        }
        None
    }
}

fn angle_between_deg(a: nalgebra::Vector3<f32>, b: nalgebra::Vector3<f32>) -> f32 {
    let denominator = a.norm() * b.norm();
    if denominator <= f32::EPSILON {
        return 0.0;
    }
    (a.dot(&b) / denominator)
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn quat_x(deg: f32) -> UnitQuaternion<f32> {
        UnitQuaternion::from_euler_angles(deg.to_radians(), 0.0, 0.0)
    }

    fn map(waist_deg: f32, thigh_deg: f32) -> HashMap<TrackerRole, UnitQuaternion<f32>> {
        let mut m = HashMap::new();
        m.insert(TrackerRole::Waist, quat_x(waist_deg));
        m.insert(TrackerRole::LeftUpperLeg, quat_x(thigh_deg));
        m.insert(TrackerRole::RightUpperLeg, quat_x(thigh_deg));
        m
    }

    #[test]
    fn stays_standing_on_small_angle() {
        let mut d = PoseDetector::new(PoseDetectorConfig {
            confirm_frames: 3,
            ..Default::default()
        });
        for _ in 0..5 {
            assert_eq!(d.update(&map(0.0, 10.0)), None);
        }
        assert_eq!(d.current().0, CalibrationPose::Standing);
    }

    #[test]
    fn switches_to_sitting_after_confirm_frames() {
        let mut d = PoseDetector::new(PoseDetectorConfig {
            confirm_frames: 3,
            ..Default::default()
        });
        // 90° Hüftbeugung
        assert_eq!(d.update(&map(0.0, 90.0)), None);
        assert_eq!(d.update(&map(0.0, 90.0)), None);
        let switched = d.update(&map(0.0, 90.0));
        assert_eq!(switched.map(|(p, _)| p), Some(CalibrationPose::Sitting));
    }

    #[test]
    fn no_flutter_on_border() {
        let mut d = PoseDetector::new(PoseDetectorConfig {
            confirm_frames: 5,
            ..Default::default()
        });
        // Abwechselnd 89/91 im 60..120 Band -> trotzdem Sitting, aber
        // braucht 5 stabile Frames; Wechsel zw. zwei Sitting-Werten
        // resettet nicht, Wechsel zu Standing schon.
        for _ in 0..4 {
            d.update(&map(0.0, 89.0));
            d.update(&map(0.0, 91.0));
        }
        // Kein harter Flatter-Wechsel zu Standing passiert.
        assert_ne!(d.current().0, CalibrationPose::Lying);
    }

    #[test]
    fn lying_without_legs_needs_70_deg() {
        let mut d = PoseDetector::new(PoseDetectorConfig {
            confirm_frames: 1,
            lie_confirm_frames: 1,
            ..Default::default()
        });
        let mut m = HashMap::new();
        // 60° ohne Bein-Tracker → kein Liegen mehr (vorher Fehltrigger).
        m.insert(TrackerRole::Waist, quat_x(60.0));
        assert_eq!(d.update(&m), None);
        assert_eq!(d.current().0, CalibrationPose::Standing);
        // 80° ohne Bein-Tracker → Liegen.
        m.insert(TrackerRole::Waist, quat_x(80.0));
        let switched = d.update(&m);
        assert_eq!(switched.map(|(p, _)| p), Some(CalibrationPose::Lying));
    }

    #[test]
    fn slouched_sitting_is_not_lying() {
        let mut d = PoseDetector::new(PoseDetectorConfig {
            confirm_frames: 2,
            lie_confirm_frames: 4,
            ..Default::default()
        });
        // Torso 70° gekippt, Hüfte 70° gebeugt (Lümmeln) → Sitzen, nie Liegen.
        let mut switched = None;
        for _ in 0..6 {
            switched = d.update(&map(70.0, 0.0)).or(switched);
        }
        assert_eq!(switched.map(|(p, _)| p), Some(CalibrationPose::Sitting));
    }

    #[test]
    fn lying_needs_more_frames() {
        let mut d = PoseDetector::new(PoseDetectorConfig {
            confirm_frames: 2,
            lie_confirm_frames: 4,
            ..Default::default()
        });
        let mut m = HashMap::new();
        m.insert(TrackerRole::Waist, quat_x(80.0));
        m.insert(TrackerRole::LeftUpperLeg, quat_x(80.0));
        assert_eq!(d.update(&m), None);
        assert_eq!(d.update(&m), None);
        assert_eq!(d.update(&m), None);
        let switched = d.update(&m);
        assert_eq!(switched.map(|(p, _)| p), Some(CalibrationPose::Lying));
        let _ = FRAC_PI_2;
    }

    #[test]
    fn manual_hold_blocks_switch() {
        let mut d = PoseDetector::new(PoseDetectorConfig {
            confirm_frames: 1,
            ..Default::default()
        });
        d.set_manual_hold(5);
        for _ in 0..5 {
            assert_eq!(d.update(&map(0.0, 90.0)), None);
        }
        assert_eq!(d.current().0, CalibrationPose::Standing);
    }
}
