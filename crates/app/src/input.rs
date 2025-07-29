use ahash::HashSet;
use glam::Vec2;
use meralus_engine::{KeyCode, MouseButton};
use meralus_shared::Point2D;

#[derive(Debug, Default)]
pub struct KeyboardController {
    pressed: HashSet<KeyCode>,
    pressed_once: HashSet<KeyCode>,
    released: HashSet<KeyCode>,
}

impl KeyboardController {
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    pub fn is_key_pressed_once(&self, key: KeyCode) -> bool {
        self.pressed_once.contains(&key)
    }

    pub fn is_key_released(&self, key: KeyCode) -> bool {
        self.released.contains(&key)
    }

    pub fn clear(&mut self) {
        self.pressed_once.clear();
        self.released.clear();
    }

    pub fn handle_keyboard_input(&mut self, code: KeyCode, is_pressed: bool, repeat: bool) {
        if is_pressed {
            if !repeat {
                self.pressed_once.insert(code);

                if self.pressed.contains(&code) {
                    self.pressed.remove(&code);
                }
            }

            self.pressed.insert(code);
        } else {
            self.pressed.remove(&code);
            self.released.insert(code);
        }
    }
}

#[derive(Debug, Default)]
pub struct MouseController {
    pub position: Point2D,
    pressed_once: HashSet<MouseButton>,
    pressed: HashSet<MouseButton>,
    released: HashSet<MouseButton>,
    pub entered: HashSet<usize>,
}

impl MouseController {
    pub fn is_pressed(&self, button: MouseButton) -> bool {
        self.pressed.contains(&button)
    }

    pub fn is_pressed_once(&self, button: MouseButton) -> bool {
        self.pressed_once.contains(&button)
    }

    pub fn is_released(&self, button: MouseButton) -> bool {
        self.released.contains(&button)
    }

    pub fn clear(&mut self) {
        self.pressed_once.clear();
        self.released.clear();
    }

    pub const fn handle_mouse_motion(&mut self, position: Vec2) {
        self.position = Point2D::new(position.x, position.y);
    }

    pub fn handle_mouse_button(&mut self, button: MouseButton, is_pressed: bool) {
        if is_pressed {
            self.pressed_once.insert(button);
            self.pressed.insert(button);
        } else {
            self.pressed.remove(&button);
            self.released.insert(button);
        }
    }
}
