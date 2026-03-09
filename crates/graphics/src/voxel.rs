use std::{array, borrow::Borrow, collections::hash_map::Entry, hash::Hash};

use ahash::{HashMap, HashMapExt};
use glium::{
    BackfaceCullingMode, Depth, DepthTest, DrawParameters, Frame, IndexBuffer, PolygonMode, Program, Surface, Texture2d, VertexBuffer,
    index::{IndicesSource, PrimitiveType},
    uniform,
    uniforms::Sampler,
};
use indexmap::IndexMap;
use meralus_engine::WindowDisplay;
use meralus_shared::{Color, Frustum, FrustumCulling, IPoint2D, Point2D, Point3D, Transform3D};
use meralus_world::{ChunkManager, SUBCHUNK_COUNT, SUBCHUNK_COUNT_F32, SUBCHUNK_SIZE, SUBCHUNK_SIZE_F32};

use super::Shader;
use crate::{BLENDING, CachedBuffers, RenderInfo, impl_vertex};

struct VoxelShader;

impl Shader for VoxelShader {
    const FRAGMENT: &str = "./resources/shaders/voxel.fs";
    const VERTEX: &str = "./resources/shaders/voxel.vs";
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelFace {
    pub position: Point3D,
    pub vertices: [Point3D; 4],
    pub uvs: [Point2D; 4],
    pub lights: [u8; 4],
    pub color: Color,
}

impl VoxelFace {
    fn cmp(&self, camera_pos: Point3D, other: &Self) -> std::cmp::Ordering {
        camera_pos
            .distance_squared(self.position)
            .total_cmp(&camera_pos.distance_squared(other.position))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct VoxelVertex {
    pub position: Point3D,
    pub uv: Point2D,
    pub color: Color,
    pub light: u8,
    pub visible: bool,
}

impl_vertex! {
    VoxelVertex {
        visible: i8,
        light: u8,
        color: [u8; 4],
        uv: [f32; 2],
        position: [f32; 3]
    }
}

pub type SubChunkKey = (IPoint2D, usize);
pub type SubChunkMesh = [(Vec<VoxelVertex>, Vec<u32>); 2];
pub type WorldMesh = HashMap<IPoint2D, [[Vec<VoxelFace>; 2]; SUBCHUNK_COUNT]>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubchunkState {
    Rendered,
    Dirty,
    Hidden,
}

impl SubchunkState {
    /// Returns `true` if the subchunk state is [`Rendered`].
    ///
    /// [`Rendered`]: SubchunkState::Rendered
    #[must_use]
    const fn is_rendered(self) -> bool {
        matches!(self, Self::Rendered)
    }
}

struct RenderChunk {
    subchunk_states: [SubchunkState; SUBCHUNK_COUNT],
    solid_buffer: CachedBuffers<VoxelVertex, u32>,
    translucent_buffer: CachedBuffers<VoxelVertex, u32>,
}

pub struct VoxelMeshBuilder {
    vertices: Vec<VoxelVertex>,
    indices: Vec<u32>,
    offset: u32,
}

impl VoxelMeshBuilder {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(capacity * 4),
            indices: Vec::with_capacity(capacity * 6),
            offset: 0,
        }
    }

    pub fn extend_from_slice(&mut self, voxels: &[VoxelFace]) {
        for voxel in voxels {
            self.push(voxel);
        }
    }

