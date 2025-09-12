use bevy::prelude::*;

/// Extensible flow field representation.
/// For M1 we keep a uniform field but design for future extension.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub enum FlowField {
    /// Uniform flow across space; `flow` is a 3D vector in world units/sec.
    /// `variance` encodes short-term stochastic deviation magnitude.
    Uniform { flow: Vec3, variance: f32 },
}

impl FlowField {
    pub fn uniform(flow: Vec3, variance: f32) -> Self {
        Self::Uniform { flow, variance }
    }

    /// Sample the flow vector and variance at a world position and time.
    /// For Uniform, returns the same values regardless of `pos`/`time`.
    pub fn sample(&self, _pos: Vec3, _time: f32) -> (Vec3, f32) {
        match *self {
            FlowField::Uniform { flow, variance } => (flow, variance),
        }
    }
}

/// Marker for the tunnel parent entity (holds bounds/field; children are shell meshes).
#[derive(Component)]
pub struct Tunnel;

/// Cached bounds for the tunnel geometry for debug sampling and later gameplay.
#[derive(Component, Copy, Clone, Debug)]
pub struct TunnelBounds {
    pub size: Vec3, // X length, Y height, Z width (local space)
}

pub fn draw_flow_gizmos(
    vis: Option<Res<crate::debug_vis::DebugVis>>,
    mut gizmos: Gizmos,
    q: Query<(&GlobalTransform, &FlowField, &TunnelBounds), With<Tunnel>>,
    time: Res<Time>,
) {
    let Some(vis) = vis else { return; };
    if !vis.flow_arrows {
        return;
    }

    for (transform, field, bounds) in &q {
        // For now, assume axis-aligned tunnel (no rotation or non-uniform scale).
        let center = transform.translation();
        let half = bounds.size * 0.5;

        let nx = 6; // samples along length
        let ny = 2; // samples along height
        let nz = 2; // samples along width

        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    let fx = (ix as f32 + 0.5) / nx as f32;
                    let fy = (iy as f32 + 0.5) / ny as f32;
                    let fz = (iz as f32 + 0.5) / nz as f32;

                    // Local position within the cuboid, centered on `center`.
                    let local = Vec3::new(
                        -half.x + bounds.size.x * fx,
                        -half.y + bounds.size.y * fy,
                        -half.z + bounds.size.z * fz,
                    );
                    let pos = center + local;

                    let (flow, variance) = field.sample(pos, time.elapsed_secs());
                    let dir = flow;
                    if dir.length_squared() > 1e-6 {
                        let len = 0.8 + variance; // visualize variance as arrow length contribution
                        gizmos.arrow(
                            pos,
                            pos + dir.normalize() * len,
                            Color::srgb(0.2, 0.7, 1.0),
                        );
                    }
                }
            }
        }
    }
}
