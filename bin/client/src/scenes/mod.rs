use std::time::Duration;

use crate::render::context::{RowStrategy, UiSubcontext};

pub mod loading_overlay;
pub mod main_screen;
pub mod pause_menu;
pub mod settings_screen;

pub trait Screen {
    fn update(&mut self, delta: Duration);
    fn draw(&self, context: &mut UiSubcontext<'_, RowStrategy, RowStrategy>);
}

#[allow(dead_code)]
pub struct ScreenManager {
    screens: Vec<Box<dyn Screen>>,
}
