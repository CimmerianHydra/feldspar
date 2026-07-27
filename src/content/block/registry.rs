use bevy::prelude::*;
use std::collections::HashMap;

use crate::content::block::definition::BlockDefinition;
use crate::voxel::BlockID;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Global resource that keeps every block in the game in memory.
#[derive(Resource)]
pub struct BlockRegistry {
    /// Indexed by `BlockID`. Public because the geometry baker walks it to
    /// fill in `models` after the definitions exist.
    pub definitions: Vec<BlockDefinition>,
    name_to_id:      HashMap<String, BlockID>,
}

impl BlockRegistry {
    pub fn get(&self, id: BlockID) -> &BlockDefinition {
        &self.definitions[id.0 as usize]
    }

    pub fn register_block(&mut self, def: BlockDefinition) -> BlockID {
        let name = def.name.clone();
        let id = BlockID(self.definitions.len() as u16);
        self.definitions.push(def);
        self.name_to_id.insert(name, id);
        id
    }

    pub fn by_name(&self, name: String) -> Option<BlockID> {
        self.name_to_id.get(&name).copied()
    }

    pub fn new() -> Self {
        let mut new_registry = Self { definitions: Vec::new(), name_to_id: HashMap::new() };
        new_registry.register_block(BlockDefinition::air());
        new_registry
    }

    pub fn size(&self) -> usize {
        self.definitions.len()
    }
}

impl Default for BlockRegistry {
    fn default() -> Self { Self::new() }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – CHUNK PALETTE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Each chunk comes with a palette that maps "local" ids to the global
/// registry. This way we can store potentially unlimited blocks in the
/// registry and compress the information into the 16 id bits of a `Voxel`.
///
/// Not yet wired up.
pub struct ChunkPalette {
    /// `entries[local_index] = global BlockID`
    entries:         Vec<BlockID>,
    /// Reverse map for O(1) insertion lookup.
    global_to_local: HashMap<BlockID, u16>,
}

impl ChunkPalette {
    pub fn new() -> Self {
        let mut p = Self { entries: Vec::new(), global_to_local: HashMap::new() };
        // Index 0 is always AIR
        p.insert(BlockID::AIR);
        p
    }

    /// Returns the local palette index, inserting if needed.
    pub fn insert(&mut self, global: BlockID) -> u16 {
        if let Some(&local) = self.global_to_local.get(&global) {
            return local;
        }
        let local = self.entries.len() as u16;
        self.entries.push(global);
        self.global_to_local.insert(global, local);
        local
    }

    pub fn local_to_global(&self, local: u16) -> BlockID {
        self.entries[local as usize]
    }
}

impl Default for ChunkPalette {
    fn default() -> Self { Self::new() }
}
