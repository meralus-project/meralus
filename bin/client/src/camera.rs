use meralus_physics::{AabbSource, PhysicsContext, RayCastResult};
use meralus_shared::{Angle, FrustumCulling, Point3D, Transform3D, Vector3D};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub position: Point3D,

    pub right: Vector3D,
    pub up: Vector3D,
    pub front: Vector3D,

    pub looking_at: Option<RayCastResult>,

    pub fov: f32,
    pub aspect_ratio: f32,
    pub z_near: f32,
    pub z_far: f32,

    pub frustum: FrustumCulling,
}

// const PLANE_NX: i32 = 0;
// const PLANE_PX: i32 = 1;
// const PLANE_NY: i32 = 2;
// const PLANE_PY: i32 = 3;
// const PLANE_NZ: i32 = 4;
// const PLANE_PZ: i32 = 5;
// const INTERSECT: i32 = -1;
// const INSIDE: i32 = -2;
// const OUTSIDE: i32 = -3;

// struct FrustumIntersection {
//     nxX: f32,
//     nxY: f32,
//     nxZ: f32,
//     nxW: f32,
//     pxX: f32,
//     pxY: f32,
//     pxZ: f32,
//     pxW: f32,
//     nyX: f32,
//     nyY: f32,
//     nyZ: f32,
//     nyW: f32,
//     pyX: f32,
//     pyY: f32,
//     pyZ: f32,
//     pyW: f32,
//     nzX: f32,
//     nzY: f32,
//     nzZ: f32,
//     nzW: f32,
//     pzX: f32,
//     pzY: f32,
//     pzZ: f32,
//     pzW: f32,
//     planes: [Vec4; 6],
// }

// impl FrustumIntersection {
//     fn set(&mut self, matrix: Mat4) {
//         self.nxX = m.row(0)[3] + m.row(0)[0];
//         self.nxY = m.row(1)[3] + m.row(1)[0];
//         self.nxZ = m.row(2)[3] + m.row(2)[0];
//         self.nxW = m.row(3)[3] + m.row(3)[0];
//         self.planes[0] = Vec4::new(self.nxX, self.nxY, self.nxZ, self.nxW);
//         self.pxX = m.row(0)[3] - m.row(0)[0];
//         self.pxY = m.row(1)[3] - m.row(1)[0];
//         self.pxZ = m.row(2)[3] - m.row(2)[0];
//         self.pxW = m.row(3)[3] - m.row(3)[0];
//         self.planes[1] = Vec4::new(self.pxX, self.pxY, self.pxZ, self.pxW);
//         self.nyX = m.row(0)[3] + m.row(0)[1];
//         self.nyY = m.row(1)[3] + m.row(1)[1];
//         self.nyZ = m.row(2)[3] + m.row(2)[1];
//         self.nyW = m.row(3)[3] + m.row(3)[1];
//         self.planes[2] = Vec4::new(self.nyX, self.nyY, self.nyZ, self.nyW);
//         self.pyX = m.row(0)[3] - m.row(0)[1];
//         self.pyY = m.row(1)[3] - m.row(1)[1];
//         self.pyZ = m.row(2)[3] - m.row(2)[1];
//         self.pyW = m.row(3)[3] - m.row(3)[1];
//         self.planes[3] = Vec4::new(self.pyX, self.pyY, self.pyZ, self.pyW);
//         self.nzX = m.row(0)[3] + m.row(0)[2];
//         self.nzY = m.row(1)[3] + m.row(1)[2];
//         self.nzZ = m.row(2)[3] + m.row(2)[2];
//         self.nzW = m.row(3)[3] + m.row(3)[2];
//         self.planes[4] = Vec4::new(self.nzX, self.nzY, self.nzZ, self.nzW);
//         self.pzX = m.row(0)[3] - m.row(0)[2];
//         self.pzY = m.row(1)[3] - m.row(1)[2];
//         self.pzZ = m.row(2)[3] - m.row(2)[2];
//         self.pzW = m.row(3)[3] - m.row(3)[2];
//         self.planes[5] = Vec4::new(self.pzX, self.pzY, self.pzZ, self.pzW);
//     }

