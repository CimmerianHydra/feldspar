use bevy::prelude::*;

use crate::sim::player::Player;
use crate::ui::screen::{UIPushOptions, UiScreenCommandsExt};
use crate::ui::widget::{build_button, ButtonPressedEvent};
use crate::GameUpdate;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE MENU
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct PauseMenu;

pub enum MenuActions {
    Resume,
    Quit,
}

#[derive(Component)]
pub struct PauseMenuButton {
    action: MenuActions,
}

/// "Show this player the pause menu." Raised by `player::input`, which owns
/// the keybinding.
#[derive(Event, Clone, Copy, Debug)]
pub struct OpenPauseMenuRequest {
    pub player: Entity,
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

    (
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
            build_button("Resume", PauseMenuButton { action: MenuActions::Resume }),
            build_button("Quit Game", PauseMenuButton { action: MenuActions::Quit }),
        ],
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – OBSERVERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn spawn_pause_menu_obs(event: On<OpenPauseMenuRequest>, mut commands: Commands) {
    // Spawn the pause menu and immediately stop the world updating.
    commands.push_ui_screen(
        event.player,
        UIPushOptions { dim: true, sources: Vec::new(), ..default() },
        build_pause_menu(),
    );
    commands.set_state(GameUpdate::Disabled);
}

fn pause_menu_actions_obs(
    button_press: On<ButtonPressedEvent>,
    interaction_query: Query<&PauseMenuButton, With<Button>>,
    player_query: Query<Entity, With<Player>>,
    mut commands: Commands,
    mut app_exit_writer: MessageWriter<AppExit>,
) {
    let Ok(button_data) = interaction_query.get(button_press.entity) else { return };
    let Ok(player_entity) = player_query.single() else { return };

    match button_data.action {
        MenuActions::Quit   => { app_exit_writer.write(AppExit::Success); }
        MenuActions::Resume => { commands.pop_ui_screen(player_entity); }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct UiPausePlugin;

impl Plugin for UiPausePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_pause_menu_obs)
            .add_observer(pause_menu_actions_obs);
    }
}
