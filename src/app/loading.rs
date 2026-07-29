//! Sequencing the load.
//!
//! Asset collections and JSON parsers are declared by the feature that owns
//! them ([`crate::content`]); the splash screen belongs to
//! [`crate::ui::splash`]. What lives here is the one thing neither of them
//! can own: **the order** — and, now, the words that describe it.
//!
//! ## Why the order is data
//!
//! It used to be a `.chain()` in `OnExit(AssetLoading)`. That schedule runs
//! *after* the transition has been decided, in a single frame, in the same
//! transition that despawns the splash — so none of it could be reported,
//! and all of it was one hitch.
//!
//! The same twelve systems are now a `Vec` of steps, each with a caption
//! and a weight, driven **one per frame** inside the loading state. The
//! renderer gets a frame between steps, which is the whole point: that is
//! what makes a progress bar possible at all.
//!
//! Each step still knows nothing about its neighbours. The list below is
//! the only place the sequence exists.

use std::time::Duration;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bevy_asset_loader::loading_state::LoadingStateSet;
use iyes_progress::{
    HiddenProgress, Progress, ProgressPlugin, ProgressReturningSystem, ProgressTracker,
};

use crate::app::progress::LoadReport;
use crate::app::schedule::LoadSet;
use crate::content::block::asset::populate_block_registry_sys;
use crate::content::item::asset::load_item_definitions_sys;
use crate::content::model_source::populate_model_source_registry_sys;
use crate::content::recipe::asset::populate_spatial_recipe_registry_sys;
use crate::content::shape_set::populate_shape_set_registry_sys;
use crate::content::substance::{generate_substance_items_sys, populate_substance_registries_sys};
use crate::content::texture::assemble_texture_arrays_sys;
use crate::content::ContentFilesReady;
use crate::render::icons::{await_block_icons_sys, register_block_items_sys};
use crate::render::material::build_voxel_materials_sys;
use crate::render::mesh::bake::bake_block_geometry_sys;
use crate::worldgen::init_active_worldgen_sys;
use crate::GameState;

/// Minimum time the splash stays up, so the logo is readable even when the
/// files are warm in the OS cache. Reported as `HiddenProgress`: it holds
/// the transition without ever appearing in the bar. Faking the *wait* is
/// fine; faking the *bar* is what we're getting rid of.
const MINIMUM_SPLASH_TIME: Duration = Duration::from_secs(1);

const COMPLETE: Progress = Progress { done: 1, total: 1 };

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE ORDER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Weights are relative costs in arbitrary units — only their ratios
/// matter. They exist because twelve equal steps make the bar lurch:
/// eleven registry passes that take microseconds, then geometry baking and
/// icon rendering eating the rest. Tune them by watching the log line from
/// `print_load_progress_sys`.
fn build_load_sequence(world: &mut World) -> LoadSequence {
    let mut seq = SequenceBuilder::new(world);

    seq // Image layers → named layer indices.
        .step("Assembling texture arrays", 60, assemble_texture_arrays_sys)
        // ...which become one PBR material.
        .step("Building voxel material", 5, build_voxel_materials_sys)
        // Shape sets have to exist before blocks can reference them.
        .step("Reading shape sets", 5, populate_shape_set_registry_sys)
        // Custom model shapes from blockbench.
        .step("Registering custom shapes", 5, populate_model_source_registry_sys)
        // Blocks: identity, appearance, texture slots, behaviours.
        .step("Registering blocks", 20, populate_block_registry_sys)
        // Worldgen resolves block names, so it needs the registry.
        .step("Preparing world generation", 5, init_active_worldgen_sys)
        // Geometry: fills every definition's ModelTable.
        .step("Baking block geometry", 150, bake_block_geometry_sys)
        // One placeable item per block; queues an icon render for each.
        .step("Creating block items", 40, register_block_items_sys)
        // Hand-written items may reference blocks by name.
        .step("Loading item definitions", 10, load_item_definitions_sys)
        // Substances and their part profiles...
        .step("Reading substances", 10, populate_substance_registries_sys)
        // ...crossed into one item per (substance, part).
        .step("Generating substance items", 20, generate_substance_items_sys)
        // Recipes resolve item names, so they go last.
        .step("Resolving recipes", 10, populate_spatial_recipe_registry_sys)
        // Last, so the registry passes above run while the studio is
        // already drawing. Spans many frames and reports its own progress.
        .tracked_step("Rendering block icons", 200, await_block_icons_sys);

    seq.finish()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE SEQUENCE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A step that finishes in one call, or one that spans frames and reports
/// how far along it is. The driver only advances past the latter when it
/// says `done == total`.
#[derive(Clone, Copy)]
enum StepKind {
    Once(SystemId<(), ()>),
    Tracked(SystemId<(), Progress>),
}

struct LoadStep {
    label: &'static str,
    weight: u32,
    kind: StepKind,
}

#[derive(Resource)]
struct LoadSequence {
    steps: Vec<LoadStep>,
    /// The step being announced or executed.
    cursor: usize,
    /// Is `cursor`'s caption already on screen? Steps announce themselves
    /// one frame before they run, so a step that blocks for half a second
    /// does it under its own name rather than the previous step's.
    announced: bool,
    /// Weight banked from finished steps.
    done_weight: u32,
    /// Sum of every weight. Fixed at build time, so the bar's denominator
    /// never moves — a growing total makes the ratio visibly fall back.
    total_weight: u32,
    /// Partial progress inside the running step, if it reports any.
    step_done: u32,
    step_total: u32,
}

impl LoadSequence {
    /// Weight earned so far inside the running step.
    fn partial_weight(&self) -> u32 {
        let Some(step) = self.steps.get(self.cursor) else {
            return 0;
        };
        if self.step_total == 0 {
            return 0;
        }
        (u64::from(step.weight) * u64::from(self.step_done) / u64::from(self.step_total)) as u32
    }
}

struct SequenceBuilder<'w> {
    world: &'w mut World,
    steps: Vec<LoadStep>,
}

