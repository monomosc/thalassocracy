//! Protocol layer for Thalassocracy.
//!
//! Defines wire messages, channel ids, and simple (de)serialization helpers.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 1;
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
    MineRequest(MineRequest),
    DockRequest(DockRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerToClient {
    JoinAck(JoinAck),
    StateDelta(StateDelta),
    MineAck(MineAck),
    DockAck(DockAck),
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputTick {
    pub tick: u64,
    // Minimal input set for Milestone 0; expand later.
    pub thrust: f32,
    pub yaw: f32,
    pub ballast: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    pub tick: u64,
    // Compact state for now; replace with snapshot diff when ready.
    pub players: Vec<NetPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetPlayer {
    pub id: Uuid,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
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
pub enum DisconnectReason {
    IncompatibleProtocol { server: u16, client: u16 },
    Kicked,
    ServerShutdown,
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
