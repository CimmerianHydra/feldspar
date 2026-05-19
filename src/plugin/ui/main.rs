use bevy::{prelude::*};
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::plugin::inventory::main::{Inventory, dev_spawn_dummy_inventory};
use crate::plugin::inventory::player::spawn_player_inventory_sys;
use crate::plugin::state::*;

use crate::plugin::ui::hotbar::*;
use crate::plugin::ui::compass::*;
use crate::plugin::ui::cursor::*;
use crate::plugin::ui::inventory::*;

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here
        app

        .add_systems(Startup, spawn_hotbar_sys)
        .add_systems(Startup, spawn_ui_compass_sys)
        .add_systems(Startup, spawn_crosshair_sys)

        .add_systems(Startup, spawn_cursor_item_display_sys.after(spawn_player_inventory_sys))


        .add_systems(Update, button_sys)
        .add_systems(Update, sync_ui_compass_sys)

        .add_systems(OnEnter(GameUpdateState::Paused), spawn_pause_menu_sys)

        .add_systems(OnEnter(UIState::Game), cursor_lock_sys)
        .add_systems(OnExit(UIState::Game), cursor_release_sys)

        .add_observer(pause_menu_actions_obs)
        .add_observer(sync_hotbar_highlight_obs)
        .add_observer(sync_hotbar_item_display_obs)
        .add_observer(sync_cursor_inventory_obs)
        .add_observer(inventory_ui_click_obs)
        .add_observer(inventory_sync_obs)
        .add_observer(inventory_changed_to_ui_sync_obs)
        .add_observer(show_requested_inventory_obs)
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

pub const UI_PANEL_COLOR: Color = Color::srgba_u8(32, 36, 44, 230);
pub const UI_PANEL_PADDING: Val = Val::Px(6.0);
pub const UI_PANEL_RADIUS: Val = Val::Px(6.0);
pub const UI_BORDER_COLOR: Color = Color::srgba_u8(90, 98, 120, 255);
pub const UI_BORDER_THICKN: Val = Val::Px(2.0);

pub const UI_SLOT_COLOR: Color = Color::srgb_u8(46, 52, 64);

pub const BUTTON_NORMAL: Color = Color::srgb(0.20, 0.20, 0.20);
pub const BUTTON_HOVERED: Color = Color::srgb(0.30, 0.30, 0.30);
pub const BUTTON_PRESSED: Color = Color::srgb(0.15, 0.45, 0.15);
pub const BUTTON_FONT_SIZE: f32 = 20.0;

pub const SLOT_SIZE: Val = Val::Px(80.0);
pub const SLOT_GAP: Val = Val::Px(6.0);


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
    let pause_menu_root = (
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
        ZIndex(100),
        Pickable::IGNORE,
    );
    return pause_menu_root
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
        Pickable::IGNORE,
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
        Pickable::IGNORE,
        children![crosshair_bundle]
    );

    // Spawn the parent node and then the crosshair as its child.
    commands.spawn(crosshair_parent);
}


pub fn show_requested_inventory_obs(
    view_requests: On<InventoryUISpawnRequest>,
    mut commands: Commands,
    inventory_q: Query<(Entity, &Inventory)>
) {
    let requested_inventory = view_requests.source_entity;
    if let Ok((source_entity, inventory)) = inventory_q.get(requested_inventory) {

        let ui_bundle = build_inventory_ui(source_entity, inventory.capacity(), 9);

        let root_bundle = (
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            DespawnOnExit(UIState::Inventory),
            ZIndex(100),
            Pickable::IGNORE,
            children![
                ui_bundle,
            ]
        );

        commands.spawn(root_bundle);

        // Send a sync request for all nonempty slots
        for (i, slot) in inventory.slots().iter().enumerate() {
            if slot.is_some() {
                bevy::log::info!("Found nonempty slot at {}, sending request to sync.", i);
                commands.trigger(InventoryUISyncRequest {
                    entity: requested_inventory,
                    index:  i,
                });
            }
        }
    }
}