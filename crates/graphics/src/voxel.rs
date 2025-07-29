use std::{borrow::Borrow, hash::Hash};

use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use glam::{IVec2, Mat4, Vec2, Vec3};
use glium::{
    BackfaceCullingMode, Depth, DepthTest, DrawParameters, Frame, PolygonMode, Program, Surface, Texture2d, VertexBuffer,
    index::{NoIndices, PrimitiveType},
    uniform,
    uniforms::Sampler,
};
use meralus_engine::WindowDisplay;
use meralus_shared::{Color, FrustumCulling};
use meralus_world::{CHUNK_SIZE_F32, Face, SUBCHUNK_COUNT_F32};

use super::Shader;
use crate::{BLENDING, RenderInfo, impl_vertex};

struct VoxelShader;

impl Shader for VoxelShader {
    const FRAGMENT: &str = include_str!("../../app/resources/shaders/voxel.fs");
    const VERTEX: &str = include_str!("../../app/resources/shaders/voxel.vs");
}

pub struct Voxel {
    pub position: Vec3,
    pub origin: IVec2,

    pub vertices: [Vec3; 4],
    pub aos: [f32; 4],
    pub uvs: [Vec2; 4],

    pub face: Face,
    pub is_opaque: bool,
    pub light: u8,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct VoxelData {
    pub position: Vec3,
    pub uv: Vec2,
    pub color: Color,
    pub light: u8,
    pub visible: bool,
}

impl_vertex! {
    VoxelData {
        position: [f32; 3],
        uv: [f32; 2],
        color: [u8; 4],
        light: u8,
        visible: i8
    }
}

pub struct VoxelRenderer {
    shader: Program,
    opaque_data: HashMap<(IVec2, Face), VertexBuffer<VoxelData>>,
    translucent_data: HashMap<(IVec2, Face), VertexBuffer<VoxelData>>,
    world_mesh: HashMap<(IVec2, Face), [Vec</* (Vec3, [ */ VoxelData /* ; 6]) */>; 2]>,
    vertices: usize,
    draw_calls: usize,
    sun_position: f32,
    rendered_chunks: HashSet<(IVec2, Face)>,
    display: WindowDisplay,
}

impl VoxelRenderer {
    pub fn new(display: &WindowDisplay, world_mesh: HashMap<(IVec2, Face), [Vec<Voxel>; 2]>) -> Self {
        let world_mesh: HashMap<(IVec2, Face), [Vec<VoxelData>; 2]> = world_mesh
            .into_iter()
            .map(|(key, voxels)| {
                (
                    key,
                    voxels.map(|voxels| {
                        voxels.into_iter().fold(Vec::new(), |mut voxels, voxel| {
                            for i in [0, 1, 2, 2, 3, 0] {
                                voxels.push(VoxelData {
                                    position: voxel.position + voxel.vertices[i],
                                    light: voxel.light,
                                    uv: voxel.uvs[i],
                                    color: voxel.color.multiply_rgb(voxel.aos[i]),
                                    visible: true,
                                });
                            }

                            voxels
                        })
                    }),
                )
            })
            .collect();

        // println!("[{:18}] All DrawCall's for OpenGL created",
        // "INFO/Rendering".bright_green(),);

        Self {
            display: display.clone(),
            shader: VoxelShader::program(display),
            opaque_data: HashMap::new(),
            translucent_data: HashMap::new(),
            world_mesh,
            vertices: 0,
            draw_calls: 0,
            sun_position: 0.0,
            rendered_chunks: HashSet::new(),
        }
    }

    pub fn set_chunk(&mut self, display: &WindowDisplay, origin: IVec2, chunk: [(Face, [Vec<Voxel>; 2]); 6]) {
        // self.world_mesh.extend(chunk);

        for (face, voxels) in chunk {
            let voxels = voxels.map(|voxels| {
                voxels.into_iter().fold(Vec::new(), |mut voxels, voxel| {
                    for i in [0, 1, 2, 2, 3, 0] {
                        voxels.push(VoxelData {
                            position: voxel.position + voxel.vertices[i],
                            light: voxel.light,
                            uv: voxel.uvs[i],
                            color: voxel.color.multiply_rgb(voxel.aos[i]),
                            visible: true,
                        });
                    }

                    voxels
                })
            });

            self.opaque_data.insert((origin, face), VertexBuffer::new(display, &voxels[0]).unwrap());

            self.translucent_data.insert((origin, face), VertexBuffer::new(display, &voxels[1]).unwrap());

            self.world_mesh.insert((origin, face), voxels);
        }
    }

