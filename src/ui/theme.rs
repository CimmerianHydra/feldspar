//! Every colour, padding and radius the UI uses, in one place.
//!
//! No systems, no components — this is the palette every other `ui`
//! submodule reads so that a panel built by `hotbar` and a panel built by
//! `crafting` cannot drift apart.

use bevy::prelude::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PANELS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const UI_PANEL_COLOR:   Color = Color::srgba_u8(32, 36, 44, 230);
pub const UI_PANEL_PADDING: Val   = Val::Px(6.0);
pub const UI_PANEL_RADIUS:  Val   = Val::Px(6.0);
pub const UI_BORDER_COLOR:  Color = Color::srgba_u8(90, 98, 120, 255);
pub const UI_BORDER_THICKN: Val   = Val::Px(2.0);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SLOTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const UI_SLOT_COLOR: Color = Color::srgb_u8(46, 52, 64);
pub const SLOT_SIZE:     Val   = Val::Px(80.0);
pub const SLOT_GAP:      Val   = Val::Px(6.0);

/// Border treatment for the selected hotbar slot.
pub const UI_HL_BORDER_COLOR:  Color = Color::srgba_u8(250, 250, 250, 255);
pub const UI_HL_BORDER_THICKN: Val   = Val::Px(4.0);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BUTTONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const BUTTON_NORMAL:    Color = Color::srgb(0.20, 0.20, 0.20);
pub const BUTTON_HOVERED:   Color = Color::srgb(0.30, 0.30, 0.30);
pub const BUTTON_PRESSED:   Color = Color::srgb(0.15, 0.45, 0.15);
pub const BUTTON_FONT_SIZE: f32   = 20.0;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ICONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const ITEM_ICON_SIZE: Val = Val::Px(64.0);
