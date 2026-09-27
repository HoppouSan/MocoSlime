use crate::error::{Result, TrackingError};
use nalgebra::{UnitQuaternion, Vector3};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FilterConfig {
    pub enable_quaternion_slerp: bool,
    pub slerp_factor: f32,
    pub enable_accel_lowpass: bool,
    pub accel_lowpass_alpha: f32,
    pub max_timestamp_jump_ms: u64,
    pub min_packet_interval_ms: u64,
    /// Adaptiver Slerp aus Winkelgeschwindigkeit (still=mehr Glättung).
    #[serde(default = "default_true")]
    pub adaptive: bool,
    /// Spike-Reject: Winkel über diesem Wert bei kleinem dt wird gehalten.
    #[serde(default = "default_spike_deg")]
    pub spike_max_deg: f32,
    /// Gravitations-Kompensation: liefert lineare Accel (ohne g).
    #[serde(default)]
    pub compensate_gravity: bool,
}

fn default_true() -> bool {
    true
}

fn default_spike_deg() -> f32 {
    35.0
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            enable_quaternion_slerp: true,
            slerp_factor: 0.1,
            enable_accel_lowpass: true,
            accel_lowpass_alpha: 0.2,
            max_timestamp_jump_ms: 100,
            min_packet_interval_ms: 5,
            adaptive: true,
            spike_max_deg: 35.0,
            compensate_gravity: false,
        }
    }
}

impl FilterConfig {
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.slerp_factor) {
            return Err(TrackingError::FilterConfigError {
                reason: "slerp_factor must be in range [0, 1]".to_string(),
            });
        }
        if !(0.0..=1.0).contains(&self.accel_lowpass_alpha) {
            return Err(TrackingError::FilterConfigError {
                reason: "accel_lowpass_alpha must be in range [0, 1]".to_string(),
            });
        }
        if !(5.0..=90.0).contains(&self.spike_max_deg) {
            return Err(TrackingError::FilterConfigError {
                reason: "spike_max_deg must be in range [5, 90]".to_string(),
            });
        }
        Ok(())
    }

    /// Effektiver Slerp aus Winkelgeschwindigkeit: still → 0.5x (ruhig),
    /// schnell → bis 3x (wenig Lag). Geklemmt auf [0.02, 0.6].
    pub fn effective_slerp(&self, angle_deg: f32) -> f32 {
        if !self.adaptive {
            return self.slerp_factor;
        }
        let scale = (angle_deg / 5.0).clamp(0.5, 3.0);
        (self.slerp_factor * scale).clamp(0.02, 0.6)
    }
}

#[derive(Default)]
struct FilterState {
    last_quaternion: Option<UnitQuaternion<f32>>,
    last_acceleration: Option<Vector3<f32>>,
    last_timestamp_ns: Option<u64>,
    packet_count: u64,
    dropped_packets: u64,
    over_rate_packets: u64,
}

pub struct TrackingFilter {
    config: Arc<RwLock<FilterConfig>>,
    state: Arc<RwLock<FilterState>>,
}

