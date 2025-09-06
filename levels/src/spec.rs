use serde::{Deserialize, Serialize};
use crate::math::Vec3f;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowFieldSpec {
    Uniform { flow: Vec3f, variance: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSpec {
    pub size: Vec3f,          // interior volume size
    pub wall_thickness: f32,  // shell thickness for floor/ceiling/walls
    pub dock_size: Vec3f,
    pub dock_pos: Vec3f,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelSpec {
    pub size: Vec3f,          // interior/open space
    pub pos: Vec3f,           // center position in world coordinates
    pub shell_thickness: f32, // shell thickness for walls
    pub flow: FlowFieldSpec,  // flow field for this tunnel segment
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChamberSpec {
    pub size: Vec3f,
    pub pos: Vec3f,
}

/// A torus‑shaped tunnel (a ring in a horizontal plane by default) with two
/// labelled exits cut into the ring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorusTunnelSpec {
    /// Center of the torus (ring center).
    pub center: Vec3f,
    /// Axis normal for the ring's plane. For a horizontal ring, use +Y.
    pub axis: Vec3f,
    /// Major radius (distance from center to centerline of the tube).
    pub major_radius: f32,
    /// Minor radius (tube interior radius = open radius; wall thickness applies outside).
    pub minor_radius: f32,
    /// Shell/wall thickness of the torus tunnel.
    pub wall_thickness: f32,
    /// Flow field specification within the torus interior.
    pub flow: FlowFieldSpec,
    /// Two exits cut into the ring, approximately opposite. Order and labels
    /// indicate where each heads (e.g., "dock" and "mining_chamber").
    pub exits: [TorusExitSpec; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorusExitSpec {
    /// Center angle of the exit gap in degrees, measured around `axis` with 0°
    /// aligned to the world's +X direction in the ring plane.
    pub angle_deg: f32,
    /// Angular width of the opening in degrees.
    pub width_deg: f32,
    /// Logical label for the exit target (e.g., "dock" or "mining_chamber").
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelSpec {
    pub room: RoomSpec,
    pub tunnel: TunnelSpec,
    pub chamber: ChamberSpec,
    /// Optional additional complex geometry. Prototype: a torus‑shaped tunnel
    /// with labelled exits. Client can render if present; physics can sample
    /// its flow field separately from the axis‑aligned `tunnel`.
    pub torus_tunnel: Option<TorusTunnelSpec>,
}

