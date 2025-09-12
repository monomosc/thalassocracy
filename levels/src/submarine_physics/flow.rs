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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::{greybox_level, torus_two_exit_level};

    #[test]
    fn tunnel_aabb_sampling() {
        let level = greybox_level();
        let center = level.tunnel.pos;
        let (flow, var) = sample_flow_at(&level, center, 0.0);
        match level.tunnel.flow {
            FlowFieldSpec::Uniform { flow: f, variance: v } => {
                assert!((flow.x - f.x).abs() < 1e-6);
                assert!((flow.y - f.y).abs() < 1e-6);
                assert!((flow.z - f.z).abs() < 1e-6);
                assert!((var - v).abs() < 1e-6);
            }
        }
        // Outside the tunnel bounds: offset in Z beyond half-width
        let half_w = level.tunnel.size.z * 0.5;
        let outside = Vec3f::new(center.x, center.y, center.z + half_w + 10.0);
        let (flow2, var2) = sample_flow_at(&level, outside, 0.0);
        assert!(flow2.length() < 1e-6 && var2.abs() < 1e-6);
    }

    #[test]
    fn average_when_inside_torus_and_tunnel() {
        let level = torus_two_exit_level();
        let center = level.tunnel.pos;
        // Pick a point on the torus ring: +major_radius along +X from ring center
        let t = level.torus_tunnel.as_ref().unwrap();
        let pos_on_ring = Vec3f::new(center.x + t.major_radius, center.y, center.z);
        let (flow, var) = sample_flow_at(&level, pos_on_ring, 0.0);

        // Expect average of tunnel and torus uniform flows/variances
        let (tunnel_flow, tunnel_var) = match level.tunnel.flow {
            FlowFieldSpec::Uniform { flow, variance } => (flow, variance),
        };
        let (ring_flow, ring_var) = match t.flow {
            FlowFieldSpec::Uniform { flow, variance } => (flow, variance),
        };
        let expected = Vec3f::new(
            0.5 * (tunnel_flow.x + ring_flow.x),
            0.5 * (tunnel_flow.y + ring_flow.y),
            0.5 * (tunnel_flow.z + ring_flow.z),
        );
        let expected_var = 0.5 * (tunnel_var + ring_var);
        assert!((flow.x - expected.x).abs() < 1e-5);
        assert!((flow.y - expected.y).abs() < 1e-5);
        assert!((flow.z - expected.z).abs() < 1e-5);
        assert!((var - expected_var).abs() < 1e-5);
    }
}
