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

#[derive(Component, Reflect, Default, Clone, Copy, Deref, DerefMut)]
#[reflect(Hash)]
pub struct HashablePosition(Vec2);

impl std::hash::Hash for HashablePosition {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.x.to_bits().hash(state);
        self.0.y.to_bits().hash(state);
    }
}
