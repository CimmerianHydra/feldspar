use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::sim::player::HotbarSelect;
use crate::ui::inventory::OpenPlayerInventoryRequest;
use crate::ui::pause::OpenPauseMenuRequest;
use crate::ui::screen::{UiBackRequest, UiCloseAllRequest, UiFocus, UiFocusChanged};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – ACTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Move;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Look;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Jump;

/// Left click in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct PrimaryFire;

/// Right click in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct SecondaryFire;

/// Middle mouse button in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct AltFire;

/// Numbers 1-9 in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct SelectItem;

/// Backspace in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct UiBack;

/// ESC in the standard layout, while a screen is open.
#[derive(InputAction)]
#[action_output(bool)]
pub struct UiCloseAll;

/// ESC in the standard layout, while playing.
#[derive(InputAction)]
#[action_output(bool)]
pub struct UiOpenPauseMenu;

/// I in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct UiOpenPlayerInventory;

/// F3 in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct ConsoleToggle;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – CONTEXTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Set of inputs available to the player when they're in the game.
#[derive(Component)]
pub struct GameInput;

/// Set of inputs available whenever the player's UI stack is non-empty.
#[derive(Component)]
pub struct UiInput;

/// Helper component for the "select n-th hotbar slot" action, to retrieve
/// which hotbar index an action instance selects.
#[derive(Component)]
pub struct HotbarSelection {
    pub index: usize,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – BINDINGS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn build_game_input_actions() -> impl Bundle {
    (
        GameInput,
        actions!(GameInput[
            (
                Action::<Look>::new(),
                Bindings::spawn(Spawn((
                    Binding::mouse_motion(),
                    Negate::all(),
                ))),
            ),
            (Action::<PrimaryFire>::new(), bindings![MouseButton::Left]),
            (Action::<SecondaryFire>::new(), bindings![MouseButton::Right]),
            (Action::<Move>::new(), DeadZone::default(), Bindings::spawn(Cardinal::wasd_keys())),
            (Action::<Jump>::new(), bindings![KeyCode::Space]),
            (
                Action::<UiOpenPauseMenu>::new(),
                ActionSettings {
                    require_reset: true,
                    ..Default::default()
                },
                bindings![KeyCode::Escape],
            ),
            (
                // `require_reset` because `UiCloseAll` uses the same input,
                // and it should only be triggerable after release.
                Action::<UiOpenPlayerInventory>::new(),
                ActionSettings {
                    require_reset: true,
                    ..Default::default()
                },
                bindings![KeyCode::KeyI],
            ),
            (Action::<ConsoleToggle>::new(), bindings![KeyCode::F3]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 0 }, bindings![KeyCode::Digit1]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 1 }, bindings![KeyCode::Digit2]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 2 }, bindings![KeyCode::Digit3]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 3 }, bindings![KeyCode::Digit4]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 4 }, bindings![KeyCode::Digit5]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 5 }, bindings![KeyCode::Digit6]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 6 }, bindings![KeyCode::Digit7]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 7 }, bindings![KeyCode::Digit8]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 8 }, bindings![KeyCode::Digit9]),
        ]),
    )
}