//     fn test_aabb(&mut self, min: Vec3, max: Vec3) -> bool {
//                self.nxX * (if self.nxX < 0.0 {min.x} else {max.x}) + self.nxY
// * (if self.nxY < 0.0 {min.y} else {max.y}) + self.nxZ * (if self.nxZ < 0.0
// {min.z} else {max.z}) >= -self.nxW &&                self.pxX * (if self.pxX
// < 0.0 {min.x} else {max.x}) + self.pxY * (if self.pxY < 0.0 {min.y} else
// {max.y}) + self.pxZ * (if self.pxZ < 0.0  {min.z} else {max.z}) >= -self.pxW
// &&                self.nyX * (if self.nyX < 0.0 {min.x} else {max.x}) +
// self.nyY * (if self.nyY < 0.0 {min.y} else {max.y}) + self.nyZ * (if self.nyZ
// < 0.0  {min.z} else {max.z}) >= -self.nyW &&                self.pyX * (if
// self.pyX < 0.0 {min.x} else {max.x}) + self.pyY * (if self.pyY < 0.0 {min.y}
// else {max.y}) + self.pyZ * (if self.pyZ < 0.0  {min.z} else {max.z}) >=
// -self.pyW &&                self.nzX * (if self.nzX < 0.0 {min.x} else
// {max.x}) + self.nzY * (if self.nzY < 0.0 {min.y} else {max.y}) + self.nzZ *
// (if self.nzZ < 0.0  {min.z} else {max.z}) >= -self.nzW &&
// self.pzX * (if self.pzX < 0.0 {min.x} else {max.x}) + self.pzY * (if self.pzY
// < 0.0 {min.y} else {max.y}) + self.pzZ * (if self.pzZ < 0.0  {min.z} else
// {max.z}) >= -self.pzW     }

//     fn intersect_aabb(&self, min: Vec3, max: Vec3) -> i32 {
//         let mut plane = PLANE_NX;
//         let mut inside = true;

//         if (self.nxX * (if self.nxX < 0.0 { min.x } else { max.x })
//             + self.nxY * (if self.nxY < 0.0 { min.y } else { max.y })
//             + self.nxZ * (if self.nxZ < 0.0 { min.z } else { max.z })
//             >= -self.nxW)
//         {
//             plane = PLANE_PX;
//             inside &= self.nxX * (if self.nxX < 0.0 { max.x } else { min.x })
//                 + self.nxY * (if self.nxY < 0.0 { max.y } else { min.y })
//                 + self.nxZ * (if self.nxZ < 0.0 { max.z } else { min.z })
//                 >= -self.nxW;

//             if (self.pxX * (if self.pxX < 0.0 { min.x } else { max.x })
//                 + self.pxY * (if self.pxY < 0.0 { min.y } else { max.y })
//                 + self.pxZ * (if self.pxZ < 0.0 { min.z } else { max.z })
//                 >= -self.pxW)
//             {
//                 plane = PLANE_NY;
//                 inside &= self.pxX * (if self.pxX < 0.0 { max.x } else {
// min.x })
//                     + self.pxY * (if self.pxY < 0.0 { max.y } else { min.y })
//                     + self.pxZ * (if self.pxZ < 0.0 { max.z } else { min.z })
//                     >= -self.pxW;

//                 if (self.nyX * (if self.nyX < 0.0 { min.x } else { max.x })
//                     + self.nyY * (if self.nyY < 0.0 { min.y } else { max.y })
//                     + self.nyZ * (if self.nyZ < 0.0 { min.z } else { max.z })
//                     >= -self.nyW)
//                 {
//                     plane = PLANE_PY;
//                     inside &= self.nyX * (if self.nyX < 0.0 { max.x } else {
// min.x })
//                         + self.nyY * (if self.nyY < 0.0 { max.y } else {
//                           min.y })
//                         + self.nyZ * (if self.nyZ < 0.0 { max.z } else {
//                           min.z })
//                         >= -self.nyW;

