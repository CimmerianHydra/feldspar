//! # Layer 8 — user interface
//!
//! Screens, widgets, and the stack that decides which of them the player is
//! looking at.
//!
//! May import: everything below, up to and including [`crate::sim`].
//!
//! Two rules keep this layer honest:
//!
//! 1. **Every submodule owns its own plugin.** `hotbar` registers the hotbar's
//!    observers, `inventory` registers the inventory's, and this file only
//!    adds child plugins. That is why almost nothing here is `pub`.
//! 2. **UI observes simulation, and asks for input by event.** It reacts to
//!    `Added<PlayerHotbar>`, `BlockEntityEvent`, `InventoryChangedEvent`. It
//!    never names an input action or an input context; `player::input`
//!    translates those into the request events declared here.

pub mod compass;
pub mod crafting;
pub mod cursor;
pub mod hotbar;
pub mod inventory;
pub mod pause;
pub mod screen;
pub mod splash;
pub mod theme;
pub mod widget;

pub use inventory::OpenPlayerInventoryRequest;
pub use pause::OpenPauseMenuRequest;
pub use screen::{
    UIPushOptions, UiBackRequest, UiCloseAllRequest, UiFocus, UiFocusChanged, UiScreen,
    UiScreenCommandsExt, UiStack, TextCapture, TextFocus,
};

use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            widget::UiWidgetPlugin,
            screen::UiScreenPlugin,
            cursor::UiCursorPlugin,
            inventory::UiInventoryPlugin,
            hotbar::UiHotbarPlugin,
            crafting::UiCraftingPlugin,
            compass::UiCompassPlugin,
            pause::UiPausePlugin,
            splash::UiSplashPlugin,
        ));
    }
}
