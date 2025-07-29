use std::time::Duration;

use ahash::{HashMap, HashMapExt};
use glam::{DVec3, IVec2, Mat4, Vec3, u16vec3, vec3};
use meralus_animation::AnimationPlayer;
use meralus_engine::WindowDisplay;
use meralus_graphics::{Line, Voxel, VoxelRenderer};
use meralus_shared::{Color, Cube3D, Lerp, Point3D, Ranged, Size3D};
use meralus_world::{Axis, CHUNK_HEIGHT_F32, CHUNK_SIZE, CHUNK_SIZE_F32, CHUNK_SIZE_U16, Chunk, ChunkManager, Face, SUBCHUNK_COUNT_U16};
use owo_colors::OwoColorize;

use crate::{camera::Camera, clock::Clock, game::WorldMesh, util::cube_outline, vertex_ao, BakedBlockModelLoader, BfsLight, LightNode, PlayerController, INVENTORY_HOTBAR_SLOTS, TPS};

const GRASS_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);

pub struct World {
    pub clock: Clock,
    pub tick_accel: Duration,
    pub ticks: usize,
    pub tick_sum: usize,
    pub current_tick: usize,

    pub camera: Camera,
    pub player: PlayerController,
    pub player_controllable: bool,
    pub inventory_slot: Ranged<u8>,

    pub chunk_manager: ChunkManager,
    pub voxel_renderer: VoxelRenderer,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Colliders {
    pub top: Option<DVec3>,
    pub bottom: Option<DVec3>,
    pub left: Option<DVec3>,
    pub right: Option<DVec3>,
    pub front: Option<DVec3>,
    pub back: Option<DVec3>,
}

impl World {
    pub fn new(world_mesh: WorldMesh, display: &WindowDisplay) -> Self {
        let player = PlayerController {
            position: vec3(2.0, 275.0, 2.0),
            ..Default::default()
        };

        Self {
            camera: Camera {
                position: player.position,
                up: player.up,
                target: player.position + player.front,
                ..Camera::default()
            },
            ticks: 0,
            tick_sum: 0,
            current_tick: 0,
            voxel_renderer: VoxelRenderer::new(display, world_mesh),
            tick_accel: Duration::ZERO,
            player,
            player_controllable: true,
            inventory_slot: Ranged::new(0, 0, INVENTORY_HOTBAR_SLOTS),
            clock: Clock::default(),
            chunk_manager: ChunkManager::new(),
        }
    }

    pub fn tick(&mut self, animation_player: &mut AnimationPlayer, paused: bool) {
        self.tick_sum += 1;
        self.current_tick = self.tick_sum % TPS;

        if !paused {
            self.clock.tick();

            let progress = self.clock.get_progress();

            self.voxel_renderer.set_sun_position(if progress > 0.5 { 1.0 - progress } else { progress });

            if self.current_tick == 0 {
                let (is_after, progress) = self.clock.get_visual_progress();

                let angle = 90f32.to_radians()
                    + if is_after {
                        0f32.lerp(&180.0, progress)
                    } else {
                        180f32.lerp(&360.0, progress)
                    }
                    .to_radians();

                animation_player.get_mut("sun").unwrap().to(angle);
                animation_player.play("sun");
            }
        }
    }

    pub fn chunk_borders(&self) -> Vec<Line> {
        self.chunk_manager.chunks().fold(Vec::new(), |mut lines, Chunk { origin, .. }| {
            let origin = origin.as_vec2() * CHUNK_SIZE_F32;

            lines.extend(cube_outline(Cube3D::new(
                Point3D::new(origin.x, 0.0, origin.y),
                Size3D::new(CHUNK_SIZE_F32, CHUNK_HEIGHT_F32, CHUNK_SIZE_F32),
            )));

            lines
        })
    }

    pub fn generate_world(&mut self, seed: u32) {
        self.chunk_manager.generate_surface(seed);
    }

    pub fn update_block_sky_light(&mut self, models: &BakedBlockModelLoader, position: Vec3) {
        let mut bfs_light = BfsLight::new();

        for face in Face::ALL {
            let position = position + face.as_normal().as_vec3();

            if let Some(chunk) = self.chunk_manager.get_chunk(&ChunkManager::to_local(position)) {
                let local = chunk.to_local(position);

                if !chunk.contains_local_position(local) {
                    continue;
                }

                if chunk.get_block_unchecked(local).is_none() {
                    bfs_light.push(LightNode(local, chunk.origin));
                }
            }
        }

        bfs_light.calculate(&mut self.chunk_manager, models, true);
    }

    pub fn generate_lights(&mut self, models: &BakedBlockModelLoader) {
        let mut bfs_light = BfsLight::new();

        for chunk in self.chunk_manager.chunks_mut() {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let position = u16vec3(x as u16, 255, z as u16);

                    if chunk
                        .get_block_unchecked(position)
                        .is_none_or(|block| !models.get(block.into()).unwrap().is_opaque())
                    {
                        chunk.set_sky_light(position, 15);

                        bfs_light.push(LightNode(position, chunk.origin));
                    }
                }
            }
        }

