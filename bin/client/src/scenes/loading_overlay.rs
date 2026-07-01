use std::time::Duration;

use mavelin_shared::Color;
use mavelin_tween::{Animation, Tween};

use crate::{
    render::context::{Arrangement, RowStrategy, UiSubcontext},
    scenes::Screen,
};

pub struct LoadingOverlay {
    pub progress: Tween<f32>,
}

impl Screen for LoadingOverlay {
    type Message = ();

    fn update(&mut self, delta: Duration) {
        self.progress.advance(delta);
    }

    fn draw(&self, scope: &mut UiSubcontext<'_, RowStrategy, RowStrategy>, _: &mut Option<()>) {
        // scope.center(|scope| {
        //     scope.abs_pos(0.0, 24.0);
        //     scope.part_of_parent_width(1.0);

        //     scope.text("MAVELIN", 72.0, "default", Color::from_hsl(110.0, 0.4, 0.7));
        // });

        scope.column(|scope| {
            scope.abs_pos(0.0, 0.0);
            scope.fill_max_size();
            scope.set_v_arrangement(Arrangement::End);

            scope.column(|scope| {
                scope.part_of_parent_width(0.4);
                scope.set_background_color(Color::from_hsl(110.0, 0.25, 0.1));

                let size = scope.parent_size();

                scope.row(|scope| {
                    scope.set_height(32.0);

                    scope.add_space(glam::Vec2::new(4.0, 0.0));
                    scope.rect(
                        glam::Vec2::new((size.x - 4.0) * 0.4 * self.progress.get_copy(), 32.0),
                        Color::from_hsl(110.0, 0.4, 0.7),
                    );
                });

                scope.add_space(glam::Vec2::new(0.0, 4.0));
            });
        });
    }
}
