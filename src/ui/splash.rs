use bevy::prelude::*;
use iyes_progress::ProgressTracker;

use crate::GameState;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE LOADING SCREEN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
struct ProgressBar;

fn spawn_loading_screen(mut commands: Commands, image_assets: Res<AssetServer>) {
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
        Text::from("Loading assets..."),
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
        )),
    );

    let temporary_camera = (Camera2d, DespawnOnExit(GameState::AssetLoading));

    commands.spawn(temporary_camera);
    commands.spawn(splash_screen_root);
}

fn update_progress(
    progress: Res<ProgressTracker<GameState>>,
    mut bar: Query<&mut Node, With<ProgressBar>>,
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct UiSplashPlugin;

impl Plugin for UiSplashPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::AssetLoading), spawn_loading_screen)
            .add_systems(
                Update,
                update_progress.run_if(in_state(GameState::AssetLoading)),
            );
    }
}
