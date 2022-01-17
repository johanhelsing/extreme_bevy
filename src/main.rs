use bevy::{prelude::*, render::camera::ScalingMode, tasks::IoTaskPool};
use bevy_ggrs::*;
use matchbox_socket::WebRtcSocket;

#[derive(Resource)]
struct Session {
    socket: Option<WebRtcSocket>,
}

#[derive(Component)]
struct Player;

struct GgrsConfig;

impl ggrs::Config for GgrsConfig {
    // 4-directions + fire fits easily in a single byte
    type Input = u8;
    type State = u8;
    // Matchbox' WebRtcSocket addresses are strings
    type Address = String;
}

const INPUT_UP: u8 = 1 << 0;
const INPUT_DOWN: u8 = 1 << 1;
const INPUT_LEFT: u8 = 1 << 2;
const INPUT_RIGHT: u8 = 1 << 3;
const INPUT_FIRE: u8 = 1 << 4;

fn main() {
    let mut app = App::new();

    GGRSPlugin::<GgrsConfig>::new()
        .with_input_system(input)
        .with_rollback_schedule(Schedule::default().with_stage(
            "ROLLBACK_STAGE",
            SystemStage::single_threaded().with_system(move_player),
        ))
        .register_rollback_component::<Transform>()
        .build(&mut app);

    app.insert_resource(ClearColor(Color::rgb(0.53, 0.53, 0.53)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                // fill the entire browser window
                fit_canvas_to_parent: true,
                ..default()
            },
            ..default()
        }))
        .add_startup_system(setup)
        .add_startup_system(start_matchbox_socket)
        .add_startup_system(spawn_players)
        .add_system(wait_for_players)
        .run();
}

fn setup(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn(camera_bundle);
}

fn spawn_players(mut commands: Commands, mut rip: ResMut<RollbackIdProvider>) {
    // Player 1
    commands.spawn((
        Player,
        Rollback::new(rip.next_id()),
        SpriteBundle {
            transform: Transform::from_translation(Vec3::new(-2., 0., 0.)),
            sprite: Sprite {
                color: Color::rgb(0., 0.47, 1.),
                custom_size: Some(Vec2::new(1., 1.)),
                ..default()
            },
            ..default()
        },
    ));

    // Player 2
    commands.spawn((
        Player,
        Rollback::new(rip.next_id()),
        SpriteBundle {
            transform: Transform::from_translation(Vec3::new(2., 0., 0.)),
            sprite: Sprite {
                color: Color::rgb(0., 0.4, 0.),
                custom_size: Some(Vec2::new(1., 1.)),
                ..default()
            },
            ..default()
        },
    ));
}

fn start_matchbox_socket(mut commands: Commands) {
    let room_url = "ws://127.0.0.1:3536/extreme_bevy?next=2";
    info!("connecting to matchbox server: {:?}", room_url);
    let (socket, message_loop) = WebRtcSocket::new(room_url);

    // The message loop needs to be awaited, or nothing will happen.
    // We do this here using bevy's task system.
    IoTaskPool::get().spawn(message_loop).detach();

    commands.insert_resource(Session {
        socket: Some(socket),
    });
}

fn wait_for_players(mut commands: Commands, mut session: ResMut<Session>) {
    let Some(socket) = &mut session.socket else {
        // If there is no socket we've already started the game
        return;
    };

    // Check for new connections
    socket.accept_new_connections();
    let players = socket.players();

    let num_players = 2;
    if players.len() < num_players {
        return; // wait for more players
    }

    info!("All peers have joined, going in-game");

    // create a GGRS P2P session
    let mut session_builder = ggrs::SessionBuilder::<GgrsConfig>::new()
        .with_num_players(num_players)
        .with_input_delay(2);

    for (i, player) in players.into_iter().enumerate() {
        session_builder = session_builder
            .add_player(player, i)
            .expect("failed to add player");
    }

    // move the socket out of the resource (required because GGRS takes ownership of it)
    let socket = session.socket.take().unwrap();

    // start the GGRS session
    let ggrs_session = session_builder
        .start_p2p_session(socket)
        .expect("failed to start session");

    commands.insert_resource(bevy_ggrs::Session::P2PSession(ggrs_session));
}

fn input(_: In<ggrs::PlayerHandle>, keys: Res<Input<KeyCode>>) -> u8 {
    let mut input = 0u8;

    if keys.any_pressed([KeyCode::Up, KeyCode::W]) {
        input |= INPUT_UP;
    }
    if keys.any_pressed([KeyCode::Down, KeyCode::S]) {
        input |= INPUT_DOWN;
    }
    if keys.any_pressed([KeyCode::Left, KeyCode::A]) {
        input |= INPUT_LEFT
    }
    if keys.any_pressed([KeyCode::Right, KeyCode::D]) {
        input |= INPUT_RIGHT;
    }
    if keys.any_pressed([KeyCode::Space, KeyCode::Return]) {
        input |= INPUT_FIRE;
    }

    input
}

fn move_player(
    inputs: Res<PlayerInputs<GgrsConfig>>,
    mut player_query: Query<&mut Transform, With<Player>>,
) {
    let mut direction = Vec2::ZERO;

    let (input, _) = inputs[0];

    if input & INPUT_UP != 0 {
        direction.y += 1.;
    }
    if input & INPUT_DOWN != 0 {
        direction.y -= 1.;
    }
    if input & INPUT_RIGHT != 0 {
        direction.x += 1.;
    }
    if input & INPUT_LEFT != 0 {
        direction.x -= 1.;
    }
    if direction == Vec2::ZERO {
        return;
    }

    let move_speed = 0.13;
    let move_delta = (direction * move_speed).extend(0.);

    for mut transform in player_query.iter_mut() {
        transform.translation += move_delta;
    }
}
