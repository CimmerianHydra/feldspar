//! Blockbench `.bbmodel` documents, parsed but not yet geometry.
//!
//! Goes in `src/content/model_source.rs`.
//!
//! This is the *content*-layer half of custom model support. It holds the
//! deserialized file and nothing else — no `Element`, no `BakedQuad`, no
//! render types at all — because two layers need to read it and only one of
//! them is allowed to know the renderer exists:
//!
//! - [`crate::content::block::asset`] needs the **texture list**, in order,
//!   to build a block's slot table.
//! - `render::mesh::import` needs the **elements** to build geometry.
//!
//! Same split as `BlockDefinitionAsset` (the JSON mirror) against
//! `BlockDefinition` (the resolved thing): the file's shape lives here, and
//! each consumer owns its own interpretation of it.
//!
//! ## What is deliberately not modelled
//!
//! Groups, animations, PBR channels, per-texture `uv_width`, and mesh
//! (non-box) elements. Serde ignores unknown fields, so a file carrying any
//! of those still loads — the importer just won't act on them, and warns
//! where that would silently lose geometry.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::Deserialize;

use crate::voxel::{asset_stem, Direction};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE DOCUMENT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct BbModel {
    /// The UV space element face rectangles are expressed in. Note this is
    /// the *texture* grid, not the geometry grid — geometry is always in
    /// sixteenths of a block regardless of what this says.
    #[serde(default = "default_resolution")]
    pub resolution: BbResolution,
    #[serde(default)]
    pub elements: Vec<BbElement>,
    /// Ordered, and that order *is* the model's slot numbering. A block's
    /// appearance names entries here to say what each surface is painted
    /// with.
    #[serde(default)]
    pub textures: Vec<BbTexture>,
}

impl BbModel {
    /// How many paintable surfaces this model declares.
    pub fn slot_count(&self) -> usize {
        self.textures.len()
    }

    pub fn surface_names(&self) -> impl Iterator<Item = &str> {
        self.textures.iter().map(|t| t.name.as_str())
    }
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct BbResolution {
    pub width: f32,
    pub height: f32,
}

fn default_resolution() -> BbResolution {
    BbResolution { width: 16.0, height: 16.0 }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BbElement {
    #[serde(default)]
    pub name: String,
    /// `"cube"` for box elements. Blockbench also writes `"mesh"` for
    /// free-form polygon soups, which the importer refuses rather than
    /// guessing.
    #[serde(rename = "type", default = "default_element_type")]
    pub kind: String,
    /// Corner, in Blockbench units. Defaulted rather than required so a
    /// stray non-box element doesn't fail the whole file.
    #[serde(default)]
    pub from: [f32; 3],
    #[serde(default)]
    pub to: [f32; 3],
    /// Euler degrees. Absent (or all-zero) means axis-aligned, which is the
    /// fast path.
    #[serde(default)]
    pub rotation: Option<[f32; 3]>,
    /// Pivot for `rotation`, in the same space as `from`/`to`.
    #[serde(default)]
    pub origin: Option<[f32; 3]>,
    /// Grows the box on every axis. Blockbench uses it to avoid z-fighting
    /// between coincident faces.
    #[serde(default)]
    pub inflate: f32,
    #[serde(default = "default_true")]
    pub visibility: bool,
    #[serde(default)]
    pub faces: BbFaces,
}

fn default_element_type() -> String {
    "cube".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BbFaces {
    pub north: Option<BbFace>,
    pub south: Option<BbFace>,
    pub east: Option<BbFace>,
    pub west: Option<BbFace>,
    pub up: Option<BbFace>,
    pub down: Option<BbFace>,
}

impl BbFaces {
    /// Blockbench's face names line up exactly with our `Direction`, which
    /// is not a coincidence — both inherited them from Minecraft's model
    /// JSON.
    pub fn get(&self, dir: Direction) -> Option<&BbFace> {
        match dir {
            Direction::North => self.north.as_ref(),
            Direction::South => self.south.as_ref(),
            Direction::East => self.east.as_ref(),
            Direction::West => self.west.as_ref(),
            Direction::Up => self.up.as_ref(),
            Direction::Down => self.down.as_ref(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct BbFace {
    /// `[x1, y1, x2, y2]` in `BbModel::resolution` space.
    ///
    /// **Reversed rectangles are meaningful.** `x1 > x2` is how Blockbench
    /// encodes a mirrored face — the chess piece's pedestal top uses one on
    /// both axes. Sorting these into min/max would silently discard the
    /// artist's intent, so nothing here ever does.
    pub uv: [f32; 4],
    /// Index into [`BbModel::textures`]. Absent or null means the face is
    /// untextured, which is Blockbench's way of saying "don't draw this" —
    /// and is what turns a zero-thickness box into a proper two-sided
    /// billboard instead of six faces, four of them degenerate slivers.
    #[serde(default)]
    pub texture: Option<usize>,
    /// 0 / 90 / 180 / 270.
    #[serde(default)]
    pub rotation: u16,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BbTexture {
    /// The file name as authored, e.g. `"base.png"`. This is the key a
    /// block's appearance uses to name this surface.
    ///
    /// Keying by name rather than position matters: reordering textures in
    /// Blockbench should not silently repaint every block using the model.
    pub name: String,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Every parsed model, keyed by file stem.
///
/// Stem rather than full path for the same reason `TextureRegistry` keys by
/// stem: a block JSON should be able to say `"chess_piece.bbmodel"` or
/// `"models/blocks/chess_piece.bbmodel"` and mean the same thing.
#[derive(Resource, Default)]
pub struct ModelSourceRegistry {
    models: HashMap<String, BbModel>,
}

impl ModelSourceRegistry {
    pub fn get(&self, path: &str) -> Option<&BbModel> {
        self.models.get(asset_stem(path))
    }

    pub fn size(&self) -> usize {
        self.models.len()
    }
}

/// Every `*.bbmodel` under `assets/models/blocks/`. Drop a file in, restart,
/// reference it by stem — no code change, same deal as textures.
#[derive(AssetCollection, Resource)]
pub struct ModelSourceAssets {
    #[asset(path = "models/blocks", collection(typed, mapped))]
    pub models: HashMap<String, Handle<BbModel>>,
}

pub fn populate_model_source_registry_sys(
    sources: Res<ModelSourceAssets>,
    assets: Res<Assets<BbModel>>,
    mut registry: ResMut<ModelSourceRegistry>,
) {
    for (path, handle) in sources.models.iter() {
        let Some(doc) = assets.get(handle) else {
            error!("Model '{path}' not parsed before resolution — check the loading state.");
            continue;
        };

        let stem = asset_stem(path).to_owned();

        // Two files with the same stem in different folders would silently
        // shadow each other, and the survivor would depend on hash order.
        if registry.models.contains_key(&stem) {
            warn!("Two models share the stem '{stem}' — '{path}' was skipped. \
                   Model files are addressed by stem, so the names must be unique.");
            continue;
        }

        if doc.textures.is_empty() {
            warn!("Model '{stem}' declares no textures, so it has no paintable \
                   surfaces and every face will fall back to 'missing'.");
        }

        registry.models.insert(stem, doc.clone());
    }

    info!("ModelSourceRegistry populated: {} models.", registry.size());
}