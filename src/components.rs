use bevy::prelude::*;
use bevy_ggrs::checksum_hasher;
use std::{
    f32::consts::PI,
    hash::{Hash, Hasher},
};

#[derive(Component, Clone, Copy)]
#[require(DistanceTraveled)]
pub struct Player {
    pub handle: usize,
}

#[derive(Component, Clone, Copy)]
pub struct BulletReady(pub bool);

#[derive(Component, Clone, Copy)]
pub struct Bullet;

#[derive(Component, Clone, Copy)]
pub struct MoveDir(pub Vec2);

impl MoveDir {
    /// Gets the index of the octant (45 degree sectors), starting from 0 (right) and going counter-clockwise:
    pub fn octant(&self) -> usize {
        // in radians, signed: 0 is right, PI/2 is up, -PI/2 is down
        let angle = self.0.to_angle();

        // divide the angle by 45 degrees (PI/4) to get the octant
        let octant = (angle / (PI / 4.)).round() as i32;

        // convert to an octant index in the range [0, 7]
        let octant = if octant < 0 { octant + 8 } else { octant } as usize;

        octant
    }
}

#[derive(Component, Default, Clone, Copy)]
pub struct DistanceTraveled(pub f32);

#[derive(Component, Clone, Copy)]
pub struct Wall;

pub fn checksum_transform(transform: &Transform) -> u64 {
    let mut hasher = checksum_hasher();

    assert!(
        transform.is_finite(),
        "Hashing is not stable for NaN f32 values."
    );

    transform.translation.x.to_bits().hash(&mut hasher);
    transform.translation.y.to_bits().hash(&mut hasher);
    transform.translation.z.to_bits().hash(&mut hasher);

    transform.rotation.x.to_bits().hash(&mut hasher);
    transform.rotation.y.to_bits().hash(&mut hasher);
    transform.rotation.z.to_bits().hash(&mut hasher);
    transform.rotation.w.to_bits().hash(&mut hasher);

    // skip transform.scale as it's not used for gameplay

    hasher.finish()
}
