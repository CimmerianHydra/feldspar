//! Sequencing the load.
//!
//! Asset collections and JSON parsers are declared by the feature that owns
//! them ([`crate::content`]); progress reporting and the splash screen belong
//! to [`crate::ui::splash`]. What lives here is the one thing neither of them
//! can own: **the order**.
//!
//! The chain below spans `content`, `render` and `worldgen`, and every step
//! depends on the previous one having finished. Expressing that with
//! `.after(...)` inside each feature would force those features to import one
//! another for scheduling reasons alone. `app` is the wiring layer, so the
//! sequence lives here and each feature stays ignorant of its neighbours.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::math::ops::ceil;
use bevy::prelude::*;
use iyes_progress::{Progress, ProgressPlugin, ProgressReturningSystem, ProgressTracker};

use crate::content::block::asset::populate_block_registry_sys;
use crate::content::item::asset::load_item_definitions_sys;
use crate::content::model_source::populate_model_source_registry_sys;
use crate::content::recipe::asset::populate_spatial_recipe_registry_sys;
use crate::content::shape_set::populate_shape_set_registry_sys;
use crate::content::substance::{generate_substance_items_sys, populate_substance_registries_sys};
use crate::content::texture::assemble_texture_arrays_sys;
use crate::render::icons::register_block_items_sys;
use crate::render::material::build_voxel_materials_sys;
use crate::render::mesh::bake::bake_block_geometry_sys;
use crate::worldgen::init_active_worldgen_sys;
use crate::GameState;

pub const LONG_FAKE_TASK_DURATION: f32 = 1.0;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            // Track progress during `AssetLoading` and continue to `InGame`
            // when everything reports complete.
            ProgressPlugin::<GameState>::new()
                .with_state_transition(GameState::AssetLoading, GameState::InGame),
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .add_systems(
            Update,
            (track_fake_long_task.track_progress::<GameState>(), print_progress)
                .chain()
                .run_if(in_state(GameState::AssetLoading))
                .after(bevy_asset_loader::loading_state::LoadingStateSet(
                    GameState::AssetLoading,
                )),
        )
        // ── THE ORDER ────────────────────────────────────────────────────
        // Each line needs everything above it to have run.
        .add_systems(
            OnExit(GameState::AssetLoading),
            (
                // Image layers → named layer indices.
                assemble_texture_arrays_sys,
                // ...which become one PBR material.
                build_voxel_materials_sys,
                // Shape sets have to exist before blocks can reference them.
                populate_shape_set_registry_sys,
                // After shapes, we build information from unique models.
                populate_model_source_registry_sys,
                // Blocks: identity, appearance, texture slots, behaviours.
                populate_block_registry_sys,
                // Worldgen resolves block names, so it needs the registry.
                init_active_worldgen_sys,
                // Geometry: fills every definition's ModelTable.
                bake_block_geometry_sys,
                // One placeable item per block, with a rendered icon —
                // hence the arena above.
                register_block_items_sys,
                // Hand-written items may reference blocks by name.
                load_item_definitions_sys,
                // Substances and their part profiles...
                populate_substance_registries_sys,
                // ...crossed into one item per (substance, part).
                generate_substance_items_sys,
                // Recipes resolve item names, so they go last.
                populate_spatial_recipe_registry_sys,
            )
                .chain(),
        );
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PROGRESS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Holds the splash screen up for a beat so the logo is readable even when
/// the assets are already warm.
fn track_fake_long_task(time: Res<Time>) -> Progress {
    Progress {
        done: ceil((time.elapsed_secs() / LONG_FAKE_TASK_DURATION) * 100.0).min(100.) as u32,
        total: 100,
    }
}

fn print_progress(
    progress: Res<ProgressTracker<GameState>>,
    diagnostics: Res<DiagnosticsStore>,
    mut last_done: Local<u32>,
) {
    let progress = progress.get_global_progress();
    if progress.done > *last_done {
        *last_done = progress.done;
        debug!(
            "[Frame {}] Asset Loading Progress: {:2}%",
            diagnostics
                .get(&FrameTimeDiagnosticsPlugin::FRAME_COUNT)
                .map(|diagnostic| diagnostic.value().unwrap_or(0.))
                .unwrap_or(0.),
            (progress.done as f32 / progress.total as f32)
        );
    }
}
