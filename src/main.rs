use bevy::prelude::*;
use bevy::light::CascadeShadowConfigBuilder;

mod plugin;
use plugin::controller::freecamera::{FreeCameraPlugin, FreeCamera};
use plugin::geometry::meshing::MeshingPlugin;
use plugin::block_registry::{BlockRegistryPlugin, BlockDefinition, BlockID, BlockRegistry};
use plugin::block_interaction::BlockInteractionPlugin;
use plugin::chunk::ChunkPlugin;
use plugin::ui::main::UIPlugin;
use plugin::weather::WeatherPlugin;
use plugin::state::StatePlugin;
use plugin::voxel::BlockShape;
use plugin::controller::main::ControlsPlugin;
use plugin::inventory::main::InventoryPlugin;
use plugin::inventory::item_registry::ItemRegistryPlugin;
use plugin::graphics::block_material::{VoxelMaterialPlugin, VoxelMaterial};
use plugin::graphics::block_textures::{BlockAppearance, FaceTextures};
use plugin::worldgen::main::WorldgenPlugin;
use plugin::controller::player::PlayerControllerPlugin;
use plugin::audio::block::BlockAudioPlugin;
use plugin::crafting::main::SpatialCraftingPlugin;

use bevy::{input::common_conditions::input_toggle_active};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use avian3d::PhysicsPlugins;

use crate::plugin::audio::block::SoundProfile;


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
        .add_plugins(MeshingPlugin)
        .add_plugins(ChunkPlugin)
        .add_plugins(UIPlugin)
        .add_plugins(BlockRegistryPlugin)
        .add_plugins(ItemRegistryPlugin)
        .add_plugins(InventoryPlugin)
        .add_plugins(SpatialCraftingPlugin)
        .add_plugins(BlockInteractionPlugin)
        .add_plugins(WorldgenPlugin)
        .add_plugins(WeatherPlugin)
        .add_plugins(BlockAudioPlugin)

        .add_plugins(EguiPlugin::default())
        .add_plugins(
            WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::F3)),
        )

        // Game systems (that can't fit into any one previous plugin neatly)
        .add_systems(PreStartup, setup)
        .add_systems(Startup, dev_initialize_registry_sys)
        .add_systems(Startup, crate::plugin::inventory::item_registry::initialize_item_registry_sys.after(dev_initialize_registry_sys))

        .add_systems(Update, crate::plugin::worldgen::main::setup_dev_chunks.run_if(run_once))
        .add_systems(Update, dev_populate_player_inventory.run_if(run_once))
        .add_systems(Update, dev_lock_cursor.run_if(run_once))

        .run();
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
) {
    // light
    // directional 'sun' light
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::AMBIENT_DAYLIGHT,
            shadows_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 16.0, 0.0),
            rotation: Quat::from_rotation_x(- std::f32::consts::PI / 4.) * Quat::from_rotation_y(- std::f32::consts::PI / 4.),
            ..default()
        },
        // The default cascade config is designed to handle large scenes.
        // As this example has a much smaller world, we can tighten the shadow
        // bounds for better visual quality.
        CascadeShadowConfigBuilder {
            ..default()
        }.build(),
    ));
}

/*
fn debug_sys(
    camera_parameters: Query<(&DDARay, &Transform), With<FreeCamera>>,
) {
    if let Ok((ray, transform)) = camera_parameters.single() {
        bevy::log::info!("Camera position: {:?}, direction: {:?}", transform.translation, transform.rotation * Vec3::Z);
    }
}
*/

