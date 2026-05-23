use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::plugin::controller::player::{Player, ClosePauseMenu, OpenPauseMenu};
use crate::plugin::inventory::player::{append_player_inventory_sys};

use crate::plugin::ui::hotbar::*;
use crate::plugin::ui::compass::*;
use crate::plugin::ui::cursor::*;
use crate::plugin::ui::inventory::*;
use crate::plugin::ui::player::*;

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here
        app

        .add_systems(Startup, spawn_ui_compass_sys)
        .add_systems(Startup, spawn_crosshair_sys)

        .add_systems(Update, spawn_cursor_item_display_sys)
        .add_systems(Update, button_sys)
        .add_systems(Update, sync_ui_compass_sys)

        .add_observer(spawn_pause_menu_obs)
        .add_observer(despawn_pause_menu_obs)
        .add_observer(pause_menu_actions_obs)

        .add_observer(cursor_lock_request_obs)
        .add_observer(sync_cursor_inventory_obs)

        .add_observer(inventory_ui_click_obs)
        .add_observer(inventory_sync_obs)
        .add_observer(inventory_changed_to_ui_sync_obs)
        .add_observer(show_requested_inventory_obs)
        .add_observer(close_requested_inventory_obs)

        .add_observer(open_inventory_player_input_action_obs)
        .add_observer(close_inventory_player_input_action_obs)

        .add_observer(show_requested_hotbar_obs)
        .add_observer(sync_hotbar_highlight_sys)
        ;
    }
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

#[derive(Component)]
pub struct PauseMenu;

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
        PauseMenu,
        ZIndex(100),
        Pickable::IGNORE,
    );
    return pause_menu_root
}

fn spawn_pause_menu_obs(
    event: On<Start<OpenPauseMenu>>,
    mut commands: Commands,
) {
    let pause_text_bundle = (
        Text::new("Game Paused"),
        TextFont {
            font_size: 40.0,
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
    commands.trigger(CursorLockRequest::Unlock);
}

fn despawn_pause_menu_obs(
    event: On<Start<ClosePauseMenu>>,
    mut commands: Commands,
   pause_menu_query: Query<Entity, With<PauseMenu>>,
) {
    for entity in pause_menu_query {
        commands.entity(entity).despawn();
    }
    commands.trigger(CursorLockRequest::Lock);
}

fn pause_menu_actions_obs(
    button_press: On<ButtonPressedEvent>,
    interaction_query: Query<&PauseMenuButton, With<Button>>,
    player_query: Query<Entity, With<Player>>,
    mut commands: Commands,
    mut app_exit_writer: MessageWriter<AppExit>,
) {
    let pressed_button_entity = button_press.entity;
    let Ok(button_data) = interaction_query.get(pressed_button_entity) else { return; };
    let Ok(player_entity) = player_query.single() else { return; };

    let resume_event = Start::<ClosePauseMenu> {
        context: player_entity,
        action: pressed_button_entity,
        value: true,
        state: TriggerState::Fired,
    };

    match button_data.action {
        MenuActions::QUIT => { app_exit_writer.write(AppExit::Success); },
        MenuActions::RESUME => { commands.trigger(resume_event); }
    }
}