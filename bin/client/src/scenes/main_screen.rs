use std::time::Duration;

use mavelin_shared::Color;

use crate::{
    render::context::{ArrangeStrategy, Arrangement, MeasureStrategy, RowStrategy, UiSubcontext, WidgetState},
    scenes::Screen,
};

fn menu_button<A: ArrangeStrategy, M: MeasureStrategy>(scope: &mut UiSubcontext<'_, A, M>, name: &str) -> WidgetState {
    scope.button(|scope| {
        // scope.part_of_parent_width(0.75);
        scope.set_background_color(Color::from_hsl(110.0, 0.4, 0.7));

        scope.column(|scope| {
            scope.row(|scope| {
                scope.add_space(glam::Vec2::new(12.0, 0.0));
                scope.text(name, 18.0, "default", Color::from_hsl(110.0, 0.25, 0.1));
                scope.add_space(glam::Vec2::new(12.0, 0.0));
            });

            scope.add_space(glam::Vec2::new(0.0, 6.0));
        });
    })
}

pub struct MainScreen;

pub enum MainScreenAction {
    StartGame,
    GoToOptions,
    CloseWindow,
}

impl Screen for MainScreen {
    type Message = MainScreenAction;

    fn update(&mut self, _: Duration) {}

    fn draw(&self, context: &mut UiSubcontext<'_, RowStrategy, RowStrategy>, action: &mut Option<MainScreenAction>) {
        context.center(|scope| {
            scope.abs_pos(0.0, 24.0);
            scope.part_of_parent_width(1.0);

            scope.column(|scope| {
                scope.set_h_arrangement(Arrangement::End);

                scope.text("MAVELIN", 72.0, "default", Color::from_hsl(110.0, 0.4, 0.7));
                scope.text("deltarune today!", 18.0, "default", Color::from_hsl(110.0, 0.3, 0.6));
            });
        });

        context.center(|scope| {
            scope.fill_max_size();
            scope.column(|scope| {
                scope.set_h_arrangement(Arrangement::Center);
                scope.set_spacing(8.0);

                if menu_button(scope, "Play").clicked {
                    action.replace(MainScreenAction::StartGame);
                }

                if menu_button(scope, "Options").clicked {
                    action.replace(MainScreenAction::GoToOptions);
                    // self.current_page = Page::Options;
                }

                if menu_button(scope, "Exit").clicked {
                    action.replace(MainScreenAction::CloseWindow);
                    //  window_context.close_window();
                }
            });
        });
    }
}
