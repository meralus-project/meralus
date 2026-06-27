use std::{borrow::Borrow, hash::Hash};

use horns::{
    BackfaceCullingMode, Blend, BlendingFactor, Depth, DepthTest, DrawParams, ElementType, Program, ProgramBinder, RenderBackend, RenderInfo, RenderPass,
    SampledTexture2d, create_shader, impl_vertex,
};
use indexmap::IndexMap;
use meralus_shared::{AsValue, Color, FromValue, Frustum, IPoint2D, IPoint3D, Point2D, Point3D, Transform3D};
use meralus_world::{SUBCHUNK_SIZE, SUBCHUNK_SIZE_F32, SUBCHUNK_SIZE_I32};

use super::RenderBuffer;
use crate::render::RenderShape;

create_shader!(pub VoxelShader => "./resources/shaders/voxel");

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelFace {
    pub position: Point3D,
    pub vertices: [Point3D; 4],
    pub uvs: [Point2D; 4],
    pub lights: [u8; 4],
    pub color: Color,
}

// #[allow(dead_code)]
// impl VoxelFace {
//     fn cmp(&self, camera_pos: Point3D, other: &Self) -> std::cmp::Ordering {
//         camera_pos
//             .distance_squared(self.position)
//             .total_cmp(&camera_pos.distance_squared(other.position))
//     }
// }

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

pub struct TranslucentSubchunk {
    buffer: RenderBuffer<VoxelVertex, VoxelShader, u32>,
    faces: Vec<VoxelFace>,
    last_pos: Point3D,
}

impl TranslucentSubchunk {
    pub fn new(backend: &RenderBackend, shader: &Program, mut faces: Vec<VoxelFace>, last_pos: Point3D, origin: IPoint2D) -> Self {
        Self::resort_faces(&mut faces, last_pos, origin);

        Self {
            buffer: VoxelMeshBuilder::build_dynamic_from_slice(backend, shader, &faces),
            faces,
            last_pos,
        }
    }

    fn update(&mut self, last_pos: Point3D, origin: IPoint2D) {
        if self.last_pos.distance_squared(last_pos) > 3.0 {
            Self::resort_faces(&mut self.faces, last_pos, origin);

            let mut builder = VoxelMeshBuilder::new();

            builder.extend_from_slice(&self.faces);

            self.buffer.vertices.dynamic_write(&builder.vertices);
            self.buffer.indices.dynamic_write(&builder.indices);
            self.last_pos = last_pos;
        }
    }

    #[track_caller]
    fn resort_faces(faces: &mut [VoxelFace], last_pos: Point3D, origin: IPoint2D) {
        let origin = origin.as_vec2() * SUBCHUNK_SIZE_F32;
        let local_camera_pos = last_pos - Point3D::new(origin.x, 0.0, origin.y);

        faces.sort_unstable_by(|a, b| {
            local_camera_pos
                .distance_squared(b.position)
                .total_cmp(&local_camera_pos.distance_squared(a.position))
        });
    }
}

struct RenderSubchunk {
    solid: RenderBuffer<VoxelVertex, VoxelShader, u32>,
    translucent: TranslucentSubchunk,
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
    pub fn build_dynamic_from_slice(backend: &RenderBackend, shader: &Program, voxels: &[VoxelFace]) -> RenderBuffer<VoxelVertex, VoxelShader, u32> {
        let mut this = Self::new();

        this.extend_from_slice(voxels);

        this.build_dynamic(backend, shader)
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
        let color = voxel.color.as_value();

        self.vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + matrix.transform_point3(voxel.vertices[i] - origin) + origin,
            light: voxel.lights[i].into(),
            uv: voxel.uvs[i],
            color,
        }));

        self.push_indices();
    }

    #[inline]
    pub fn push(&mut self, voxel: &VoxelFace) {
        let color = voxel.color.as_value();

        self.vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + voxel.vertices[i],
            light: voxel.lights[i].into(),
            uv: voxel.uvs[i],
            color,
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

    #[inline]
    pub fn build_dynamic(self, backend: &RenderBackend, shader: &Program) -> RenderBuffer<VoxelVertex, VoxelShader, u32> {
        RenderBuffer::new_dynamic(backend, &self.vertices, shader, ElementType::Triangles, &self.indices).unwrap()
    }
}

pub struct ChunkRenderer {
    pub shader: Program,
    subchunks: IndexMap<(IPoint2D, usize), RenderSubchunk>,
    last_position: Point3D,
    sun_position: f32,
    fog_color: Color,
}

impl ChunkRenderer {
    #[inline]
    pub fn new(backend: &RenderBackend) -> Self {
        Self {
            shader: backend.create_program(&VoxelShader).unwrap(),
            subchunks: IndexMap::new(),
            last_position: Point3D::NAN,
            sun_position: 0.0,
            fog_color: Color::BLACK,
        }
    }

    #[inline]
    pub fn set_subchunk(&mut self, origin: (IPoint2D, usize), solid: RenderBuffer<VoxelVertex, VoxelShader, u32>, translucent: TranslucentSubchunk) {
        self.subchunks.insert(origin, RenderSubchunk { solid, translucent });
    }

    #[inline]
    pub const fn set_sun_position(&mut self, value: f32) {
        self.sun_position = value;
    }

    #[inline]
    pub const fn set_fog_color(&mut self, value: Color) {
        self.fog_color = value;
    }

