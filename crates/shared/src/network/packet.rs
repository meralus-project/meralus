use glam::{IVec2, Vec3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub uuid: Uuid,
    pub nickname: String,
    pub position: Vec3,
}

impl Player {
    pub fn new(nickname: String, position: Vec3) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            nickname,
            position,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IncomingPacket {
    GetPlayers,
    PlayerConnected { name: String },
    PlayerMoved { uuid: Uuid, position: Vec3 },
    RequestChunk { origin: IVec2 },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum OutgoingPacket {
    UuidAssigned { uuid: Uuid },
    PlayerConnected { uuid: Uuid, name: String },
    PlayerDisonnected { uuid: Uuid },
    PlayerMoved { uuid: Uuid, position: Vec3 },
    PlayersList { players: Vec<Player> },
    ChunkData { data: Vec<u8> },
}
