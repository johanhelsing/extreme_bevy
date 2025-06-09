use args::Args;
use bevy::{camera::ScalingMode, prelude::*};
use bevy_asset_loader::prelude::*;
use bevy_egui::{
    EguiContexts, EguiPlugin,
    egui::{self, Align2, Color32, FontId, RichText},
};
use bevy_ggrs::{ggrs::DesyncDetection, prelude::*, *};
use bevy_matchbox::prelude::*;
use bevy_roll_safe::prelude::*;
use clap::Parser;
use components::*;
use input::*;
use rand::{Rng, RngCore, SeedableRng, rng};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::f32::consts::PI;

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

#[derive(Resource, Clone, Deref, DerefMut)]
struct RoundEndTimer(Timer);

#[derive(Resource, Default, Clone, Copy, Debug)]
struct Scores(u32, u32);

impl Default for RoundEndTimer {
    fn default() -> Self {
        RoundEndTimer(Timer::from_seconds(1.0, TimerMode::Repeating))
    }
}

#[derive(Resource, Default, Clone, Copy, Debug, Deref, DerefMut)]
struct SessionSeed(u64);

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
            RollbackSchedulePlugin::new_ggrs(),
            EguiPlugin::default(),
        ))
        .init_state::<GameState>()
        .add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .load_collection::<ImageAssets>()
                .continue_to_state(GameState::Matchmaking),
        )
        .init_ggrs_state::<RollbackState>()
        .rollback_resource_with_clone::<RoundEndTimer>()
        .rollback_resource_with_copy::<Scores>()
        .rollback_component_with_clone::<Transform>()
        .rollback_component_with_copy::<Bullet>()
        .rollback_component_with_copy::<BulletReady>()
        .rollback_component_with_copy::<Player>()
        .rollback_component_with_copy::<Wall>()
        .rollback_component_with_copy::<MoveDir>()
        .rollback_component_with_copy::<DistanceTraveled>()
        .rollback_component_with_clone::<Sprite>()
        .checksum_component::<Transform>(checksum_transform)
        .insert_resource(args)
        .insert_resource(ClearColor(Color::srgb(0.53, 0.53, 0.53)))
        .init_resource::<RoundEndTimer>()
        .init_resource::<Scores>()
        .add_systems(
            OnEnter(GameState::Matchmaking),
            (setup, start_matchbox_socket.run_if(p2p_mode)),
        )
        .add_systems(
            Update,
            (
                (
                    wait_for_players.run_if(p2p_mode),
                    start_synctest_session.run_if(synctest_mode),
                )
                    .run_if(in_state(GameState::Matchmaking)),
                (camera_follow, update_score_ui, handle_ggrs_events)
                    .run_if(in_state(GameState::InGame)),
            ),
        )
        .add_systems(ReadInputs, read_local_inputs)
        .add_systems(
            OnEnter(RollbackState::InRound),
            (generate_map, spawn_players.after(generate_map)),
        )
        .add_systems(
            RollbackUpdate,
            (
                move_players,
                update_player_sprites
                    .after(move_players)
                    // both systems operate on the `Sprite` component, but not on the same entities
                    .ambiguous_with(resolve_wall_collisions)
                    // both systems operate on the `Sprite` component, but not on the same entities
                    .ambiguous_with(bullet_wall_collisions),
                resolve_wall_collisions.after(move_players),
                reload_bullet,
                fire_bullets
                    .after(move_players)
                    .after(reload_bullet)
                    .after(resolve_wall_collisions),
                move_bullet.after(fire_bullets),
                bullet_wall_collisions.after(move_bullet),
                kill_players.after(move_bullet).after(move_players),
            )
                .run_if(in_state(RollbackState::InRound))
                .after(bevy_roll_safe::apply_state_transition::<RollbackState>),
        )
        .add_systems(
            RollbackUpdate,
            round_end_timeout
                .run_if(in_state(RollbackState::RoundEnd))
                .ambiguous_with(kill_players),
        )
        .run();
}

const MAP_SIZE: i32 = 41;
const GRID_WIDTH: f32 = 0.05;

