// use std::{error::Error, sync::Arc, time::Duration};

// use ahash::{HashMap, HashMapExt};
// use glam::{IVec2, USizeVec3, Vec3};
// use meralus_shared::{IncomingPacket, OutgoingPacket, Player, ServerConnection};
// use meralus_world::{BfsLight, Chunk, ChunkGenerator, ChunkManager, LightNode, SUBCHUNK_SIZE};
// use tokio::{
//     fs,
//     net::TcpListener,
//     sync::{
//         RwLock,
//         mpsc::{self, Sender},
//     },
//     time::{Instant, sleep},
// };
// use uuid::Uuid;

// struct ServerState {
//     players: HashMap<Uuid, Player>,
//     player_channels: HashMap<Uuid, Sender<OutgoingPacket>>,
//     chunks: ChunkManager,
//     chunk_generator: ChunkGenerator,
// }

// impl ServerState {
//     pub async fn new() -> Self {
//         let mut chunks = ChunkManager::from_range(-3..3, &(-3..3));
//         let chunk_generator = ChunkGenerator::new(128);

//         let mut blocking_save = false;

//         for chunk in chunks.chunks_mut() {
//             let path = std::path::PathBuf::from(format!("world/{}_{}.bin", chunk.origin.x, chunk.origin.y));

//             if path.exists()
//                 && let Ok(data) = fs::read(path).await
//             {
//                 *chunk = Chunk::deserialize(data).unwrap();
//             } else {
//                 chunk_generator.generate_bare_terrain(chunk);

//                 blocking_save = true;
//             }
//         }

//         chunks.generate_sky_lights();

//         let state = Self {
//             players: HashMap::new(),
//             player_channels: HashMap::new(),
//             chunks,
//             chunk_generator,
//         };

//         if blocking_save {
//             state.blocking_save().await;
//         }

//         state
//     }

//     pub async fn blocking_save(&self) {
//         let path: &std::path::Path = "world".as_ref();

//         if !path.exists() {
//             fs::create_dir(path).await.unwrap();
//         }

//         for chunk in self.chunks.chunks() {
//             fs::write(format!("world/{}_{}.bin", chunk.origin.x, chunk.origin.y), chunk.serialize())
//                 .await
//                 .unwrap();
//         }
//     }

//     pub fn save(&self) {
//         let world = self.chunks.clone();

//         tokio::spawn(async move {
//             let path: &std::path::Path = "world".as_ref();

//             if !path.exists() {
//                 fs::create_dir(path).await.unwrap();
//             }

//             for chunk in world.take_chunks() {
//                 fs::write(format!("world/{}_{}.bin", chunk.origin.x, chunk.origin.y), chunk.into_serialized())
//                     .await
//                     .unwrap();
//             }

//             println!("Saved world");
//         });
//     }

//     pub fn load_chunk(&mut self, origin: IVec2) -> &Chunk {
//         let mut chunk = Chunk::new(origin);

//         self.chunk_generator.generate_bare_terrain(&mut chunk);

//         let mut queue = Vec::new();

//         for z in 0..SUBCHUNK_SIZE {
//             for x in 0..SUBCHUNK_SIZE {
//                 let position = USizeVec3::new(x, 255, z);

//                 if chunk.get_block_unchecked(position).is_none() {
//                     chunk.set_sky_light(position, 15);
//                     queue.push((LightNode(position, chunk.origin), 15));
//                 }
//             }
//         }

//         self.chunks.push(chunk);

//         let mut bfs_light = BfsLight::new(&mut self.chunks).apply_to_sky_light();

//         bfs_light.addition_queue = queue;
//         bfs_light.calculate();

//         let Some(chunk) = self.chunks.get_chunk(&origin) else { unreachable!() };

//         chunk
//     }

//     pub fn try_load_chunk(&mut self, origin: IVec2) -> &Chunk {
//         if self.chunks.contains_chunk(&origin) {
//             let Some(chunk) = self.chunks.get_chunk(&origin) else { unreachable!() };

//             chunk
//         } else {
//             self.load_chunk(origin)
//         }
//     }

//     pub fn players_excluding(&self, uuid: Uuid) -> impl Iterator<Item = &Sender<OutgoingPacket>> {
//         self.player_channels
//             .iter()
//             .filter_map(move |(key, value)| if key == &uuid { None } else { Some(value) })
//     }
// }

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     let server = TcpListener::bind("0.0.0.0:3248").await?;
//     let state = Arc::new(RwLock::new(ServerState::new().await));

//     println!("Server listening on {}", server.local_addr()?);

//     let cloned_state = state.clone();

//     tokio::spawn(async move {
//         loop {
//             sleep(Duration::from_mins(5)).await;

//             cloned_state.read().await.save();
//         }
//     });

//     loop {
//         let (socket, addr) = server.accept().await?;

//         println!("Accepted connection from {addr}");

//         let state = state.clone();

//         tokio::spawn(async move {
//             let mut connection = ServerConnection::new(socket);
//             let mut current_player_uuid = None;

//             let (tx, mut rx) = mpsc::channel::<OutgoingPacket>(128);

//             loop {
//                 tokio::select! {
//                     Some(packet) = rx.recv() => {
//                         println!("{packet:?}");

