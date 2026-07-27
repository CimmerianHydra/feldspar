use bevy::prelude::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ITEM DISPLAY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// GregTech-style tinting: part sprites are authored in grayscale, and the
// substance's color rides in as `DisplayLayer::tint`, multiplied by the GPU
// through `ImageNode::color`. No shaders needed. When a layer eventually
// wants a real shader (enchant shimmer, animated gems), add an optional
// `effect: Option<Handle<YourUiMaterial>>` field to `DisplayLayer` and branch
// in the UI's layer builder — the data model already accommodates it.

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

/// Renderer-agnostic description of how an item looks.
///
/// It lives with the item definition rather than with the UI on purpose: the
/// UI consumes this to build `ImageNode`s, and a future world-sprite builder
/// (dropped items, held tools) will consume the *same* data to build
/// `Sprite`s. Never store UI types in here.
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
