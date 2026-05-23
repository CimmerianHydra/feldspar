use bevy::prelude::*;
use bevy::platform::collections::hash_map::HashMap;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TEXTURE REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Stable name → layer-index lookup for the two texture arrays.
/// Layer 0 of base is `missing`, layer 0 of overlay is `none`.
#[derive(Resource, Default)]
pub struct TextureRegistry {
    pub base:    HashMap<String, u32>,
    pub overlay: HashMap<String, u32>,
}

impl TextureRegistry {
    pub const MISSING_LAYER:    u32 = 0;
    pub const NO_OVERLAY_LAYER: u32 = 0;

    /// Returns the layer or warns + returns MISSING_LAYER.
    pub fn base_layer(&self, name: &str) -> u32 {
        match self.base.get(name) {
            Some(&l) => l,
            None => {
                bevy::log::warn!("Unknown base texture '{}' — falling back to 'missing'", name);
                Self::MISSING_LAYER
            }
        }
    }

    pub fn overlay_layer(&self, name: &str) -> u32 {
        match self.overlay.get(name) {
            Some(&l) => l,
            None => {
                bevy::log::warn!("Unknown overlay texture '{}' — falling back to 'none'", name);
                Self::NO_OVERLAY_LAYER
            }
        }
    }
}