//                         connection.send(packet).await.unwrap();
//                     }
//                     packet = connection.receive() => {
//                         match packet {
//                             Some(Ok(packet)) => match packet {
//                                 IncomingPacket::PlayerConnected(name) => {
//                                     let player = Player::new(name, Vec3::ZERO);
//                                     let uuid = player.uuid;
//                                     let name = player.nickname.clone();

//                                     current_player_uuid.replace(uuid);

//                                     {
//                                         let mut state = state.write().await;

//                                         state.player_channels.insert(uuid, tx.clone());
//                                         state.players.insert(uuid, player);
//                                     }

//                                     connection.send(OutgoingPacket::UuidAssigned { uuid }).await.unwrap();

//                                     for player in state.read().await.players_excluding(uuid) {
//                                         player.send(OutgoingPacket::PlayerConnected { uuid, name: name.clone() }).await.unwrap();
//                                     }
//                                 }
//                                 packet => {
//                                     let Some(current_uuid) = current_player_uuid else {
//                                         continue;
//                                     };

//                                     match packet {
//                                         IncomingPacket::PlayerMoved { uuid, position } => {
//                                             if let Some(player) = state.write().await.players.get_mut(&uuid) {
//                                                 player.position = position;
//                                             }

//                                             for player in state.read().await.players_excluding(current_uuid) {
//                                                 player.send(OutgoingPacket::PlayerMoved { uuid, position }).await.unwrap();
//                                             }
//                                         }
//                                         IncomingPacket::GetPlayers => connection
//                                             .send(OutgoingPacket::PlayersList {
//                                                 players: state.read().await.players.values().cloned().collect(),
//                                             })
//                                             .await
//                                             .unwrap(),
//                                         IncomingPacket::RemoveBlock(chunk, block) => {
//                                             {
//                                                 let mut state = state.write().await;

//                                                 state.chunks.get_chunk_mut(&chunk).unwrap().set_block(block, 0);

//                                                 let mut bfs_light = BfsLight::new(&mut state.chunks).apply_to_sky_light();

//                                                 bfs_light.remove(LightNode(block, chunk));
//                                                 bfs_light.calculate();

//                                                 let mut queue = Vec::new();
//                                                 let up = block + USizeVec3::Y;

//                                                 if up.y < 256 && bfs_light.chunk_manager[chunk].get_sky_light(up) == 15 {
//                                                     let mut y = block.y;

//                                                     loop {
//                                                         if bfs_light.chunk_manager[chunk].get_block(block.with_y(y)).is_some() {
//                                                             break;
//                                                         }

//                                                         queue.push((LightNode(block.with_y(y), chunk), 15));

//                                                         if y == 0 {
//                                                             break;
//                                                         }

//                                                         y -= 1;
//                                                     }
//                                                 }

//                                                 bfs_light.addition_queue = queue;
//                                                 bfs_light.calculate();
//                                             }

//                                             for player in state.read().await.players_excluding(current_uuid) {
//                                                 player.send(OutgoingPacket::RemoveBlock(chunk, block)).await.unwrap();
//                                             }
//                                         }
//                                         IncomingPacket::RequestChunk(origin) => {
//                                             let data = state.write().await.try_load_chunk(origin).serialize();

//                                             connection.send(OutgoingPacket::ChunkData { data }).await.unwrap();
//                                         }
//                                         IncomingPacket::PlayerConnected { .. } => unreachable!(),
//                                     }
//                                 }
//                             },
//                             Some(Err(err)) => println!("{err}"),
//                             None => break,
//                         }
//                     }
//                 }
//             }

//             println!("Closed connection from {addr}");

//             if let Some(uuid) = current_player_uuid {
//                 state.write().await.players.remove(&uuid);
//                 state.write().await.player_channels.remove(&uuid);

//                 for player in state.read().await.player_channels.values() {
//                     player.send(OutgoingPacket::PlayerDisonnected { uuid }).await.unwrap();
//                 }
//             }
//         });
//     }
// }

// #[cfg(test)]
// mod tests {
//     use async_compression::tokio::write::{ZlibDecoder, ZlibEncoder};
//     use glam::IVec2;
//     use meralus_world::Chunk;
//     use tokio::io::AsyncWriteExt;

//     #[tokio::test]
//     async fn test_chunk_compressing() {
//         let mut chunk = Chunk::new(IVec2::new(0, 0));

//         chunk.generate_surface(0);

//         let serialized = chunk.serialize();
//         let mut compressed = Vec::new();

//         let mut encoder = ZlibEncoder::new(&mut compressed);

//         encoder.write_all(&serialized).await.unwrap();
//         encoder.shutdown().await.unwrap();

//         println!("Serialized: {} bytes. Compressed: {} bytes.", serialized.len(), compressed.len());

//         let mut data = Vec::new();
//         let mut decoder = ZlibDecoder::new(&mut data);

//         decoder.write_all(&compressed).await.unwrap();
//         decoder.shutdown().await.unwrap();

//         let deserialized = Chunk::deserialize(&data).unwrap();

//         assert_eq!(chunk.origin, deserialized.origin);
//         // assert_eq!(chunk.blocks, deserialized.blocks);
//         // assert_eq!(chunk.light_levels, deserialized.light_levels);
//     }
// }

fn main() {
    
}