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
