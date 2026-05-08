use bevy::{prelude::*};
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::plugin::state::GameUpdateState;

// Contains UI plugins and systems, such as block highlighting when looking at a block, and interaction prompts.
pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here
        app

        .add_systems(Update, button_sys)

        .add_systems(OnEnter(GameUpdateState::Running), cursor_lock_sys)
        .add_systems(OnEnter(GameUpdateState::Running), spawn_crosshair_sys)

        .add_systems(OnEnter(GameUpdateState::Paused), cursor_release_sys)
        .add_systems(OnEnter(GameUpdateState::Paused), spawn_pause_menu_sys)

        .add_observer(pause_menu_actions_obs)
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

const PANEL_COLOR: Color = Color::srgba(0.1, 0.1, 0.1, 0.95);

const BUTTON_NORMAL: Color = Color::srgb(0.20, 0.20, 0.20);
const BUTTON_HOVERED: Color = Color::srgb(0.30, 0.30, 0.30);
const BUTTON_PRESSED: Color = Color::srgb(0.15, 0.45, 0.15);

const QUIT_BUTTON_NORMAL: Color = Color::srgb(0.35, 0.15, 0.15);
const QUIT_BUTTON_HOVERED: Color = Color::srgb(0.50, 0.20, 0.20);

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
                    font_size: FontSize::Px(24.0),
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
    );
    return pause_menu_bundle
}

fn spawn_pause_menu_sys(
    mut commands: Commands,
) {
    let pause_text_bundle = (
        Text::new("Game Paused"),
        TextFont {
            font_size: FontSize::Px(40.0),
            ..default()
        },
        TextColor::default(),
    );
    
    // Spawn the parent node and then the text as its child.
    commands.spawn(build_pause_menu())
        .with_children(|parent| {
            parent.spawn(pause_text_bundle);
            spawn_button(parent, "Resume", (PauseMenuButton { action: MenuActions::RESUME }));
            spawn_button(parent, "Quit Game", (PauseMenuButton { action: MenuActions::QUIT }));
    });
}

fn pause_menu_actions_obs(
    button_press: On<ButtonPressedEvent>,
    interaction_query: Query<(&PauseMenuButton), (With<Button>)>,
    mut next_state: ResMut<NextState<GameUpdateState>>,
    mut app_exit_writer: MessageWriter<AppExit>,
) {
    let pressed_button_entity = button_press.entity;
    if let Ok(a) = interaction_query.get(pressed_button_entity) {
        match a.action {
            MenuActions::QUIT => { app_exit_writer.write(AppExit::Success); },
            MenuActions::RESUME => { next_state.set(GameUpdateState::Running);}
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

    // Large node to center the crosshair
    let crosshair_parent = (Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
    DespawnOnExit(GameUpdateState::Running));

    // For now, just spawn a small square as a placeholder for a crosshair.
    let crosshair_entity = (
        Node {
            width: px(10),
            height: px(10),
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 1.0, 1.0)),
    );

    // Spawn the parent node and then the crosshair as its child.
    commands.spawn(crosshair_parent)
        .with_children(|parent| {
            parent.spawn(crosshair_entity);
    });
}