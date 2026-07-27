//! # Feldspar
//!
//! A voxel game where the world is made of *spaces*: a dimension and a
//! flying ship are the same kind of thing, differing only in whether their
//! rigid body moves.
//!
//! ## The layering rule
//!
//! Folders are **features**, not kinds. There is no `components/`,
//! `systems/` or `resources/` — a module owns everything about one part of
//! the game, including its own `Plugin`.
//!
//! Features form a **strict dependency order**, and here it is. A module may
//! import from anything *below* it in this list, and from nothing above:
//!
//! | # | module | what it owns |
//! |---|--------|--------------|
//! | 1 | [`voxel`]    | vocabulary: the packed voxel, directions, rotations, shapes, ids, chunk math |
//! | 2 | [`command`]  | the command system: registry, parser, dispatcher, scrollback |
//! | 3 | [`space`]    | the authoritative "what is where": chunks, spaces, the write path |
//! | 4 | [`content`]  | definitions and registries, and the JSON that fills them |
//! | 5 | [`sim`], [`worldgen`] | gameplay: block entities, inventories, crafting, terrain |
//! | 6 | [`render`], [`audio`], [`physics`] | presentation and derived representations |
//! | 7 | [`ui`]       | screens, widgets, the screen stack |
//! | 8 | [`player`]   | input bindings, camera, movement, place/break intent |
//! | 9 | [`console`]  | the on-screen console panel |
//! |10 | [`app`]      | states, system sets, load order — the wiring |
//! |11 | [`dev`]      | dev-only fixtures, behind the `dev` feature |
//!
//! When you catch yourself wanting to import upward, that is the signal to
//! invert with an event or a marker component instead. Every such inversion
//! in this codebase is commented where it happens; the three biggest are:
//!
//! - **`space` does not know it is drawn.** A voxel write touches
//!   `VoxelChunk` through `DerefMut`; `render` and `physics` each query
//!   `Changed<VoxelChunk>` for themselves. There is no `NeedsRemeshing`.
//! - **Presentation observes simulation.** A barrel declares `Inventory +
//!   Interactable`; `ui::inventory` observes `BlockEntityEvent` filtered on
//!   `With<Inventory>` and opens a screen. The barrel imports no UI at all,
//!   and the next twenty machines get their screen for free.
//! - **Cross-feature ordering goes through named sets, not imports.**
//!   `BakeSet::{Mesh, Collide}`, `GameplaySet::{Input, Simulate, Present}`
//!   and `ConsoleSet::{Input, Dispatch, Present}` are declared and ordered
//!   once in [`app::schedule`]; no plugin names another plugin's system.
//!
//! Two modules are exempt from the rule, and say so at the top of their own
//! files: [`app::state`] and [`app::schedule`] are leaves with no
//! crate-internal imports, re-exported at the crate root so that call sites
//! read `use crate::GameState;` rather than reaching upward into `app`.



#[cfg(feature = "dev")]
pub mod dev;

pub mod app;
pub mod audio;
pub mod command;
pub mod console;
pub mod content;
pub mod physics;
pub mod player;
pub mod render;
pub mod sim;
pub mod space;
pub mod ui;
pub mod voxel;
pub mod worldgen;
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CRATE VOCABULARY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub use app::{BakeSet, ConsoleSet, GameMode, GameState, GameUpdate, GameplaySet};

/// What most files want in scope. Import as `use feldspar::prelude::*;`
/// (or `crate::prelude::*` from inside the crate).
pub mod prelude {
    pub use crate::space::{BlockPos, VoxelChunk, VoxelWorld, VoxelWriteRequest};
    pub use crate::voxel::{
        BlockID, BlockRotation, BlockShape, Direction, ItemID, Voxel, CHUNK_SIZE,
    };
    pub use crate::{BakeSet, ConsoleSet, GameState, GameUpdate, GameplaySet};
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE GAME
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

use bevy::prelude::*;

/// Every layer, in order. Adding a feature is one line here and one folder.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            // The wiring goes in first, even though it is the top layer:
            // states and system sets have to exist before the features that
            // register into them.
            app::AppPlugin,
            // 1-2: vocabulary has no plugin; the command system does.
            command::CommandPlugin,
            // 3-4
            space::SpacePlugin,
            content::ContentPlugin,
            // 5
            sim::SimPlugin,
            // 6
            render::RenderPlugin,
            physics::ChunkPhysicsPlugin,
            audio::BlockAudioPlugin,
            // 7-9
            ui::UiPlugin,
            player::PlayerPlugin,
            console::ConsolePlugin,
        ));
    }
}
