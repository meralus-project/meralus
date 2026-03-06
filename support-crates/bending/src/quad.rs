use glam::{Vec2, Vec3};

#[derive(Debug)]
pub struct QuadVertex {
    pub original_position: Vec3,
    pub position: Vec3,
    pub uv: Vec2,
}

#[derive(Debug)]
pub struct Quad {
    pub vertices: Vec<QuadVertex>,
}

impl Quad {
    pub fn new(vertices: [Vec3; 4], uv1: Vec2, uv2: Vec2, flip: bool) -> Self {
        let mut t_vertices = vec![
            QuadVertex {
                original_position: vertices[0],
                position: vertices[0],
                uv: Vec2::new(uv2.x, uv1.y),
            },
            QuadVertex {
                original_position: vertices[1],
                position: vertices[1],
                uv: uv1,
            },
            QuadVertex {
                original_position: vertices[2],
                position: vertices[2],
                uv: Vec2::new(uv1.x, uv2.y),
            },
            QuadVertex {
                original_position: vertices[3],
                position: vertices[3],
                uv: uv2,
            },
        ];

        if flip {
            let i = vertices.len();

            for j in 0..i / 2 {
                t_vertices.swap(j, i - 1 - j);
            }
        }

        Self { vertices: t_vertices }
    }

    // pub fn render(&self, vertices: &mut Vec<VoxelData>) {
    //     for i in [0, 1, 2, 2, 3, 0].into_iter().rev() {
    //         let vertex = self.vertices[i];
    //         let data = (vertex.2, Vec2::new(vertex.0, vertex.1));

    //         vertices.push(VoxelData {
    //             position: data.0,
    //             uv: data.1,
    //             color: Color::RED,
    //             light: 240,
    //             visible: true,
    //         });
    //     }
    // }
}