    pub const fn get_debug_info(&self) -> RenderInfo {
        RenderInfo {
            draw_calls: self.draw_calls,
            vertices: self.vertices,
        }
    }

    pub fn rendered_chunks(&self) -> usize {
        self.rendered_chunks.len()
    }

    pub fn total_chunks(&self) -> usize {
        self.world_mesh.len()
    }

    pub const fn set_sun_position(&mut self, value: f32) {
        self.sun_position = value;
    }

    fn is_chunk_visible(frustum: &FrustumCulling, origin: IVec2) -> bool {
        let origin = origin.as_vec2() * CHUNK_SIZE_F32;
        let origin = Vec3::new(origin.x, 0.0, origin.y);
        let chunk_size = CHUNK_SIZE_F32;
        let chunk_height = CHUNK_SIZE_F32 * SUBCHUNK_COUNT_F32;

        frustum.is_box_visible(origin, origin + Vec3::new(chunk_size, chunk_height, chunk_size))
    }

    pub fn contains_chunk<Q: ?Sized + Hash + Eq>(&self, k: &Q) -> bool
    where
        (IVec2, Face): Borrow<Q>,
    {
        self.opaque_data.contains_key(k) || self.translucent_data.contains_key(k)
    }

    pub fn render_with_params(&mut self, frame: &mut Frame, frustum: &FrustumCulling, matrix: Mat4, atlas: Sampler<'_, Texture2d>, params: &DrawParameters) {
        for key in self.world_mesh.keys() {
            if Self::is_chunk_visible(frustum, key.0) {
                if self.rendered_chunks.insert(*key) && !self.contains_chunk(key) {
                    let voxels = self.world_mesh.get(key).unwrap();

                    self.opaque_data.insert(*key, VertexBuffer::new(&self.display, &voxels[0]).unwrap());

                    self.translucent_data.insert(*key, VertexBuffer::new(&self.display, &voxels[1]).unwrap());
                }
            } else if self.rendered_chunks.remove(key) {
                self.opaque_data.remove(key);
                self.translucent_data.remove(key);
            }
        }

        let uniforms = uniform! {
            // origin: origin.to_array(),
            sun_position: [0.0, self.sun_position, 0.0],
            matrix: matrix.to_cols_array_2d(),
            tex: atlas,
            with_tex: true,
        };

        self.draw_calls = 0;

        for key in &self.rendered_chunks {
            if let Some(buffer) = self.opaque_data.get(key) {
                frame
                    .draw(buffer, NoIndices(PrimitiveType::TrianglesList), &self.shader, &uniforms, params)
                    .expect("failed to draw!");

                self.draw_calls += 1;
            }

            if let Some(buffer) = self.translucent_data.get(key) {
                frame
                    .draw(buffer, NoIndices(PrimitiveType::TrianglesList), &self.shader, &uniforms, params)
                    .expect("failed to draw!");

                self.draw_calls += 1;
            }
        }

        self.vertices = self
            .rendered_chunks
            .iter()
            .filter_map(|chunk| self.world_mesh.get(chunk).map(|voxels| voxels[0].len() + voxels[1].len()))
            .sum();
    }

    pub fn render(&mut self, frame: &mut Frame, frustum: &FrustumCulling, matrix: Mat4, atlas: Sampler<'_, Texture2d>, wireframe: bool) {
        self.render_with_params(frame, frustum, matrix, atlas, &DrawParameters {
            depth: Depth {
                test: DepthTest::IfLessOrEqual,
                write: true,
                ..Depth::default()
            },
            backface_culling: BackfaceCullingMode::CullCounterClockwise,
            polygon_mode: if wireframe { PolygonMode::Line } else { PolygonMode::Fill },
            blend: BLENDING,
            ..DrawParameters::default()
        });
    }
}
