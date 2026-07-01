use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub uuid: Uuid,
    pub nickname: String,
    pub position: glam::Vec3,
}

impl Player {
    pub fn new<T: Into<String>>(nickname: T, position: glam::Vec3) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            nickname: nickname.into(),
            position,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IncomingPacket {
    GetPlayers,
    RemoveBlock(glam::IVec2, glam::USizeVec3),
    PlayerConnected(String),
    PlayerMoved { uuid: Uuid, position: glam::Vec3 },
    RequestChunk(glam::IVec2),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum OutgoingPacket {
    UuidAssigned { uuid: Uuid },
    PlayerConnected { uuid: Uuid, name: String },
    PlayerDisconnected { uuid: Uuid },
    PlayerMoved { uuid: Uuid, position: glam::Vec3 },
    PlayersList { players: Vec<Player> },
    ChunkData { data: Vec<u8> },
    RemoveBlock(glam::IVec2, glam::USizeVec3),
}
