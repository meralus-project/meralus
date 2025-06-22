use ahash::HashSet;
use meralus_engine::KeyCode;

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
