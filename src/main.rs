use bevy::{math::Vec3Swizzles, prelude::*, render::camera::ScalingMode, tasks::IoTaskPool};
use bevy_asset_loader::prelude::*;
use bevy_ggrs::{ggrs::PlayerType, *};
use components::*;
use input::*;
use matchbox_socket::WebRtcSocket;

mod components;
mod input;

#[derive(Resource)]
struct Session {
    socket: Option<WebRtcSocket>,
}

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
    Interlude,
}

#[derive(Resource)]
struct LocalPlayerHandle(usize);

#[derive(Resource, Default)]
struct InterludeTimer(usize);

fn main() {
    let mut app = App::new();

    GGRSPlugin::<GgrsConfig>::new()
        .with_input_system(input)
        .with_rollback_schedule(
            Schedule::default().with_stage(
                "ROLLBACK_STAGE",
                SystemStage::single_threaded()
                    .with_system_set(State::<GameState>::get_driver())
                    .with_system_set(
                        SystemSet::on_enter(GameState::Interlude)
                            .with_system(reset_interlude_timer),
                    )
                    .with_system_set(
                        SystemSet::on_update(GameState::Interlude).with_system(interlude_timer),
                    )
                    .with_system_set(
                        SystemSet::on_enter(GameState::InGame).with_system(spawn_players),
                    )
                    .with_system_set(
                        SystemSet::on_update(GameState::InGame)
                            .with_system(move_players)
                            .with_system(reload_bullet)
                            .with_system(fire_bullets.after(move_players).after(reload_bullet))
                            .with_system(move_bullet)
                            .with_system(kill_players.after(move_bullet).after(move_players)),
                    ),
            ),
        )
        .register_rollback_component::<Transform>()
        .register_rollback_component::<BulletReady>()
        .register_rollback_component::<MoveDir>()
        .build(&mut app);

    app.add_state(GameState::AssetLoading)
        .add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .with_collection::<ImageAssets>()
                .continue_to_state(GameState::Matchmaking),
        )
        .init_resource::<InterludeTimer>()
        .insert_resource(ClearColor(Color::rgb(0.53, 0.53, 0.53)))
        // .insert_resource(bevy::ecs::schedule::ReportExecutionOrderAmbiguities)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                // fill the entire browser window
                fit_canvas_to_parent: true,
                ..default()
            },
            ..default()
        }))
        .add_system_set(
            SystemSet::on_enter(GameState::Matchmaking)
                .with_system(start_matchbox_socket)
                .with_system(setup),
        )
        .add_system_set(SystemSet::on_update(GameState::Matchmaking).with_system(wait_for_players))
        .add_system_set(SystemSet::on_update(GameState::InGame).with_system(camera_follow))
        .run();
}

const MAP_SIZE: i32 = 41;
const GRID_WIDTH: f32 = 0.05;

#[derive(AssetCollection, Resource)]
struct ImageAssets {
    #[asset(path = "bullet.png")]
    bullet: Handle<Image>,
}

fn reset_interlude_timer(mut timer: ResMut<InterludeTimer>) {
    timer.0 = 60 * 1;
}

fn interlude_timer(mut timer: ResMut<InterludeTimer>, mut state: ResMut<State<GameState>>) {
    if timer.0 == 0 {
        state.set(GameState::InGame).unwrap();
    } else {
        timer.0 -= 1;
    }
}

