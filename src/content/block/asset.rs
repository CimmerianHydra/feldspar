use bevy::audio::AudioSource;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;


use crate::content::behavior::BehaviorEntry;
use crate::content::BlockBehaviorRegistry;
use crate::content::block::definition::{
    BlockDefinition, BlockMaterial, FaceTextures, RenderClass, SoundProfile,
};
use crate::content::block::registry::BlockRegistry;
use crate::content::block::surface::{resolve_slots, Surface, SurfaceLayout};
use crate::content::model_def::ModelDefRegistry;
use crate::content::model_source::ModelSourceRegistry;
use crate::content::texture::TextureRegistry;
use crate::voxel::{BlockShape, ModelTable};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BLOCK DEFINITIONS FROM JSON
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// One rule underlies the whole file: a shape declares an ordered list of
// surface names, an appearance is a name -> texture map, and slot N is
// whatever the Nth name resolves to.
//
// That rule is what removed the five `BlockAppearance` variants. Every one
// of them — uniform, top/bottom/side, per-face, uniform-with-internal — was
// a *distribution rule*, and distribution rules are what named surfaces
// make unnecessary. `top_bot_side` isn't a kind of appearance; it's three
// keys. The resolution itself lives in `surface.rs`; this file is only
// concerned with getting JSON into the shapes it wants.

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – JSON-SHAPED MIRRORS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct BlockDefinitionAsset {
    pub name: String,
    pub display_name: String,

    /// One appearance, or several.
    ///
    /// This is where `shape`, `shapes` and `shape_set` went. A shape set
    /// could only say "the same textures on N different shapes"; a list of
    /// appearances says that *and* lets each member paint itself, which is
    /// what a slab of a two-tone block always wanted.
    #[serde(default)]
    pub appearance: Appearances,

    #[serde(default = "default_true")]
    pub has_collision: bool,
    #[serde(default)]
    pub material: BlockMaterialAsset,
    #[serde(default)]
    pub sound_profile: SoundProfileAsset,
    #[serde(default)]
    pub behaviors: Vec<BehaviorEntry>,
    /// Default render class for every surface. A surface's own `render`
    /// overrides it.
    #[serde(default)]
    pub render: RenderClass,
}

fn default_true() -> bool {
    true
}

/// `"appearance": {…}` or `"appearance": [{…}, {…}]`.
///
/// Untagged is safe here in a way it isn't for most enums: an object and an
/// array can't be mistaken for one another, so a malformed appearance
/// reports the error from inside `AppearanceAsset` rather than the useless
/// "data did not match any variant" untagged usually produces.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Appearances {
    One(AppearanceAsset),
    Many(Vec<AppearanceAsset>),
}

impl Default for Appearances {
    fn default() -> Self {
        Appearances::One(AppearanceAsset::default())
    }
}

impl Appearances {
    pub fn as_slice(&self) -> &[AppearanceAsset] {
        match self {
            Appearances::One(a) => std::slice::from_ref(a),
            Appearances::Many(v) => v.as_slice(),
        }
    }
}

