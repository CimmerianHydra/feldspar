use bevy::prelude::*;
use std::collections::HashMap;

use crate::plugin::voxel::BlockShape;
use crate::plugin::graphics::block_textures::{BlockAppearance, FaceTextures};
use crate::plugin::block::material::BlockMaterial;
use crate::plugin::audio::block::SoundProfile;


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Basic Definitions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockID(pub u16);


/// A fully resolved, immutable description of one block type.
/// Created once at startup, lives in the global BlockRegistry.
pub struct BlockDefinition {
    pub id:             BlockID,
    pub name:           String,          // e.g. "oreIronAndesite"
    pub display_name:   String,          // e.g. "Andesite Iron Ore"
    pub shape:          BlockShape,
    pub appearance:     BlockAppearance,
    pub has_collision:  bool,
    pub material:       BlockMaterial,
    pub sound_profile:  SoundProfile,
}

impl BlockDefinition {
    pub fn air() -> BlockDefinition {
        BlockDefinition {
            name: "air".to_string(),
            display_name: "Air".to_string(),
            ..default()
        }
    }
}

impl Default for BlockDefinition {
    fn default() -> Self {
        BlockDefinition {
            id: BlockID(0),
            name: "default_cube".to_string(),
            display_name: "Default Cube".to_string(),
            shape: BlockShape::default(),
            appearance: BlockAppearance::default(),
            has_collision: true,
            material: BlockMaterial::default(),
            sound_profile: SoundProfile::default(),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Registry Resource
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Global resource that keeps in memory all the blocks in the game
#[derive(Resource)]
pub struct BlockRegistry {
    definitions:         Vec<BlockDefinition>,      // indexed by BlockID
    name_to_id:          HashMap<String, BlockID>
}

impl BlockRegistry {
    pub fn get(&self, id: BlockID) -> &BlockDefinition {
        &self.definitions[id.0 as usize]
    }

    pub fn register_block(&mut self, def: BlockDefinition) -> BlockID {
        let id = BlockID(self.definitions.len() as u16);
        let name = def.name.clone();
        self.definitions.push(BlockDefinition { id, ..def });
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Chunk Palette
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Each chunk comes with a Palette that maps "local" ids to the global registry
/// This way we can store potentially unlimited blocks in the registry and compress
/// the information in the 16 bits of the Voxel data structure.
/// 
/// Not yet implemented

pub struct ChunkPalette {
    /// palette[local_index] = global BlockId
    entries:     Vec<BlockID>,
    /// Reverse map for O(1) insertion lookup
    global_to_local: HashMap<BlockID, u16>,
}

impl ChunkPalette {
    pub fn new() -> Self {
        let mut p = Self { entries: Vec::new(), global_to_local: HashMap::new() };
        // Index 0 is always AIR
        p.insert(BlockID(0));
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