fn setup(mut commands: Commands) {
    // Horizontal lines
    for i in 0..=MAP_SIZE {
        commands.spawn(SpriteBundle {
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
        commands.spawn(SpriteBundle {
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
    commands.spawn(camera_bundle);
}

fn spawn_players(
    mut commands: Commands,
    mut rip: ResMut<RollbackIdProvider>,
    player_query: Query<Entity, With<Player>>,
    bullet_query: Query<Entity, With<Bullet>>,
) {
    info!("Spawning players");

    for player in player_query.iter() {
        commands.entity(player).despawn_recursive();
    }
    for bullet in bullet_query.iter() {
        commands.entity(bullet).despawn_recursive();
    }

    // Player 1
    commands.spawn((
        Player { handle: 0 },
        BulletReady(true),
        MoveDir(-Vec2::X),
        Rollback::new(rip.next_id()),
        SpriteBundle {
            transform: Transform::from_translation(Vec3::new(-2., 0., 100.)),
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
        Player { handle: 1 },
        BulletReady(true),
        MoveDir(Vec2::X),
        Rollback::new(rip.next_id()),
        SpriteBundle {
            transform: Transform::from_translation(Vec3::new(2., 0., 100.)),
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

fn wait_for_players(
    mut commands: Commands,
    mut session: ResMut<Session>,
    mut state: ResMut<State<GameState>>,
    mut interlude_timer: ResMut<InterludeTimer>,
) {
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
        if player == PlayerType::Local {
            commands.insert_resource(LocalPlayerHandle(i));
        }

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

    interlude_timer.0 = 3 * 60;
    state.set(GameState::Interlude).unwrap();
}

fn move_players(
    inputs: Res<PlayerInputs<GgrsConfig>>,
    mut player_query: Query<(&mut Transform, &mut MoveDir, &Player)>,
) {
    for (mut transform, mut move_direction, player) in player_query.iter_mut() {
        let (input, _) = inputs[player.handle];
        let direction = direction(input);

        if direction == Vec2::ZERO {
            continue;
        }

        move_direction.0 = direction;

        let move_speed = 0.13;
        let move_delta = direction * move_speed;

        let old_pos = transform.translation.xy();
        let limit = Vec2::splat(MAP_SIZE as f32 / 2. - 0.5);
        let new_pos = (old_pos + move_delta).clamp(-limit, limit);

        transform.translation.x = new_pos.x;
        transform.translation.y = new_pos.y;
    }
}

fn reload_bullet(
    inputs: Res<PlayerInputs<GgrsConfig>>,
    mut query: Query<(&mut BulletReady, &Player)>,
) {
    for (mut can_fire, player) in query.iter_mut() {
        let (input, _) = inputs[player.handle];
        if !fire(input) {
            can_fire.0 = true;
        }
    }
}

fn fire_bullets(
    mut commands: Commands,
    inputs: Res<PlayerInputs<GgrsConfig>>,
    images: Res<ImageAssets>,
    mut player_query: Query<(&Transform, &Player, &mut BulletReady, &MoveDir)>,
    mut rip: ResMut<RollbackIdProvider>,
) {
    for (transform, player, mut bullet_ready, move_dir) in player_query.iter_mut() {
        let (input, _) = inputs[player.handle];
        if fire(input) && bullet_ready.0 {
            let player_pos = transform.translation.xy();
            let pos = player_pos + move_dir.0 * PLAYER_RADIUS + BULLET_RADIUS;
            commands.spawn((
                Bullet,
                Rollback::new(rip.next_id()),
                *move_dir,
                SpriteBundle {
                    transform: Transform::from_translation(pos.extend(200.))
                        .with_rotation(Quat::from_rotation_arc_2d(Vec2::X, move_dir.0)),
                    texture: images.bullet.clone(),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(0.3, 0.1)),
                        ..default()
                    },
                    ..default()
                },
            ));
            bullet_ready.0 = false;
        }
    }
}

fn move_bullet(mut query: Query<(&mut Transform, &MoveDir), With<Bullet>>) {
    for (mut transform, dir) in query.iter_mut() {
        let delta = (dir.0 * 0.35).extend(0.);
        transform.translation += delta;
    }
}

const PLAYER_RADIUS: f32 = 0.5;
const BULLET_RADIUS: f32 = 0.025;

fn kill_players(
    mut commands: Commands,
    mut state: ResMut<State<GameState>>,
    player_query: Query<(Entity, &Transform), (With<Player>, Without<Bullet>)>,
    bullet_query: Query<&Transform, With<Bullet>>,
) {
    for (player, player_transform) in player_query.iter() {
        for bullet_transform in bullet_query.iter() {
            let distance = Vec2::distance(
                player_transform.translation.xy(),
                bullet_transform.translation.xy(),
            );
            if distance < PLAYER_RADIUS + BULLET_RADIUS {
                commands.entity(player).despawn_recursive();
                let _ = state.set(GameState::Interlude);
            }
        }
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
