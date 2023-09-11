use args::Args;
use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_asset_loader::prelude::*;
use bevy_ggrs::{ggrs::DesyncDetection, prelude::*, *};
use bevy_matchbox::prelude::*;
use bevy_roll_safe::prelude::*;
use clap::Parser;
use components::*;
use input::*;

mod args;
mod components;
mod input;

// The first generic parameter, u8, is the input type: 4-directions + fire fits
// easily in a single byte
// The second parameter is the address type of peers: Matchbox' WebRtcSocket
// addresses are called `PeerId`s
type Config = bevy_ggrs::GgrsConfig<u8, PeerId>;

#[derive(States, Clone, Eq, PartialEq, Debug, Hash, Default)]
enum GameState {
    #[default]
    AssetLoading,
    Matchmaking,
    InGame,
}

#[derive(States, Clone, Eq, PartialEq, Debug, Hash, Default)]
enum RollbackState {
    /// When the characters running and gunning
    #[default]
    InRound,
    /// When one character is dead, and we're transitioning to the next round
    RoundEnd,
}

fn main() {
    let args = Args::parse();
    eprintln!("{args:?}");

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    // fill the entire browser window
                    fit_canvas_to_parent: true,
                    // don't hijack keyboard shortcuts like F5, F6, F12, Ctrl+R etc.
                    prevent_default_event_handling: false,
                    ..default()
                }),
                ..default()
            }),
            GgrsPlugin::<Config>::default(),
        ))
        .init_state::<GameState>()
        .add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .load_collection::<ImageAssets>()
                .continue_to_state(GameState::Matchmaking),
        )
        .init_ggrs_state::<RollbackState>()
        .rollback_component_with_clone::<Transform>()
        .rollback_component_with_copy::<Bullet>()
        .rollback_component_with_copy::<BulletReady>()
        .rollback_component_with_copy::<Player>()
        .rollback_component_with_copy::<MoveDir>()
        .rollback_component_with_clone::<Sprite>()
        .checksum_component::<Transform>(checksum_transform)
        .insert_resource(args)
        .insert_resource(ClearColor(Color::srgb(0.53, 0.53, 0.53)))
        .add_systems(
            OnEnter(GameState::Matchmaking),
            (setup, start_matchbox_socket.run_if(p2p_mode)),
        )
        .add_systems(OnEnter(GameState::InGame), spawn_players)
        .add_systems(
            Update,
            (
                (
                    wait_for_players.run_if(p2p_mode),
                    start_synctest_session.run_if(synctest_mode),
                )
                    .run_if(in_state(GameState::Matchmaking)),
                (camera_follow, handle_ggrs_events).run_if(in_state(GameState::InGame)),
            ),
        )
        .add_systems(ReadInputs, read_local_inputs)
        .add_systems(
            GgrsSchedule,
            (
                move_players,
                reload_bullet,
                fire_bullets.after(move_players).after(reload_bullet),
                move_bullet.after(fire_bullets),
                kill_players.after(move_bullet).after(move_players),
            )
                .run_if(in_state(RollbackState::InRound))
                .after(apply_state_transition::<RollbackState>),
        )
        .run();
}

const MAP_SIZE: i32 = 41;
const GRID_WIDTH: f32 = 0.05;

#[derive(AssetCollection, Resource)]
struct ImageAssets {
    #[asset(path = "bullet.png")]
    bullet: Handle<Image>,
}

fn synctest_mode(args: Res<Args>) -> bool {
    args.synctest
}

fn p2p_mode(args: Res<Args>) -> bool {
    !args.synctest
}

fn setup(mut commands: Commands) {
    // Horizontal lines
    for i in 0..=MAP_SIZE {
        commands.spawn((
            Transform::from_translation(Vec3::new(0., i as f32 - MAP_SIZE as f32 / 2., 0.)),
            Sprite {
                color: Color::srgb(0.27, 0.27, 0.27),
                custom_size: Some(Vec2::new(MAP_SIZE as f32, GRID_WIDTH)),
                ..default()
            },
        ));
    }

    // Vertical lines
    for i in 0..=MAP_SIZE {
        commands.spawn((
            Transform::from_translation(Vec3::new(i as f32 - MAP_SIZE as f32 / 2., 0., 0.)),
            Sprite {
                color: Color::srgb(0.27, 0.27, 0.27),
                custom_size: Some(Vec2::new(GRID_WIDTH, MAP_SIZE as f32)),
                ..default()
            },
        ));
    }

    commands.spawn((
        Camera2d,
        OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 10.,
            },
            ..OrthographicProjection::default_2d()
        },
    ));
}

fn spawn_players(mut commands: Commands) {
    info!("Spawning players");

    // Player 1
    commands
        .spawn((
            Player { handle: 0 },
            Transform::from_translation(Vec3::new(-2., 0., 100.)),
            BulletReady(true),
            MoveDir(-Vec2::X),
            Sprite {
                color: Color::srgb(0., 0.47, 1.),
                custom_size: Some(Vec2::new(1., 1.)),
                ..default()
            },
        ))
        .add_rollback();

    // Player 2
    commands
        .spawn((
            Player { handle: 1 },
            Transform::from_translation(Vec3::new(2., 0., 100.)),
            BulletReady(true),
            MoveDir(-Vec2::X),
            Sprite {
                color: Color::srgb(0., 0.4, 0.),
                custom_size: Some(Vec2::new(1., 1.)),
                ..default()
            },
        ))
        .add_rollback();
}

