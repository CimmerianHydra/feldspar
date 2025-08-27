use bevy::prelude::*;

pub const CHUNK_SIZE : u32 = 32;

#[derive(Component)]
pub struct Chunk {
    visibility_array : &u32,
}