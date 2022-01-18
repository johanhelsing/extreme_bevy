use bevy::prelude::*;

#[derive(Component)]
pub struct Player {
    pub handle: usize,
}

#[derive(Component, Reflect, Default)]
pub struct BulletReady(pub bool);

#[derive(Component, Reflect, Default)]
pub struct Bullet;
