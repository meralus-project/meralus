use std::f32;

use ahash::HashMap;
use mavelin_engine::KeyCode;
use mavelin_physics::{Aabb, PhysicsBody};
use mavelin_shared::Lerp;

use crate::{Camera, get_movement_direction, get_rotation_directions, input::Input};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemType {
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Item {
    pub id: u32,
    pub ty: ItemType,
    pub amount: usize,
}

#[derive(Default)]
pub struct Inventory {
    item_to_slots: HashMap<(u32, ItemType), Vec<(usize, usize)>>,
    filled_slots: HashMap<(usize, usize), Item>,
}

impl Inventory {
    pub const HOTBAR_ROW: usize = 4;
    pub const MAX_ITEM_AMOUNT: usize = 64;
    pub const MAX_SLOT_COLUMNS: usize = 9;
    pub const MAX_SLOT_ROWS: usize = 5;

    #[inline]
    pub fn is_hotbar_filled(&self) -> bool {
        self.filled_slots.keys().filter(|slot| slot.0 == Self::HOTBAR_ROW).count() == Self::MAX_SLOT_COLUMNS
    }

    #[allow(dead_code)]
    #[inline]
    pub fn get_row_items(&self, row: usize) -> impl Iterator<Item = (usize, &Item)> {
        (0..Self::MAX_SLOT_COLUMNS).filter_map(move |column| self.get_item(row, column).map(|item| (column, item)))
    }

    #[inline]
    pub fn get_hotbar_items(&self) -> impl Iterator<Item = (usize, &Item)> {
        (0..Self::MAX_SLOT_COLUMNS).filter_map(|column| self.get_hotbar_item(column).map(|item| (column, item)))
    }

    #[inline]
    pub fn take_hotbar_item(&mut self, column: usize) -> Option<(u32, ItemType)> {
        self.take_item(Self::HOTBAR_ROW, column)
    }

    #[inline]
    pub fn get_hotbar_item(&self, column: usize) -> Option<&Item> {
        self.get_item(Self::HOTBAR_ROW, column)
    }

    #[inline]
    pub fn take_item(&mut self, row: usize, column: usize) -> Option<(u32, ItemType)> {
        let mut remove_item = false;

        let item = self.filled_slots.get_mut(&(row, column)).map(|item| {
            if item.amount == 1 {
                remove_item = true;
            }

            item.amount -= 1;

            (item.id, item.ty)
        });

        if remove_item {
            self.filled_slots.remove(&(row, column));
        }

        item
    }

    #[inline]
    pub fn get_item(&self, row: usize, column: usize) -> Option<&Item> {
        self.filled_slots.get(&(row, column))
    }

    pub fn try_insert(&mut self, item: &Item) {
        if let Some(slot) = self.item_to_slots.get(&(item.id, item.ty)).and_then(|slots| slots.last())
            && let Some(slot) = self.filled_slots.get_mut(slot).filter(|slot| slot.amount < Self::MAX_ITEM_AMOUNT)
        {
            slot.amount += 1;
        } else if self.filled_slots.len() < Self::MAX_SLOT_COLUMNS * Self::MAX_SLOT_ROWS {
            let mut slot = None;

            if self.is_hotbar_filled() {
                for row in 0..Self::MAX_SLOT_ROWS {
                    if row == Self::HOTBAR_ROW {
                        continue;
                    }

                    for column in 0..Self::MAX_SLOT_COLUMNS {
                        if !self.filled_slots.contains_key(&(row, column)) {
                            slot = Some((row, column));

                            break;
                        }
                    }
                }
            } else {
                for column in 0..Self::MAX_SLOT_COLUMNS {
                    if !self.filled_slots.contains_key(&(Self::HOTBAR_ROW, column)) {
                        slot = Some((Self::HOTBAR_ROW, column));

                        break;
                    }
                }
            }

            if let Some(slot) = slot {
                self.filled_slots.insert(slot, item.clone());
                self.item_to_slots
                    .entry((item.id, item.ty))
                    .and_modify(|slots| slots.push(slot))
                    .or_insert_with(|| vec![slot]);
            }
        }
    }
}

pub struct Player {
    // START CAMERA
    pub yaw: f32,
    pub pitch: f32,
    // END CAMERA
    // START PHYSICS
    pub body: PhysicsBody,
    // END PHYSICS
    // CAMERA BOBBING START
    pub bob_time: f32,
    pub bob_offset: glam::Vec3,

    pub dash_time: f32,
    // INVENTORY
    pub inventory: Inventory,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            body: PhysicsBody::new(glam::Vec3::Y, Self::PLAYER_SIZE.as_vec3()),
            bob_time: 0.0,
            bob_offset: glam::Vec3::ZERO,
            dash_time: 0.0,
            inventory: Inventory::default(),
        }
    }
}

