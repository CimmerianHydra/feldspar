use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

use crate::sim::player::Player;
use crate::ui::theme::*;
use crate::GameState;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// NAVIGATION UI
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const UI_COMPASS_WIDTH:  Val = Val::Px(400.0);
pub const UI_COMPASS_HEIGHT: Val = Val::Px(40.0);

#[derive(Component)]
pub struct UICompass {
    /// The compass UI shows `current_angle ± half sector`.
    current_angle: f32,
    /// The span of angles the compass covers.
    sector: f32,
}

#[derive(Component)]
pub struct UICompassMarkerFixed {
    /// The fixed angle at which this marker is located.
    angle: f32,
}

pub fn spawn_ui_compass_sys(mut commands: Commands) {
    let root = (
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        ZIndex(3),
    );

    let compass_panel = (
        Node {
            width: UI_COMPASS_WIDTH,
            height: UI_COMPASS_HEIGHT,
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            margin: UiRect::top(Val::Px(20.)),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        UICompass { current_angle: 0., sector: 2. },
    );

    // Hardcoded directions. Bevy uses yaw between +pi and -pi, zero = North.
    let dir_with_name = [
        (0., "N"),
        (FRAC_PI_2, "W"),
        (PI, "S"),
        (-FRAC_PI_2, "E"),
    ];

    // Build the panel first, then attach it to the root.
    let compass_entity = commands.spawn(compass_panel).id();
    for (dir, name) in dir_with_name {
        let marker = (
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Text(name.to_string()),
            UICompassMarkerFixed { angle: dir },
            Visibility::Hidden,
        );
        let marker_entity = commands.spawn(marker).id();
        commands.entity(compass_entity).add_child(marker_entity);
    }

    commands.spawn(root).add_child(compass_entity);
}

/// Reads the *player body's* yaw rather than the camera's.
///
/// They are the same number — the body owns yaw and the camera child owns
/// pitch — but the body is simulation state, and presentation may read that.
/// A camera marker lives in `player`, one layer above this file.
pub fn sync_ui_compass_sys(
    player_q: Query<&GlobalTransform, With<Player>>,
    mut compass_q: Query<&mut UICompass>,
    mut compass_markers: Query<(&mut Node, &mut Visibility, &UICompassMarkerFixed)>,
) {
    let Ok(mut compass_data) = compass_q.single_mut() else { return };
    let Ok(g_transform) = player_q.single() else { return };

    let yaw = g_transform.rotation().to_euler(EulerRot::YXZ).0;
    compass_data.current_angle = yaw;

    let sector = compass_data.sector;
    let lower_bound = yaw - sector * 0.5;
    let upper_bound = yaw + sector * 0.5;

    for (mut node, mut visibility, marker_data) in compass_markers.iter_mut() {
        if let Some(fraction) = angle_fraction(marker_data.angle, lower_bound, upper_bound) {
            *visibility = Visibility::Visible;
            node.left = percent(100. * (1.0 - fraction));
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn wrap_angle(angle: f32) -> f32 {
    let tau = 2.0 * PI;
    (angle + PI).rem_euclid(tau) - PI
}

fn ccw_distance(from: f32, to: f32) -> f32 {
    wrap_angle(to - from).rem_euclid(2.0 * PI)
}

fn angle_fraction(angle: f32, start: f32, end: f32) -> Option<f32> {
    let total = ccw_distance(start, end);
    let delta = ccw_distance(start, angle);

    if delta <= total { Some(delta / total) } else { None }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct UiCompassPlugin;

impl Plugin for UiCompassPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_ui_compass_sys)
            .add_systems(Update, sync_ui_compass_sys);
    }
}
