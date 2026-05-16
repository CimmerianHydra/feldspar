use bevy::{prelude::*};
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::plugin::controller::player::FPSCamera;
use crate::plugin::inventory::item_display::{build_ui_item_display};
use crate::plugin::inventory::item_registry::ItemRegistry;
use crate::plugin::inventory::main::{Inventory, InventoryChangedEvent};
use crate::plugin::state::*;
use crate::plugin::inventory::player_inventory::*;

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here
        app

        .add_systems(Startup, spawn_gameui_sys)
        .add_systems(Startup, spawn_ui_compass)

        .add_systems(Update, button_sys)
        .add_systems(Update, sync_ui_compass_sys)

        .add_systems(OnEnter(GameUpdateState::Running), cursor_lock_sys)
        .add_systems(OnEnter(GameUpdateState::Running), spawn_crosshair_sys)

        .add_systems(OnEnter(GameUpdateState::Paused), cursor_release_sys)
        .add_systems(OnEnter(GameUpdateState::Paused), spawn_pause_menu_sys)


        .add_observer(pause_menu_actions_obs)
        .add_observer(sync_hotbar_highlight_obs)
        .add_observer(sync_hotbar_item_display_obs)
        ;
    }
}

pub fn cursor_lock_sys(
    mut cursor_options: Single<&mut CursorOptions, With<PrimaryWindow>>
) {
    cursor_options.grab_mode = CursorGrabMode::Locked;
    cursor_options.visible = false;
}