        bfs_light.calculate(&mut self.chunk_manager, models, true);
    }

    pub fn set_block_light(&mut self, models: &BakedBlockModelLoader, position: Vec3, light_level: u8) {
        let mut bfs_light = BfsLight::new();

        if let Some(chunk) = self.chunk_manager.get_chunk_mut(&ChunkManager::to_local(position)) {
            let position = chunk.to_local(position);

            chunk.set_block_light(position, light_level);

            bfs_light.push(LightNode(position, chunk.origin));
        }

        bfs_light.calculate(&mut self.chunk_manager, models, false);
    }

    pub fn compute_chunk_mesh_at(&self, models: &BakedBlockModelLoader, position: IVec2) -> Option<[(Face, [Vec<Voxel>; 2]); 6]> {
        self.chunk_manager.get_chunk(&position).map(|chunk| self.compute_chunk_mesh(models, chunk))
    }

    #[allow(clippy::too_many_lines)]
    pub fn compute_chunk_mesh(&self, models: &BakedBlockModelLoader, chunk: &Chunk) -> [(Face, [Vec<Voxel>; 2]); 6] {
        let origin = chunk.origin.as_vec2();
        let mut voxels = Face::ALL.map(|face| (face, [const { Vec::new() }; 2]));

        for y in 0..(CHUNK_SIZE_U16 * SUBCHUNK_COUNT_U16) {
            for z in 0..CHUNK_SIZE_U16 {
                for x in 0..CHUNK_SIZE_U16 {
                    let local_position = u16vec3(x, y, z);
                    let world_position = local_position.as_vec3() + (vec3(origin.x, 0.0, origin.y) * CHUNK_SIZE_F32);

                    if let Some(model) = chunk.get_block(local_position).and_then(|block_id| models.get(block_id.into())) {
                        let position = local_position.as_vec3() + (vec3(origin.x, 0.0, origin.y) * CHUNK_SIZE_F32);

                        for element in &model.elements {
                            let matrix = element.rotation.map(|rotation| {
                                let angle = rotation.angle.to_radians();

                                let matrix;
                                let mut scale = Vec3::ZERO;

                                match rotation.axis {
                                    Axis::X => {
                                        matrix = Mat4::from_rotation_x(angle);

                                        scale.y = 1.0;
                                        scale.z = 1.0;
                                    }
                                    Axis::Y => {
                                        matrix = Mat4::from_rotation_y(angle);

                                        scale.x = 1.0;
                                        scale.z = 1.0;
                                    }
                                    Axis::Z => {
                                        matrix = Mat4::from_rotation_z(angle);

                                        scale.x = 1.0;
                                        scale.y = 1.0;
                                    }
                                }

                                scale = Vec3::ONE;

                                (matrix, rotation.origin, scale)
                            });

                            for model_face in element.faces.iter().flatten() {
                                let neighbour_position = world_position + model_face.face.as_normal().as_vec3();

                                let culled = model_face.cull_face.is_some_and(|cull_face| {
                                    let neighbour = self.chunk_manager.get_block(world_position + cull_face.as_normal().as_vec3());

                                    neighbour.and_then(|neighbour| models.get(neighbour.into())).is_some_and(|model| {
                                        if model.is_opaque() {
                                            true
                                        } else {
                                            let opposite_face = cull_face.opposite();

                                            model.elements.iter().any(|element| {
                                                element.faces[opposite_face.normal_index()]
                                                    .as_ref()
                                                    .is_some_and(|face| if face.is_opaque { true } else { face.uv.eq(&model_face.uv) })
                                            })
                                        }
                                    })
                                });

                                if !culled {
                                    let mut vertices = model_face.face.as_vertices().map(|vertice| {
                                        Vec3::from_array(element.cube.origin.to_array()) + vertice * Vec3::from_array(element.cube.size.to_array())
                                    });

                                    let mut uvs = model_face.face.as_uv();

                                    let mut aos = model_face.face.as_vertice_corners().map(|corner| {
                                        let [side1, side2, corner] = corner.get_neighbours(model_face.face).map(|neighbour| {
                                            self.chunk_manager
                                                .get_block(position + neighbour.as_vec3())
                                                .is_some_and(|block| models.get(block.into()).unwrap().ambient_occlusion)
                                        });

                                        vertex_ao(side1, side2, corner)
                                    });

                                    // let mut aos_flipped = false;

                                    if aos[1] + aos[2] > aos[0] + aos[3] {
                                        // aos_flipped = true;

                                        // aos = aos[1], aos[2], aos[3], aos[0]

                                        vertices.swap(0, 1);
                                        vertices.swap(1, 2);
                                        vertices.swap(2, 3);

                                        aos.swap(0, 1);
                                        aos.swap(1, 2);
                                        aos.swap(2, 3);

                                        uvs.swap(0, 1);
                                        uvs.swap(1, 2);
                                        uvs.swap(2, 3);
                                    }

                                    let vertices = matrix.map_or(vertices, |(matrix, origin, scale)| {
                                        vertices.map(|vertice| matrix.transform_point3(vertice - origin) * scale + origin)
                                    });

                                    let voxels = &mut voxels[model_face.face.normal_index()].1;
                                    let voxels = if model_face.is_opaque { &mut voxels[0] } else { &mut voxels[1] };

                                    voxels.push(Voxel {
                                        position,
                                        vertices,
                                        face: model_face.face,
                                        origin: chunk.origin,
                                        aos,
                                        light: self.chunk_manager.get_light(neighbour_position),
                                        color: if model.name == "grass_block" && model_face.tint {
                                            GRASS_COLOR
                                        } else {
                                            Color::WHITE
                                        },
                                        uvs: uvs.map(|uv| model_face.uv.offset + uv * model_face.uv.scale),
                                        is_opaque: model_face.is_opaque,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        voxels
    }

    #[must_use]
    pub fn compute_world_mesh(&self, models: &BakedBlockModelLoader) -> WorldMesh {
        let mut meshes = HashMap::new();

        for chunk in self.chunk_manager.chunks() {
            for (face, data) in self.compute_chunk_mesh(models, chunk) {
                meshes.insert((chunk.origin, face), data);
            }

            println!(
                "[{:18}] Generated mesh for chunk at {}",
                "INFO/Rendering".bright_green(),
                format!("{:>2} {:>2}", chunk.origin.x, chunk.origin.y).bright_blue().bold()
            );
        }

        meshes
    }
}
