use bevy::prelude::*;

/// Shared resource for client thrust inputs.
#[derive(Resource, Debug, Clone)]
pub struct ThrustInput {
    pub value: f32, // -1.0 .. 1.0 (forward/back)
    /// Rudder in [-1,1]. Convention: +1 = right rudder (nose right under forward motion).
    pub yaw: f32, // -1.0 .. 1.0 (right rudder positive)
    /// Forward ballast pump speed in [-1,1]. +1 pumps water in, -1 pumps out.
    pub pump_fwd: f32,
    /// Aft ballast pump speed in [-1,1]. +1 pumps water in, -1 pumps out.
    pub pump_aft: f32,
    pub tick: u64,
}

impl Default for ThrustInput {
    fn default() -> Self {
        Self {
            value: 0.0,
            yaw: 0.0,
            pump_fwd: 0.0,
            pump_aft: 0.0,
            tick: 0,
        }
    }
}
