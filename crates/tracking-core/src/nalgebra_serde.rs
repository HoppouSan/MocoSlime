use nalgebra::{UnitQuaternion, Vector3};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QuaternionSerde {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<UnitQuaternion<f32>> for QuaternionSerde {
    fn from(q: UnitQuaternion<f32>) -> Self {
        Self {
            w: q.w,
            x: q.i,
            y: q.j,
            z: q.k,
        }
    }
}

impl From<QuaternionSerde> for UnitQuaternion<f32> {
    fn from(s: QuaternionSerde) -> Self {
        UnitQuaternion::new_normalize(nalgebra::Quaternion::new(s.w, s.x, s.y, s.z))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vector3Serde {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Vector3<f32>> for Vector3Serde {
    fn from(v: Vector3<f32>) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<Vector3Serde> for Vector3<f32> {
    fn from(s: Vector3Serde) -> Self {
        Vector3::new(s.x, s.y, s.z)
    }
}

pub fn serialize_quat<S>(q: &UnitQuaternion<f32>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    QuaternionSerde::from(*q).serialize(serializer)
}

pub fn deserialize_quat<'de, D>(deserializer: D) -> Result<UnitQuaternion<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    QuaternionSerde::deserialize(deserializer).map(Into::into)
}

pub fn serialize_vec3<S>(v: &Vector3<f32>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    Vector3Serde::from(*v).serialize(serializer)
}

pub fn deserialize_vec3<'de, D>(deserializer: D) -> Result<Vector3<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    Vector3Serde::deserialize(deserializer).map(Into::into)
}

pub fn serialize_option_quat<S>(
    q: &Option<UnitQuaternion<f32>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    q.map(QuaternionSerde::from).serialize(serializer)
}

pub fn deserialize_option_quat<'de, D>(
    deserializer: D,
) -> Result<Option<UnitQuaternion<f32>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<QuaternionSerde>::deserialize(deserializer).map(|o| o.map(Into::into))
}

pub fn serialize_option_vec3<S>(v: &Option<Vector3<f32>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    v.map(Vector3Serde::from).serialize(serializer)
}

pub fn deserialize_option_vec3<'de, D>(deserializer: D) -> Result<Option<Vector3<f32>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vector3Serde>::deserialize(deserializer).map(|o| o.map(Into::into))
}
