use bevy::{camera::ScalingMode, prelude::*};

#[derive(Component)]
struct Player;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // fill the entire browser window
                fit_canvas_to_parent: true,
                // don't hijack keyboard shortcuts like F5, F6, F12, Ctrl+R etc.
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.53, 0.53, 0.53)))
        .add_systems(Startup, (setup, spawn_player))
        .add_systems(Update, move_player)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMax {
                max_width: 16.0,
                max_height: 9.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn spawn_player(mut commands: Commands) {
    commands.spawn((
        Player,
        Sprite {
            color: Color::srgb(0., 0.47, 1.),
            custom_size: Some(Vec2::new(1., 1.)),
            ..default()
        },
    ));
}

fn move_player(
    mut players: Query<&mut Transform, With<Player>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let mut direction = Vec2::ZERO;
    if keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
        direction.y += 1.;
    }
    if keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
        direction.y -= 1.;
    }
    if keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
        direction.x += 1.;
    }
    if keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
        direction.x -= 1.;
    }
    if direction == Vec2::ZERO {
        return;
    }

    let move_speed = 7.;
    let move_delta = direction * move_speed * time.delta_secs();

    for mut transform in &mut players {
        transform.translation += move_delta.extend(0.);
    }
}
