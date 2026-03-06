use ahash::{HashMap, HashSet};
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
    #[allow(dead_code)]
    pub fn is_pressed(&self, button: MouseButton) -> bool {
        self.pressed.contains(&button)
    }

    pub fn is_pressed_once(&self, button: MouseButton) -> bool {
        self.pressed_once.contains(&button)
    }

    #[allow(dead_code)]
    pub fn is_released(&self, button: MouseButton) -> bool {
        self.released.contains(&button)
    }

    pub fn clear(&mut self) {
        self.pressed_once.clear();
        self.released.clear();
    }

    pub const fn handle_mouse_motion(&mut self, position: Point2D) {
        self.position = position;
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

#[derive(Default)]
pub struct Input {
    pub mouse: MouseController,
    pub keyboard: KeyboardController,
    binds: HashMap<String, KeyCode>,
}

impl Input {
    pub fn with_binds<T: Into<String>, I: IntoIterator<Item = (T, KeyCode)>>(binds: I) -> Self {
        Self {
            mouse: MouseController::default(),
            keyboard: KeyboardController::default(),
            binds: binds.into_iter().map(|(name, key)| (name.into(), key)).collect(),
        }
    }

    #[allow(dead_code)]
    pub fn bind<T: Into<String>>(&mut self, name: T, key: KeyCode) {
        self.binds.insert(name.into(), key);
    }

    pub fn is_pressed<T: AsRef<str>>(&self, name: T) -> bool {
        self.binds.get(name.as_ref()).is_some_and(|&key| self.keyboard.is_key_pressed(key))
    }

    #[allow(dead_code)]
    pub fn is_pressed_once<T: AsRef<str>>(&self, name: T) -> bool {
        self.binds.get(name.as_ref()).is_some_and(|&key| self.keyboard.is_key_pressed_once(key))
    }

    #[allow(dead_code)]
    pub fn is_released<T: AsRef<str>>(&self, name: T) -> bool {
        self.binds.get(name.as_ref()).is_some_and(|&key| self.keyboard.is_key_released(key))
    }
}
