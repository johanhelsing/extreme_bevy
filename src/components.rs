use bevy::prelude::*;

#[derive(Component)]
pub struct Player {
    pub handle: usize,
}

#[derive(Component, Reflect, Default)]
pub struct BulletReady(pub bool);

#[derive(Component, Reflect, Default)]
pub struct Bullet;

#[derive(Component, Reflect, Default, Clone, Copy)]
pub struct MoveDir(pub Vec2);
