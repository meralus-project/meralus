#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Box2 {
    pub min: glam::Vec2,
    pub max: glam::Vec2,
}

impl Box2 {
    pub const fn to_array(self) -> [glam::Vec2; 2] {
        [self.min, self.max]
    }
}
