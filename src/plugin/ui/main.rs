use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::plugin::controller::player::Player;
use crate::plugin::controller::player::UiOpenPauseMenu;

use crate::plugin::state::GameState;
use crate::plugin::state::GameUpdate;

use crate::plugin::ui::crafting::*;
use crate::plugin::ui::hotbar::*;
use crate::plugin::ui::compass::*;
use crate::plugin::ui::cursor::*;
use crate::plugin::ui::inventory::*;
use crate::plugin::ui::player::*;
use crate::plugin::ui::screen::*;

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here
        app

        .add_systems(OnEnter(GameState::InGame), spawn_ui_compass_sys)
        .add_systems(OnEnter(GameState::InGame), spawn_crosshair_sys)

        .add_systems(Update, spawn_cursor_item_display_sys)
        .add_systems(Update, button_sys)
        .add_systems(Update, sync_ui_compass_sys)

        .add_observer(ui_back_obs)
        .add_observer(ui_close_all_obs)
        .add_systems(Update, reconcile_ui_stack_sys)

        .add_observer(spawn_pause_menu_obs)
        .add_observer(pause_menu_actions_obs)

        .add_observer(cursor_lock_request_obs)
        .add_observer(sync_cursor_inventory_obs)

        .add_observer(inventory_ui_click_obs)
        .add_observer(inventory_sync_obs)
        .add_observer(inventory_changed_to_ui_sync_obs)
        .add_observer(entity_ui_session_end_obs)

        .add_observer(spatial_inventory_click_obs)
        .add_observer(spatial_inventory_changed_to_ui_sync_obs)

        .add_observer(recipe_output_click_obs)
        .add_observer(recipe_output_sync_obs)
        .add_observer(recipe_shape_overlay_sync_obs)

        .add_observer(open_player_inventory_obs)

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

fn build_button(
    text: &str,
    with_bundle: impl Bundle,
) -> impl Bundle {
    (
        Button,
        Node {
            width: px(220),
            height: px(50),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            margin: UiRect::all(SLOT_GAP),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_SLOT_COLOR),
        with_bundle,
        children![
            (
                Text::new(text),
                TextFont {
                    font_size: BUTTON_FONT_SIZE,
                    ..default()
                },
                TextColor(Color::WHITE),
            )
        ]
    )
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

    let pause_text_bundle = (
            Text::new("Game Paused"),
            TextFont {
                font_size: 40.0,
                ..default()
            },
            TextColor::default(),
        );

    let pause_menu_root = (
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        PauseMenu,
        ZIndex(100),
        Pickable::IGNORE,
        children![
            pause_text_bundle,
            build_button("Resume", PauseMenuButton { action: MenuActions::RESUME }),
            build_button("Quit Game", PauseMenuButton { action: MenuActions::QUIT }),
        ]
    );

    return pause_menu_root
}

fn spawn_pause_menu_obs(
    event: On<Start<UiOpenPauseMenu>>,
    mut commands: Commands,
) {
    let panel = build_pause_menu();
    
    // Spawn the pause menu and immediately set the world to not update
    commands.push_ui_screen(
        event.context, 
        UIPushOptions { dim: true, sources: Vec::new() },
        panel);
    commands.set_state(GameUpdate::Disabled);
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

    match button_data.action {
        MenuActions::QUIT => { app_exit_writer.write(AppExit::Success); },
        MenuActions::RESUME => { commands.pop_ui_screen(player_entity); }
    }
}