pub fn cursor_release_sys(
    mut cursor_options: Single<&mut CursorOptions, With<PrimaryWindow>>
) {
    cursor_options.grab_mode = CursorGrabMode::None;
    cursor_options.visible = true;
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// COLORS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const UI_PANEL_COLOR: Color = Color::srgba_u8(32, 36, 44, 230);
const UI_PANEL_PADDING: Val = Val::Px(6.0);
const UI_PANEL_RADIUS: Val = Val::Px(6.0);
const UI_BORDER_COLOR: Color = Color::srgba_u8(90, 98, 120, 255);
const UI_BORDER_THICKN: Val = Val::Px(2.0);
const UI_HL_BORDER_COLOR: Color = Color::srgba_u8(250, 250, 250, 255);
const UI_HL_BORDER_THICKN: Val = Val::Px(4.0);

const UI_SLOT_COLOR: Color = Color::srgb_u8(46, 52, 64);

const BUTTON_NORMAL: Color = Color::srgb(0.20, 0.20, 0.20);
const BUTTON_HOVERED: Color = Color::srgb(0.30, 0.30, 0.30);
const BUTTON_PRESSED: Color = Color::srgb(0.15, 0.45, 0.15);
const BUTTON_FONT_SIZE: f32 = 20.0;

const SLOT_SIZE: Val = Val::Px(80.0);
const SLOT_GAP: Val = Val::Px(6.0);


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BUTTON BUILDER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    with_bundle: impl Bundle,
) {
    parent
        .spawn((
            Button,
            Node {
                width: px(220),
                height: px(50),

                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,

                ..default()
            },
            BackgroundColor(BUTTON_NORMAL),
            with_bundle,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(text),
                TextFont {
                    font_size: BUTTON_FONT_SIZE,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

#[derive(Event)]
pub struct ButtonPressedEvent {
    entity: Entity,
}

pub fn button_sys(
    mut commands: Commands,
    mut interaction_query: Query<(Entity, &Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (e, interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BUTTON_PRESSED.into();
                commands.trigger(ButtonPressedEvent {entity: e});
            }

            Interaction::Hovered => {
                *color = BUTTON_HOVERED.into();
            }

            Interaction::None => {
                *color = BUTTON_NORMAL.into();
            }
        }
    }
}
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PAUSE MENU
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub enum MenuActions {
    RESUME,
    QUIT,
}

#[derive(Component)]
pub struct PauseMenuButton {
    action: MenuActions,
}

pub fn build_pause_menu() -> impl Bundle {
    let pause_menu_bundle = (
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        DespawnOnExit(GameUpdateState::Paused),
        ZIndex(1000),
    );
    return pause_menu_bundle
}

fn spawn_pause_menu_sys(
    mut commands: Commands,
) {
    let pause_text_bundle = (
        Text::new("Game Paused"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor::default(),
    );
    
    // Spawn the parent node and then the text as its child.
    commands.spawn(build_pause_menu())
        .with_children(|parent| {
            parent.spawn(pause_text_bundle);
            spawn_button(parent, "Resume", PauseMenuButton { action: MenuActions::RESUME });
            spawn_button(parent, "Quit Game", PauseMenuButton { action: MenuActions::QUIT });
    });
}

fn pause_menu_actions_obs(
    button_press: On<ButtonPressedEvent>,
    interaction_query: Query<&PauseMenuButton, With<Button>>,
    mut game_next_state: ResMut<NextState<GameUpdateState>>,
    mut ui_next_state: ResMut<NextState<UIState>>,
    mut app_exit_writer: MessageWriter<AppExit>,
) {
    let pressed_button_entity = button_press.entity;
    if let Ok(a) = interaction_query.get(pressed_button_entity) {
        match a.action {
            MenuActions::QUIT => { app_exit_writer.write(AppExit::Success); },
            MenuActions::RESUME => {
                game_next_state.set(GameUpdateState::Running);
                ui_next_state.set(UIState::Game);
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// GAME UI
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct HotbarSlot {
    index: usize,
}

pub fn build_hotbar_item_slot(
    index: usize,
) -> impl Bundle {
    (Node {
        width: SLOT_SIZE,
        height: SLOT_SIZE,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Column,
        border_radius: BorderRadius::all(UI_PANEL_RADIUS),
        border: UiRect::all(UI_BORDER_THICKN),
        margin: UiRect::all(SLOT_GAP),
        ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_SLOT_COLOR),
        HotbarSlot { index },
    )
}

pub fn build_hotbar_item_slot_highlighted(
    index: usize,
) -> impl Bundle {
    (Node {
        width: SLOT_SIZE,
        height: SLOT_SIZE,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Column,
        border_radius: BorderRadius::all(UI_PANEL_RADIUS),
        border: UiRect::all(UI_HL_BORDER_THICKN),
        margin: UiRect::all(SLOT_GAP),
        ..default()
        },
        BorderColor::all(UI_HL_BORDER_COLOR),
        BackgroundColor(UI_SLOT_COLOR),
        HotbarSlot { index },
    )
}

pub fn spawn_crosshair_sys(
    mut commands: Commands,
) {
    // Spawn a simple crosshair in the center of the screen using a UI node.

    // For now, just spawn a small square as a placeholder for a crosshair.
    let crosshair_bundle = (
        Node {
            width: px(10),
            height: px(10),
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 1.0, 1.0)),
    );

    // Large node to center the crosshair
    let crosshair_parent = (Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        DespawnOnExit(GameUpdateState::Running),
        children![crosshair_bundle]
    );

    // Spawn the parent node and then the crosshair as its child.
    commands.spawn(crosshair_parent);
}

pub fn spawn_gameui_sys(
    mut commands: Commands,
) {
    let game_ui_root = (Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        ZIndex(0),
    );

    let hotbar_panel = (Node {
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Row,
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            margin: UiRect::bottom(Val::Px(20.)),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
    );

    commands.spawn(game_ui_root)
        .with_children(|parent| {
            parent.spawn(hotbar_panel)
                .with_children(|hotbar| {
                    // Spawn first hotbar slot already highlighted
                    let slot_node = build_hotbar_item_slot_highlighted(0);
                    hotbar.spawn(slot_node);

                    // Spawn the other hotbar slots not highlighted
                    for index in 1..HOTBAR_CAPACITY {
                        let slot_node = build_hotbar_item_slot(index);
                        hotbar.spawn(slot_node);
                    }
                });
        });
}

fn sync_hotbar_highlight_obs(
    event: On<PlayerHotbarSelectedChange>,
    mut query: Query<(&mut Node, &mut BorderColor, &HotbarSlot)>
) {
    let new_index = event.new_index;

    for (mut node, mut border_color, slot_data) in query.iter_mut() {
        if slot_data.index == new_index {
            *border_color = BorderColor::all(UI_HL_BORDER_COLOR);
            node.border = UiRect::all(UI_HL_BORDER_THICKN);
        } else {
            *border_color = BorderColor::all(UI_BORDER_COLOR);
            node.border = UiRect::all(UI_BORDER_THICKN);
        }
    }
}


/// Redraws the hotbar when the player inventory changes, if the affected slots are in the hotbar
/// This is a temporary function. We need to have a much better system in place for updating
/// inventory UI later on.

fn sync_hotbar_item_display_obs(
    event: On<InventoryChangedEvent>,
    mut commands: Commands,
    hotbar_query: Query<(Entity, &HotbarSlot)>,
    player_inventory_query: Query<&Inventory, With<PlayerInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    if let Ok(player_inventory) = player_inventory_query.get(event.entity) {
        let affected_index = event.index;

        // If the affected index is in the hotbar, we update the hotbar
        if affected_index < HOTBAR_CAPACITY {

            let slots = player_inventory.slots();

            // Update the hotbar slot by refreshing it entirely. It's fine for now
            for (slot_entity, slot_data) in hotbar_query.iter() {

                if slot_data.index == affected_index {
                    // Remove everything
                    commands.entity(slot_entity).despawn_children();

                    if let Some(stack) = slots[affected_index] {
                        // Add the image of the stack if there's something there
                        let slot_image = commands.spawn(build_ui_item_display(&item_registry.get(stack.id).display, stack.count)).id();
                        commands.entity(slot_entity).add_child(slot_image);
                    }
                }
            }
        }
    }
}

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

fn spawn_ui_compass (
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

fn sync_ui_compass_sys(
    player_camera_q: Query<(&GlobalTransform), With<FPSCamera>>,
    mut compass_q: Query<(&mut UICompass)>,
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

            // There's probably a much smarter way of doing this
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