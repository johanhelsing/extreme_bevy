use bevy::prelude::*;

#[derive(Component)]
pub struct Player {
    pub handle: usize,
}

#[derive(Component, Clone, Copy)]
pub struct BulletReady(pub bool);

#[derive(Component)]
pub struct Bullet;

#[derive(Component, Clone, Copy)]
pub struct MoveDir(pub Vec2);