    pub fn push_transformed(&mut self, voxel: &VoxelFace, matrix: &Transform3D, origin: Point3D) {
        self.vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + matrix.transform_point3(voxel.vertices[i] - origin.to_vector()) + origin,
            light: voxel.lights[i],
            uv: voxel.uvs[i],
            color: voxel.color,
            visible: true,
        }));

        // if  {
        //     self.indices
        //         .extend([self.offset + 1, self.offset + 3, self.offset, self.offset +
        // 2, self.offset, self.offset + 3]); } else {
        self.indices
            .extend([self.offset, self.offset + 1, self.offset + 2, self.offset + 3, self.offset + 2, self.offset + 1]);
        // }

        self.offset += 4;
    }

    pub fn push(&mut self, voxel: &VoxelFace) {
        self.vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + voxel.vertices[i],
            light: voxel.lights[i],
            uv: voxel.uvs[i],
            color: voxel.color,
            visible: true,
        }));

        self.indices
            .extend([self.offset, self.offset + 1, self.offset + 2, self.offset + 3, self.offset + 2, self.offset + 1]);

        self.offset += 4;
    }

    pub fn render_full_bright(
        self,
        renderer: &VoxelRenderer,
        frame: &mut Frame,
        wireframe: bool,
        matrix: Transform3D,
        atlas: Sampler<'_, Texture2d>,
        lightmap: Sampler<'_, Texture2d>,
    ) {
        let (vertices, indices) = self.build(&renderer.display);

        renderer.draw_full_bright(frame, &vertices, &indices, wireframe, matrix, atlas, lightmap);
    }

    pub fn render<T: Surface>(
        self,
        renderer: &VoxelRenderer,
        frame: &mut T,
        wireframe: bool,
        matrix: Transform3D,
        atlas: Sampler<'_, Texture2d>,
        lightmap: Sampler<'_, Texture2d>,
    ) {
        let (vertices, indices) = self.build(&renderer.display);

        renderer.draw(frame, &vertices, &indices, wireframe, matrix, atlas, lightmap);
    }

    pub fn build(self, display: &WindowDisplay) -> (VertexBuffer<VoxelVertex>, IndexBuffer<u32>) {
        (
            VertexBuffer::new(display, &self.vertices).unwrap(),
            IndexBuffer::new(display, PrimitiveType::TrianglesList, &self.indices).unwrap(),
        )
    }

    pub fn build_buffers(self, display: &WindowDisplay) -> CachedBuffers<VoxelVertex, u32> {
        CachedBuffers::new(display, &self.vertices, PrimitiveType::TrianglesList, &self.indices).unwrap()
    }
}

pub struct VoxelRenderer {
    shader: Program,
    world_mesh: WorldMesh,
    vertices: usize,
    draw_calls: usize,
    sun_position: f32,
    rendered_chunks: IndexMap<IPoint2D, RenderChunk>,
    display: WindowDisplay,
}

impl VoxelRenderer {
    pub fn new(display: &WindowDisplay) -> Self {
        Self {
            display: display.clone(),
            shader: VoxelShader::program(display),
            world_mesh: HashMap::new(),
            vertices: 0,
            draw_calls: 0,
            sun_position: 0.0,
            rendered_chunks: IndexMap::new(),
        }
    }

