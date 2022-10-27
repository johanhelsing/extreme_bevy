use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            // fill the entire browser window
            fit_canvas_to_parent: true,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .run();
}
