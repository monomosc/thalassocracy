use crate::{FlowFieldSpec, LevelSpec, Vec3f};
use super::util::{vadd, vsub, vscale};

/// Sample the flow field and variance at a world position.
/// Currently only the tunnel contributes; extend later for multiple fields.
pub fn sample_flow_at(level: &LevelSpec, pos: Vec3f, time: f32) -> (Vec3f, f32) {
    let mut flow = Vec3f::new(0.0, 0.0, 0.0);
    let mut variance = 0.0f32;
    let mut count = 0.0f32;

    // Tunnel AABB check
    let half = Vec3f::new(
        level.tunnel.size.x * 0.5,
        level.tunnel.size.y * 0.5,
        level.tunnel.size.z * 0.5,
    );
    let min = vsub(level.tunnel.pos, half);
    let max = vadd(level.tunnel.pos, half);
    if pos.x >= min.x
        && pos.x <= max.x
        && pos.y >= min.y
        && pos.y <= max.y
        && pos.z >= min.z
        && pos.z <= max.z
    {
        match level.tunnel.flow {
            FlowFieldSpec::Uniform { flow: f, variance: var } => {
                flow = vadd(flow, f);
                variance += var;
                count += 1.0;
            }
        }
    }

    // Torus tunnel interior check (if present). See original file for geometry notes.
    if let Some(t) = &level.torus_tunnel {
        let axis_len2 = t.axis.x * t.axis.x + t.axis.y * t.axis.y + t.axis.z * t.axis.z;
        if axis_len2 > 1e-8 {
            let axis_len = axis_len2.sqrt();
            let n = Vec3f::new(t.axis.x / axis_len, t.axis.y / axis_len, t.axis.z / axis_len);
            let d = vsub(pos, t.center);
            let h = d.x * n.x + d.y * n.y + d.z * n.z; // signed height from ring plane
            let p = Vec3f::new(d.x - n.x * h, d.y - n.y * h, d.z - n.z * h);
            let p_len = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
            let tube = ((p_len - t.major_radius).abs().powi(2) + h * h).sqrt();
            if tube <= t.minor_radius {
                match t.flow {
                    FlowFieldSpec::Uniform { flow: f, variance: var } => {
                        flow = vadd(flow, f);
                        variance += var;
                        count += 1.0;
                    }
                }
            }
        }
    }

    if count > 0.0 {
        flow = vscale(flow, 1.0 / count);
        variance /= count;
    }

    let _ = time;
    (flow, variance)
}

