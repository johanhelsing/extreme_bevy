use bevy::{math::Vec3Swizzles, prelude::*, render::camera::ScalingMode, tasks::IoTaskPool};
use bevy_asset_loader::prelude::*;
use bevy_ggrs::*;
use components::*;
use ggrs::{InputStatus, PlayerType};
use input::*;
use matchbox_socket::WebRtcSocket;

mod components;
mod input;

struct GgrsConfig;

impl ggrs::Config for GgrsConfig {
    // 4-directions + fire fits easily in a single byte
    type Input = u8;
    type State = u8;
    // Matchbox' WebRtcSocket addresses are strings
    type Address = String;
}

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
enum GameState {
    AssetLoading,
    Matchmaking,
    InGame,
}

struct LocalPlayerHandle(usize);

fn main() {
    let mut app = App::new();

    GGRSPlugin::<GgrsConfig>::new()
        .with_input_system(input)
        .with_rollback_schedule(Schedule::default().with_stage(
            "ROLLBACK_STAGE",
            SystemStage::single_threaded().with_system(move_players),
        ))
        .register_rollback_type::<Transform>()
        .build(&mut app);

    app.add_state(GameState::AssetLoading)
        .add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .with_collection::<ImageAssets>()
                .continue_to_state(GameState::Matchmaking),
        )
        .insert_resource(ClearColor(Color::rgb(0.53, 0.53, 0.53)))
        .insert_resource(WindowDescriptor {
            // fill the entire browser window
            fit_canvas_to_parent: true,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_system_set(
            SystemSet::on_enter(GameState::Matchmaking)
                .with_system(start_matchbox_socket)
                .with_system(setup),
        )
        .add_system_set(SystemSet::on_update(GameState::Matchmaking).with_system(wait_for_players))
        .add_system_set(SystemSet::on_enter(GameState::InGame).with_system(spawn_players))
        .add_system_set(SystemSet::on_update(GameState::InGame).with_system(camera_follow))
        .run();
}

const MAP_SIZE: i32 = 41;
const GRID_WIDTH: f32 = 0.05;

#[derive(AssetCollection)]
struct ImageAssets {
    #[asset(path = "bullet.png")]
    bullet: Handle<Image>,
}

fn setup(mut commands: Commands) {
    // Horizontal lines
    for i in 0..=MAP_SIZE {
        commands.spawn_bundle(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(
                0.,
                i as f32 - MAP_SIZE as f32 / 2.,
                0.,
            )),
            sprite: Sprite {
                color: Color::rgb(0.27, 0.27, 0.27),
                custom_size: Some(Vec2::new(MAP_SIZE as f32, GRID_WIDTH)),
                ..default()
            },
            ..default()
        });
    }

    // Vertical lines
    for i in 0..=MAP_SIZE {
        commands.spawn_bundle(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(
                i as f32 - MAP_SIZE as f32 / 2.,
                0.,
                0.,
            )),
            sprite: Sprite {
                color: Color::rgb(0.27, 0.27, 0.27),
                custom_size: Some(Vec2::new(GRID_WIDTH, MAP_SIZE as f32)),
                ..default()
            },
            ..default()
        });
    }

    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn_bundle(camera_bundle);
}

fn spawn_players(mut commands: Commands, mut rip: ResMut<RollbackIdProvider>) {
    info!("Spawning players");

    // Player 1
    commands
        .spawn_bundle(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(-2., 0., 100.)),
            sprite: Sprite {
                color: Color::rgb(0., 0.47, 1.),
                custom_size: Some(Vec2::new(1., 1.)),
                ..default()
            },
            ..default()
        })
        .insert(Player { handle: 0 })
        .insert(Rollback::new(rip.next_id()));

    // Player 2
    commands
        .spawn_bundle(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(2., 0., 100.)),
            sprite: Sprite {
                color: Color::rgb(0., 0.4, 0.),
                custom_size: Some(Vec2::new(1., 1.)),
                ..default()
            },
            ..default()
        })
        .insert(Player { handle: 1 })
        .insert(Rollback::new(rip.next_id()));
}

fn start_matchbox_socket(mut commands: Commands) {
    let room_url = "ws://127.0.0.1:3536/extreme_bevy?next=2";
    info!("connecting to matchbox server: {:?}", room_url);
    let (socket, message_loop) = WebRtcSocket::new(room_url);

    // The message loop needs to be awaited, or nothing will happen.
    // We do this here using bevy's task system.
    IoTaskPool::get().spawn(message_loop).detach();

    commands.insert_resource(Some(socket));
}

fn wait_for_players(
    mut commands: Commands,
    mut socket: ResMut<Option<WebRtcSocket>>,
    mut state: ResMut<State<GameState>>,
) {
    let socket = socket.as_mut();

    // If there is no socket we've already started the game
    if socket.is_none() {
        return;
    }

    // Check for new connections
    socket.as_mut().unwrap().accept_new_connections();
    let players = socket.as_ref().unwrap().players();

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
        if player == PlayerType::Local {
            commands.insert_resource(LocalPlayerHandle(i));
        }

        session_builder = session_builder
            .add_player(player, i)
            .expect("failed to add player");
    }

    // move the socket out of the resource (required because GGRS takes ownership of it)
    let socket = socket.take().unwrap();

    // start the GGRS session
    let session = session_builder
        .start_p2p_session(socket)
        .expect("failed to start session");

    commands.insert_resource(session);
    commands.insert_resource(SessionType::P2PSession);

    state.set(GameState::InGame).unwrap();
}

fn move_players(
    inputs: Res<Vec<(u8, InputStatus)>>,
    mut player_query: Query<(&mut Transform, &Player)>,
) {
    for (mut transform, player) in player_query.iter_mut() {
        let (input, _) = inputs[player.handle];
        let direction = direction(input);

        if direction == Vec2::ZERO {
            continue;
        }

        let move_speed = 0.13;
        let move_delta = direction * move_speed;

        let old_pos = transform.translation.xy();
        let limit = Vec2::splat(MAP_SIZE as f32 / 2. - 0.5);
        let new_pos = (old_pos + move_delta).clamp(-limit, limit);

        transform.translation.x = new_pos.x;
        transform.translation.y = new_pos.y;
    }
}

fn camera_follow(
    player_handle: Option<Res<LocalPlayerHandle>>,
    player_query: Query<(&Player, &Transform)>,
    mut camera_query: Query<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    let player_handle = match player_handle {
        Some(handle) => handle.0,
        None => return, // Session hasn't started yet
    };

    for (player, player_transform) in player_query.iter() {
        if player.handle != player_handle {
            continue;
        }

        let pos = player_transform.translation;

        for mut transform in camera_query.iter_mut() {
            transform.translation.x = pos.x;
            transform.translation.y = pos.y;
        }
    }
}
