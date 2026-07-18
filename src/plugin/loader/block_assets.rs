use bevy::prelude::*;
use serde::Deserialize;
use bevy_asset_loader::prelude::*;

use crate::plugin::voxel::BlockShape;
use crate::plugin::graphics::block_textures::{BlockAppearance, FaceTextures};
use crate::plugin::block::material::BlockMaterial;
use crate::plugin::audio::block::SoundProfile;

use crate::plugin::loader::texture_registry::TextureRegistry;
use crate::plugin::loader::block_registry::{BlockID, BlockDefinition, BlockRegistry};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ASSET COLLECTION  (bevy_asset_loader)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(AssetCollection, Resource)]
pub struct BlockDefinitionAssets {
    /// Every *.json under assets/templates/blocks/, typed.
    #[asset(path = "templates\\blocks", collection(typed))]
    pub blocks: Vec<Handle<BlockDefinitionAsset>>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// JSON-SHAPED MIRRORS  (bevy_common_assets reads these)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct BlockDefinitionAsset {
    pub name:          String,
    pub display_name:  String,
    #[serde(default)] pub shape:         BlockShape,
    #[serde(default)] pub appearance:    BlockAppearanceAsset,
    #[serde(default = "default_true")]
    pub has_collision: bool,
    #[serde(default)] pub material:      BlockMaterialAsset,
    #[serde(default)] pub sound_profile: SoundProfileAsset,
}

fn default_true() -> bool { true }

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockAppearanceAsset {
    Uniform { texture: FaceTexturesAsset },
    TopBotSide {
        up:   FaceTexturesAsset,
        down: FaceTexturesAsset,
        side: FaceTexturesAsset,
    },
    PerFace {
        up:    FaceTexturesAsset, down:  FaceTexturesAsset,
        north: FaceTexturesAsset, south: FaceTexturesAsset,
        east:  FaceTexturesAsset, west:  FaceTexturesAsset,
    },
    UniformWithInternal {
        ext: FaceTexturesAsset,
        int: FaceTexturesAsset,
    },
}

impl Default for BlockAppearanceAsset {
    fn default() -> Self {
        BlockAppearanceAsset::Uniform {
            texture: FaceTexturesAsset::Simple { texture: "missing".to_string() },
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FaceTexturesAsset {
    Simple  { texture: String },
    Bilayer { texture: String, overlay: String },
    Tinted  { texture: String, overlay: String, tint: [u8; 3] },
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct BlockMaterialAsset {
    #[serde(default)] pub name:           String,
    #[serde(default)] pub hardness:       f32,
    #[serde(default)] pub mass_of_block:  f32,
    #[serde(default)] pub required_tool:  Option<String>,   // resolved by name later
    #[serde(default)] pub hardness_tier:  u32,
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct SoundProfileAsset {
    #[serde(default)] pub on_break: Option<String>,
    #[serde(default)] pub on_place: Option<String>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// JSON → REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn populate_block_registry_sys(
    block_assets:   Res<BlockDefinitionAssets>,
    assets:         Res<Assets<BlockDefinitionAsset>>,
    asset_server:   Res<AssetServer>,
    mut registry:   ResMut<BlockRegistry>,
    tex:            Res<TextureRegistry>,
) {
    // Deterministic order = deterministic in-memory IDs across runs (handy for debugging).
    // Save files should still serialize names, not IDs — see register_block warning on dupes.
    let mut defs: Vec<&BlockDefinitionAsset> = block_assets.blocks.iter()
        .filter_map(|h| assets.get(h))
        .collect();
    defs.sort_by(|a, b| a.name.cmp(&b.name));

    for src in defs {
        let def = BlockDefinition {
            id:            BlockID(0), // overwritten inside register_block
            name:          src.name.clone(),
            display_name:  src.display_name.clone(),
            shape:         src.shape.clone(),
            appearance:    resolve_appearance(&src.appearance, &tex),
            has_collision: src.has_collision,
            material:      resolve_material(&src.material),
            sound_profile: resolve_sound_profile(&src.sound_profile, &asset_server),
        };
        registry.register_block(def);
    }

    bevy::log::info!("BlockRegistry populated from JSON: {} entries.", registry.size());
}

fn resolve_appearance(src: &BlockAppearanceAsset, tex: &TextureRegistry) -> BlockAppearance {
    match src {
        BlockAppearanceAsset::Uniform { texture } =>
            BlockAppearance::Uniform(resolve_face(texture, tex)),
        BlockAppearanceAsset::TopBotSide { up, down, side } =>
            BlockAppearance::TopBotSide {
                up: resolve_face(up, tex), down: resolve_face(down, tex), side: resolve_face(side, tex),
            },
        BlockAppearanceAsset::PerFace { up, down, north, south, east, west } =>
            BlockAppearance::PerFace {
                up: resolve_face(up, tex),       down: resolve_face(down, tex),
                north: resolve_face(north, tex), south: resolve_face(south, tex),
                east: resolve_face(east, tex),   west: resolve_face(west, tex),
            },
        BlockAppearanceAsset::UniformWithInternal { ext, int } =>
            BlockAppearance::UniformWithInternal {
                ext: resolve_face(ext, tex), int: resolve_face(int, tex),
            },
    }
}

fn resolve_face(src: &FaceTexturesAsset, tex: &TextureRegistry) -> FaceTextures {
    match src {
        FaceTexturesAsset::Simple { texture } =>
            FaceTextures::Simple(tex.base_layer(texture)),
        FaceTexturesAsset::Bilayer { texture, overlay } =>
            FaceTextures::Bilayer(tex.base_layer(texture), tex.overlay_layer(overlay)),
        FaceTexturesAsset::Tinted { texture, overlay, tint } => {
            let [r, g, b] = *tint;
            FaceTextures::Tinted(
                tex.base_layer(texture),
                tex.overlay_layer(overlay),
                Color::srgb_u8(r, g, b),
            )
        }
    }
}

fn resolve_material(_src: &BlockMaterialAsset) -> BlockMaterial {
    // BlockMaterial's fields are currently private — either make them `pub`
    // and copy across, or add a `BlockMaterial::from_asset(&BlockMaterialAsset)`.
    BlockMaterial::default()
}

fn resolve_sound_profile(src: &SoundProfileAsset, srv: &AssetServer) -> SoundProfile {
    SoundProfile {
        on_break: src.on_break.as_ref().map(|p| srv.load(p)),
        on_place: src.on_place.as_ref().map(|p| srv.load(p)),
        ..default()
    }
}