    #[inline]
    fn is_subchunk_visible<T: Frustum>(frustum: &T, (origin, subchunk): (IPoint2D, usize)) -> bool {
        let origin = origin.as_vec2() * SUBCHUNK_SIZE_F32;
        let y = (subchunk * SUBCHUNK_SIZE) as f32;
        let origin = Point3D::new(origin.x, y, origin.y);
        let chunk_size = SUBCHUNK_SIZE_F32;
        let chunk_height = SUBCHUNK_SIZE_F32;

        frustum.is_box_visible(origin, origin + Point3D::new(chunk_size, chunk_height, chunk_size))
    }

    #[inline]
    pub fn is_subchunk_rendered<Q: ?Sized + Hash + Eq>(&self, k: &Q) -> bool
    where
        (IPoint2D, usize): Borrow<Q>,
    {
        self.subchunks.contains_key(k)
    }

    #[inline]
    fn bind_shader(&self, matrix: Transform3D, atlas: SampledTexture2d, lightmap: SampledTexture2d, sun_y: f32) -> ProgramBinder<'_> {
        self.shader
            .bind()
            .with_uniform("matrix", matrix)
            .with_uniform("tex", atlas)
            .with_uniform("lightmap", lightmap)
            .with_uniform("sun_position", [0.0, sun_y, 0.0])
            .with_uniform("with_tex", true)
            .with_uniform("with_fog", false)
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
        )
        .with_uniform("chunk", IPoint3D::ZERO)
        .with_uniform("camera_pos", Point3D::ZERO);

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

    pub fn filter_by_shape(&mut self, center: IPoint2D, shape: RenderShape) -> impl Iterator<Item = IPoint2D> {
        self.subchunks
            .extract_if(.., move |&(origin, _), _| !shape.test(center, origin))
            .map(|((k, _), _)| k)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render<T: Frustum>(
        &mut self,
        pass: &mut RenderPass,
        camera_pos: Point3D,
        frustum: &T,
        matrix: Transform3D,
        atlas: SampledTexture2d,
        lightmap: SampledTexture2d,
    ) -> RenderInfo {
        if self.last_position.is_nan() || self.last_position.distance_squared(camera_pos) > 3.0 {
            self.subchunks.sort_unstable_by(|&a, _, &b, _| {
                #[inline]
                const fn center((pos, idx): (IPoint2D, usize)) -> Point3D {
                    Point3D::new(
                        pos.x as f32 * SUBCHUNK_SIZE_F32 + SUBCHUNK_SIZE_F32 * 0.5,
                        idx as f32 * SUBCHUNK_SIZE_F32 + SUBCHUNK_SIZE_F32 * 0.5,
                        pos.y as f32 * SUBCHUNK_SIZE_F32 + SUBCHUNK_SIZE_F32 * 0.5,
                    )
                }

                camera_pos.distance_squared(center(a)).total_cmp(&camera_pos.distance_squared(center(b))) // ascending — solid uses forward, translucent uses .rev()
            });

            self.last_position = camera_pos;
        }

        let mut render_info = RenderInfo::default();
        let mut binder = self
            .shader
            .bind()
            .with_uniform("matrix", matrix)
            .with_uniform("tex", atlas)
            .with_uniform("lightmap", lightmap)
            .with_uniform("sun_position", [0.0, self.sun_position, 0.0])
            .with_uniform("with_tex", true)
            .with_uniform("with_fog", true)
            .with_uniform("fog_color", <[f32; 4]>::from_value(&self.fog_color))
            .with_uniform("fog_env_start", SUBCHUNK_SIZE_F32)
            .with_uniform("fog_env_end", SUBCHUNK_SIZE_F32 * 4.0)
            .with_uniform("fog_render_dist_start", SUBCHUNK_SIZE_F32 * 3.0)
            .with_uniform("fog_render_dist_end", SUBCHUNK_SIZE_F32 * 5.0)
            .with_uniform("camera_pos", camera_pos);

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

        for (&key, subchunk) in &self.subchunks {
            if Self::is_subchunk_visible(frustum, key) && !subchunk.solid.indices.is_empty() {
                binder.set_uniform("chunk", IPoint3D::new(key.0.x * SUBCHUNK_SIZE_I32, 0, key.0.y * SUBCHUNK_SIZE_I32));

                pass.draw_elements(&subchunk.solid.vertices, &subchunk.solid.indices);

                render_info.draw_calls += 1;
            }
        }

        pass.apply_params(DrawParams {
            blend: Some(Blend {
                color: (BlendingFactor::SourceAlpha, BlendingFactor::OneMinusSourceAlpha),
                alpha: (BlendingFactor::One, BlendingFactor::OneMinusSourceAlpha),
            }),
            depth: Some(Depth {
                test: DepthTest::IfLessOrEqual,
                write: false,
            }),
            culling: Some(BackfaceCullingMode::CullCounterClockwise),
        });

        for (&key, subchunk) in self.subchunks.iter_mut().rev() {
            if Self::is_subchunk_visible(frustum, key) && !subchunk.translucent.buffer.indices.is_empty() {
                subchunk.translucent.update(camera_pos, key.0);

                binder.set_uniform("chunk", IPoint3D::new(key.0.x * SUBCHUNK_SIZE_I32, 0, key.0.y * SUBCHUNK_SIZE_I32));

                pass.draw_elements(&subchunk.translucent.buffer.vertices, &subchunk.translucent.buffer.indices);

                render_info.draw_calls += 1;
            }
        }

        pass.reset_params();

        render_info
    }
}