#[derive(AssetCollection, Resource)]
struct ImageAssets {
    #[asset(path = "bullet.png")]
    bullet: Handle<Image>,
    #[asset(path = "player_1.png")]
    player_1: Handle<Image>,
    #[asset(path = "player_2.png")]
    player_2: Handle<Image>,
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
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMax {
                max_width: 16.0,
                max_height: 9.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn generate_map(
    mut commands: Commands,
    walls: Query<Entity, With<Wall>>,
    scores: Res<Scores>,
    session_seed: Res<SessionSeed>,
) {
    // despawn walls from previous round (if any)
    for wall in &walls {
        commands.entity(wall).despawn();
    }

    let mut rng = Xoshiro256PlusPlus::seed_from_u64((scores.0 + scores.1) as u64 ^ **session_seed);

    for _ in 0..20 {
        let max_box_size = MAP_SIZE / 4;
        let width = rng.random_range(1..max_box_size);
        let height = rng.random_range(1..max_box_size);

        let cell_x = rng.random_range(0..=(MAP_SIZE - width));
        let cell_y = rng.random_range(0..=(MAP_SIZE - height));

        let size = Vec2::new(width as f32, height as f32);

        commands.spawn((
            Wall,
            Transform::from_translation(Vec3::new(
                cell_x as f32 + size.x / 2. - MAP_SIZE as f32 / 2.,
                cell_y as f32 + size.y / 2. - MAP_SIZE as f32 / 2.,
                10.,
            )),
            Sprite {
                color: Color::srgb(0.27, 0.27, 0.27),
                custom_size: Some(size),
                ..default()
            },
        ));
    }
}

fn spawn_players(
    mut commands: Commands,
    players: Query<Entity, With<Player>>,
    bullets: Query<Entity, With<Bullet>>,
    scores: Res<Scores>,
    session_seed: Res<SessionSeed>,
    images: Res<ImageAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    info!("Spawning players");

    for player in &players {
        commands.entity(player).despawn();
    }

    for bullet in &bullets {
        commands.entity(bullet).despawn();
    }

    let mut rng = Xoshiro256PlusPlus::seed_from_u64((scores.0 + scores.1) as u64 ^ **session_seed);
    let half = MAP_SIZE as f32 / 2.;
    let p1_pos = Vec2::new(rng.random_range(-half..half), rng.random_range(-half..half));
    let p2_pos = Vec2::new(rng.random_range(-half..half), rng.random_range(-half..half));

    // 8 directional animations per player, up to 6 frames each
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(22), 6, 8, None, None);
    let layout = texture_atlas_layouts.add(layout.clone());

    // Player 1
    commands
        .spawn((
            Player { handle: 0 },
            Transform::from_translation(p1_pos.extend(100.)),
            BulletReady(true),
            MoveDir(-Vec2::X),
            Sprite {
                image: images.player_1.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: layout.clone(),
                    index: 0,
                }),
                custom_size: Some(Vec2::splat(1.4)),
                ..default()
            },
        ))
        .add_rollback();

    // Player 2
    commands
        .spawn((
            Player { handle: 1 },
            Transform::from_translation(p2_pos.extend(100.)),
            BulletReady(true),
            MoveDir(-Vec2::X),
            Sprite {
                image: images.player_2.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: layout.clone(),
                    index: 0,
                }),
                custom_size: Some(Vec2::splat(1.4)),
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

    // determine the seed
    let id = socket.id().expect("no peer id assigned").0.as_u64_pair();
    let mut seed = id.0 ^ id.1;
    for peer in socket.connected_peers() {
        let peer_id = peer.0.as_u64_pair();
        seed ^= peer_id.0 ^ peer_id.1;
    }
    commands.insert_resource(SessionSeed(seed));

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
    commands.insert_resource(SessionSeed(rng().next_u64()));
    next_state.set(GameState::InGame);
}

fn handle_ggrs_events(mut session: ResMut<Session<Config>>) {
    if let Session::P2P(s) = session.as_mut() {
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
                    error!(
                        "Desync on frame {frame}. Local checksum: {local_checksum:X}, remote checksum: {remote_checksum:X}"
                    );
                }
                _ => info!("GGRS event: {event:?}"),
            }
        }
    }
}

fn move_players(
    mut players: Query<(&mut Transform, &mut MoveDir, &mut DistanceTraveled, &Player)>,
    inputs: Res<PlayerInputs<Config>>,
    time: Res<Time>,
) {
    for (mut transform, mut move_direction, mut distance, player) in &mut players {
        let (input, _) = inputs[player.handle];

        let direction = direction(input);

        if direction == Vec2::ZERO {
            continue;
        }

        move_direction.0 = direction;

        let move_speed = 6.;
        let move_delta = direction * move_speed * time.delta_secs();

        let old_pos = transform.translation.xy();
        let limit = Vec2::splat(MAP_SIZE as f32 / 2. - 0.5);
        let new_pos = (old_pos + move_delta).clamp(-limit, limit);

        transform.translation.x = new_pos.x;
        transform.translation.y = new_pos.y;

        distance.0 += move_delta.length();
    }
}

