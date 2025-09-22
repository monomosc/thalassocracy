//! Shared level data for client and server.
//!
//! This crate keeps dependencies minimal and uses `bevy_math` for vector and
//! quaternion math types (`Vec3`, `Quat`) which provide serde support and are
//! widely used across the codebase.

// Re-export math types so downstream code can continue using `Vec3f`/`Quatf`.
pub use bevy_math::{Quat as Quatf, Vec3 as Vec3f};
mod spec;
pub use spec::{
    ChamberSpec, FlowFieldSpec, LevelSpec, RoomSpec, TorusExitSpec, TorusTunnelSpec, TunnelSpec,
};

pub mod builtins;

pub mod submarine_physics;
pub use submarine_physics::{
    sample_flow_at, step_submarine, step_submarine_dbg, SubInputs, SubState, SubStepDebug,
};

mod sub_specs;
pub use sub_specs::subspecs;
pub use sub_specs::{BallastTankSpec, SubPhysicsSpec};
