use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    pub const fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
}

impl Default for Vec3f {
    fn default() -> Self { Self::new(0.0, 0.0, 0.0) }
}

// Minimal quaternion for shared physics (no external deps)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Quatf { pub w: f32, pub x: f32, pub y: f32, pub z: f32 }

impl Quatf {
    pub const fn identity() -> Self { Self { w: 1.0, x: 0.0, y: 0.0, z: 0.0 } }
    pub fn from_axis_angle(axis: Vec3f, angle: f32) -> Self {
        let len = (axis.x*axis.x + axis.y*axis.y + axis.z*axis.z).sqrt().max(1e-12);
        let nx = axis.x / len; let ny = axis.y / len; let nz = axis.z / len;
        let h = 0.5 * angle; let (s, c) = h.sin_cos();
        Self { w: c, x: nx * s, y: ny * s, z: nz * s }
    }
    pub fn from_yaw(yaw: f32) -> Self {
        // Positive yaw turns left (toward −Z)
        let h = 0.5 * yaw; let (s, c) = h.sin_cos(); Self { w: c, x: 0.0, y: s, z: 0.0 }
    }
    pub fn normalize(self) -> Self {
        let n2 = self.w*self.w + self.x*self.x + self.y*self.y + self.z*self.z;
        if n2 <= 0.0 { return Self::identity(); }
        let inv = n2.sqrt().recip();
        Self { w: self.w*inv, x: self.x*inv, y: self.y*inv, z: self.z*inv }
    }
    pub fn mul_q(self, o: Self) -> Self {
        // Hamilton product: self * o
        Self {
            w: self.w*o.w - self.x*o.x - self.y*o.y - self.z*o.z,
            x: self.w*o.x + self.x*o.w + self.y*o.z - self.z*o.y,
            y: self.w*o.y - self.x*o.z + self.y*o.w + self.z*o.x,
            z: self.w*o.z + self.x*o.y - self.y*o.x + self.z*o.w,
        }
    }
    pub fn conj(self) -> Self { Self { w: self.w, x: -self.x, y: -self.y, z: -self.z } }
    pub fn rotate_vec3(self, v: Vec3f) -> Vec3f {
        let qv = Quatf { w: 0.0, x: v.x, y: v.y, z: v.z };
        let r = self.mul_q(qv).mul_q(self.conj());
        Vec3f { x: r.x, y: r.y, z: r.z }
    }
    pub fn to_yaw(self) -> f32 {
        // Project forward vector to XZ and compute atan2(-z, x)
        let fwd = self.rotate_vec3(Vec3f::new(1.0, 0.0, 0.0));
        (-fwd.z).atan2(fwd.x)
    }
}