impl TrackingFilter {
    pub fn new(config: FilterConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            state: Arc::new(RwLock::new(FilterState::default())),
        })
    }

    pub fn new_default() -> Self {
        // Infallible by construction: `FilterConfig::default()` is a fixed
        // constant whose validity is pinned by `test_filter_config_validate`.
        // Runtime-provided configs still go through `new()` + validation.
        Self {
            config: Arc::new(RwLock::new(FilterConfig::default())),
            state: Arc::new(RwLock::new(FilterState::default())),
        }
    }

    pub fn process(
        &self,
        quat: UnitQuaternion<f32>,
        accel: Vector3<f32>,
        timestamp_ns: u64,
    ) -> Result<(UnitQuaternion<f32>, Vector3<f32>)> {
        let config = self.config.read();
        let mut state = self.state.write();

        if let Some(last_ts) = state.last_timestamp_ns {
            // Handle out-of-order timestamps: counter wrapped or clock went backwards.
            // Timestamps must be monotonic; older packets are dropped and counted.
            if timestamp_ns < last_ts {
                state.dropped_packets += 1;
                return Err(TrackingError::TimestampOutOfOrder {
                    expected: last_ts,
                    actual: timestamp_ns,
                });
            }
            let dt_ms = (timestamp_ns - last_ts) / 1_000_000;

            if dt_ms > config.max_timestamp_jump_ms {
                // Stream resync (sleep, reconnect, tracker pause): rejecting
                // here WITHOUT moving the baseline bricks the filter
                // forever, because every later packet exceeds the gap too.
                // Instead re-baseline on the new packet and pass it through
                // unfiltered. Still counted as a discontinuity for stats.
                state.dropped_packets += 1;
                state.last_quaternion = None;
                state.last_acceleration = None;
                state.last_timestamp_ns = Some(timestamp_ns);
                state.packet_count += 1;
                return Ok((quat, accel));
            }

            // Over-Rate: schneller als min_packet_interval → verarbeiten,
            // aber zählen, damit die GUI warnen kann (T1-Fix: vorher nur Kommentar).
            if dt_ms < config.min_packet_interval_ms {
                state.over_rate_packets += 1;
            }
        }

        // Gravity provides an absolute tilt reference. Slowly correct roll and
        // pitch while stationary; yaw is deliberately untouched because an
        // accelerometer cannot observe heading.
        let quat = gravity_tilt_correct(
            quat,
            accel,
            state.last_quaternion,
            state.last_timestamp_ns,
            timestamp_ns,
            config.compensate_gravity,
        );

        let filtered_quat = if config.enable_quaternion_slerp {
            if let Some(last_quat) = state.last_quaternion {
                let angle_deg = last_quat.angle_to(&quat) * 180.0 / std::f32::consts::PI;
                // Spike-Reject: unmöglicher Sprung bei kleinem dt → halten.
                let dt_ms = state
                    .last_timestamp_ns
                    .map(|last_ts| (timestamp_ns.saturating_sub(last_ts)) / 1_000_000)
                    .unwrap_or(u64::MAX);
                if dt_ms <= 20 && angle_deg > config.spike_max_deg {
                    state.dropped_packets += 1;
                    let acc = state.last_acceleration.unwrap_or(accel);
                    return Ok((last_quat, acc));
                }
                last_quat.slerp(&quat, config.effective_slerp(angle_deg))
            } else {
                quat
            }
        } else {
            quat
        };

        // Gravitations-Kompensation: lineare Accel = raw − g_sensor.
        let linear = if config.compensate_gravity {
            let g_world = Vector3::new(0.0, -9.81, 0.0);
            let g_sensor = filtered_quat.inverse() * g_world;
            accel - g_sensor
        } else {
            accel
        };

        let filtered_accel = if config.enable_accel_lowpass {
            if let Some(last_accel) = state.last_acceleration {
                // Bei Gravity-Komp ist der State linear; Legacy-State war raw.
                // Mix ist ok (konvergiert in wenigen Frames).
                last_accel + (linear - last_accel) * config.accel_lowpass_alpha
            } else {
                linear
            }
        } else {
            linear
        };

        state.last_quaternion = Some(filtered_quat);
        state.last_acceleration = Some(filtered_accel);
        state.last_timestamp_ns = Some(timestamp_ns);
        state.packet_count += 1;

        Ok((filtered_quat, filtered_accel))
    }

    pub fn reset(&self) {
        let mut state = self.state.write();
        *state = FilterState::default();
    }

    pub fn get_stats(&self) -> FilterStats {
        let state = self.state.read();
        FilterStats {
            packet_count: state.packet_count,
            dropped_packets: state.dropped_packets,
            last_timestamp_ns: state.last_timestamp_ns,
            over_rate_packets: state.over_rate_packets,
        }
    }

    pub fn set_config(&self, new_config: FilterConfig) -> Result<()> {
        new_config.validate()?;
        *self.config.write() = new_config;
        Ok(())
    }
}