fn start_matchbox_socket(mut commands: Commands) {
    let room_url = "ws://127.0.0.1:3536/extreme_bevy?next=2";
    info!("connecting to matchbox server: {room_url}");
    commands.insert_resource(MatchboxSocket::new_unreliable(room_url));
}

fn wait_for_players(
    mut commands: Commands,
    mut socket: ResMut<MatchboxSocket>,
    mut next_state: ResMut<NextState<GameState>>,
    args: Res<Args>,
) {
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
        .with_desync_detection_mode(DesyncDetection::On { interval: 1 })
        .with_input_delay(args.input_delay);

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
    next_state.set(GameState::InGame);
}

fn start_synctest_session(mut commands: Commands, mut next_state: ResMut<NextState<GameState>>) {
    info!("Starting synctest session");
    let num_players = 2;

    let mut session_builder = ggrs::SessionBuilder::<Config>::new().with_num_players(num_players);

    for i in 0..num_players {
        session_builder = session_builder
            .add_player(PlayerType::Local, i)
            .expect("failed to add player");
    }

    let ggrs_session = session_builder
        .start_synctest_session()
        .expect("failed to start session");

    commands.insert_resource(bevy_ggrs::Session::SyncTest(ggrs_session));
    next_state.set(GameState::InGame);
}

fn handle_ggrs_events(mut session: ResMut<Session<Config>>) {
    match session.as_mut() {
        Session::P2P(s) => {
            for event in s.events() {
                match event {
                    GgrsEvent::Disconnected { .. } | GgrsEvent::NetworkInterrupted { .. } => {
                        warn!("GGRS event: {event:?}")
                    }
                    GgrsEvent::DesyncDetected {
                        local_checksum,
                        remote_checksum,
                        frame,
                        ..
                    } => {
                        error!("Desync on frame {frame}. Local checksum: {local_checksum:X}, remote checksum: {remote_checksum:X}");
                    }
                    _ => info!("GGRS event: {event:?}"),
                }
            }
        }
        _ => {}
    }
}

fn move_players(
    mut players: Query<(&mut Transform, &mut MoveDir, &Player)>,
    inputs: Res<PlayerInputs<Config>>,
    time: Res<Time>,
) {
    for (mut transform, mut move_direction, player) in &mut players {
        let (input, _) = inputs[player.handle];

        let direction = direction(input);

        if direction == Vec2::ZERO {
            continue;
        }

        move_direction.0 = direction;

        let move_speed = 7.;
        let move_delta = direction * move_speed * time.delta_secs();

        let old_pos = transform.translation.xy();
        let limit = Vec2::splat(MAP_SIZE as f32 / 2. - 0.5);
        let new_pos = (old_pos + move_delta).clamp(-limit, limit);

        transform.translation.x = new_pos.x;
        transform.translation.y = new_pos.y;
    }
}

fn reload_bullet(
    inputs: Res<PlayerInputs<Config>>,
    mut players: Query<(&mut BulletReady, &Player)>,
) {
    for (mut can_fire, player) in players.iter_mut() {
        let (input, _) = inputs[player.handle];
        if !fire(input) {
            can_fire.0 = true;
        }
    }
}

fn fire_bullets(
    mut commands: Commands,
    inputs: Res<PlayerInputs<Config>>,
    images: Res<ImageAssets>,
    mut players: Query<(&Transform, &Player, &mut BulletReady, &MoveDir)>,
) {
    for (transform, player, mut bullet_ready, move_dir) in &mut players {
        let (input, _) = inputs[player.handle];
        if fire(input) && bullet_ready.0 {
            let player_pos = transform.translation.xy();
            let pos = player_pos + move_dir.0 * PLAYER_RADIUS + BULLET_RADIUS;
            commands
                .spawn((
                    Bullet,
                    Transform::from_translation(pos.extend(200.))
                        .with_rotation(Quat::from_rotation_arc_2d(Vec2::X, move_dir.0)),
                    *move_dir,
                    Sprite {
                        image: images.bullet.clone(),
                        custom_size: Some(Vec2::new(0.3, 0.1)),
                        ..default()
                    },
                ))
                .add_rollback();
            bullet_ready.0 = false;
        }
    }
}

fn move_bullet(mut bullets: Query<(&mut Transform, &MoveDir), With<Bullet>>, time: Res<Time>) {
    for (mut transform, dir) in &mut bullets {
        let speed = 20.;
        let delta = dir.0 * speed * time.delta_secs();
        transform.translation += delta.extend(0.);
    }
}

const PLAYER_RADIUS: f32 = 0.5;
const BULLET_RADIUS: f32 = 0.025;

fn kill_players(
    mut commands: Commands,
    players: Query<(Entity, &Transform), (With<Player>, Without<Bullet>)>,
    bullets: Query<&Transform, With<Bullet>>,
    mut next_state: ResMut<NextState<RollbackState>>,
) {
    for (player, player_transform) in &players {
        for bullet_transform in &bullets {
            let distance = Vec2::distance(
                player_transform.translation.xy(),
                bullet_transform.translation.xy(),
            );
            if distance < PLAYER_RADIUS + BULLET_RADIUS {
                commands.entity(player).despawn_recursive();
                next_state.set(RollbackState::RoundEnd);
            }
        }
    }
}

fn camera_follow(
    local_players: Res<LocalPlayers>,
    players: Query<(&Player, &Transform)>,
    mut cameras: Query<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    for (player, player_transform) in &players {
        // only follow the local player
        if !local_players.0.contains(&player.handle) {
            continue;
        }

        let pos = player_transform.translation;

        for mut transform in &mut cameras {
            transform.translation.x = pos.x;
            transform.translation.y = pos.y;
        }
    }
}
