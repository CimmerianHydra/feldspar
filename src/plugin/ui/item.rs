use bevy::prelude::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BASIC DEFINITIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(PartialEq, Reflect)]
pub enum ItemDisplay {
    /// A single static image loaded from the asset folder.
    ///
    /// Example:
    /// ```rust
    /// ItemDisplay::Image {
    ///     image: asset_server.load("items/iron_ore.png"),
    /// }
    /// ```
    /// 
    /// This is used to attach an image to an item, which informs the UI
    /// or any other system how to represent the item visually in the game.
    
    Image {
        image: Handle<Image>,
    },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// UI ICON SPAWNER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const ITEM_ICON_SIZE: Val = Val::Px(64.0);
const ITEM_ICON_ZINDEX: i32 = 1;
const ITEM_COUNT_ZINDEX: i32 = 2;

/// Builds the visual representation of an item, adding the necessary children.
/// Can be spawned into an UI node as a child.
///
/// `composite_materials` is only needed when the display is `Composite`;
/// you can obtain it via `ResMut<Assets<CompositeItemMaterial>>` in a system.

pub fn build_ui_item_display(
    display:    &ItemDisplay,
    count:      u16,
) -> impl Bundle {
    let icon_node = Node {
        width:  ITEM_ICON_SIZE,
        height: ITEM_ICON_SIZE,
        // Keep the image centred within the slot.
        align_self:   AlignSelf::Center,
        justify_self: JustifySelf::Center,
        ..default()
    };

    match display {
        // ── Static ───────────────────────────────────────────────────────
        ItemDisplay::Image { image } => { return (
                icon_node,
                ImageNode {
                    image: image.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
                ZIndex(ITEM_ICON_ZINDEX),
                Pickable::IGNORE,
                children![
                    build_ui_item_count(count)
                ]
            )
        }
    }
}

fn build_ui_item_count(
    count: u16,
) -> impl Bundle {
    let count_text = if count == 1 {"".to_string()} else {count.to_string()};
    return (
        Node {
        position_type: PositionType::Absolute,
        bottom: percent(0.0),
        right: percent(0.0),
        ..default()
        },
        Text::new(count_text),
        TextColor(Color::WHITE),
        TextLayout::default(),
        ZIndex(ITEM_COUNT_ZINDEX),
        Pickable::IGNORE,
    )
}