/// Geometry plus the textures that go into it. Together, how a block looks.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct AppearanceAsset {
    #[serde(default)]
    pub shape: BlockShape,

    /// Paintable surfaces by name. Keys may be surface names the shape
    /// exposes, group names (`side`, `sides`), aliases (`top`, `bottom`,
    /// `inside`), or `all`. See `surface::fallback_chain`.
    #[serde(default)]
    pub surfaces: BTreeMap<String, SurfaceAsset>,

    /// Name suffix when a block declares several appearances. Defaults to
    /// the shape's own slug, so a family of primitives needs no slugs at
    /// all.
    #[serde(default)]
    pub slug: Option<String>,
    /// Display suffix, same story: `"Slate"` + `"Slab"` -> `"Slate (Slab)"`.
    #[serde(default)]
    pub display_suffix: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SurfaceAsset {
    /// `kind` and its fields, inline — a surface reads as one flat object
    /// rather than a texture nested inside a wrapper.
    #[serde(flatten)]
    pub textures: FaceTexturesAsset,
    #[serde(default)]
    pub render: Option<RenderClass>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FaceTexturesAsset {
    Simple { texture: String },
    Bilayer { texture: String, overlay: String },
    Tinted { texture: String, overlay: String, tint: [u8; 3] },
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct BlockMaterialAsset {
    #[serde(default)] pub name:          String,
    #[serde(default)] pub hardness:      f32,
    #[serde(default)] pub mass_of_block: f32,
    #[serde(default)] pub required_tool: Option<String>,   // resolved by name later
    #[serde(default)] pub hardness_tier: u32,
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct SoundProfileAsset {
    #[serde(default)] pub on_break: Option<String>,
    #[serde(default)] pub on_place: Option<String>,
}

#[derive(AssetCollection, Resource)]
pub struct BlockDefinitionAssets {
    #[asset(path = "templates\\blocks", collection(typed))]
    pub blocks: Vec<Handle<BlockDefinitionAsset>>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – JSON → REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One appearance of one block, named and ready to register.
struct PendingBlock<'a> {
    name: String,
    display_name: String,
    shape: BlockShape,
    surfaces: BTreeMap<String, Surface>,
    src: &'a BlockDefinitionAsset,
}

pub fn populate_block_registry_sys(
    block_assets: Res<BlockDefinitionAssets>,
    assets: Res<Assets<BlockDefinitionAsset>>,
    asset_server: Res<AssetServer>,
    mut registry: ResMut<BlockRegistry>,
    tex: Res<TextureRegistry>,
    behaviors: Res<BlockBehaviorRegistry>,
    defs: Res<ModelDefRegistry>,
    sources: Res<ModelSourceRegistry>,
) {
    // ── Fan out: one asset → one definition per appearance ─────────────
    let mut pending: Vec<PendingBlock<'_>> = Vec::new();

    for handle in &block_assets.blocks {
        let Some(src) = assets.get(handle) else {
            error!("Block definition asset not parsed before resolution — check loading state.");
            continue;
        };

        let appearances = src.appearance.as_slice();
        let many = appearances.len() > 1;

        for appearance in appearances {
            // A lone appearance keeps the block's own name, whatever its
            // shape — `"shape": "Slab"` on its own registers as `slate`,
            // not `slate_slab`. Suffixes exist to disambiguate, so they
            // appear exactly when there is something to disambiguate.
            let (name, display_name) = if many {
                derive_names(appearance, &src.name, &src.display_name)
            } else {
                (src.name.clone(), src.display_name.clone())
            };

            pending.push(PendingBlock {
                name,
                display_name,
                shape: appearance.shape.clone(),
                surfaces: resolve_surfaces(&appearance.surfaces, &tex),
                src,
            });
        }
    }

    // Sorting the GENERATED names, rather than the source files, makes
    // in-memory BlockIDs a function of which blocks exist — not of the
    // order appearances happen to be listed in. Deterministic across runs,
    // which is handy for debugging. Save files still serialize names.
    pending.sort_by(|a, b| a.name.cmp(&b.name));

    // ── Resolve and register ───────────────────────────────────────────
    for block in pending {
        // `register_block` would otherwise overwrite the name→id entry and
        // leave an unreachable definition behind.
        if registry.by_name(block.name.clone()).is_some() {
            warn!(
                "Generated block '{}' collides with an existing block — skipped. \
                 Either a hand-written file already claims that name, or two \
                 appearances resolved to the same slug.",
                block.name
            );
            continue;
        }

        // Which surfaces the shape exposes, then what goes in each. Both
        // halves live in `surface.rs`; this is the only place they meet.
        //
        // Geometry is *not* resolved here: `render::mesh::bake` fills
        // `models` in a later pass, because only the renderer owns an arena.
        let layout = SurfaceLayout::resolve(&block.shape, &defs, &sources, &block.name);
        let texture_slots =
            resolve_slots(&layout, &block.surfaces, block.src.render, &block.name);

        registry.register_block(BlockDefinition {
            name:          block.name.clone(),
            display_name:  block.display_name,
            shape:         block.shape,
            models:        ModelTable::default(),
            texture_slots,
            surfaces:      block.surfaces,
            has_collision: block.src.has_collision,
            material:      resolve_material(&block.src.material),
            sound_profile: resolve_sound_profile(&block.src.sound_profile, &asset_server),
            behaviors:     behaviors.resolve(&block.src.behaviors, (), &block.name),
        });
    }

    info!("BlockRegistry populated from JSON: {} entries.", registry.size());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – RESOLUTION HELPERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// `"slate"` + a Slab appearance → `("slate_slab", "Slate (Slab)")`.
///
/// An explicit `slug` wins, then the shape's own suffix. Two appearances of
/// the *same* shape — two paint jobs on one cube — have no suffix to fall
/// back on, so that case is reported rather than silently collapsed into a
/// name collision.
fn derive_names(appearance: &AppearanceAsset, name: &str, display: &str) -> (String, String) {
    let shape_pair = appearance.shape.suffixes();

    let slug = appearance
        .slug
        .clone()
        .or_else(|| shape_pair.as_ref().map(|(s, _)| s.to_string()));

    let label = appearance
        .display_suffix
        .clone()
        .or_else(|| shape_pair.as_ref().map(|(_, l)| l.to_string()));

    match (slug, label) {
        (Some(slug), Some(label)) => {
            (format!("{name}_{slug}"), format!("{display} ({label})"))
        }
        _ => {
            warn!(
                "Block '{name}' lists several appearances but one of them has no \
                 slug and a shape that supplies none. Give it \"slug\": \"…\"."
            );
            (name.to_owned(), display.to_owned())
        }
    }
}

fn resolve_surfaces(
    src: &BTreeMap<String, SurfaceAsset>,
    tex: &TextureRegistry,
) -> BTreeMap<String, Surface> {
    src.iter()
        .map(|(name, s)| {
            (
                name.clone(),
                Surface { textures: resolve_face(&s.textures, tex), render: s.render },
            )
        })
        .collect()
}

fn resolve_face(src: &FaceTexturesAsset, tex: &TextureRegistry) -> FaceTextures {
    match src {
        FaceTexturesAsset::Simple { texture } => {
            FaceTextures::Simple(tex.base_layer(texture))
        }
        FaceTexturesAsset::Bilayer { texture, overlay } => {
            FaceTextures::Bilayer(tex.base_layer(texture), tex.overlay_layer(overlay))
        }
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
    // TODO: copy the asset's fields across. They were private on
    // `BlockMaterial` when this was written; they're `pub` now, so this is
    // a straight field-by-field move whenever hardness starts mattering.
    BlockMaterial::default()
}

fn resolve_sound_profile(src: &SoundProfileAsset, srv: &AssetServer) -> SoundProfile {
    SoundProfile {
        on_break: src.on_break.as_ref().map(|p| srv.load(p)),
        on_place: src.on_place.as_ref().map(|p| srv.load(p)),
    }
}

// Kept so the `AudioSource` handle type stays in scope for `SoundProfile`.
const _: Option<Handle<AudioSource>> = None;