pub fn build_ui_input_actions() -> impl Bundle {
    (
        UiInput,
        actions!(UiInput[
            (
                Action::<UiBack>::new(),
                ActionSettings { require_reset: true, ..Default::default() },
                bindings![KeyCode::Backspace],
            ),
            (
                // Escape and I both bail out entirely. Escape is contextual for
                // free: in GameInput it opens the pause menu, here it closes.
                Action::<UiCloseAll>::new(),
                ActionSettings { require_reset: true, ..Default::default() },
                bindings![KeyCode::Escape, KeyCode::KeyI],
            ),
            (Action::<ConsoleToggle>::new(), bindings![KeyCode::F3]),
        ]),
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – SCROLL WHEEL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// bevy_enhanced_input has no scroll-wheel binding, so this fills the gap.
/// Replace with a real binding as soon as one exists.
#[derive(Event, PartialEq)]
pub enum MouseScrollEvent {
    ScrollUp,
    ScrollDown,
}

fn mouse_scroll_handling_sys(
    mut scroll_wheel: MessageReader<MouseWheel>,
    mut commands: Commands,
) {
    for event in scroll_wheel.read() {
        let scroll_direction = match event.unit {
            MouseScrollUnit::Line => event.y.signum(),
            MouseScrollUnit::Pixel => {
                (event.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR).signum()
            }
        };
        if scroll_direction < 0.0 {
            commands.trigger(MouseScrollEvent::ScrollDown);
        } else {
            commands.trigger(MouseScrollEvent::ScrollUp);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – TRANSLATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// This is the whole reason `player` sits *above* `ui` and `sim` rather than
// beside them: every observer below turns a keypress into a request that a
// lower layer already understands. Rebinding a key, or adding a gamepad,
// touches this file and nothing else.

fn open_pause_menu_obs(event: On<Start<UiOpenPauseMenu>>, mut commands: Commands) {
    commands.trigger(OpenPauseMenuRequest { player: event.context });
}

fn open_player_inventory_obs(event: On<Start<UiOpenPlayerInventory>>, mut commands: Commands) {
    commands.trigger(OpenPlayerInventoryRequest { player: event.context });
}

fn ui_back_obs(event: On<Start<UiBack>>, mut commands: Commands) {
    commands.trigger(UiBackRequest { player: event.context });
}

fn ui_close_all_obs(event: On<Start<UiCloseAll>>, mut commands: Commands) {
    commands.trigger(UiCloseAllRequest { player: event.context });
}

fn hotbar_scroll_obs(event: On<MouseScrollEvent>, mut commands: Commands) {
    commands.trigger(match *event.event() {
        MouseScrollEvent::ScrollDown => HotbarSelect::Next,
        MouseScrollEvent::ScrollUp   => HotbarSelect::Previous,
    });
}

fn hotbar_digit_obs(
    event: On<Start<SelectItem>>,
    action_data_query: Query<&HotbarSelection>,
    mut commands: Commands,
) {
    let Ok(action_data) = action_data_query.get(event.action) else { return };
    commands.trigger(HotbarSelect::Index(action_data.index));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – FOCUS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The UI stack decides *what the player's attention is on*; this decides
/// what that means for input contexts.
///
/// The split is what lets `ui::screen` reconcile the stack without ever
/// naming `GameInput` or `UiInput`, which live a layer above it.
fn apply_ui_focus_obs(event: On<UiFocusChanged>, mut commands: Commands) {
    let (game, ui) = match event.focus {
        UiFocus::World   => (ContextActivity::<GameInput>::ACTIVE, ContextActivity::<UiInput>::INACTIVE),
        UiFocus::Screens => (ContextActivity::<GameInput>::INACTIVE, ContextActivity::<UiInput>::ACTIVE),
        // A text-capturing screen swallows everything, so both contexts go
        // quiet and the keystrokes reach the console buffer.
        UiFocus::Text    => (ContextActivity::<GameInput>::INACTIVE, ContextActivity::<UiInput>::INACTIVE),
    };
    commands.entity(event.player).insert((game, ui));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct PlayerInputPlugin;

impl Plugin for PlayerInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EnhancedInputPlugin)
            .add_input_context::<GameInput>()
            .add_input_context::<UiInput>()
            .add_systems(Update, mouse_scroll_handling_sys)
            .add_observer(open_pause_menu_obs)
            .add_observer(open_player_inventory_obs)
            .add_observer(ui_back_obs)
            .add_observer(ui_close_all_obs)
            .add_observer(hotbar_scroll_obs)
            .add_observer(hotbar_digit_obs)
            .add_observer(apply_ui_focus_obs);
    }
}
