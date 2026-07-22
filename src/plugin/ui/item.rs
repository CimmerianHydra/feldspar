use bevy::prelude::*;
use bevy::ecs::spawn::SpawnIter;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BASIC DEFINITIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// GregTech-style tinting: part sprites are authored in grayscale, and the
// substance's color rides in as `DisplayLayer::tint`, multiplied by the GPU
// through `ImageNode::color`. No shaders needed. When a layer eventually
// wants a real shader (enchant shimmer, animated gems), add an optional
// `effect: Option<Handle<YourUiMaterial>>` field to `DisplayLayer` and branch
// in `build_layer_node` — the data model already accommodates it.

/// One visual layer of an item icon: a sprite multiplied by a tint.
/// White tint == the sprite as authored. Layers are drawn bottom-to-top
/// in Vec order.
#[derive(Clone, PartialEq, Reflect)]
pub struct DisplayLayer {
    pub image: Handle<Image>,
    pub tint:  Color,
}

impl DisplayLayer {
    /// An untinted layer — the sprite exactly as authored.
    pub fn plain(image: Handle<Image>) -> Self {
        Self { image, tint: Color::WHITE }
    }
}

/// Renderer-agnostic description of how an item looks. UI consumes this to
/// build ImageNodes; a future world-sprite builder (dropped items, held
/// tools) consumes the SAME data to build Sprites. Never store UI types here.
#[derive(Clone, PartialEq, Reflect)]
pub enum ItemDisplay {
    /// A single static image loaded from the asset folder.
    Image { image: Handle<Image> },

    /// An ordered stack of tinted sprites composited into one icon.
    /// This is how substance-generated items (grayscale base × substance
    /// color) and modular tools (one layer per part) are drawn.
    Layered { layers: Vec<DisplayLayer> },
}

impl ItemDisplay {
    /// Normalize into the layer list — the one representation every
    /// renderer actually iterates.
    pub fn layers(&self) -> Vec<DisplayLayer> {
        match self {
            ItemDisplay::Image { image } => vec![DisplayLayer::plain(image.clone())],
            ItemDisplay::Layered { layers } => layers.clone(),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// UI ICON SPAWNER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const ITEM_ICON_SIZE: Val = Val::Px(64.0);

/// Builds the visual representation of an item, adding the necessary
/// children. Can be spawned into a UI node as a child.
///
/// The container is position-relative; each layer is an absolutely-
/// positioned full-size ImageNode child. Later children draw on top, so
/// layer order in the Vec is bottom-to-top, and the count text (spawned
/// last) always sits above every layer.
pub fn build_ui_item_display(
    display: &ItemDisplay,
    count:   u16,
) -> impl Bundle {

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
            color:      layer.tint,          // GPU multiply — the whole trick
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
        Pickable::IGNORE,
    )
}

fn build_ui_item_count(
    count: u16,
) -> impl Bundle {
    let count_text = if count == 1 { "".to_string() } else { count.to_string() };
    (
        Node {
            position_type: PositionType::Absolute,
            bottom: percent(0.0),
            left:  percent(0.0),
            ..default()
        },
        Text::new(count_text),
        TextColor(Color::WHITE),
        TextLayout::default(),
        Pickable::IGNORE,
    )
}