use bevy::math::ops::exp;
use bevy::prelude::*;
use iyes_progress::ProgressTracker;

use crate::app::{GameState};
use crate::app::{LoadReport, LoadSet};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE LOADING SCREEN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// How fast the bar chases the real value, in e-folds per second.
const BAR_EASE_RATE: f32 = 12.0;

#[derive(Component, Default)]
struct ProgressBar {
    /// Eased display width. Real progress lands in chunks — a whole
    /// geometry bake completes at once — so the bar chases the value
    /// instead of snapping to it.
    shown: f32,
}

#[derive(Component)]
struct ProgressLabel;

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
        ProgressBar::default(),
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
        Text::from(LoadReport::default().label),
        ProgressLabel,
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


fn update_progress_bar_sys(
    time: Res<Time>,
    tracker: Res<ProgressTracker<GameState>>,
    mut bars: Query<(&mut Node, &mut ProgressBar)>,
) {
    let Ok((mut node, mut bar)) = bars.single_mut() else { return };

    let p = tracker.get_global_progress();
    let target = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };

    // Frame-rate independent: loading frames are wildly irregular — one
    // step can eat half a second — and a fixed per-frame fraction would
    // crawl across exactly the frames where the bar should be moving.
    // `max` because the bar should never walk backwards, whatever the
    // tracker does while entries are still registering.
    let t = 1.0 - exp(-BAR_EASE_RATE * time.delta_secs());
    bar.shown += (target.max(bar.shown) - bar.shown) * t;

    node.width = percent(bar.shown * 100.0);
}

fn update_progress_label_sys(
    report: Res<LoadReport>,
    mut labels: Query<&mut Text, With<ProgressLabel>>,
) {
    if !report.is_changed() {
        return;
    }
    let Ok(mut text) = labels.single_mut() else { return };

    // `steps == 0` means the sequence hasn't started: files are still
    // coming off disk and a step counter would be a lie.
    *text = Text::new(if report.steps == 0 {
        report.label.to_string()
    } else {
        format!("{}  ({}/{})", report.label, report.step + 1, report.steps)
    });
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
                (update_progress_bar_sys, update_progress_label_sys).in_set(LoadSet::Present),
            );
    }
}