//                     if (self.pyX * (if self.pyX < 0.0 { min.x } else { max.x
// })
//                         + self.pyY * (if self.pyY < 0.0 { min.y } else {
//                           max.y })
//                         + self.pyZ * (if self.pyZ < 0.0 { min.z } else {
//                           max.z })
//                         >= -self.pyW)
//                     {
//                         plane = PLANE_NZ;
//                         inside &= self.pyX * (if self.pyX < 0.0 { max.x }
// else { min.x })
//                             + self.pyY * (if self.pyY < 0.0 { max.y } else {
//                               min.y })
//                             + self.pyZ * (if self.pyZ < 0.0 { max.z } else {
//                               min.z })
//                             >= -self.pyW;

//                         if (self.nzX * (if self.nzX < 0.0 { min.x } else {
// max.x })
//                             + self.nzY * (if self.nzY < 0.0 { min.y } else {
//                               max.y })
//                             + self.nzZ * (if self.nzZ < 0.0 { min.z } else {
//                               max.z })
//                             >= -self.nzW)
//                         {
//                             plane = PLANE_PZ;
//                             inside &= self.nzX * (if self.nzX < 0.0 { max.x }
// else { min.x })
//                                 + self.nzY * (if self.nzY < 0.0 { max.y }
//                                   else { min.y })
//                                 + self.nzZ * (if self.nzZ < 0.0 { max.z }
//                                   else { min.z })
//                                 >= -self.nzW;

//                             if (self.pzX * (if self.pzX < 0.0 { min.x } else
// { max.x })
//                                 + self.pzY * (if self.pzY < 0.0 { min.y }
//                                   else { max.y })
//                                 + self.pzZ * (if self.pzZ < 0.0 { min.z }
//                                   else { max.z })
//                                 >= -self.pzW)
//                             {
//                                 inside &= self.pzX * (if self.pzX < 0.0 {
// max.x } else { min.x })
//                                     + self.pzY * (if self.pzY < 0.0 { max.y }
//                                       else { min.y })
//                                     + self.pzZ * (if self.pzZ < 0.0 { max.z }
//                                       else { min.z })
//                                     >= -self.pzW;

//                                 return if inside { INSIDE } else { INTERSECT
// };                             }
//                         }
//                     }
//                 }
//             }
//         }

//         plane
//     }
// }

// struct BetterFrustum {
//     intersection: FrustumIntersection,
//     matrix: Mat4,
//     view_vector: Vec4,
//     camera: DVec3,
// }

// impl BetterFrustum {
//     fn offset_to_fully_include_camera_cube(&mut self, offset: i32) {
//         let offset = f64::from(offset);

//         let d0 = (self.camera.x / offset).floor() * offset;
//         let d1 = (self.camera.y / offset).floor() * offset;
//         let d2 = (self.camera.z / offset).floor() * offset;
//         let d3 = (self.camera.x / offset).ceil() * offset;
//         let d4 = (self.camera.y / offset).ceil() * offset;
//         let d5 = (self.camera.z / offset).ceil() * offset;

//         while self.intersection.intersect_aabb(
//             (DVec3::new(d0, d1, d2) - self.camera).as_vec3(),
//             (DVec3::new(d3, d4, d5) - self.camera).as_vec3(),
//         ) != -2
//         {
//             self.camera.x -= (self.view_vector.x * 4.0).into();
//             self.camera.y -= (self.view_vector.y * 4.0).into();
//             self.camera.z -= (self.view_vector.z * 4.0).into()
//         }
//     }

