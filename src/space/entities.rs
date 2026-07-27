use bevy::prelude::*;
use std::collections::HashMap;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BLOCK-ENTITY INDEX
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Sparse map of local block positions -> block-entities, on the chunk
/// entity beside `VoxelChunk`.
///
/// Chunk-relative by construction, so it works identically for terrain and
/// ships. Only present on chunks that contain block-entities, so plain
/// terrain pays nothing.
///
/// `space` owns the *index* but knows nothing about what a block-entity is
/// or does — that's [`crate::sim::block_entity`], which keeps this table in
/// sync through its own component hooks.
#[derive(Component, Default, Debug)]
pub struct ChunkBlockEntities {
    map: HashMap<UVec3, Entity>,
}

impl ChunkBlockEntities {
    pub fn insert(&mut self, local: UVec3, entity: Entity) { self.map.insert(local, entity); }
    pub fn remove(&mut self, local: UVec3) -> Option<Entity> { self.map.remove(&local) }
    #[inline]
    pub fn get(&self, local: UVec3) -> Option<Entity> { self.map.get(&local).copied() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
    pub fn len(&self) -> usize { self.map.len() }
    pub fn iter(&self) -> impl Iterator<Item = (UVec3, Entity)> + '_ {
        self.map.iter().map(|(l, e)| (*l, *e))
    }
}
