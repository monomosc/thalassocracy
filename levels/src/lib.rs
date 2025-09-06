//! Shared level data for client and server.
//!
//! This crate intentionally avoids any Bevy types. It exposes a simple,
//! serializable schema the server can use for validation and the client can
//! translate into meshes/volumes.

mod math;
pub use math::{Vec3f, Quatf};
mod spec;
pub use spec::{FlowFieldSpec, RoomSpec, TunnelSpec, ChamberSpec, TorusTunnelSpec, TorusExitSpec, LevelSpec};


pub mod builtins;

pub mod submarine_physics;
pub use submarine_physics::{SubState, SubInputs, SubStepDebug, step_submarine, step_submarine_dbg, sample_flow_at};

mod sub_specs;
pub use sub_specs::{SubPhysicsSpec, BallastTankSpec};
pub use sub_specs::subspecs;