    pub fn push_voxel_mesh(voxel: &VoxelFace, offset: &mut u32, vertices: &mut Vec<VoxelVertex>, indices: &mut Vec<u32>) {
        vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + voxel.vertices[i],
            light: voxel.lights[i],
            uv: voxel.uvs[i],
            color: voxel.color,
            visible: true,
        }));

        // 0, 1, 2, 3, 2, 1
        indices.extend([*offset, *offset + 1, *offset + 2, *offset + 3, *offset + 2, *offset + 1]);

        *offset += 4;
    }

    pub fn get_voxels_mesh(voxels: &[VoxelFace]) -> (Vec<VoxelVertex>, Vec<u32>) {
        let count = voxels.len();
        let mut vertices = Vec::with_capacity(count * 4);
        let mut indices = Vec::with_capacity(count * 6);

        let mut offset = 0;

        for voxel in voxels {
            Self::push_voxel_mesh(voxel, &mut offset, &mut vertices, &mut indices);
        }

        (vertices, indices)
    }

    pub fn set_subchunk(&mut self, origin: (IPoint2D, usize), [opaque, translucent]: [Vec<VoxelFace>; 2]) {
        match self.world_mesh.entry(origin.0) {
            Entry::Occupied(mut occupied_entry) => {
                occupied_entry.get_mut()[origin.1] = [opaque, translucent];

                if let Some(chunk) = self.rendered_chunks.get_mut(&origin.0) {
                    chunk.subchunk_states[origin.1] = SubchunkState::Dirty;
                }
            }
            Entry::Vacant(vacant_entry) => {
                let mut subchunks = array::from_fn(|_| [Vec::new(), Vec::new()]);

                subchunks[origin.1] = [opaque, translucent];

                vacant_entry.insert(subchunks);
            }
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

    pub fn total_subchunks(&self) -> usize {
        self.world_mesh.len() * SUBCHUNK_COUNT
    }

    pub const fn set_sun_position(&mut self, value: f32) {
        self.sun_position = value;
    }

    fn is_chunk_visible<T: Frustum>(frustum: &T, origin: IPoint2D) -> bool {
        let origin = origin.as_::<f32>() * SUBCHUNK_SIZE_F32;
        let origin = Point3D::new(origin.x, 0.0, origin.y);
        let chunk_size = SUBCHUNK_SIZE_F32;
        let chunk_height = SUBCHUNK_SIZE_F32 * SUBCHUNK_COUNT_F32;

        frustum.is_box_visible(origin, origin + Point3D::new(chunk_size, chunk_height, chunk_size))
    }

    fn is_subchunk_visible<T: Frustum>(frustum: &T, (origin, subchunk): (IPoint2D, usize)) -> bool {
        let origin = origin.as_::<f32>() * SUBCHUNK_SIZE_F32;
        let y = (subchunk * SUBCHUNK_SIZE) as f32;
        let origin = Point3D::new(origin.x, y, origin.y);
        let chunk_size = SUBCHUNK_SIZE_F32;
        let chunk_height = SUBCHUNK_SIZE_F32;

        frustum.is_box_visible(origin, origin + Point3D::new(chunk_size, chunk_height, chunk_size))
    }

    pub fn contains_chunk<Q: ?Sized + Hash + Eq>(&self, k: &Q) -> bool
    where
        IPoint2D: Borrow<Q>,
    {
        self.rendered_chunks.contains_key(k)
    }

    pub fn draw_full_bright<'a, I: Into<IndicesSource<'a>>>(
        &self,
        frame: &mut Frame,
        vertices: &VertexBuffer<VoxelVertex>,
        indices: I,
        wireframe: bool,
        matrix: Transform3D,
        atlas: Sampler<'_, Texture2d>,
        lightmap: Sampler<'_, Texture2d>,
    ) {
        let uniforms = uniform! {
            // origin: origin.to_array(),
            sun_position: [0.0, const { (1.0 - 0.5) / 0.96 }, 0.0f32],
            matrix: matrix.to_cols_array_2d(),
            tex: atlas,
            lightmap: lightmap,
            with_tex: true,
        };

        frame
            .draw(vertices, indices, &self.shader, &uniforms, &DrawParameters {
                // depth: Depth {
                //     test: DepthTest::IfLessOrEqual,
                //     write: true,
                //     ..Depth::default()
                // },
                backface_culling: BackfaceCullingMode::CullCounterClockwise,
                polygon_mode: if wireframe { PolygonMode::Line } else { PolygonMode::Fill },
                blend: BLENDING,
                ..DrawParameters::default()
            })
            .unwrap();
    }

    pub fn draw<'a, F: Surface, I: Into<IndicesSource<'a>>>(
        &self,
        frame: &mut F,
        vertices: &VertexBuffer<VoxelVertex>,
        indices: I,
        wireframe: bool,
        matrix: Transform3D,
        atlas: Sampler<'_, Texture2d>,
        lightmap: Sampler<'_, Texture2d>,
    ) {
        let uniforms = uniform! {
            // origin: origin.to_array(),
            sun_position: [0.0, self.sun_position, 0.0],
            matrix: matrix.to_cols_array_2d(),
            tex: atlas,
            lightmap: lightmap,
            with_tex: true,
        };

        frame
            .draw(vertices, indices, &self.shader, &uniforms, &DrawParameters {
                depth: Depth {
                    test: DepthTest::IfLessOrEqual,
                    write: true,
                    ..Depth::default()
                },
                backface_culling: BackfaceCullingMode::CullCounterClockwise,
                polygon_mode: if wireframe { PolygonMode::Line } else { PolygonMode::Fill },
                blend: BLENDING,
                ..DrawParameters::default()
            })
            .unwrap();
    }

    pub fn render_with_params<F: Surface, T: Frustum>(
        &mut self,
        frame: &mut F,
        camera_pos: Point3D,
        frustum: &T,
        matrix: Transform3D,
        atlas: Sampler<'_, Texture2d>,
        lightmap: Sampler<'_, Texture2d>,
        params: &DrawParameters,
    ) {
        for (&origin, subchunks) in &mut self.world_mesh {
            if Self::is_chunk_visible(frustum, origin) {
                let mut rendered_subchunks = 0;
                let subchunk_states = array::from_fn(|i| {
                    if Self::is_subchunk_visible(frustum, (origin, i)) {
                        rendered_subchunks += 1;

                        SubchunkState::Rendered
                    } else {
                        SubchunkState::Hidden
                    }
                });

                match self.rendered_chunks.entry(origin) {
                    indexmap::map::Entry::Occupied(mut entry) => {
                        let entry = entry.get_mut();

                        if entry.subchunk_states != subchunk_states {
                            let mut solid_faces = Vec::with_capacity(rendered_subchunks * SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE * 6);
                            let mut translucent_faces = Vec::with_capacity(rendered_subchunks * SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE * 6);

                            // generate chunk rendering data if any of subchunks are dirty or now visible
                            for (state, subchunk) in subchunk_states.iter().zip(subchunks) {
                                if state.is_rendered() {
                                    solid_faces.extend_from_slice(&subchunk[0]);
                                    translucent_faces.extend_from_slice(&subchunk[1]);
                                }
                            }

                            solid_faces.sort_unstable_by(|a, b| a.cmp(camera_pos, b));
                            translucent_faces.sort_unstable_by(|a, b| a.cmp(camera_pos, b).reverse());

                            let solid_voxels = Self::get_voxels_mesh(&solid_faces);
                            let translucent_voxels = Self::get_voxels_mesh(&translucent_faces);

                            *entry = RenderChunk {
                                subchunk_states,
                                solid_buffer: CachedBuffers::new(&self.display, &solid_voxels.0, PrimitiveType::TrianglesList, &solid_voxels.1).unwrap(),
                                translucent_buffer: CachedBuffers::new(
                                    &self.display,
                                    &translucent_voxels.0,
                                    PrimitiveType::TrianglesList,
                                    &translucent_voxels.1,
                                )
                                .unwrap(),
                            };
                        }
                    }
                    indexmap::map::Entry::Vacant(entry) => {
                        let mut solid_faces = Vec::with_capacity(rendered_subchunks * SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE * 6);
                        let mut translucent_faces = Vec::with_capacity(rendered_subchunks * SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE * 6);

                        // generate chunk rendering data if any of subchunks are dirty or now visible
                        for (state, subchunk) in subchunk_states.iter().zip(subchunks) {
                            if state.is_rendered() {
                                solid_faces.extend_from_slice(&subchunk[0]);
                                translucent_faces.extend_from_slice(&subchunk[1]);
                            }
                        }

                        solid_faces.sort_unstable_by(|a, b| a.cmp(camera_pos, b));
                        translucent_faces.sort_unstable_by(|a, b| a.cmp(camera_pos, b).reverse());

                        let solid_voxels = Self::get_voxels_mesh(&solid_faces);
                        let translucent_voxels = Self::get_voxels_mesh(&translucent_faces);

                        entry.insert(RenderChunk {
                            subchunk_states,
                            solid_buffer: CachedBuffers::new(&self.display, &solid_voxels.0, PrimitiveType::TrianglesList, &solid_voxels.1).unwrap(),
                            translucent_buffer: CachedBuffers::new(&self.display, &translucent_voxels.0, PrimitiveType::TrianglesList, &translucent_voxels.1)
                                .unwrap(),
                        });
                    }
                }
            } else {
                self.rendered_chunks.swap_remove(&origin);
            }
        }

        let uniforms = uniform! {
            // origin: origin.to_array(),
            sun_position: [0.0, self.sun_position, 0.0],
            matrix: matrix.to_cols_array_2d(),
            tex: atlas,
            lightmap: lightmap,
            with_tex: true,
        };

        self.draw_calls = 0;

        self.rendered_chunks.sort_unstable_by(|&a, _, &b, _| {
            let a = (ChunkManager::to_local(camera_pos.as_()) - a).as_::<f32>().length_squared();
            let b = (ChunkManager::to_local(camera_pos.as_()) - b).as_::<f32>().length_squared();

            a.total_cmp(&b)
        });

        for chunk in self.rendered_chunks.values() {
            if chunk.solid_buffer.vertices.len() > 0 {
                frame
                    .draw(&chunk.solid_buffer.vertices, &chunk.solid_buffer.indices, &self.shader, &uniforms, params)
                    .expect("failed to draw!");

                self.draw_calls += 1;
            }
        }

        self.rendered_chunks.reverse();

        for chunk in self.rendered_chunks.values() {
            if chunk.translucent_buffer.vertices.len() > 0 {
                frame
                    .draw(
                        &chunk.translucent_buffer.vertices,
                        &chunk.translucent_buffer.indices,
                        &self.shader,
                        &uniforms,
                        params,
                    )
                    .expect("failed to draw!");

                self.draw_calls += 1;
            }
        }

        self.vertices = self
            .rendered_chunks
            .values()
            .map(|chunk| chunk.solid_buffer.vertices.len() + chunk.translucent_buffer.vertices.len())
            .sum();
    }

    pub fn render<T: Surface>(
        &mut self,
        frame: &mut T,
        camera_pos: Point3D,
        frustum: &FrustumCulling,
        matrix: Transform3D,
        atlas: Sampler<'_, Texture2d>,
        lightmap: Sampler<'_, Texture2d>,
        wireframe: bool,
    ) {
        self.render_with_params(frame, camera_pos, frustum, matrix, atlas, lightmap, &DrawParameters {
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
