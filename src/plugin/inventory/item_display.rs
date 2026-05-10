use bevy::prelude::*;
use crate::plugin::inventory::{item_registry::ItemRegistry, main::*};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


#[derive(PartialEq, Reflect)]
pub enum ItemDisplay {
    /// A single static image loaded from the asset folder.
    ///
    /// Example:
    /// ```rust
    /// ItemDisplay::Static {
    ///     image: asset_server.load("items/iron_ore.png"),
    /// }
    /// ```
    Simple {
        image: Handle<Image>,
    },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ASSET LOADING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

use bevy::asset::LoadState;

/// Holds all loaded image handles for item displays.
/// Populated during the Loading state; safe to clone handles freely after that.
#[derive(Resource, Default)]
pub struct ItemAssetLoader {
    pub handles: Vec<Handle<Image>>,
}

impl ItemAssetLoader {
    /// Convenience: load a single image and store the handle, returning it.
    pub fn load(&mut self, path: &'static str, asset_server: &AssetServer) -> Handle<Image> {
        let handle = asset_server.load(path);
        self.handles.push(handle.clone());
        handle
    }

    /// Returns true once every tracked image has finished loading.
    pub fn all_loaded(&self, asset_server: &AssetServer) -> bool {
        self.handles.iter().all(|h| {
            matches!(asset_server.load_state(h), LoadState::Loaded)
        })
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// UI ICON SPAWNER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const ITEM_ICON_SIZE: Val = Val::Px(64.0);

/// Spawn the visual representation of an item as a child of `parent`.
///
/// `composite_materials` is only needed when the display is `Composite`;
/// you can obtain it via `ResMut<Assets<CompositeItemMaterial>>` in a system.
pub fn build_ui_item_display(
    display:            &ItemDisplay,
    count:              u16,
) -> impl Bundle {
    let icon_node = Node {
        width:  ITEM_ICON_SIZE,
        height: ITEM_ICON_SIZE,
        // Keep the image centred within the slot.
        align_self:   AlignSelf::Center,
        justify_self: JustifySelf::Center,
        ..default()
    };

    match display {
        // ── Static ───────────────────────────────────────────────────────
        ItemDisplay::Simple { image } => { return (
                icon_node,
                ImageNode {
                    image: image.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            )
        }
    }
}