fn resolve_wall_collisions(
    mut players: Query<&mut Transform, With<Player>>,
    walls: Query<(&Transform, &Sprite), (With<Wall>, Without<Player>)>,
) {
    for mut player_transform in &mut players {
        for (wall_transform, wall_sprite) in &walls {
            let wall_size = wall_sprite.custom_size.expect("wall doesn't have a size");
            let wall_pos = wall_transform.translation.xy();
            let player_pos = player_transform.translation.xy();

            let wall_to_player = player_pos - wall_pos;
            // exploit the symmetry of the problem,
            // treat things as if they are in the first quadrant
            let wall_to_player_abs = wall_to_player.abs();
            let wall_corner_to_player_center = wall_to_player_abs - wall_size / 2.;

            let corner_to_corner = wall_corner_to_player_center - Vec2::splat(PLAYER_RADIUS);

            if corner_to_corner.x > 0. || corner_to_corner.y > 0. {
                // no collision
                continue;
            }

            if corner_to_corner.x > corner_to_corner.y {
                // least overlap on x axis
                player_transform.translation.x -= wall_to_player.x.signum() * corner_to_corner.x;
            } else {
                // least overlap on y axis
                player_transform.translation.y -= wall_to_player.y.signum() * corner_to_corner.y;
            }
        }
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

fn bullet_wall_collisions(
    mut commands: Commands,
    bullets: Query<(Entity, &Transform), With<Bullet>>,
    walls: Query<(&Transform, &Sprite), (With<Wall>, Without<Bullet>)>,
) {
    let map_limit = MAP_SIZE as f32 / 2.;

    for (bullet_entity, bullet_transform) in &bullets {
        let bullet_pos = bullet_transform.translation.xy();

        if bullet_pos.x.abs() > map_limit || bullet_pos.y.abs() > map_limit {
            commands.entity(bullet_entity).despawn();
            continue;
        }

        for (wall_transform, wall_sprite) in &walls {
            let wall_size = wall_sprite.custom_size.expect("wall doesn't have a size");
            let wall_pos = wall_transform.translation.xy();
            let center_to_center = wall_pos - bullet_pos;
            // exploit symmetry
            let center_to_center = center_to_center.abs();
            let corner_to_center = center_to_center - wall_size / 2.;
            if corner_to_center.x < 0. && corner_to_center.y < 0. {
                // we're inside a wall
                commands.entity(bullet_entity).despawn();
                break;
            }
        }
    }
}

const PLAYER_RADIUS: f32 = 0.5;
const BULLET_RADIUS: f32 = 0.025;

fn kill_players(
    mut commands: Commands,
    players: Query<(Entity, &Transform, &Player), Without<Bullet>>,
    bullets: Query<&Transform, With<Bullet>>,
    mut next_state: ResMut<NextState<RollbackState>>,
    mut scores: ResMut<Scores>,
) {
    for (player_entity, player_transform, player) in &players {
        for bullet_transform in &bullets {
            let distance = Vec2::distance(
                player_transform.translation.xy(),
                bullet_transform.translation.xy(),
            );
            if distance < PLAYER_RADIUS + BULLET_RADIUS {
                commands.entity(player_entity).despawn();
                next_state.set(RollbackState::RoundEnd);

                if player.handle == 0 {
                    scores.1 += 1;
                } else {
                    scores.0 += 1;
                }
                info!("player died: {scores:?}")
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

fn round_end_timeout(
    mut timer: ResMut<RoundEndTimer>,
    mut state: ResMut<NextState<RollbackState>>,
    time: Res<Time>,
) {
    timer.tick(time.delta());

    if timer.just_finished() {
        state.set(RollbackState::InRound);
    }
}

fn update_score_ui(mut contexts: EguiContexts, scores: Res<Scores>) -> Result {
    let Scores(p1_score, p2_score) = *scores;

    egui::Area::new("score".into())
        .anchor(Align2::CENTER_TOP, (0., 25.))
        .show(contexts.ctx_mut()?, |ui| {
            ui.label(
                RichText::new(format!("{p1_score} - {p2_score}"))
                    .color(Color32::BLACK)
                    .font(FontId::proportional(72.0)),
            );
        });

    Ok(())
}

fn update_player_sprites(
    mut players: Query<(&mut Sprite, &MoveDir, &DistanceTraveled), With<Player>>,
) {
    for (mut sprite, move_dir, distance) in &mut players {
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            // 8 directional animations, each 45 degrees apart

            // in radians, signed: 0 is right, PI/2 is up, -PI/2 is down
            let angle = move_dir.0.to_angle();

            // divide the angle by 45 degrees (PI/4) to get the octant
            let octant = (angle / (PI / 4.)).round() as i32;

            // convert to an octant index in the range [0, 7]
            let octant = if octant < 0 { octant + 8 } else { octant } as usize;

            // each row has 6 frames, so we multiply the octant index by 6
            // to get the index of the first frame in that row in the texture atlas.
            let anim_start = octant * 6;

            // get animation length based on octant (row in the sprite sheet)
            let anim_len = match octant {
                0 => 5,
                1 => 5,
                2 => 4,
                3 => 5,
                4 => 5,
                5 => 4,
                6 => 4,
                7 => 5,
                _ => unreachable!(),
            };

            let anim_speed = 4.0; // frames per units of distance traveled
            let current_frame = (distance.0 * anim_speed) as usize % anim_len;

            atlas.index = anim_start + current_frame;
        }
    }
}
