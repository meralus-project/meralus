use std::{borrow::Borrow, hash::Hash};

use horns::{
    BackfaceCullingMode, Blend, BlendingFactor, Depth, DepthTest, DrawParams, ElementType, IndexBuffer, Program, RenderBackend, RenderPass, SampledTexture2d,
    VertexBuffer, create_shader, impl_vertex,
};
use indexmap::IndexMap;
use meralus_shared::{AsValue, Color, FromValue, Frustum, FrustumCulling, IPoint2D, Point2D, Point3D, Transform3D};
use meralus_world::{ChunkManager, SUBCHUNK_SIZE, SUBCHUNK_SIZE_F32};

use crate::{
    get_sky_color,
    render::{RenderBuffer, context::RenderInfo},
};

create_shader!(pub VoxelShader => "./resources/shaders/voxel");

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

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct VoxelVertex {
    pub position: Point3D,
    pub uv: Point2D,
    pub color: [u8; 4],
    pub light: u32,
}

impl_vertex! {
    VoxelVertex {
        position: [f32; 3],
        uv: [f32; 2],
        color: [u8; 4],
        light: [u32; 1]
    }
}

struct RenderSubchunk {
    solid: RenderBuffer<VoxelVertex, VoxelShader, u32>,
    translucent: RenderBuffer<VoxelVertex, VoxelShader, u32>,
}

pub struct VoxelMeshBuilder {
    vertices: Vec<VoxelVertex>,
    indices: Vec<u32>,
    offset: u32,
}

impl VoxelMeshBuilder {
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(capacity * 4),
            indices: Vec::with_capacity(capacity * 6),
            offset: 0,
        }
    }

    #[inline]
    pub fn build_from_slice(backend: &RenderBackend, shader: &Program, voxels: &[VoxelFace]) -> RenderBuffer<VoxelVertex, VoxelShader, u32> {
        let mut this = Self::new();

        this.extend_from_slice(voxels);

        this.build(backend, shader)
    }

    #[inline]
    pub fn extend_from_slice(&mut self, voxels: &[VoxelFace]) {
        for voxel in voxels {
            self.push(voxel);
        }
    }

    #[inline]
    fn push_indices(&mut self) {
        let offset = self.offset;

        self.offset += 4;
        self.indices.extend([offset, offset + 1, offset + 2, offset + 3, offset + 2, offset + 1]);
    }

    #[inline]
    pub fn push_transformed(&mut self, voxel: &VoxelFace, matrix: &Transform3D, origin: Point3D) {
        self.vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + matrix.transform_point3(voxel.vertices[i] - origin) + origin,
            light: voxel.lights[i] as u32,
            uv: voxel.uvs[i],
            color: voxel.color.as_value(),
        }));

        self.push_indices();
    }

    #[inline]
    pub fn push(&mut self, voxel: &VoxelFace) {
        self.vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + voxel.vertices[i],
            light: voxel.lights[i] as u32,
            uv: voxel.uvs[i],
            color: voxel.color.as_value(),
        }));

        self.push_indices();
    }

    #[inline]
    pub fn render_full_bright(
        self,
        backend: &RenderBackend,
        renderer: &ChunkRenderer,
        pass: &mut RenderPass,
        matrix: Transform3D,
        atlas: SampledTexture2d,
        lightmap: SampledTexture2d,
    ) -> RenderInfo {
        let buffer = self.build(backend, &renderer.shader);

        renderer.render_buffer(pass, &buffer, matrix, atlas, lightmap, true)
    }

    #[inline]
    pub fn render(
        self,
        backend: &RenderBackend,
        renderer: &ChunkRenderer,
        pass: &mut RenderPass,
        matrix: Transform3D,
        atlas: SampledTexture2d,
        lightmap: SampledTexture2d,
    ) -> RenderInfo {
        let buffer = self.build(backend, &renderer.shader);

        renderer.render_buffer(pass, &buffer, matrix, atlas, lightmap, false)
    }

    #[inline]
    pub fn build(self, backend: &RenderBackend, shader: &Program) -> RenderBuffer<VoxelVertex, VoxelShader, u32> {
        RenderBuffer::new(backend, &self.vertices, shader, ElementType::Triangles, &self.indices).unwrap()
    }
}

pub struct ChunkRenderer {
    pub shader: Program,
    sun_position: f32,
    subchunks: IndexMap<(IPoint2D, usize), RenderSubchunk>,
    last_position: IPoint2D,
}

impl ChunkRenderer {
    pub fn new(backend: &RenderBackend) -> Self {
        Self {
            shader: backend.create_program(&VoxelShader).unwrap(),
            sun_position: 0.0,
            subchunks: IndexMap::new(),
            last_position: IPoint2D::MAX,
        }
    }

    pub fn set_subchunk(
        &mut self,
        origin: (IPoint2D, usize),
        solid: RenderBuffer<VoxelVertex, VoxelShader, u32>,
        translucent: RenderBuffer<VoxelVertex, VoxelShader, u32>,
    ) {
        self.subchunks.insert(origin, RenderSubchunk { solid, translucent });
    }

    #[inline]
    pub const fn set_sun_position(&mut self, value: f32) {
        self.sun_position = value;
    }

