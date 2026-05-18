use bevy::prelude::*;

use crate::plugin::ui::main::*;
use crate::plugin::controller::player::FPSCamera;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// NAVIGATION UI
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

use std::f32::consts::PI;
use std::f32::consts::FRAC_PI_2;

#[derive(Component)]
pub struct UICompass {
    current_angle: f32, // The current angle of the compass. The compass UI shows current_angle +- half sector.
    sector:        f32, // The span of angles that the UI compass should show
}

#[derive(Component)]
pub struct UICompassMarkerFixed {
    angle:          f32, // The fixed angle at which this marker is located
}

pub const UI_COMPASS_WIDTH: Val = Val::Px(400.0);
pub const UI_COMPASS_HEIGHT: Val = Val::Px(40.0);

pub fn spawn_ui_compass_sys (
    mut commands: Commands,
) {
    let root = (Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        ZIndex(3),
    );

    let compass_panel = (Node {
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

    // Hardcoded directions
    // Bevy uses yaw between plus pi and minus pi (with zero being North).
    let dir_with_name = [ (0., "N"),
                                            (FRAC_PI_2, "W"),
                                            (PI, "S"),
                                            (- FRAC_PI_2, "E")];
    
    // We first build the entire thing, then add it to the root as a child
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

pub fn sync_ui_compass_sys(
    player_camera_q: Query<&GlobalTransform, With<FPSCamera>>,
    mut compass_q: Query<&mut UICompass>,
    mut compass_markers: Query<(&mut Node, &mut Visibility, &UICompassMarkerFixed)>,
) {
    if let Ok(mut compass_data) = compass_q.single_mut() {
        if let Ok(g_transform) = player_camera_q.single() {
            let yaw = g_transform.rotation().to_euler(EulerRot::YXZ).0;

            compass_data.current_angle = yaw;

            let sector = compass_data.sector;
            let lower_bound = yaw - sector * 0.5;
            let upper_bound = yaw + sector * 0.5;

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

                if delta <= total {
                    Some(delta / total)
                } else {
                    None
                }
            }

            for (mut node, mut visibility, marker_data) in compass_markers.iter_mut() {
                let marker_angle = marker_data.angle;

                if let Some(fraction) = angle_fraction(marker_angle, lower_bound, upper_bound) {
                    *visibility = Visibility::Visible;
                    node.left = percent(100. * (1.0 - fraction));
                } else {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}