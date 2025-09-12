//! Protocol layer for Thalassocracy.
//!
//! Defines wire messages, channel ids, and simple (de)serialization helpers.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 3;
// Shared netcode protocol id used by client and server handshakes
pub const NETCODE_PROTOCOL_ID: u64 = 7;

// Network channel layout (configurable at runtime; ids are defaults)
#[repr(u8)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Channel {
    // Reliable control path
    Reliable = 0,
    // Unreliable, sequenced state updates
    State = 1,
    // Unreliable inputs (client->server), sequenced
    Input = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientToServer {
    Hello(ClientHello),
    InputTick(InputTick),
    /// Time-stamped control event in server time (ms) for clean scheduling.
    InputEvent(InputEvent),
    MineRequest(MineRequest),
    DockRequest(DockRequest),
    PauseRequest(PauseRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerToClient {
    JoinAck(JoinAck),
    StateDelta(StateDelta),
    InputAck(InputAck),
    MineAck(MineAck),
    DockAck(DockAck),
    PauseState(PauseState),
    Disconnect(DisconnectReason),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    pub protocol: u16,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinAck {
    pub player_id: Uuid,
    /// Server physics tick rate (Hz) for client fixed-step prediction.
    pub tick_hz: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputTick {
    pub tick: u64,
    // Minimal input set for Milestone 0; expand later.
    pub thrust: f32,
    /// Rudder input in [-1,1]. Convention: +1 = right rudder (nose yaws right
    /// under forward motion), −1 = left rudder.
    pub yaw: f32,
    /// Forward ballast pump speed in [-1,1]. +1 pumps water in (fill), -1 pumps out.
    pub pump_fwd: f32,
    /// Aft ballast pump speed in [-1,1]. +1 pumps water in (fill), -1 pumps out.
    pub pump_aft: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputAck {
    pub tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    pub tick: u64,
    /// Server time in milliseconds since an arbitrary start (monotonic).
    pub server_ms: u64,
    // Compact state for now; replace with snapshot diff when ready.
    pub players: Vec<NetPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetPlayer {
    pub id: Uuid,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    /// Full body orientation as quaternion, `[x, y, z, w]` order.
    pub orientation: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MineRequest {
    pub node_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MineAck {
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockAck {
    pub credits_after: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauseRequest { pub paused: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauseState { pub paused: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisconnectReason {
    IncompatibleProtocol { server: u16, client: u16 },
    Kicked,
    ServerShutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    /// Effective time in server milliseconds when the input should take effect.
    pub t_ms: u64,
    pub thrust: f32,
    pub yaw: f32,
    pub pump_fwd: f32,
    pub pump_aft: f32,
}

// Simple helpers for bincode encoding.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(msg)
}

pub fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(bytes)
}

/// AOI Note (not implemented):
/// For underground 3D spaces, an octree spatial partition is the natural fit
/// for culling StateDelta payloads; a quadtree only partitions 2D space. An
/// octree allows pruning by 3D bounds and better matches cave volumes. We can
/// start with a simple uniform grid (XYZ bins) and evolve to an octree when
/// entity counts warrant. Leaving the specific structure undefined for now.
pub struct Nothing {}
