use bevy::prelude::*;
use std::collections::HashMap;

use crate::plugin::block_registry::{BlockID, BlockRegistry, initialize_registry_sys};
use crate::plugin::inventory::item_display::ItemDisplay;
use crate::plugin::inventory::main::MAX_STACK;
use crate::plugin::state::GameUpdateState;
use crate::plugin::voxel::BlockShape;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct ItemRegistryPlugin;

impl Plugin for ItemRegistryPlugin {
    fn build(&self, app: &mut App) {
        app
            // Resources
            .insert_resource(ItemRegistry::new())

            // Startup Systems
            .add_systems(Startup, initialize_item_registry_sys.after(initialize_registry_sys))
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ITEM DEFINITIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ItemID(pub u16);

/// What kind of thing this item is.
#[derive(Clone, Debug)]
pub enum ItemKind {
    /// Can be placed into the world as a block.
    /// The block_id field is the ID of the block that will be created.
    Block { block_id: BlockID },
    /// Pure resource — ore, wire, circuit board, etc.
    Resource,
    /// A tool with optional durability cap.
    Tool { max_durability: Option<u32> },
}

impl Default for ItemKind {
    /// The default kind of item is just a resource.
    fn default() -> Self {
        ItemKind::Resource
    }
}

pub struct ItemDefinition {
    pub id:           ItemID,
    pub name:         String,
    pub display_name: String,
    pub max_stack:    u16,       // e.g. 99 for ore, 1 for unique tools
    pub kind:         ItemKind,
    pub display:      ItemDisplay,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ITEM REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Mirror of BlockRegistry — same pattern.
#[derive(Resource)]
pub struct ItemRegistry {
    items: Vec<ItemDefinition>,
    /// Fast reverse lookup: BlockID → the item that places it
    block_to_item: HashMap<BlockID, ItemID>,
}

impl ItemRegistry {
    pub fn new() -> Self {
        Self { 
            items: Vec::new(),
            block_to_item: HashMap::new(),
        }
    }

    pub fn get(&self, id: ItemID) -> &ItemDefinition {
        &self.items[id.0 as usize]
    }

    pub fn block_to_item(&self, block: BlockID) -> Option<ItemID> {
        self.block_to_item.get(&block).copied()
    }

    pub fn register(&mut self, def: ItemDefinition) -> ItemID {
        let id = ItemID(self.items.len() as u16);

        // If this item places a block, record the reverse link
        if let ItemKind::Block { block_id } = def.kind {
            self.block_to_item.insert(block_id, id);
        }

        self.items.push(ItemDefinition { id, ..def });
        id
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – Example Systems
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Hardcoded block registry initialization.
/// In the future we need to have helpers that take this information from JSON files
/// and use it to build the registry for the actual game, as well as building any custom
/// object that only exists in a world in the registry (such as custom tools).

pub fn initialize_item_registry_sys(
    block_registry: Res<BlockRegistry>,
    mut item_registry: ResMut<ItemRegistry>,
    _game_state: Res<State<GameUpdateState>>,
    mut next_game_state: ResMut<NextState<GameUpdateState>>,
    asset_server: Res<AssetServer>,
) {
    // First we register all the blocks as items.
    // In the future we'll do this by looking through JSON files.
    for id in 0..block_registry.size() {
        let block = block_registry.get(BlockID(id as u16));
        item_registry.register(
            ItemDefinition {
                id: ItemID(0 as u16),
                name: block.name.clone(),
                display_name: block.display_name.clone(),
                max_stack: MAX_STACK,
                kind: ItemKind::Block { block_id: BlockID(id as u16) },
                display: ItemDisplay::Simple{image: asset_server.load(
                    match block.shape {
                        BlockShape::Cube => "icons\\items\\cube.png",
                        BlockShape::Slab => "icons\\items\\hslab.png",
                        BlockShape::Stair => "icons\\items\\stair.png",
                        BlockShape::Slope => "icons\\items\\slope.png",
                        _ => "icons\\items\\cube.png",
                    }
                )},
            }
        );
    }

    bevy::log::info_once!("ItemRegistry successfully initialized.");


    // After we're done, we're free to play the game
    // We need to change this into an event and create a "loading checklist" in the future
    next_game_state.set(GameUpdateState::Running);
}