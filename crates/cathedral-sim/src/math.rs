//! f64 vector math (`sim.py:45-80`).
//!
//! `glam::DVec3` is the sim's position type; f32 would round `20.0 + 1e-6` to
//! `20.0` and silently change the hearing boundary. glam validates nothing, so
//! every path that can introduce a non-finite component goes through the
//! constructors here.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use serde_json::Value;

use crate::error::SpatialUpdateError;

pub use glam::DVec3 as Vec3;

/// A finiteness-validated `Vec3`.
pub fn vec3(x: f64, y: f64, z: f64) -> Result<Vec3, SpatialUpdateError> {
    for (name, value) in [("x", x), ("y", y), ("z", z)] {
        if !value.is_finite() {
            return Err(SpatialUpdateError::invalid(format!(
                "position {name} must be a finite number"
            )));
        }
    }
    Ok(Vec3::new(x, y, z))
}

/// Parse `{"x":…, "y":…, "z":…}` — exactly those keys, all finite numbers.
pub fn vec3_from_json(value: &Value) -> Result<Vec3, SpatialUpdateError> {
    let Value::Object(map) = value else {
        return Err(SpatialUpdateError::invalid(
            "position_m must be an object with x, y, and z",
        ));
    };
    if map.len() != 3 || !["x", "y", "z"].iter().all(|key| map.contains_key(*key)) {
        return Err(SpatialUpdateError::invalid(
            "position_m must contain exactly x, y, and z",
        ));
    }
    let mut components = [0.0f64; 3];
    for (slot, name) in components.iter_mut().zip(["x", "y", "z"]) {
        // Booleans and strings are not `Value::Number`, so `as_f64` rejects
        // them exactly like Python's isinstance check.
        let number = map[name].as_f64().filter(|value| value.is_finite());
        *slot = number.ok_or_else(|| {
            SpatialUpdateError::invalid(format!("position {name} must be a finite number"))
        })?;
    }
    Ok(Vec3::new(components[0], components[1], components[2]))
}

/// Render as `{"x":…, "y":…, "z":…}`.
pub fn vec3_to_json(value: Vec3) -> Value {
    let mut map = serde_json::Map::with_capacity(3);
    map.insert("x".into(), json_number(value.x));
    map.insert("y".into(), json_number(value.y));
    map.insert("z".into(), json_number(value.z));
    Value::Object(map)
}

fn json_number(value: f64) -> Value {
    serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
}

/// `#[serde(with = …)]` for `Vec3` fields: the `{x, y, z}` object shape, with
/// the exact-keys and finiteness rules glam's own serde impl (a sequence) lacks.
pub mod vec3_serde {
    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Repr {
        x: f64,
        y: f64,
        z: f64,
    }

    pub fn serialize<S: Serializer>(value: &Vec3, serializer: S) -> Result<S::Ok, S::Error> {
        Repr {
            x: value.x,
            y: value.y,
            z: value.z,
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec3, D::Error> {
        let repr = Repr::deserialize(deserializer)?;
        vec3(repr.x, repr.y, repr.z).map_err(|error| D::Error::custom(error.message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_json_requires_exactly_xyz_and_finite_numbers() {
        assert_eq!(
            vec3_from_json(&json!({"x": 1, "y": 2.5, "z": -3})).unwrap(),
            Vec3::new(1.0, 2.5, -3.0)
        );
        for bad in [
            json!({"x": 1, "y": 2, "z": 3, "w": 4}),
            json!({"x": 1, "y": 2}),
            json!({"x": true, "y": 0, "z": 0}),
            json!({"x": "1", "y": 0, "z": 0}),
            json!([1, 2, 3]),
        ] {
            assert!(vec3_from_json(&bad).is_err(), "accepted {bad}");
        }
        assert!(vec3(f64::NAN, 0.0, 0.0).is_err());
        assert!(vec3(0.0, f64::INFINITY, 0.0).is_err());
        assert!(vec3(0.0, 0.0, f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn f64_keeps_the_hearing_boundary_resolvable() {
        // The whole reason the sim is f64: this is 20.0 in f32.
        let outside = Vec3::new(0.0, 0.0, 20.0 + 1e-6);
        assert!(outside.distance_squared(Vec3::ZERO) > 20.0 * 20.0);
    }
}
