//! The primitives every screen is built out of: buttons, slot frames, and
//! item icons.
//!
//! Nothing here knows what it is showing. `build_ui_item_display` takes an
//! `ItemDisplay` — a content-layer description — and turns it into nodes; it
//! never touches a registry.

use bevy::ecs::spawn::SpawnIter;
use bevy::prelude::*;

use crate::content::item::{DisplayLayer, ItemDisplay};
use crate::ui::theme::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – BUTTONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn build_button(text: &str, with_bundle: impl Bundle) -> impl Bundle {
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
        children![(
            Text::new(text),
            TextFont {
                font_size: BUTTON_FONT_SIZE,
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    )
}

/// Emitted when any `Button` is pressed. Screens observe this and match on
/// their own marker component, so no button ever needs a bespoke system.
#[derive(Event)]
pub struct ButtonPressedEvent {
    pub entity: Entity,
}

pub fn button_sys(
    mut commands: Commands,
    mut interaction_query: Query<
        (Entity, &Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (e, interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BUTTON_PRESSED.into();
                commands.trigger(ButtonPressedEvent { entity: e });
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
// SECTION 2 – SLOT FRAMES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The bare slot frame. Inventory slots, hotbar slots and the cursor slot
/// all start from this, which is why they cannot look different by accident.
pub fn build_base_ui_item_slot() -> impl Bundle {
    (
        Node {
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
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – ITEM ICONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Builds the visual representation of an item, adding the necessary
/// children. Can be spawned into a UI node as a child.
///
/// The container is position-relative; each layer is an absolutely-
/// positioned full-size ImageNode child. Later children draw on top, so
/// layer order in the Vec is bottom-to-top, and the count text (spawned
/// last) always sits above every layer.
pub fn build_ui_item_display(display: &ItemDisplay, count: u16) -> impl Bundle {
    let icon_node = Node {
        width:  ITEM_ICON_SIZE,
        height: ITEM_ICON_SIZE,
        // Keep the icon centred within the slot.
        align_self:    AlignSelf::Center,
        justify_self:  JustifySelf::Center,
        // Anchor for the absolutely-positioned layer children.
        position_type: PositionType::Relative,
        ..default()
    };

    let layers = display.layers();

    (
        icon_node,
        Pickable::IGNORE,
        Children::spawn((
            SpawnIter(layers.into_iter().map(build_layer_node)),
            Spawn(build_ui_item_count(count)),
        )),
    )
}

/// One full-size, tinted image layer inside the icon container.
fn build_layer_node(layer: DisplayLayer) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left:   percent(0.0),
            top:    percent(0.0),
            width:  percent(100.0),
            height: percent(100.0),
            ..default()
        },
        ImageNode {
            image:      layer.image,
            color:      layer.tint,
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
        Pickable::IGNORE,
    )
}

fn build_ui_item_count(count: u16) -> impl Bundle {
    let count_text = if count == 1 { String::new() } else { count.to_string() };
    (
        Node {
            position_type: PositionType::Absolute,
            bottom: percent(0.0),
            left:   percent(0.0),
            ..default()
        },
        Text::new(count_text),
        TextColor(Color::WHITE),
        TextLayout::default(),
        Pickable::IGNORE,
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct UiWidgetPlugin;

impl Plugin for UiWidgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, button_sys);
    }
}
