use core::fmt;

use horns::{ElementType, Error, GlPrimitive, IndexBuffer, Program, RenderBackend, Shader, Vertex, VertexBuffer};
use meralus_shared::IPoint2D;

pub mod chunk;
pub mod common;
pub mod context;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum RenderShape {
    Circle(u16),
    Rect(u16, u16),
    Square(u16),
}

impl fmt::Display for RenderShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Circle(r) => write!(f, "circle (radius = {r})"),
            Self::Rect(width, height) => write!(f, "rect ({width}x{height})"),
            Self::Square(size) => write!(f, "square ({size}x{size})"),
        }
    }
}

#[allow(clippy::inline_always)]
#[inline(always)]
const fn round_up(v: u16) -> u16 {
    if v.is_multiple_of(2) { v + 1 } else { v }
}

#[derive(Debug, Copy, Clone)]
enum Leg {
    Center,
    Right,
    Top,
    Left,
    Bottom,
}

pub struct RenderShapeIter {
    shape: RenderShape,
    max_distance: i32,
    center: IPoint2D,

    point: IPoint2D,
    layer: i32,
    leg: Leg,
}

impl RenderShapeIter {
    const fn new(center: IPoint2D, shape: RenderShape) -> Self {
        let [w, h] = match shape {
            RenderShape::Circle(r) => [round_up(r * 2) as i32; 2],
            RenderShape::Rect(w, h) => [round_up(w) as i32, round_up(h) as i32],
            RenderShape::Square(s) => [round_up(s) as i32; 2],
        };

        Self {
            max_distance: if w > h { w } else { h },
            center,
            shape,
            point: IPoint2D::ZERO,
            layer: 1,
            leg: Leg::Center,
        }
    }

    const fn next_pair(&mut self) -> Option<IPoint2D> {
        match self.leg {
            Leg::Center => {
                self.leg = Leg::Right;
            }
            Leg::Right => {
                self.point.x = self.point.x.wrapping_add(1);

                if self.point.x == self.layer {
                    self.leg = Leg::Top;

                    if self.layer == self.max_distance {
                        return None;
                    }
                }
            }
            Leg::Top => {
                self.point.y = self.point.y.wrapping_add(1);

                if self.point.y == self.layer {
                    self.leg = Leg::Left;
                }
            }
            Leg::Left => {
                self.point.x = self.point.x.wrapping_sub(1);

                // -self.point.x == self.layer
                if self.point.x.wrapping_add(self.layer) == 0 {
                    self.leg = Leg::Bottom;
                }
            }
            Leg::Bottom => {
                self.point.y = self.point.y.wrapping_sub(1);

                // -self.point.y == self.layer
                if self.point.y.wrapping_add(self.layer) == 0 {
                    self.leg = Leg::Right;

                    self.layer += 1;
                }
            }
        }

        Some(self.center.wrapping_add(self.point))
    }
}

impl Iterator for RenderShapeIter {
    type Item = IPoint2D;

    fn next(&mut self) -> Option<IPoint2D> {
        let mut p = self.next_pair()?;

        while !self.shape.test(self.center, p) {
            p = self.next_pair()?;
        }

        Some(p)
    }
}

impl RenderShape {
    pub const fn test(self, center: IPoint2D, p: IPoint2D) -> bool {
        match self {
            Self::Circle(r) => (p.x - center.x).pow(2) + (p.y - center.y).pow(2) <= (r as i32).pow(2),
            Self::Rect(w, h) => {
                let w = w as i32 / 2;
                let h = h as i32 / 2;

                p.x.abs() <= w && p.y.abs() <= h
            }
            Self::Square(s) => {
                let s = s as i32 / 2;

                p.x.abs() <= s && p.y.abs() <= s
            }
        }
    }

    pub const fn iter_from_center(self, center: IPoint2D) -> RenderShapeIter {
        RenderShapeIter::new(center, self)
    }
}

pub struct RenderBuffer<V: Vertex, S: Shader, I: GlPrimitive> {
    pub vertices: VertexBuffer<V, S>,
    pub indices: IndexBuffer<I>,
}

impl<V: Vertex, S: Shader, I: GlPrimitive> RenderBuffer<V, S, I> {
    #[inline]
    pub fn new(backend: &RenderBackend, vertices: &[V], shader: &Program, element_type: ElementType, indices: &[I]) -> Result<Self, Error> {
        Ok(Self {
            vertices: backend.create_vertex_buffer(vertices, shader, false)?,
            indices: backend.create_index_buffer(element_type, indices, false)?,
        })
    }

    #[inline]
    pub fn new_dynamic(backend: &RenderBackend, vertices: &[V], shader: &Program, element_type: ElementType, indices: &[I]) -> Result<Self, Error> {
        Ok(Self {
            vertices: backend.create_vertex_buffer(vertices, shader, true)?,
            indices: backend.create_index_buffer(element_type, indices, true)?,
        })
    }
}

pub struct RawRenderBuffer<V: Vertex, I: GlPrimitive> {
    pub vertices: Vec<V>,
    pub indices: Vec<I>,
}

#[allow(dead_code)]
impl<V: Vertex, I: GlPrimitive> RawRenderBuffer<V, I> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    #[inline]
    pub fn with_capacity(vertices: usize, indices: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(vertices),
            indices: Vec::with_capacity(indices),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }
}

impl<V: Vertex, I: GlPrimitive> Default for RawRenderBuffer<V, I> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
