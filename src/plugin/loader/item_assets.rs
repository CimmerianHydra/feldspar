use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::Deserialize;

use crate::plugin::inventory::main::MAX_STACK;
use crate::plugin::item::components::ItemComponents;
use crate::plugin::loader::block_registry::BlockRegistry;
use crate::plugin::loader::item_registry::{ItemDefinition, ItemID, ItemKind, ItemRegistry};
use crate::plugin::ui::item::{DisplayLayer, ItemDisplay};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// JSON SCHEMA
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Mirrors the existing block_assets.rs pattern: a serde asset type parsed by
// bevy_common_assets' JsonAssetPlugin, gathered by a bevy_asset_loader
// collection during GameState::AssetLoading, then resolved into the live
// ItemRegistry by a system that runs after blocks have been registered.
//
// File extension is "*.item.json" so it never collides with the block files
// that already claim the generic "json" extension.
//
// Example — assets/definitions/items/base.item.json:
// [
//   {
//     "name": "circuit_basic",
//     "display_name": "Basic Circuit",
//     "max_stack": 99,
//     "kind": "resource",
//     "display": "icons/items/circuit_basic.png"
//   },
//   {
//     "name": "flint_hatchet",
//     "display_name": "Flint Hatchet",
//     "max_stack": 1,
//     "kind": { "tool": { "max_durability": 60 } },
//     "display": [
//       { "sprite": "icons/items/hatchet_head.png", "tint": "#9AA0A6" },
//       { "sprite": "icons/items/hatchet_handle.png" }
//     ]
//   }
// ]

/// One `*.item.json` file: a list of item definitions.
#[derive(Deserialize, Asset, TypePath)]
pub struct ItemDefinitionAsset(pub Vec<ItemDefJson>);

#[derive(Deserialize)]
pub struct ItemDefJson {
    pub name:         String,
    pub display_name: String,
    #[serde(default = "default_max_stack")]
    pub max_stack:    u16,
    #[serde(default)]
    pub kind:         ItemKindJson,
    pub display:      DisplayJson,
}

fn default_max_stack() -> u16 { MAX_STACK }

/// JSON-side mirror of ItemKind. Block references are BY NAME and resolved
/// against the BlockRegistry at load time, so item files never contain raw
/// numeric IDs — IDs are a runtime detail, names are the stable currency.
#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ItemKindJson {
    #[default]
    Resource,
    Block(String),
    Tool { max_durability: Option<u32> },
}

/// Either a plain sprite path (the common case) or a list of tinted layers.
/// The untagged enum lets JSON authors write the simple thing simply:
///   "display": "icons/items/x.png"
///   "display": [ { "sprite": "...", "tint": "#RRGGBB" }, ... ]
#[derive(Deserialize)]
#[serde(untagged)]
pub enum DisplayJson {
    Single(String),
    Layered(Vec<LayerJson>),
}

#[derive(Deserialize)]
pub struct LayerJson {
    pub sprite: String,
    /// Hex color like "#D8D8D8" or "D8D8D8". Missing → white (no tint).
    #[serde(default)]
    pub tint:   Option<String>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// RESOLUTION HELPERS  (shared with the substance generator)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Parse "#RRGGBB[AA]" into a Color, falling back to white with a warning
/// rather than aborting the whole load over a typo.
pub fn parse_hex_color(s: &str) -> Color {
    match Srgba::hex(s.trim_start_matches('#')) {
        Ok(srgba) => Color::from(srgba),
        Err(_) => {
            warn!("Invalid hex color '{s}', defaulting to white.");
            Color::WHITE
        }
    }
}

pub fn resolve_display(json: &DisplayJson, srv: &AssetServer) -> ItemDisplay {
    match json {
        DisplayJson::Single(path) => ItemDisplay::Image {
            image: srv.load(path.clone()),
        },
        DisplayJson::Layered(layers) => ItemDisplay::Layered {
            layers: layers.iter().map(|l| DisplayLayer {
                image: srv.load(l.sprite.clone()),
                tint:  l.tint.as_deref().map(parse_hex_color).unwrap_or(Color::WHITE),
            }).collect(),
        },
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ASSET COLLECTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Every *.item.json under assets/templates/items, loaded and parsed
/// before GameState::AssetLoading completes.
#[derive(AssetCollection, Resource)]
pub struct ItemDefinitionAssets {
    #[asset(path = "templates/items", collection(typed))]
    pub files: Vec<Handle<ItemDefinitionAsset>>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// LOADING SYSTEM
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Resolve every parsed item file into the ItemRegistry. Runs AFTER
/// populate_item_registry_sys (blocks) and generate_substance_items_sys, so
/// hand-written names can be checked against everything already registered.
pub fn load_item_definitions_sys(
    assets:         Res<ItemDefinitionAssets>,
    parsed:         Res<Assets<ItemDefinitionAsset>>,
    block_registry: Res<BlockRegistry>,
    mut item_registry: ResMut<ItemRegistry>,
    asset_server:   Res<AssetServer>,
) {
    let mut loaded = 0usize;

    for handle in &assets.files {
        // The loading state guarantees these are parsed; a miss here means
        // a scheduling bug, so log it loudly instead of silently skipping.
        let Some(file) = parsed.get(handle) else {
            error!("Item definition asset not parsed before resolution — check loading state.");
            continue;
        };

        for def in &file.0 {
            if item_registry.by_name(def.name.clone()).is_some() {
                warn!("Duplicate item '{}' — keeping the first, skipping this one.", def.name);
                continue;
            }

            let kind = match &def.kind {
                ItemKindJson::Resource => ItemKind::Resource,
                ItemKindJson::Tool { max_durability } =>
                    ItemKind::Tool { max_durability: *max_durability },
                ItemKindJson::Block(block_name) => {
                    match block_registry.by_name(block_name.clone()) {
                        Some(block_id) => ItemKind::Block { block_id },
                        None => {
                            warn!("Item '{}' references unknown block '{}' — skipped.",
                                  def.name, block_name);
                            continue;
                        }
                    }
                }
            };

            item_registry.register(ItemDefinition {
                name:         def.name.clone(),
                display_name: def.display_name.clone(),
                max_stack:    def.max_stack,
                kind,
                display:      resolve_display(&def.display, &asset_server),
                components: ItemComponents::default(), // WIP: need to parse item components from JSON
            });
            loaded += 1;
        }
    }

    info!("ItemRegistry: {} explicit items loaded from JSON ({} total).",
          loaded, item_registry.len());
}