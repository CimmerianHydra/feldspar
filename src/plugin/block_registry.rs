use bevy::prelude::*;
use std::collections::HashMap;

use crate::plugin::{graphics::block_textures::{BlockAppearance, FaceTextures}, voxel::BlockShape};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – Plugin Definition
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


pub struct BlockRegistryPlugin;

impl Plugin for BlockRegistryPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to block registry management here
        app
        .insert_resource(BlockRegistry::new())
        
        .add_systems(Startup, initialize_registry_sys)
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – Basic Definitions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockID(pub u16);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – Block Definitions and Registry
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


/// A fully resolved, immutable description of one block type.
/// Created once at startup, lives in the global BlockRegistry.
pub struct BlockDefinition {
    pub id:           BlockID,
    pub name:         String,          // e.g. "oreIronAndesite"
    pub display_name: String,          // e.g. "Andesite Iron Ore"
    pub shape:        BlockShape,
    pub appearance:   BlockAppearance,
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
        }
    }
}


/// Global resource that keeps in memory all the blocks in the game
#[derive(Resource)]
pub struct BlockRegistry {
    blocks:       Vec<BlockDefinition>,      // indexed by BlockID
}

impl BlockRegistry {
    pub fn get(&self, id: BlockID) -> &BlockDefinition {
        &self.blocks[id.0 as usize]
    }

    pub fn register_block(&mut self, def: BlockDefinition) -> BlockID {
        let id = BlockID(self.blocks.len() as u16);
        self.blocks.push(BlockDefinition { id, ..def });
        id
    }

    pub fn new() -> Self {
        let mut new_registry = Self { blocks: Vec::new() };
        new_registry.register_block(BlockDefinition::air());
        new_registry
    }

    pub fn size(&self) -> usize {
        self.blocks.len()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – Chunk Palette
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HARDCODED REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Test function that will provide with a few variations of basic blocks
pub fn initialize_registry_sys(
    mut registry: ResMut<BlockRegistry>
) {
    // We're just going to add some blocks manually

    // In the future we'll generate blocks from JSON files with their whole definition
    for shape in [
        BlockShape::Cube,
        BlockShape::Slab,
        BlockShape::Stair,
        BlockShape::Slope,
        ] {
        let base_name = "test".to_string();
        let base_display_name = "Test".to_string();

        let (name, display_name) = match shape {
            BlockShape::Cube => (format!("{}_{}", base_name, "cube"), format!("{} {}", base_display_name, "Cube")),
            BlockShape::Slab => (format!("{}_{}", base_name, "slab"), format!("{} {}", base_display_name, "Slab")),
            BlockShape::Stair => (format!("{}_{}", base_name, "stair"), format!("{} {}", base_display_name, "Stair")),
            BlockShape::Slope => (format!("{}_{}", base_name, "slope"), format!("{} {}", base_display_name, "Slope")),
            _ => ("test".to_string(), "Test".to_string())
        };

        let definition = BlockDefinition {
            name,
            display_name,
            shape,
            ..default()
        };
        registry.register_block(definition);
    }

    registry.register_block(
        BlockDefinition {
            name: "limestone".to_string(),
            display_name: "Limestone".to_string(),
            appearance: BlockAppearance::Uniform(FaceTextures::Simple(3, 0)),
            ..default()
        }
    );

    registry.register_block(
        BlockDefinition {
            name: "slate_with_grass".to_string(),
            display_name: "Grassy Slate".to_string(),
            appearance: BlockAppearance::TopBottomSides {
                up: FaceTextures::Tinted(1, 1, Color::from(bevy::color::palettes::basic::GREEN)),
                down: FaceTextures::Simple(1, 0),
                side: FaceTextures::Simple(1, 0),
            },
            ..default()
        }
    );

    bevy::log::info_once!("BlockRegistry successfully initialized.");
}