// src/plugin/audio/loader.rs  (or wherever you like)

use bevy::prelude::*;
use bevy::asset::LoadState;

/// Tracks audio handles requested during the Loading state.
/// Populated as blocks/materials are registered.
#[derive(Resource, Default)]
pub struct AudioAssetLoader {
    pub handles: Vec<Handle<AudioSource>>,
}

impl AudioAssetLoader {
    /// Load a sound and record its handle for readiness tracking.
    pub fn load(&mut self, path: &'static str, asset_server: &AssetServer) -> Handle<AudioSource> {
        let handle = asset_server.load(path);
        self.handles.push(handle.clone());
        handle
    }

    /// True once every tracked sound has finished loading.
    pub fn all_loaded(&self, asset_server: &AssetServer) -> bool {
        self.handles.iter().all(|h| {
            matches!(asset_server.load_state(h), LoadState::Loaded)
        })
    }
}