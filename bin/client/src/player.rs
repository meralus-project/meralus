use std::f32;

use ahash::HashMap;
use meralus_engine::KeyCode;
use meralus_physics::{Aabb, PhysicsBody};
use meralus_shared::{DSize3D, DVector3D, Lerp, Point3D, Vector2D};

use crate::{Camera, get_movement_direction, get_rotation_directions, input::Input};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemType {
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Item {
    pub id: usize,
    pub ty: ItemType,
    pub amount: usize,
}

#[derive(Default)]
pub struct Inventory {
    item_to_slots: HashMap<(usize, ItemType), Vec<(usize, usize)>>,
    filled_slots: HashMap<(usize, usize), Item>,
}

impl Inventory {
    pub const HOTBAR_ROW: usize = 4;
    pub const MAX_ITEM_AMOUNT: usize = 64;
    pub const MAX_SLOT_COLUMNS: usize = 10;
    pub const MAX_SLOT_ROWS: usize = 5;

    pub fn is_hotbar_filled(&self) -> bool {
        self.filled_slots.keys().filter(|slot| slot.0 == Self::HOTBAR_ROW).count() == Self::MAX_SLOT_COLUMNS
    }

    #[allow(dead_code)]
    pub fn get_row_items(&self, row: usize) -> impl Iterator<Item = (usize, &Item)> {
        (0..Self::MAX_SLOT_COLUMNS).filter_map(move |column| self.get_item(row, column).map(|item| (column, item)))
    }

    pub fn get_hotbar_items(&self) -> impl Iterator<Item = (usize, &Item)> {
        (0..Self::MAX_SLOT_COLUMNS).filter_map(|column| self.get_hotbar_item(column).map(|item| (column, item)))
    }

    pub fn take_hotbar_item(&mut self, column: usize) -> Option<(usize, ItemType)> {
        self.take_item(Self::HOTBAR_ROW, column)
    }

    pub fn get_hotbar_item(&self, column: usize) -> Option<&Item> {
        self.get_item(Self::HOTBAR_ROW, column)
    }

    pub fn take_item(&mut self, row: usize, column: usize) -> Option<(usize, ItemType)> {
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

    pub fn get_item(&self, row: usize, column: usize) -> Option<&Item> {
        self.filled_slots.get(&(row, column))
    }

    pub fn try_insert(&mut self, item: Item) {
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
                self.filled_slots.insert(slot, item);
                self.item_to_slots
                    .entry((item.id, item.ty))
                    .and_modify(|slots| slots.push(slot))
                    .or_insert_with(|| vec![slot]);
            }
        }
    }
}

pub struct PlayerController {
    // START CAMERA
    pub yaw: f32,
    pub pitch: f32,
    // END CAMERA
    // START PHYSICS
    pub body: PhysicsBody,
    // END PHYSICS
    // CAMERA BOBBING START
    pub bob_time: f32,
    pub bob_offset: Point3D,
    // INVENTORY
    pub inventory: Inventory,
}

impl Default for PlayerController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            body: PhysicsBody::new(Point3D::Y, Self::PLAYER_SIZE.as_()),
            bob_time: 0.0,
            bob_offset: Point3D::ZERO,
            inventory: Inventory::default(),
        }
    }
}

impl PlayerController {
    pub const AFFECTED_BY_PHYSICS: bool = true;
    pub const CAMERA_OFFSET: Point3D = if Self::IS_THIRD_PERSON { Point3D::new(-2.0, 0.5, 0.0) } else { Point3D::ZERO };
    pub const IS_THIRD_PERSON: bool = false;
    pub const LOOK_SPEED: f32 = 0.1;
    pub const MOUSE_SENSE: f32 = 0.05;
    pub const MOVE_SPEED: f32 = 4.0;
    pub const PLAYER_HALF_SIZE: DSize3D = DSize3D::new(0.35 / 2.0, 1.625 / 2.0, 0.35 / 2.0);
    pub const PLAYER_SIZE: DSize3D = DSize3D::new(0.35, 1.625, 0.35);

    pub fn calc_player_aabb(position: Point3D) -> Aabb {
        let position = position.as_();

        Aabb::new(position - Self::PLAYER_HALF_SIZE.to_vector(), position + Self::PLAYER_HALF_SIZE.to_vector())
    }

    pub fn player_aabb(&self) -> Aabb {
        Self::calc_player_aabb(self.body.position)
    }

    pub fn camera_position(&self) -> Point3D {
        self.body.position + Point3D::Y * (Self::PLAYER_HALF_SIZE.height as f32 - 0.15) + self.bob_offset + Self::CAMERA_OFFSET
    }

    pub fn get_vector_for_rotation(&self) -> DVector3D {
        let _f = (self.yaw - f32::consts::PI).cos();
        let _f1 = (self.yaw - f32::consts::PI).sin();
        let _f2 = -(self.pitch).cos();
        let _f3 = (self.pitch).sin();

        DVector3D::new(f64::from(self.pitch), f64::from(self.yaw), 0.0)
    }

    pub fn handle_mouse(&mut self, delta: Vector2D) -> (f32, f32) {
        self.yaw += delta.x * Self::MOUSE_SENSE * Self::LOOK_SPEED;
        self.pitch += delta.y * Self::MOUSE_SENSE * -Self::LOOK_SPEED;
        self.pitch = self.pitch.clamp(-1.5, 1.5);

        (self.yaw, self.pitch)
    }

    pub fn physics_step(&mut self, input: &Input, camera: &mut Camera, delta: f32) {
        const BOB_SPEED: f32 = 3.0;
        const BOB_FREQ: f32 = 3.0;
        const BOB_AMP: f32 = 0.15;

        if (self.body.velocity.x == 0.0 && self.body.velocity.z == 0.0) || !self.body.is_on_ground {
            self.bob_time = 0.0;
            self.bob_offset = self.bob_offset.lerp(Point3D::ZERO, (delta * 16.0).min(1.0));
        } else {
            self.bob_time += BOB_SPEED * delta;
            self.bob_offset = Point3D::new(
                BOB_AMP * (self.bob_time * BOB_FREQ).sin(),
                BOB_AMP * (self.bob_time * BOB_FREQ * 2.0).sin(),
                0.0,
            );
        }

        let direction = get_movement_direction(input);
        let (front, right, _) = get_rotation_directions(self.yaw, 0.0);

        let velocity = ((front * direction.z) + (right * direction.x))
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

        self.body.velocity.x = velocity.x;
        self.body.velocity.z = velocity.z;

        if Self::AFFECTED_BY_PHYSICS {
            if input.keyboard.is_key_pressed(KeyCode::Space) && self.body.is_on_ground {
                self.body.velocity.y = 8.0;
            }
        } else {
            if input.keyboard.is_key_pressed(KeyCode::Space) {
                self.body.position.y += 0.5;
            }

            if input.keyboard.is_key_pressed(KeyCode::ControlLeft) {
                self.body.position.y -= 0.5;
            }
        }
    }
}
