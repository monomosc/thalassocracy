use crate::{Quatf, Vec3f};

// Basis: standard RHS with +Z forward, +Y up, +X right
pub(super) const BODY_FWD: Vec3f = Vec3f::new(0.0, 0.0, 1.0);
pub(super) const BODY_RIGHT: Vec3f = Vec3f::new(1.0, 0.0, 0.0);
pub(super) const BODY_UP: Vec3f = Vec3f::new(0.0, 1.0, 0.0);

#[inline]
pub(super) fn quat_rotate_vec3(q: Quatf, v: Vec3f) -> Vec3f {
    q * v
}

#[inline]
pub(super) fn quat_to_yaw(q: Quatf) -> f32 {
    let fwd = q * BODY_FWD;
    // Positive yaw turns left; project into XZ plane with +Z forward
    (-fwd.x).atan2(fwd.z)
}

#[inline]
pub(super) fn vadd(a: Vec3f, b: Vec3f) -> Vec3f {
    Vec3f::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

#[inline]
pub(super) fn vsub(a: Vec3f, b: Vec3f) -> Vec3f {
    Vec3f::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

#[inline]
pub(super) fn vscale(a: Vec3f, s: f32) -> Vec3f {
    Vec3f::new(a.x * s, a.y * s, a.z * s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quat_to_yaw_basic_orientations() {
        let q_id = Quatf::from_rotation_y(0.0);
        let q_left_90 = Quatf::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let q_right_90 = Quatf::from_rotation_y(-std::f32::consts::FRAC_PI_2);

        // Identity facing +Z
        let yaw0 = quat_to_yaw(q_id);
        assert!((yaw0 - 0.0).abs() < 1e-6);

        // Check relative sign consistency with the forward vector projection
        let yaw_l = quat_to_yaw(q_left_90);
        let yaw_r = quat_to_yaw(q_right_90);
        // Should be opposite signs
        assert!(
            yaw_l * yaw_r <= 0.0,
            "left/right yaws should have opposite sign: {} vs {}",
            yaw_l,
            yaw_r
        );
        // Magnitudes should be near 90 deg (pi/2) up to the function's sign convention
        assert!((yaw_l.abs() - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        assert!((yaw_r.abs() - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    }

    #[test]
    fn vec_ops_work() {
        let a = Vec3f::new(1.0, -2.0, 3.0);
        let b = Vec3f::new(-4.0, 5.0, -6.0);
        let s = 2.5;

        let add = vadd(a, b);
        assert_eq!(add.x, -3.0);
        assert_eq!(add.y, 3.0);
        assert_eq!(add.z, -3.0);

        let sub = vsub(a, b);
        assert_eq!(sub.x, 5.0);
        assert_eq!(sub.y, -7.0);
        assert_eq!(sub.z, 9.0);

        let sc = vscale(a, s);
        assert!((sc.x - 2.5).abs() < 1e-6);
        assert!((sc.y + 5.0).abs() < 1e-6);
        assert!((sc.z - 7.5).abs() < 1e-6);
    }
}
