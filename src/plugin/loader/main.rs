use bevy::math::ops::ceil;
use bevy::prelude::*;
use bevy_common_assets::json::JsonAssetPlugin;
use bevy_asset_loader::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use iyes_progress::{Progress, ProgressPlugin, ProgressReturningSystem, ProgressTracker};


use crate::plugin::state::GameState;

use crate::plugin::loader::texture_registry::*;
use crate::plugin::loader::block_registry::*;
use crate::plugin::loader::item_registry::*;

use crate::plugin::loader::block_assets::*;
use crate::plugin::loader::texture_assets::*;
pub struct AssetLoaderPlugin;

impl Plugin for AssetLoaderPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins((
            // track progress during `GameState::Loading` and continue to `GameState::Running` when progress is completed
            ProgressPlugin::<GameState>::new()
                .with_state_transition(GameState::AssetLoading, GameState::InGame),
            FrameTimeDiagnosticsPlugin::default(),
        ))

        .insert_resource(TextureRegistry::new())
        .insert_resource(BlockRegistry::new())
        .insert_resource(ItemRegistry::new())

        .insert_resource(VoxelMaterialHandle::default())

        // Parser for *.json block definition files
        .add_plugins(JsonAssetPlugin::<BlockDefinitionAsset>::new(&["json"]))

        // Gate Loading -> Running on every block file being parsed
        .add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .load_collection::<BlockDefinitionAssets>()
                .load_collection::<BlockTextureAssets>()
        )

        // Splash screen and loading bar
        .add_systems(OnEnter(GameState::AssetLoading), spawn_loading_screen)

        .add_systems(
            Update,
            (
                track_fake_long_task.track_progress::<GameState>(),
                print_progress,
                update_progress,
            )
                .chain()
                .run_if(in_state(GameState::AssetLoading))
                .after(LoadingStateSet(GameState::AssetLoading)),
        )

        // Gate Loading -> Running on every block file being parsed
        .add_systems(
            OnExit(GameState::AssetLoading),
            (
                assemble_texture_arrays_sys,
                populate_block_registry_sys,
                populate_item_registry_sys,
            ).chain(),
        )
        ;
    }
}


#[derive(Component)]
struct ProgressBar;

fn spawn_loading_screen(
    mut commands: Commands,
    image_assets: Res<AssetServer>,
) {
    let title_card_handle = image_assets.load::<Image>("title/logo_black.png");

    let title_card = (
        Node {
            width: percent(90),
            ..default()
        },
        ImageNode::new(title_card_handle),
    );

    let progress_bar = (
        Node {
            width: percent(0.),
            height: percent(100),
            justify_content: JustifyContent::Start,
            border_radius: BorderRadius::all(px(5.0)),
            ..default()
        },
        BackgroundColor::from(Color::linear_rgb(1.0, 0.42, 0.0)),
        ProgressBar,
    );
    
    let progress_bar_base = (
        Node {
            width: percent(50),
            height: px(20),
            border_radius: BorderRadius::all(px(5.0)),
            ..default()
        },
        BackgroundColor::from(Color::linear_rgb(0.1, 0.1, 0.1)),
        Children::spawn(Spawn(progress_bar)),
    );

    let progress_text = (
        Node {
            width: percent(50),
            height: px(20),
            align_content: AlignContent::Center,
            justify_content: JustifyContent::Start,
            ..default()
        },
        TextColor::from(Color::linear_rgb(1.0, 0.42, 0.0)),
        Text::from("Loading assets...")
    );

    let splash_screen_root = (
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor::from(Color::linear_rgb(0., 0., 0.)),
        DespawnOnExit(GameState::AssetLoading),
        Children::spawn((
            Spawn(title_card),
            Spawn(progress_bar_base),
            Spawn(progress_text),
        ))
    );

    let temporary_camera = (
        Camera2d,
        DespawnOnExit(GameState::AssetLoading),
    );

    commands.spawn(temporary_camera);
    commands.spawn(splash_screen_root);
    
}

fn update_progress(
    progress: Res<ProgressTracker<GameState>>,
    mut bar:  Query<&mut Node, With<ProgressBar>>,
) {
    let Ok(mut node) = bar.single_mut() else { return };

    let p = progress.get_global_progress();
    let ratio = if p.total > 0 {
        p.done as f32 / p.total as f32
    } else {
        0.0
    };
    node.width = percent(ratio * 100.0);
}


pub const LONG_FAKE_TASK_DURATION: f32 = 1.0;

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
