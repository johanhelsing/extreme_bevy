use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // fill the entire browser window
                // TODO: re-enable in Bevy 0.14
                // fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .run();
}