//     fn prepare(&mut self, camera: DVec3) {
//         self.camera = camera;
//     }

//     fn calculate_frustum(&mut self, a: Mat4, b: Mat4) {
//         self.matrix = a * b;
//         self.intersection.set(self.matrix);
//         self.view_vector = self.matrix.transpose() * Vec4::new(0.0, 0.0, 1.0,
// 0.0);     }

//     fn cube_in_frustum(&mut self, start: DVec3, end: DVec3) -> i32 {
//         let f12 = (start - self.camera).as_vec3();
//         let f345 = (start - self.camera).as_vec3();

//         self.intersection.intersect_aabb(f12, f345)
//     }
// }

impl Camera {
    pub fn default() -> Self {
        let yaw = 0.0f32;
        let pitch = 0.0f32;

        let front = Vector3D::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();

        let right = front.cross(Vector3D::Y).normalize();
        let up = right.cross(front).normalize();

        Self {
            position: Point3D::ZERO,
            right,
            up,
            front,
            looking_at: None,
            fov: 55f32.to_radians(),
            z_near: 0.01,
            z_far: 10000.0,
            aspect_ratio: 1024.0 / 768.0,
            frustum: FrustumCulling::default(),
        }
    }

    pub fn new(position: Point3D) -> Self {
        Self { position, ..Self::default() }
    }

    pub const fn target(&self) -> Point3D {
        Point3D::new(self.position.x + self.front.x, self.position.y + self.front.y, self.position.z + self.front.z)
    }

    pub fn set_position<T: AabbSource>(&mut self, context: &PhysicsContext<T>, position: Point3D) {
        self.position = position;
        self.update_looking_at(context);
        self.update_frustum();
    }

    pub fn update_looking_at<T: AabbSource>(&mut self, context: &PhysicsContext<T>) {
        const BLOCK_REACH_DISTANCE: f32 = 20f32;

        let origin = self.position.as_();
        let target = origin + (self.front * BLOCK_REACH_DISTANCE).as_();

        self.looking_at = context.raycast(origin, target, true).filter(RayCastResult::is_block);
    }

    pub fn handle_mouse<T: AabbSource>(&mut self, context: &PhysicsContext<T>, (yaw, pitch): (f32, f32)) {
        self.front = Vector3D::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();
        self.right = self.front.cross(Vector3D::Y).normalize();
        self.up = self.right.cross(self.front).normalize();

        self.update_looking_at(context);

        // println!("{}", Self::calc_order(self.front));
    }

    pub fn projection(&self) -> Transform3D {
        Transform3D::perspective_rh_gl(Angle::from_radians(self.fov), self.aspect_ratio, self.z_near, self.z_far)
    }

    pub fn view(&self) -> Transform3D {
        Transform3D::look_at_rh(self.position, self.target(), self.up)
    }

    pub fn matrix(&self) -> Transform3D {
        self.projection() * self.view()
    }

    pub fn update_frustum(&mut self) {
        self.frustum.update(self.matrix());
    }

    #[allow(dead_code)]
    pub const fn calc_order(direction: Vector3D) -> usize {
        let sx = (direction.x < 0.0) as usize;
        let sy = (direction.y < 0.0) as usize;
        let sz = (direction.z < 0.0) as usize;
        let ax = direction.x.abs();
        let ay = direction.y.abs();
        let az = direction.z.abs();

        if ax > ay && ax > az {
            if ay > az {
                (sx << 2) | (sy << 1) | sz
            } else {
                8 + ((sx << 2) | (sz << 1) | sy)
            }
        } else if ay > az {
            if ax > az {
                16 + ((sy << 2) | (sx << 1) | sz)
            } else {
                24 + ((sy << 2) | (sz << 1) | sx)
            }
        } else if ax > ay {
            32 + ((sz << 2) | (sx << 1) | sy)
        } else {
            40 + ((sz << 2) | (sy << 1) | sx)
        }
    }
}
