use meralus_shared::{IPoint2D, Point3D, USizePoint3D};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub uuid: Uuid,
    pub nickname: String,
    pub position: Point3D,
}

impl Player {
    pub fn new<T: Into<String>>(nickname: T, position: Point3D) -> Self {
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
    RemoveBlock(IPoint2D, USizePoint3D),
    PlayerConnected(String),
    PlayerMoved { uuid: Uuid, position: Point3D },
    RequestChunk(IPoint2D),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum OutgoingPacket {
    UuidAssigned { uuid: Uuid },
    PlayerConnected { uuid: Uuid, name: String },
    PlayerDisconnected { uuid: Uuid },
    PlayerMoved { uuid: Uuid, position: Point3D },
    PlayersList { players: Vec<Player> },
    ChunkData { data: Vec<u8> },
    RemoveBlock(IPoint2D, USizePoint3D),
}