/// Test function that will provide with a few variations of basic blocks
pub fn dev_initialize_registry_sys(
    asset_server: Res<AssetServer>,
    mut registry: ResMut<BlockRegistry>,
) {
    // Initialize texture files

    // Initialize audio files
    let sample_break_audio = asset_server.load("audio\\block\\stone_on_break.ogg");
    let sample_place_audio = asset_server.load("audio\\block\\stone_on_place.ogg");

    let sound_profile = SoundProfile {
        on_break: Some(sample_break_audio),
        on_place: Some(sample_place_audio),
        ..default()
    };

    bevy::log::info_once!("Audio assets successfully loaded.");



    // We're just going to add some blocks manually
    registry.register_block(
        BlockDefinition {
            name: "dirt".to_string(),
            display_name: "Dirt".to_string(),
            appearance: BlockAppearance::Uniform(FaceTextures::Simple(1)),
            sound_profile: sound_profile.clone(),
            ..default()
        }
    );

    registry.register_block(
        BlockDefinition {
            name: "slate".to_string(),
            display_name: "Slate".to_string(),
            appearance: BlockAppearance::Uniform(FaceTextures::Simple(2)),
            sound_profile: sound_profile.clone(),
            ..default()
        }
    );

    let green_color = Color::srgb_u8(108, 185, 71);

    registry.register_block(
        BlockDefinition {
            name: "grass".to_string(),
            display_name: "Grass".to_string(),
            appearance: BlockAppearance::TopBotSide {
                up: FaceTextures::Tinted(1, 1, green_color),
                down: FaceTextures::Simple(1),
                side: FaceTextures::Tinted(1, 2, green_color),
            },
            sound_profile: sound_profile.clone(),
            ..default()
        }
    );

    // In the future we'll generate blocks from JSON files with their whole definition
    for shape in [
        BlockShape::Cube,
        BlockShape::Slab,
        BlockShape::Stair,
        BlockShape::Slope,
        ] {
        let base_name = "test".to_string();
        let base_display_name = "Test".to_string();

        let (name, display_name) = match shape {
            BlockShape::Cube => (format!("{}_{}", base_name, "cube"), format!("{} {}", base_display_name, "Cube")),
            BlockShape::Slab => (format!("{}_{}", base_name, "slab"), format!("{} {}", base_display_name, "Slab")),
            BlockShape::Stair => (format!("{}_{}", base_name, "stair"), format!("{} {}", base_display_name, "Stair")),
            BlockShape::Slope => (format!("{}_{}", base_name, "slope"), format!("{} {}", base_display_name, "Slope")),
            _ => ("test".to_string(), "Test".to_string())
        };

        let definition = BlockDefinition {
            name,
            display_name,
            shape,
            ..default()
        };
        registry.register_block(definition);
    }

    bevy::log::info_once!("BlockRegistry successfully initialized.");
}

use crate::plugin::inventory::main::*;
use crate::plugin::inventory::player::*;
use crate::plugin::inventory::item_registry::*;
use crate::plugin::ui::cursor::CursorLockRequest;

/// Hardcoded function to spawn some items into the player's inventory.
/// Since I hardcoded a few blocks in the block registry, I'll add them here.
pub fn dev_populate_player_inventory(
    mut commands: Commands,
    mut player_hotbar_query: Query<(Entity, &mut Inventory), With<PlayerHotbar>>,
    mut player_inventory_query: Query<(Entity, &mut Inventory), (With<PlayerInventory>, Without<PlayerHotbar>)>,
    item_registry: Res<ItemRegistry>,
) {
    if let Ok((entity, mut inventory)) = player_hotbar_query.single_mut() {
        for id in 1..5 {
            let item_id = ItemID(id as u16);
            let result = inventory.insert(item_id, 5, &item_registry);

            bevy::log::info!("Added [{}]x{} to player hotbar.", item_registry.get(item_id).name, result.transferred);
            commands.trigger(InventoryChangedEvent {
                entity,
                index: id - 1,
            });
        };
    }

    if let Ok((entity, mut inventory)) = player_inventory_query.single_mut() {
        for slot in 0..3 {
            let item_id = ItemID(1);
            let result = inventory.insert_at_slot(item_id, 40, slot, &item_registry);

            bevy::log::info!("Added [{}]x{} to player inventory.", item_registry.get(item_id).name, result.transferred);
            commands.trigger(InventoryChangedEvent {
                entity,
                index: slot,
            });
        };
    }


}


/// Lock the cursor to the screen at the beginning of the game
pub fn dev_lock_cursor(
    mut commands: Commands,
) {
    commands.trigger(CursorLockRequest::Lock);
}