fn gravity_tilt_correct(
    quat: UnitQuaternion<f32>,
    accel: Vector3<f32>,
    previous: Option<UnitQuaternion<f32>>,
    previous_ts: Option<u64>,
    timestamp_ns: u64,
    enabled: bool,
) -> UnitQuaternion<f32> {
    if !enabled {
        return quat;
    }
    let magnitude = accel.norm();
    // Ignore dynamic acceleration and sensors that report linear accel only.
    if !(8.5..=11.2).contains(&magnitude) {
        return quat;
    }
    let (Some(previous), Some(previous_ts)) = (previous, previous_ts) else {
        return quat;
    };
    let dt = timestamp_ns.saturating_sub(previous_ts) as f32 * 1e-9;
    if !(0.0..=0.1).contains(&dt) || dt == 0.0 {
        return quat;
    }
    let angular_speed = previous.angle_to(&quat).to_degrees() / dt;
    if angular_speed > 90.0 {
        return quat;
    }

    let measured_world_down = quat * (accel / magnitude);
    let world_down = Vector3::new(0.0, -1.0, 0.0);
    let Some(error) = UnitQuaternion::rotation_between(&measured_world_down, &world_down) else {
        return quat;
    };
    let gain = (0.06 * dt).clamp(0.0, 0.01);
    UnitQuaternion::identity().slerp(&error, gain) * quat
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterStats {
    pub packet_count: u64,
    pub dropped_packets: u64,
    pub last_timestamp_ns: Option<u64>,
    /// Pakete schneller als `min_packet_interval_ms` (verarbeitet, aber gezählt).
    #[serde(default)]
    pub over_rate_packets: u64,
}

/// 1-Frame-Prädiktion (T4): extrapoliert aus Winkelgeschwindigkeit,
/// max 15ms Horizont gegen Überschwingen. Nur für Streaming-Output,
/// nie für Kalibrier-Samples.
pub fn predict_orientation(
    prev: UnitQuaternion<f32>,
    last: UnitQuaternion<f32>,
    dt_ns: u64,
    horizon_ns: u64,
) -> UnitQuaternion<f32> {
    if dt_ns == 0 {
        return last;
    }
    let horizon = horizon_ns.min(15_000_000) as f32 / dt_ns as f32;
    let k = horizon.clamp(0.0, 1.0);
    if k <= 0.0 {
        return last;
    }
    let delta = prev.inverse() * last;
    last * UnitQuaternion::identity().slerp(&delta, k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{UnitQuaternion, Vector3};

    #[test]
    fn test_filter_config_validate() {
        let config = FilterConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_filter_config_invalid_slerp() {
        let config = FilterConfig {
            slerp_factor: 1.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_filter_process_first_packet() {
        let filter = TrackingFilter::new_default();
        let quat = UnitQuaternion::from_euler_angles(0.1, 0.2, 0.3);
        let accel = Vector3::new(1.0, 2.0, 3.0);
        let ts = 1_000_000_000;

        let (fq, fa) = filter.process(quat, accel, ts).unwrap();
        assert_eq!(fq, quat);
        assert_eq!(fa, accel);
    }

    #[test]
    fn test_filter_slerp() {
        let config = FilterConfig {
            enable_quaternion_slerp: true,
            slerp_factor: 0.5,
            spike_max_deg: 90.0,
            ..Default::default()
        };
        let filter = TrackingFilter::new(config).unwrap();

        let q1 = UnitQuaternion::identity();
        let q2 = UnitQuaternion::from_euler_angles(0.3, 0.0, 0.0);

        let (fq1, _) = filter.process(q1, Vector3::zeros(), 0).unwrap();
        let (fq2, _) = filter.process(q2, Vector3::zeros(), 10_000_000).unwrap();

        let angle = fq1.angle_to(&fq2);
        assert!(angle > 0.0 && angle < 0.3);
    }

    #[test]
    fn test_filter_accel_lowpass() {
        let config = FilterConfig {
            enable_accel_lowpass: true,
            accel_lowpass_alpha: 0.5,
            ..Default::default()
        };
        let filter = TrackingFilter::new(config).unwrap();

        let a1 = Vector3::new(0.0, 0.0, 0.0);
        let a2 = Vector3::new(10.0, 0.0, 0.0);

        let (_, _first) = filter.process(UnitQuaternion::identity(), a1, 0).unwrap();
        let (_, fa2) = filter
            .process(UnitQuaternion::identity(), a2, 10_000_000)
            .unwrap();

        assert!((fa2.x - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_filter_rejects_backwards_timestamp_and_counts_drop() {
        let filter = TrackingFilter::new_default();
        filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 20_000_000)
            .unwrap();
        let err = filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 10_000_000)
            .unwrap_err();
        assert!(matches!(err, TrackingError::TimestampOutOfOrder { .. }));
        assert_eq!(filter.get_stats().dropped_packets, 1);
        // Stream recovers on the next monotonic packet.
        filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 30_000_000)
            .unwrap();
        assert_eq!(filter.get_stats().packet_count, 2);
    }

    #[test]
    fn test_filter_resyncs_after_timestamp_jump() {
        let filter = TrackingFilter::new_default();
        filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 0)
            .unwrap();
        // A 10 s gap (sleep/reconnect) resyncs instead of erroring, so
        // the stream recovers immediately instead of stalling forever.
        let (fq, _) = filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 10_000_000_000)
            .unwrap();
        assert_eq!(fq, UnitQuaternion::identity());
        assert_eq!(filter.get_stats().dropped_packets, 1);
        // And the very next packet flows normally again.
        filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 10_020_000_000)
            .unwrap();
        assert_eq!(filter.get_stats().packet_count, 3);
    }

    #[test]
    fn test_effective_slerp_adaptive() {
        let cfg = FilterConfig {
            slerp_factor: 0.1,
            ..Default::default()
        };
        assert!((cfg.effective_slerp(0.5) - 0.05).abs() < 1e-5);
        assert!((cfg.effective_slerp(5.0) - 0.1).abs() < 1e-5);
        assert!((cfg.effective_slerp(90.0) - 0.3).abs() < 1e-5);
        let fixed = FilterConfig {
            adaptive: false,
            slerp_factor: 0.2,
            ..Default::default()
        };
        assert!((fixed.effective_slerp(90.0) - 0.2).abs() < 1e-5);
    }

    #[test]
    fn test_spike_rejected_and_held() {
        let filter = TrackingFilter::new_default();
        filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 0)
            .unwrap();
        // 90° in 10ms = Spike → gehaltener Wert + Drop-Counter.
        let spike = UnitQuaternion::from_euler_angles(1.57, 0.0, 0.0);
        let (fq, _) = filter.process(spike, Vector3::zeros(), 10_000_000).unwrap();
        assert_eq!(fq, UnitQuaternion::identity());
        assert_eq!(filter.get_stats().dropped_packets, 1);
    }

    #[test]
    fn test_gravity_compensation_static() {
        let filter = TrackingFilter::new(FilterConfig {
            compensate_gravity: true,
            accel_lowpass_alpha: 1.0,
            ..Default::default()
        })
        .unwrap();
        // Identity, Sensor misst g = (0,-9.81,0) → linear ≈ 0.
        let (_, fa) = filter
            .process(UnitQuaternion::identity(), Vector3::new(0.0, -9.81, 0.0), 0)
            .unwrap();
        assert!(fa.norm() < 0.05, "linear={fa:?}");
    }

    #[test]
    fn test_predict_extrapolates_and_caps() {
        use super::predict_orientation;
        let a = UnitQuaternion::identity();
        let b = UnitQuaternion::from_euler_angles(0.1, 0.0, 0.0);
        // 10ms Schritt, 8ms Horizont → weiter als b, aber < 2x.
        let p = predict_orientation(a, b, 10_000_000, 8_000_000);
        assert!(p.angle_to(&b) > 0.0);
        assert!(p.angle_to(&b) < a.angle_to(&b));
        // Horizont-Cap 15ms: 100ms angefragt → wie 15ms.
        let p15 = predict_orientation(a, b, 10_000_000, 100_000_000);
        let p15b = predict_orientation(a, b, 10_000_000, 15_000_000);
        assert!((p15.angle_to(&b) - p15b.angle_to(&b)).abs() < 1e-5);
        // Stillstand → identity.
        let still = predict_orientation(a, a, 10_000_000, 8_000_000);
        assert_eq!(still, a);
    }

    #[test]
    fn test_filter_reset() {
        let filter = TrackingFilter::new_default();
        filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 0)
            .unwrap();
        filter.reset();

        let stats = filter.get_stats();
        assert_eq!(stats.packet_count, 0);
    }

    #[test]
    fn test_over_rate_counted_not_dropped() {
        let filter = TrackingFilter::new(FilterConfig {
            min_packet_interval_ms: 5,
            spike_max_deg: 90.0,
            ..Default::default()
        })
        .unwrap();
        filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 0)
            .unwrap();
        // 2ms später = over-rate, aber verarbeitet.
        filter
            .process(UnitQuaternion::identity(), Vector3::zeros(), 2_000_000)
            .unwrap();
        let stats = filter.get_stats();
        assert_eq!(stats.over_rate_packets, 1);
        assert_eq!(stats.packet_count, 2);
    }
}
