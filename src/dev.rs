//! Development fixtures.
//!
//! Everything in here exists to give you a world to stand in before there is
//! a real one: a slab of pre-generated chunks, a test ship on F4, a `/give`
//! on F5. It is behind the `dev` feature (on by default) so that a headless
//! or shipping build drops it without touching anything else.
//!
//! It sits at the very top of the layer stack and may name anything.

pub mod freecamera;

use avian3d::dynamics::rigid_body::mass_properties::components::ColliderDensity;
use avian3d::prelude::*;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;

use crate::command::{CommandSource, PendingCommands};
use crate::content::block::BlockRegistry;
use crate::physics::CHUNK_COLLIDER_DENSITY;
use crate::sim::player::Player;
use crate::space::{
    build_moving_grid, BlockPos, ChunkSlot, DimensionRegistry, VoxelChunk, VoxelWriteRequest,
};
use crate::voxel::{BlockID, Voxel};
use crate::worldgen::{ActiveWorldGenerator, WorldGenerator};
use crate::GameState;

/// How many chunks out from origin to pre-spawn on each axis.
/// Total chunk count = (2*R + 1)² * (2*H + 1).
/// Drop these while testing if startup feels heavy.
const DEV_CHUNK_RADIUS: i32 = 4;
const DEV_CHUNK_HEIGHT: i32 = 4;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – TERRAIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn setup_dev_chunks(
    mut commands: Commands,
    worldgen:     Res<ActiveWorldGenerator>,
    dimensions:   Res<DimensionRegistry>,
) {
    for cx in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
        for cy in -DEV_CHUNK_HEIGHT..=DEV_CHUNK_HEIGHT {
            for cz in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
                let chunk_pos = IVec3::new(cx, cy, cz);

                let mut chunk_data = VoxelChunk::empty();
                worldgen.generate_chunk(chunk_pos, &mut chunk_data);

                debug!("Generating static chunk at position ({cx}, {cy}, {cz})");

                // No dirty markers: spawning the chunk makes `VoxelChunk`
                // Added, which is `Changed`, which is what the mesher and
                // the collider builder are watching for.
                commands.spawn((
                    ChunkSlot { space: dimensions.overworld(), coord: chunk_pos },
                    chunk_data,
                    Name::new(format!("Overworld ({cx}, {cy}, {cz})")),
                    InheritedVisibility::default(),
                ));
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – A SHIP TO PUSH AROUND
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn spawn_dev_ship(mut commands: Commands, registry: Res<BlockRegistry>) {
    let Some(barrel_id) = registry.by_name("barrel".to_string()) else {
        error!("dev grid: no 'barrel' block in the registry — check barrel.json");
        return;
    };
    let hull_id = resolve_hull_block(&registry);

    // ---- the space owns the body ----------------------------------------
    let spawn_transform = Transform::from_translation(Vec3::new(0.0, 16.0, 0.0));

    let grid = commands
        .spawn((
            build_moving_grid(spawn_transform, "TestBarrelCrate"),
            Friction::new(0.5),
            Restitution::new(0.05),
        ))
        .id();

    // ---- the chunk supplies geometry ------------------------------------
    // An empty chunk in the ship's body, populated below by simulating a
    // place event for every block so block entities are spawned properly.
    commands.spawn((
        Name::new("TestBarrelCrate/chunk"),
        ChunkSlot { space: grid, coord: IVec3::ZERO },
        VoxelChunk::empty(),
        ColliderDensity(CHUNK_COLLIDER_DENSITY),
        Visibility::Visible,
    ));

    // ---- author the chunk contents --------------------------------------
    // `VoxelWriteRequest` is the authoritative path, so block entities get
    // spawned exactly as they would for a player-placed block. This step
    // fails if no chunk entity exists containing the requested coords.
    //
    // Fairly inefficient even for small grids, because each write dirties
    // the chunk and triggers a remesh + recollide. Bulk edits (explosions)
    // will want a batched path.
    for x in 0..2i32 {
        for y in 0..2i32 {
            for z in 0..2i32 {
                let coords = IVec3::new(x, y, z);
                // One barrel, so there's exactly one thing to right-click.
                let id = if coords == IVec3::new(1, 1, 1) { barrel_id } else { hull_id };
                commands.trigger(VoxelWriteRequest {
                    at: BlockPos { space: grid, pos: coords },
                    voxel: Voxel::full(id.0),
                });
            }
        }
    }

    info!("Dev Grid spawned at: {}", spawn_transform.translation);
}

/// Pick a solid block for the hull, tolerating an unknown name.
fn resolve_hull_block(registry: &BlockRegistry) -> BlockID {
    if let Some(id) = registry.by_name("slate".to_string()) {
        return id;
    }
    warn!("dev grid: block not found, falling back to the first registered block");
    // Index 0 is always air, so 1 is the first real block.
    BlockID(1.min(registry.size().saturating_sub(1) as u16))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – A COMMAND TO POKE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn dev_submit_command(mut pending: ResMut<PendingCommands>, player: Single<Entity, With<Player>>) {
    pending.push(CommandSource::Player(*player), "/give slate 64");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), setup_dev_chunks)
            .add_systems(
                Update,
                spawn_dev_ship.run_if(input_just_pressed(KeyCode::F4)),
            )
            .add_systems(
                Update,
                dev_submit_command.run_if(input_just_pressed(KeyCode::F5)),
            );
    }
}
