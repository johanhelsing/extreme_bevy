use crate::Config;
use bevy::{prelude::*, utils::HashMap};
use bevy_ggrs::{LocalInputs, LocalPlayers};

const INPUT_UP: u8 = 1 << 0;
const INPUT_DOWN: u8 = 1 << 1;
const INPUT_LEFT: u8 = 1 << 2;
const INPUT_RIGHT: u8 = 1 << 3;
const INPUT_FIRE: u8 = 1 << 4;

pub fn read_local_inputs(
    mut commands: Commands,
    keys: Res<Input<KeyCode>>,
    local_players: Res<LocalPlayers>,
) {
    let mut local_inputs = HashMap::new();

    for handle in &local_players.0 {
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

        local_inputs.insert(*handle, input);
    }

    commands.insert_resource(LocalInputs::<Config>(local_inputs));
}

pub fn direction(input: u8) -> Vec2 {
    let mut direction = Vec2::ZERO;
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
    direction
}