    fn is_subchunk_visible<T: Frustum>(frustum: &T, (origin, subchunk): (IPoint2D, usize)) -> bool {
        let origin = origin.as_vec2() * SUBCHUNK_SIZE_F32;
        let y = (subchunk * SUBCHUNK_SIZE) as f32;
        let origin = Point3D::new(origin.x, y, origin.y);
        let chunk_size = SUBCHUNK_SIZE_F32;
        let chunk_height = SUBCHUNK_SIZE_F32;

        frustum.is_box_visible(origin, origin + Point3D::new(chunk_size, chunk_height, chunk_size))
    }

    pub fn is_subchunk_rendered<Q: ?Sized + Hash + Eq>(&self, k: &Q) -> bool
    where
        (IPoint2D, usize): Borrow<Q>,
    {
        self.subchunks.contains_key(k)
    }

    fn bind_shader(&self, matrix: Transform3D, atlas: SampledTexture2d, lightmap: SampledTexture2d, sun_y: f32) {
        self.shader
            .bind()
            .with_uniform("matrix", matrix)
            .with_uniform("tex", atlas)
            .with_uniform("lightmap", lightmap)
            .with_uniform("sun_position", [0.0, sun_y, 0.0])
            .with_uniform("with_tex", true)
            .with_uniform("fog_color", <[f32; 4]>::from_value(&get_sky_color((false, 0.5), 0.0)))
            .with_uniform("fog_env_start", 32.0)
            .with_uniform("fog_env_end", 144.0)
            .with_uniform("fog_render_dist_start", 112.0)
            .with_uniform("fog_render_dist_end", 160.0);
    }

    pub fn render_buffer(
        &self,
        pass: &mut RenderPass,
        buffer: &RenderBuffer<VoxelVertex, VoxelShader, u32>,
        matrix: Transform3D,
        atlas: SampledTexture2d,
        lightmap: SampledTexture2d,
        full_bright: bool,
    ) -> RenderInfo {
        self.bind_shader(
            matrix,
            atlas,
            lightmap,
            if full_bright { const { (1.0 - 0.5) / 0.96 } } else { self.sun_position },
        );

        pass.apply_params(DrawParams {
            blend: Some(Blend {
                color: (BlendingFactor::SourceAlpha, BlendingFactor::OneMinusSourceAlpha),
                alpha: (BlendingFactor::One, BlendingFactor::OneMinusSourceAlpha),
            }),
            depth: Some(Depth {
                test: DepthTest::IfLessOrEqual,
                write: true,
            }),
            culling: Some(BackfaceCullingMode::CullCounterClockwise),
        });

        pass.draw_elements(&buffer.vertices, &buffer.indices);
        pass.reset_params();

        RenderInfo {
            draw_calls: 1,
            vertices: buffer.vertices.len(),
        }
    }

    pub fn render_with_params<T: Frustum>(
        &mut self,
        pass: &mut RenderPass,
        camera_pos: Point3D,
        frustum: &T,
        matrix: Transform3D,
        atlas: SampledTexture2d,
        lightmap: SampledTexture2d,
        params: DrawParams,
    ) -> RenderInfo {
        let camera_pos = ChunkManager::<()>::to_local(camera_pos.as_ivec3());

        if self.last_position != camera_pos {
            self.subchunks.sort_unstable_by(|&a, _, &b, _| {
                let a = (camera_pos - a.0).as_vec2().length_squared();
                let b = (camera_pos - b.0).as_vec2().length_squared();

                a.total_cmp(&b)
            });

            self.last_position = camera_pos;
        }

        let mut render_info = RenderInfo::default();

        self.bind_shader(matrix, atlas, lightmap, self.sun_position);

        pass.apply_params(params);

        for (&key, subchunk) in self.subchunks.iter() {
            if Self::is_subchunk_visible(frustum, key) && !subchunk.solid.indices.is_empty() {
                pass.draw_elements(&subchunk.solid.vertices, &subchunk.solid.indices);

                render_info.draw_calls += 1;
                render_info.vertices += subchunk.solid.vertices.len();
            }
        }

        for (&key, subchunk) in self.subchunks.iter() {
            if Self::is_subchunk_visible(frustum, key) && !subchunk.translucent.indices.is_empty() {
                pass.draw_elements(&subchunk.translucent.vertices, &subchunk.translucent.indices);

                render_info.draw_calls += 1;
                render_info.vertices += subchunk.translucent.vertices.len();
            }
        }

        pass.reset_params();

        render_info
    }

    pub fn render(
        &mut self,
        pass: &mut RenderPass,
        camera_pos: Point3D,
        frustum: &FrustumCulling,
        matrix: Transform3D,
        atlas: SampledTexture2d,
        lightmap: SampledTexture2d,
    ) -> RenderInfo {
        self.render_with_params(pass, camera_pos, frustum, matrix, atlas, lightmap, DrawParams {
            blend: Some(Blend {
                color: (BlendingFactor::SourceAlpha, BlendingFactor::OneMinusSourceAlpha),
                alpha: (BlendingFactor::One, BlendingFactor::OneMinusSourceAlpha),
            }),
            depth: Some(Depth {
                test: DepthTest::IfLessOrEqual,
                write: true,
            }),
            culling: Some(BackfaceCullingMode::CullCounterClockwise),
        })
    }
}
