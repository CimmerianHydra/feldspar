use bevy::math::{IVec3, Vec3};
use bevy::prelude::*;
use serde::Deserialize;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE SIX DIRECTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The vocabulary for neighbours, cull tags and connection masks.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Deserialize)]
pub enum Direction {
    North = 0,   // -Z
    South = 1,   // +Z
    East  = 2,   // +X
    West  = 3,   // -X
    Up    = 4,   // +Y
    Down  = 5,   // -Y
}

impl Direction {
    #[inline]
    pub fn as_ivec3(self) -> IVec3 {
        match self {
            Direction::North => IVec3::new(0, 0, -1),
            Direction::South => IVec3::new(0, 0, 1),
            Direction::East  => IVec3::new(1, 0, 0),
            Direction::West  => IVec3::new(-1, 0, 0),
            Direction::Up    => IVec3::new(0, 1, 0),
            Direction::Down  => IVec3::new(0, -1, 0),
        }
    }

    #[inline]
    pub fn as_vec3(self) -> Vec3 {
        match self {
            Direction::North => Vec3::new(0., 0., -1.),
            Direction::South => Vec3::new(0., 0., 1.),
            Direction::East  => Vec3::new(1., 0., 0.),
            Direction::West  => Vec3::new(-1., 0., 0.),
            Direction::Up    => Vec3::new(0., 1., 0.),
            Direction::Down  => Vec3::new(0., -1., 0.),
        }
    }

    #[inline]
    pub fn opposite(self) -> Self {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East  => Direction::West,
            Direction::West  => Direction::East,
            Direction::Up    => Direction::Down,
            Direction::Down  => Direction::Up,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – CANONICAL ORDER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Canonical iteration order. Matches the discriminants already used by
/// `Direction`, so `ALL_DIRECTIONS[dir_index(d)] == d` for every `d`.
///
/// Everything downstream — occlusion bitmasks, cull tags, the per-box face
/// array, texture slots — indexes by this order. Change it here and the
/// whole pipeline follows; change it anywhere else and you get silent
/// corruption.
pub const ALL_DIRECTIONS: [Direction; 6] = [
    Direction::North,   // 0   -Z
    Direction::South,   // 1   +Z
    Direction::East,    // 2   +X
    Direction::West,    // 3   -X
    Direction::Up,      // 4   +Y
    Direction::Down,    // 5   -Y
];

#[inline]
pub fn dir_index(d: Direction) -> usize {
    match d {
        Direction::North => 0,
        Direction::South => 1,
        Direction::East  => 2,
        Direction::West  => 3,
        Direction::Up    => 4,
        Direction::Down  => 5,
    }
}

#[inline]
pub fn dir_from_index(i: usize) -> Direction {
    debug_assert!(i < 6, "direction index {i} out of range");
    ALL_DIRECTIONS[i.min(5)]
}

/// Inverse of `Direction::as_ivec3`. Only defined for the six unit axis
/// vectors; anything else is a bug in the caller, so it panics loudly in
/// debug and falls back to North in release.
pub fn dir_from_ivec3(v: IVec3) -> Direction {
    match (v.x, v.y, v.z) {
        (0, 0, -1) => Direction::North,
        (0, 0,  1) => Direction::South,
        (1, 0,  0) => Direction::East,
        (-1, 0, 0) => Direction::West,
        (0, 1,  0) => Direction::Up,
        (0, -1, 0) => Direction::Down,
        _ => {
            debug_assert!(false, "dir_from_ivec3: {v:?} is not a unit axis vector");
            Direction::North
        }
    }
}