impl<'w> SequenceBuilder<'w> {
    fn new(world: &'w mut World) -> Self {
        Self { world, steps: Vec::new() }
    }

    fn step<M: 'static>(
        &mut self,
        label: &'static str,
        weight: u32,
        system: impl IntoSystem<(), (), M> + 'static,
    ) -> &mut Self {
        let kind = StepKind::Once(self.world.register_system(system));
        self.steps.push(LoadStep { label, weight, kind });
        self
    }

    fn tracked_step<M: 'static>(
        &mut self,
        label: &'static str,
        weight: u32,
        system: impl IntoSystem<(), Progress, M> + 'static,
    ) -> &mut Self {
        let kind = StepKind::Tracked(self.world.register_system(system));
        self.steps.push(LoadStep { label, weight, kind });
        self
    }

    fn finish(self) -> LoadSequence {
        let total_weight = self.steps.iter().map(|s| s.weight).sum();
        LoadSequence {
            steps: self.steps,
            cursor: 0,
            announced: false,
            done_weight: 0,
            total_weight,
            step_done: 0,
            step_total: 0,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE DRIVER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One step per frame, in two beats: announce, then work.
///
/// Exclusive because it has to read a step's return value, which `Commands`
/// discards. `World::run_system` applies the step's deferred commands
/// before returning, so the next step sees a settled world — the same
/// guarantee `.chain()` used to give via its automatic sync points.
fn drive_load_sequence_sys(world: &mut World) {
    let Some((cursor, steps, label, weight, kind, announced)) =
        world.get_resource::<LoadSequence>().and_then(|seq| {
            let step = seq.steps.get(seq.cursor)?;
            Some((seq.cursor, seq.steps.len(), step.label, step.weight, step.kind, seq.announced))
        })
    else {
        return; // sequence exhausted; the reporter now reads complete
    };

    // BEAT ONE — announce. The caption reaches the screen at the end of
    // this frame, before the work blocks the next one.
    if !announced {
        world.resource_mut::<LoadSequence>().announced = true;
        world
            .resource_mut::<LoadReport>()
            .set_if_neq(LoadReport { label, step: cursor, steps });
        return;
    }

    // BEAT TWO — work.
    let progress = match kind {
        StepKind::Once(id) => match world.run_system(id) {
            Ok(()) => COMPLETE,
            Err(err) => {
                error!("Load step '{label}' failed: {err:?} — skipping it.");
                COMPLETE
            }
        },
        StepKind::Tracked(id) => match world.run_system(id) {
            Ok(progress) => progress,
            Err(err) => {
                error!("Load step '{label}' failed: {err:?} — skipping it.");
                COMPLETE
            }
        },
    };

    let mut seq = world.resource_mut::<LoadSequence>();
    if progress.done >= progress.total {
        seq.done_weight += weight;
        seq.cursor += 1;
        seq.announced = false;
        seq.step_done = 0;
        seq.step_total = 0;
    } else {
        seq.step_done = progress.done;
        seq.step_total = progress.total;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PROGRESS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The sequence's contribution to the global bar. Separate from the driver
/// so the driver can stay exclusive and this can stay an ordinary tracked
/// system.
fn report_load_sequence_sys(seq: Res<LoadSequence>) -> Progress {
    Progress {
        done: seq.done_weight + seq.partial_weight(),
        total: seq.total_weight,
    }
}

/// Holds the transition open so the logo is readable. `HiddenProgress`
/// counts toward the transition but never toward the bar.
fn hold_splash_screen_sys(time: Res<Time>, mut elapsed: Local<Duration>) -> HiddenProgress {
    *elapsed += time.delta();
    (*elapsed >= MINIMUM_SPLASH_TIME).into()
}

fn print_load_progress_sys(
    tracker: Res<ProgressTracker<GameState>>,
    report: Res<LoadReport>,
    diagnostics: Res<DiagnosticsStore>,
    mut last_done: Local<u32>,
) {
    let progress = tracker.get_global_progress();
    if progress.done <= *last_done {
        return;
    }
    *last_done = progress.done;

    let frame = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_COUNT)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    let ratio = if progress.total > 0 {
        progress.done as f32 / progress.total as f32
    } else {
        0.0
    };

    debug!("[Frame {frame}] {:>3.0}% — {}", ratio * 100.0, report.label);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        // Built here, at plugin time, rather than when the files finish:
        // that fixes the bar's denominator from the very first frame.
        let sequence = build_load_sequence(app.world_mut());

        app.add_plugins((
            ProgressPlugin::<GameState>::new()
                .with_state_transition(GameState::AssetLoading, GameState::InGame),
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .insert_resource(sequence)
        .init_resource::<LoadReport>()
        // The collections are inserted inside the loader's private
        // schedule, so everything here has to follow it.
        .configure_sets(
            Update,
            LoadSet::Advance.after(LoadingStateSet(GameState::AssetLoading)),
        )
        .add_systems(
            Update,
            (
                // Gated: no step may run before its collection exists.
                drive_load_sequence_sys.run_if(resource_exists::<ContentFilesReady>),
                report_load_sequence_sys.track_progress::<GameState>(),
                hold_splash_screen_sys.track_progress_and_stop::<GameState>(),
                print_load_progress_sys,
            )
                .chain()
                .in_set(LoadSet::Advance),
        );
    }
}