#[allow(dead_code)]
impl Player {
    pub const AFFECTED_BY_PHYSICS: bool = true;
    pub const CAMERA_OFFSET: glam::Vec3 = if Self::IS_THIRD_PERSON {
        glam::Vec3::new(-2.0, 0.5, 0.0)
    } else {
        glam::Vec3::ZERO
    };
    pub const IS_THIRD_PERSON: bool = false;
    pub const LOOK_SPEED: f32 = 0.1;
    pub const MOUSE_SENSE: f32 = 0.05;
    pub const MOVE_SPEED: f32 = 4.0;
    pub const PLAYER_HALF_SIZE: glam::DVec3 = glam::DVec3::new(0.35 / 2.0, 1.625 / 2.0, 0.35 / 2.0);
    pub const PLAYER_SIZE: glam::DVec3 = glam::DVec3::new(0.35, 1.625, 0.35);

    #[inline]
    pub fn calc_player_aabb(position: glam::Vec3) -> Aabb {
        let position = position.as_dvec3();

        Aabb::new(position - Self::PLAYER_HALF_SIZE, position + Self::PLAYER_HALF_SIZE)
    }

    #[inline]
    pub fn aabb(&self) -> Aabb {
        Self::calc_player_aabb(self.body.position)
    }

    #[inline]
    pub fn camera_position(&self) -> glam::Vec3 {
        self.body.position + glam::Vec3::Y * (Self::PLAYER_HALF_SIZE.y as f32) + self.bob_offset + Self::CAMERA_OFFSET
    }

    #[inline]
    pub fn handle_mouse(&mut self, delta: glam::Vec2) -> (f32, f32) {
        self.yaw = (delta.x * Self::MOUSE_SENSE).mul_add(Self::LOOK_SPEED, self.yaw);
        self.pitch = (delta.y * Self::MOUSE_SENSE).mul_add(-Self::LOOK_SPEED, self.pitch);
        self.pitch = self.pitch.clamp(-1.5, 1.5);

        (self.yaw, self.pitch)
    }

    pub fn handle_keyboard(&mut self, input: &Input) {
        const DASH_SPEED: f32 = 30.0;
        const DASH_DURATION: f32 = 0.2;

        if input.keyboard.is_key_pressed_once(KeyCode::KeyE) && !self.body.is_on_ground {
            let (front, _right, _) = get_rotation_directions(self.yaw, self.pitch);

            self.body.velocity += front * DASH_SPEED;
        }

        if self.body.is_on_ground && input.keyboard.is_key_pressed(KeyCode::Space) {
            self.body.velocity.y = 8.0;
        }
    }

    pub fn physics_step(&mut self, input: &Input, camera: &mut Camera, delta: f32) {
        const BOB_SPEED: f32 = 3.0;
        const BOB_FREQ: f32 = 2.0;
        const BOB_AMP: f32 = 0.1;

        let was_dashing = self.dash_time > 0.0;

        if was_dashing {
            self.dash_time -= delta;

            if self.dash_time <= 0.0 {
                self.body.velocity.x = 0.0;
                self.body.velocity.z = 0.0;

                if self.body.velocity.y > 0.0 {
                    self.body.velocity.y *= 0.5;
                }
            }
        }

        if (self.body.velocity.x == 0.0 && self.body.velocity.z == 0.0) || !self.body.is_on_ground || self.dash_time > 0.0 {
            self.bob_time = 0.0;
            self.bob_offset = self.bob_offset.lerp(glam::Vec3::ZERO, (delta * 16.0).min(1.0));
        } else {
            let mut amp = BOB_AMP;
            let mut freq = BOB_FREQ;

            if input.keyboard.is_key_pressed(KeyCode::ShiftLeft) {
                amp *= 1.5;
                freq *= 1.5;
            }

            self.bob_time = BOB_SPEED.mul_add(delta, self.bob_time);
            self.bob_offset = glam::Vec3::new(amp * (self.bob_time * freq).sin(), amp * (self.bob_time * freq * 2.0).sin(), 0.0);
        }

        let direction = get_movement_direction(input);

        let (front, right, _) = get_rotation_directions(self.yaw, 0.0);

        let velocity = (front * direction.z + right * direction.x)
            * if input.keyboard.is_key_pressed(KeyCode::ShiftLeft) && direction.z > 0.0 {
                camera.fov = camera.fov.lerp(
                    &(65f32.to_radians() * (self.body.velocity.y.abs() / 8.0).clamp(1.0, 1.75)),
                    (delta * 16.0).min(1.0),
                );

                Self::MOVE_SPEED * 1.5
            } else {
                camera.fov = camera.fov.lerp(
                    &(55f32.to_radians() * (self.body.velocity.y.abs() / 8.0).clamp(1.0, 1.75)),
                    (delta * 16.0).min(1.0),
                );

                Self::MOVE_SPEED
            };

        if self.body.is_on_ground {
            self.body.velocity.x = velocity.x;
            self.body.velocity.z = velocity.z;
        } else {
            self.body.velocity.x = velocity.x.mul_add(delta, self.body.velocity.x);
            self.body.velocity.z = velocity.z.mul_add(delta, self.body.velocity.z);
        }
    }
}
