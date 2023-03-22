use bevy::prelude::*;

#[derive(Component)]
pub struct Player {
    pub handle: usize,
}

#[derive(Component, Clone, Copy)]
pub struct BulletReady(pub bool);
