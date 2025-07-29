use std::{error::Error, sync::Arc};

use ahash::{HashMap, HashMapExt};
use glam::{IVec2, Vec3};
use meralus_shared::{IncomingPacket, OutgoingPacket, Player, ServerConnection};
use meralus_world::{Chunk, ChunkManager};
use tokio::{
    net::TcpListener,
    sync::{
        RwLock,
        mpsc::{self, Sender},
    },
};
use uuid::Uuid;

struct ServerState {
    players: HashMap<Uuid, Player>,
    player_channels: HashMap<Uuid, Sender<OutgoingPacket>>,
    chunks: ChunkManager,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            player_channels: HashMap::new(),
            chunks: ChunkManager::new(),
        }
    }

    pub fn load_chunk(&mut self, origin: IVec2) -> &Chunk {
        self.chunks.push(Chunk::new(origin));

        let Some(chunk) = self.chunks.get_chunk(&origin) else { unreachable!() };

        chunk
    }

    pub fn try_load_chunk(&mut self, origin: IVec2) -> &Chunk {
        if self.chunks.contains_chunk(&origin) {
            let Some(chunk) = self.chunks.get_chunk(&origin) else { unreachable!() };

            chunk
        } else {
            self.load_chunk(origin)
        }
    }

    pub fn players_excluding(&self, uuid: Uuid) -> impl Iterator<Item = &Sender<OutgoingPacket>> {
        self.player_channels
            .iter()
            .filter_map(move |(key, value)| if key == &uuid { Some(value) } else { None })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let server = TcpListener::bind("192.168.1.5:37565").await?;

    println!("Server listening on {}", server.local_addr()?);

    let state = Arc::new(RwLock::new(ServerState::new()));

    loop {
        let (socket, addr) = server.accept().await?;

        println!("Accepted connection from {addr}");

        let state = state.clone();

        tokio::spawn(async move {
            let mut connection = ServerConnection::new(socket);
            let mut current_player_uuid = None;

            let (tx, mut rx) = mpsc::channel::<OutgoingPacket>(128);

            while let Some(packet) = rx.recv().await {
                connection.send(packet).await.unwrap();
            }

            while let Some(packet) = connection.receive().await {
                match packet {
                    Ok(packet) => match packet {
                        IncomingPacket::PlayerConnected { name } => {
                            let player = Player::new(name, Vec3::ZERO);
                            let uuid = player.uuid;
                            let name = player.nickname.clone();

                            current_player_uuid.replace(uuid);

                            {
                                let mut state = state.write().await;

                                state.player_channels.insert(uuid, tx.clone());
                                state.players.insert(uuid, player);
                            }

                            connection.send(OutgoingPacket::UuidAssigned { uuid }).await.unwrap();

                            for player in state.read().await.players_excluding(uuid) {
                                player.send(OutgoingPacket::PlayerConnected { uuid, name: name.clone() }).await.unwrap();
                            }
                        }
                        packet => {
                            let Some(current_uuid) = current_player_uuid else {
                                break;
                            };

                            match packet {
                                IncomingPacket::PlayerMoved { uuid, position } => {
                                    if let Some(player) = state.write().await.players.get_mut(&uuid) {
                                        player.position = position;
                                    }

                                    for player in state.read().await.players_excluding(current_uuid) {
                                        player.send(OutgoingPacket::PlayerMoved { uuid, position }).await.unwrap();
                                    }
                                }
                                IncomingPacket::GetPlayers => connection
                                    .send(OutgoingPacket::PlayersList {
                                        players: state.read().await.players.values().cloned().collect(),
                                    })
                                    .await
                                    .unwrap(),
                                IncomingPacket::RequestChunk { origin } => {
                                    let data = state.write().await.try_load_chunk(origin).serialize();

                                    connection.send(OutgoingPacket::ChunkData { data }).await.unwrap();
                                }
                                IncomingPacket::PlayerConnected { .. } => unreachable!(),
                            }
                        }
                    },
                    Err(err) => println!("{err}"),
                }
            }

            println!("Closed connection from {addr}");

            if let Some(uuid) = current_player_uuid {
                state.write().await.players.remove(&uuid);
                state.write().await.player_channels.remove(&uuid);

                for player in state.read().await.player_channels.values() {
                    player.send(OutgoingPacket::PlayerDisonnected { uuid }).await.unwrap();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use async_compression::tokio::write::{ZlibDecoder, ZlibEncoder};
    use glam::IVec2;
    use meralus_world::Chunk;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_chunk_compressing() {
        let mut chunk = Chunk::new(IVec2::new(0, 0));

        chunk.generate_surface(0);

        let serialized = chunk.serialize();
        let mut compressed = Vec::new();

        let mut encoder = ZlibEncoder::new(&mut compressed);

        encoder.write_all(&serialized).await.unwrap();
        encoder.shutdown().await.unwrap();

        println!("Serialized: {} bytes. Compressed: {} bytes.", serialized.len(), compressed.len());

        let mut data = Vec::new();
        let mut decoder = ZlibDecoder::new(&mut data);

        decoder.write_all(&compressed).await.unwrap();
        decoder.shutdown().await.unwrap();

        let deserialized = Chunk::deserialize(&data).unwrap();

        assert_eq!(chunk.origin, deserialized.origin);
        // assert_eq!(chunk.blocks, deserialized.blocks);
        // assert_eq!(chunk.light_levels, deserialized.light_levels);
    }
}
