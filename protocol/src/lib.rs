//! Protocol message definitions will go here.

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Hello,
    JoinAck { player_id: u32 },
    InputTick { seq: u32, thrust: f32, yaw: f32, ballast: f32 },
    StateDelta { seq: u32 },
    MineRequest { node_id: u32 },
    MineAck { node_id: u32, added_kg: f32 },
    DockRequest,
    DockAck { credits_delta: i32 },
}
