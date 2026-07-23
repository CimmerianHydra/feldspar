
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;

mod plugin;
use plugin::geometry::meshing::MeshingPlugin;
use plugin::block::interaction::BlockInteractionPlugin;
use plugin::ui::main::UIPlugin;
use plugin::weather::WeatherPlugin;
use plugin::state::StatePlugin;
use plugin::controller::main::ControlsPlugin;
use plugin::inventory::main::InventoryPlugin;
use plugin::graphics::block_material::VoxelMaterialPlugin;
use plugin::worldgen::main::WorldgenPlugin;
use plugin::controller::player::PlayerControllerPlugin;
use plugin::audio::block::BlockAudioPlugin;
use plugin::crafting::main::CraftingPlugin;
use plugin::loader::main::AssetLoaderPlugin;

use plugin::inventory::main::*;
use plugin::inventory::player::*;
use plugin::loader::item_registry::*;
use plugin::state::GameUpdate;

use avian3d::PhysicsPlugins;
use plugin::state::GameState;

use plugin::block::prelude::BlockPlugin;
use plugin::chunk::{NeedsRemeshing, VoxelChunk};
use plugin::loader::texture_assets::VoxelMaterialHandle;
use plugin::space::prelude::SpacePlugin;
use plugin::worldgen::main::{WorldGenerator, ActiveWorldGenerator};




fn main() {
    App::new()
        // Plugins
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(StatePlugin)
        //.add_plugins(FreeCameraPlugin)
        .add_plugins(PlayerControllerPlugin)
        .add_plugins(ControlsPlugin)
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(AssetLoaderPlugin)
        .add_plugins(SpacePlugin)
        .add_plugins(BlockPlugin)
        .add_plugins(MeshingPlugin)
        .add_plugins(UIPlugin)
        .add_plugins(InventoryPlugin)
        .add_plugins(BlockInteractionPlugin)
        .add_plugins(WorldgenPlugin)
        .add_plugins(WeatherPlugin)
        .add_plugins(BlockAudioPlugin)
        .add_plugins(CraftingPlugin)

        .add_systems(OnEnter(GameState::InGame), setup_dev_chunks)

        .add_systems(Update, enable_game.after(setup_dev_chunks))

        .add_systems(Update, dev_populate_player_inventory)

        .add_systems(Update, spawn_dev_ship.run_if(input_just_pressed(KeyCode::F4)))

        .run();
}

fn enable_game(
    mut commands: Commands,
) {
    commands.set_state(GameUpdate::Enabled);
}

/// Hardcoded function to spawn some items into the player's inventory.
/// Since I hardcoded a few blocks in the block registry, I'll add them here.
pub fn dev_populate_player_inventory(
    mut player_hotbar_query: Query<(Entity, &mut Inventory), Added<PlayerInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    if let Ok((entity, mut inventory)) = player_hotbar_query.single_mut() {
        for id in 1..7 {
            let item_id = ItemID(id as u16);
            let result = inventory.insert(item_id, 10, &item_registry);

            bevy::log::info!("Added [{}]x{} to player inventory.", item_registry.get(item_id).name, result.transferred);
        };

        for name in [
            "iron_metal_ingot", "iron_metal_plate", "iron_metal_gear", "iron_metal_rod",
            "copper_metal_ingot", "copper_metal_plate", "copper_metal_gear", "copper_metal_rod",
            ] {
            let Some(item_id) = item_registry.by_name(name.to_string()) else { continue ;};
            let result = inventory.insert(item_id, 10, &item_registry);

            bevy::log::info!("Added [{}]x{} to player inventory.", name.to_string(), result.transferred);
        }
    }
}

use crate::plugin::geometry::collision::NeedsColliderRebuild;
use crate::plugin::loader::block_registry::BlockID;
use crate::plugin::loader::block_registry::BlockRegistry;
use crate::plugin::space::prelude::DimensionRegistry;
use crate::plugin::space::prelude::ChunkSlot;
use crate::plugin::space::prelude::build_moving_grid;
use crate::plugin::voxel::Voxel;
use avian3d::dynamics::rigid_body::*;

/// How many chunks out from origin to pre-spawn on each axis.
/// Total chunk count = (2*R + 1)^3 — with R=8 that's 4913 chunks.
/// Drop this to 2 or 3 if startup feels heavy while testing.
const DEV_CHUNK_RADIUS: i32 = 4;
const DEV_CHUNK_HEIGHT: i32 = 4;

fn setup_dev_chunks(
    mut commands:                   Commands,
    terrain_material_handle_res:    Res<VoxelMaterialHandle>,
    worldgen:                       Res<ActiveWorldGenerator>,
    dimensions:                     Res<DimensionRegistry>,
    block_registry:                 Res<BlockRegistry>,
) {
    // ── chunk generation + spawn ──────────────────────────────────────────

    for cx in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
        for cy in -DEV_CHUNK_HEIGHT..=DEV_CHUNK_HEIGHT {
            for cz in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
                let chunk_pos = IVec3::new(cx, cy, cz);

                let mut chunk_data = VoxelChunk::empty();
                worldgen.generate_chunk(chunk_pos, &mut chunk_data);
                
                bevy::log::debug!("Generating static chunk at position ({}, {}, {})", cx, cy, cz);

                let chunk_slot = ChunkSlot { space: dimensions.overworld(), coord: chunk_pos };

                commands.spawn((
                    chunk_slot.clone(),
                    chunk_data.clone(),
                    MeshMaterial3d(terrain_material_handle_res.0.clone()),
                    NeedsRemeshing,
                ));
            }
        }
    }
}

fn spawn_dev_ship(
    mut commands:       Commands,
    registry:           Res<BlockRegistry>,
) {
    let Some(barrel_id) = registry.by_name("barrel".to_string()) else {
        error!("dev grid: no 'barrel' block in the registry — check barrel.json");
        return;
    };
    let hull_id = resolve_hull_block(&registry);
 
    // ---- author the chunk contents -------------------------------------
    let mut chunk = VoxelChunk::empty();
    for x in 0..2u32 {
        for y in 0..2u32 {
            for z in 0..2u32 {
                let local = UVec3::new(x, y, z);
                // One barrel, so there's exactly one thing to right-click.
                let id = if local == UVec3::new(1, 1, 1) { barrel_id } else { hull_id };
                chunk.set_local(local, Voxel::full(id.0));
            }
        }
    }
 
    // ---- the space owns the body ----------------------------------------
    let spawn_transform = Transform::from_translation(Vec3::new(0.0, 16.0, 0.0))
        .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.3, 0.6, 0.15));
 
    let grid = commands
        .spawn((
            build_moving_grid(spawn_transform, "TestBarrelCrate"),
            Friction::new(0.5),
            Restitution::new(0.05),
            // A shove, so it tumbles instead of settling flat.
            LinearVelocity(Vec3::new(0.0, 0.0, 3.0)),
            AngularVelocity(Vec3::new(1.2, 0.4, 0.0)),
        ))
        .id();
 
    // ---- the chunk supplies geometry -------------------------------------
    // ChunkSlot's hook parents it, places it, and registers it in the grid's
    // ChunkMap. Hydration gives the barrel voxel its entity, since no
    // BlockEvent::Place ever fires for a prefabricated chunk.
    commands.spawn((
        Name::new("TestBarrelCrate/chunk"),
        ChunkSlot { space: grid, coord: IVec3::ZERO },
        chunk,
        NeedsRemeshing,
    ));
 
    info!("dev grid spawned at {}", spawn_transform.translation);
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