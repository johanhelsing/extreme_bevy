use bevy::{prelude::*, render::camera::ScalingMode, utils::HashMap};
use bevy_ggrs::*;
use bevy_matchbox::prelude::*;
use input::*;

mod input;

#[derive(Component)]
struct Player {
    handle: usize,
}

// The first generic parameter, u8, is the input type: 4-directions + fire fits
// easily in a single byte
// The second parameter is the address type of peers: Matchbox' WebRtcSocket
// addresses are called `PeerId`s
type Config = bevy_ggrs::GgrsConfig<u8, PeerId>;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    // fill the entire browser window
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            }),
            GgrsPlugin::<Config>::default(),
        ))
        .rollback_component_with_clone::<Transform>()
        .insert_resource(ClearColor(Color::rgb(0.53, 0.53, 0.53)))
        .add_systems(Startup, (setup, spawn_players, start_matchbox_socket))
        .add_systems(Update, wait_for_players)
        .add_systems(ReadInputs, read_local_inputs)
        .add_systems(GgrsSchedule, move_players)
        .run();
}

fn setup(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn(camera_bundle);
}

fn spawn_players(mut commands: Commands) {
    // Player 1
    commands
        .spawn((
            Player { handle: 0 },
            SpriteBundle {
                transform: Transform::from_translation(Vec3::new(-2., 0., 0.)),
                sprite: Sprite {
                    color: Color::rgb(0., 0.47, 1.),
                    custom_size: Some(Vec2::new(1., 1.)),
                    ..default()
                },
                ..default()
            },
        ))
        .add_rollback();

    // Player 2
    commands
        .spawn((
            Player { handle: 1 },
            SpriteBundle {
                transform: Transform::from_translation(Vec3::new(2., 0., 0.)),
                sprite: Sprite {
                    color: Color::rgb(0., 0.4, 0.),
                    custom_size: Some(Vec2::new(1., 1.)),
                    ..default()
                },
                ..default()
            },
        ))
        .add_rollback();
}

fn start_matchbox_socket(mut commands: Commands) {
    let room_url = "ws://127.0.0.1:3536/extreme_bevy?next=2";
    info!("connecting to matchbox server: {room_url}");
    commands.insert_resource(MatchboxSocket::new_ggrs(room_url));
}

fn wait_for_players(mut commands: Commands, mut socket: ResMut<MatchboxSocket<SingleChannel>>) {
    if socket.get_channel(0).is_err() {
        return; // we've already started
    }

    // Check for new connections
    socket.update_peers();
    let players = socket.players();

    let num_players = 2;
    if players.len() < num_players {
        return; // wait for more players
    }

    info!("All peers have joined, going in-game");

    // create a GGRS P2P session
    let mut session_builder = ggrs::SessionBuilder::<Config>::new()
        .with_num_players(num_players)
        .with_input_delay(2);

    for (i, player) in players.into_iter().enumerate() {
        session_builder = session_builder
            .add_player(player, i)
            .expect("failed to add player");
    }

    // move the channel out of the socket (required because GGRS takes ownership of it)
    let socket = socket.take_channel(0).unwrap();

    // start the GGRS session
    let ggrs_session = session_builder
        .start_p2p_session(socket)
        .expect("failed to start session");

    commands.insert_resource(bevy_ggrs::Session::P2P(ggrs_session));
}

fn move_players(
    mut players: Query<(&mut Transform, &Player)>,
    inputs: Res<PlayerInputs<Config>>,
    time: Res<Time>,
) {
    for (mut transform, player) in &mut players {
        let (input, _) = inputs[player.handle];

        let direction = direction(input);

        if direction == Vec2::ZERO {
            continue;
        }

        let move_speed = 7.;
        let move_delta = direction * move_speed * time.delta_seconds();
        transform.translation += move_delta.extend(0.);
    }
}
