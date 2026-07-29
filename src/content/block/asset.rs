use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::Deserialize;

use crate::content::block::components::{
    BlockBehaviorRegistry, BlockComponents, InteractsOnSecondary,
};
use crate::content::block::definition::{
    BlockAppearance, BlockDefinition, BlockMaterial, FaceTextures, SoundProfile, TextureSlot,
};
use crate::content::block::registry::BlockRegistry;
use crate::content::shape_set::ShapeSetRegistry;
use crate::content::texture::TextureRegistry;
use crate::voxel::{slots, BlockShape, Direction, ModelTable, ALL_DIRECTIONS};

use std::collections::BTreeMap;
use crate::content::block::definition::{ModelSurface, RenderClass};
use crate::content::model_source::ModelSourceRegistry;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – ASSET COLLECTION  (bevy_asset_loader)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(AssetCollection, Resource)]
pub struct BlockDefinitionAssets {
    /// Every *.json under assets/templates/blocks/, typed.
    #[asset(path = "templates\\blocks", collection(typed))]
    pub blocks: Vec<Handle<BlockDefinitionAsset>>,
}

/// One (asset × shape) pair with its generated identity, waiting to be
/// registered.
struct PendingBlock<'a> {
    name:         String,
    display_name: String,
    shape:        BlockShape,
    src:          &'a BlockDefinitionAsset,
}

/// Either one shape or a list of them, so both spellings work:
///   "shape":  "Slab"
///   "shapes": ["Cube", "Slab", "Slope"]
///
/// Untagged buffers the input and tries the variants in order, so a typo'd
/// shape reports as "data did not match any variant of untagged enum
/// ShapeSpec" rather than "unknown variant `Slabb`" — worth knowing when a
/// block file mysteriously refuses to load.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ShapeSpec {
    One(BlockShape),
    Many(Vec<BlockShape>),
}

impl ShapeSpec {
    /// One accessor for both variants — `from_ref` makes the single case a
    /// one-element slice with no allocation, so callers never branch.
    pub fn as_slice(&self) -> &[BlockShape] {
        match self {
            ShapeSpec::One(shape)   => std::slice::from_ref(shape),
            ShapeSpec::Many(shapes) => shapes.as_slice(),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – JSON-SHAPED MIRRORS  (bevy_common_assets reads these)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct BlockDefinitionAsset {
    pub name:          String,
    pub display_name:  String,
    #[serde(default, alias = "shape")]
    pub shapes:        Option<ShapeSpec>,
    /// Named sets from `*.shapeset.json`, unioned with `shapes`.
    #[serde(default)]
    pub shape_sets:    Vec<String>,
    #[serde(default)]
    pub appearance:    BlockAppearanceAsset,
    #[serde(default = "default_true")]
    pub has_collision: bool,
    #[serde(default)]
    pub material:      BlockMaterialAsset,
    #[serde(default)]
    pub sound_profile: SoundProfileAsset,
    /// Names resolved against `BlockBehaviorRegistry` at load time.
    #[serde(default)]
    pub behaviors:     Vec<String>,
    #[serde(default)]
    pub interactable:  bool,
    /// Default render class for every surface. Per-surface `render` on a
    /// `model` appearance overrides it.
    #[serde(default)]
    pub render: RenderClass,
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
    Model {
        surfaces: BTreeMap<String, ModelSurfaceAsset>
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
// SECTION 3 – JSON → REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn populate_block_registry_sys(
    block_assets: Res<BlockDefinitionAssets>,
    assets:       Res<Assets<BlockDefinitionAsset>>,
    asset_server: Res<AssetServer>,
    mut registry: ResMut<BlockRegistry>,
    tex:          Res<TextureRegistry>,
    behaviors:    Res<BlockBehaviorRegistry>,
    shape_sets:   Res<ShapeSetRegistry>,
    sources:      Res<ModelSourceRegistry>,
) {
    // ── Fan out: one asset → one definition per shape ──────────────────
    let mut pending: Vec<PendingBlock<'_>> = Vec::new();

    for handle in &block_assets.blocks {
        let Some(src) = assets.get(handle) else {
            error!("Block definition asset not parsed before resolution — check loading state.");
            continue;
        };

        for shape in resolve_shapes(src, &shape_sets) {
            let (name, display_name) = derive_names(&shape, &src.name, &src.display_name);
            pending.push(PendingBlock { name, display_name, shape, src });
        }
    }

    // Sorting the GENERATED names, rather than the source files, makes
    // in-memory BlockIDs a function of which blocks exist — not of the
    // order shapes happen to be listed in, or of which set contributed
    // them. Deterministic across runs, which is handy for debugging.
    // Save files still serialize names, never IDs.
    pending.sort_by(|a, b| a.name.cmp(&b.name));

    // ── Resolve and register ───────────────────────────────────────────
    for block in pending {
        // BlockRegistry::register_block would otherwise overwrite the
        // name→id entry and leave an unreachable definition behind.
        if registry.by_name(block.name.clone()).is_some() {
            warn!("Generated block '{}' collides with an existing block — skipped. \
                   Either a hand-written file already claims that name, or two \
                   shape families overlap.", block.name);
            continue;
        }

        // Resolved per generated block rather than once per asset: the
        // warning then names the block that actually failed, and the
        // AssetServer returns the same handle for a repeated path, so the
        // audio in a five-shape family is loaded once, not five times.
        let mut components = BlockComponents::default();
        if let Some(spawns) = behaviors.resolve(&block.src.behaviors, &block.name) {
            components = components.with(spawns);
        }
        if block.src.interactable {
            components = components.with(InteractsOnSecondary);
        }

        let appearance = resolve_appearance(&block.src.appearance, &tex);
        // Texture slots are resolved here, next to the appearance they come
        // from. Geometry is not: `render::mesh::bake` fills `models` in a
        // later pass, because only the renderer owns a model arena.
        let texture_slots = resolve_slots(&appearance, &block.shape, &sources, RenderClass::Mask);

        registry.register_block(BlockDefinition {
            name:          block.name,
            display_name:  block.display_name,
            shape:         block.shape,
            models:        ModelTable::default(),
            texture_slots,
            appearance,
            has_collision: block.src.has_collision,
            material:      resolve_material(&block.src.material),
            sound_profile: resolve_sound_profile(&block.src.sound_profile, &asset_server),
            components,
        });
    }

    info!("BlockRegistry populated from JSON: {} entries.", registry.size());
}

/// Union of the block's shape sets (in declaration order) with its inline
/// shapes, deduplicated. Sets first so an inline list reads as "and also
/// these".
///
/// An empty result means the file said nothing usable about shape, which is
/// the pre-shape-families case: one cube. Note that a block naming only
/// unknown sets lands here too — it warns, then falls back to a cube rather
/// than vanishing from the registry.
fn resolve_shapes(src: &BlockDefinitionAsset, sets: &ShapeSetRegistry) -> Vec<BlockShape> {
    let mut out: Vec<BlockShape> = Vec::new();
    let push = |shape: &BlockShape, out: &mut Vec<BlockShape>| {
        if !out.contains(shape) {
            out.push(shape.clone());
        }
    };

    for set_name in &src.shape_sets {
        match sets.get(set_name) {
            Some(shapes) => for shape in shapes { push(shape, &mut out); },
            // Reported here, at load time, not at bake time — a typo
            // should scream early.
            None => warn!("Block '{}' references unknown shape set '{}'.",
                          src.name, set_name),
        }
    }

    if let Some(spec) = &src.shapes {
        for shape in spec.as_slice() { push(shape, &mut out); }
    }

    if out.is_empty() {
        out.push(BlockShape::default());
    }
    out
}

/// "slate" + Slab → ("slate_slab", "Slate (Slab)"). Cube passes through
/// unchanged — see `BlockShape::suffixes`.
fn derive_names(shape: &BlockShape, name: &str, display: &str) -> (String, String) {
    match shape.suffixes() {
        Some((slug, label)) => (format!("{name}_{slug}"), format!("{display} ({label})")),
        None                => (name.to_owned(), display.to_owned()),
    }
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
        BlockAppearanceAsset::Model { surfaces } => BlockAppearance::Model(
            surfaces
                .iter()
                .map(|(name, s)| {
                    (name.clone(), ModelSurface {
                        textures: resolve_face(&s.textures, tex),
                        render:   s.render,
                    })
                })
                .collect(),
        ),
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – APPEARANCE → TEXTURE SLOTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Fallback for appearances that have no per-direction notion of a texture.
static MISSING_FACE: FaceTextures = FaceTextures::Simple(TextureRegistry::MISSING_LAYER);

/// Which `FaceTextures` a given face of a given appearance uses.
fn resolve_face_texture(
    appearance: &BlockAppearance,
    face_dir: Direction,
    is_internal: bool,
) -> &FaceTextures {
    match appearance {
        BlockAppearance::Uniform(ft) => ft,

        BlockAppearance::TopBotSide { up, down, side } => match face_dir {
            Direction::Up   => up,
            Direction::Down => down,
            _               => side, // sides AND interior faces
        },

        BlockAppearance::PerFace { up, down, north, south, east, west } => {
            match face_dir {
                Direction::Up    => up,
                Direction::Down  => down,
                Direction::North => north,
                Direction::South => south,
                Direction::East  => east,
                Direction::West  => west,
            }
        }
        BlockAppearance::UniformWithInternal { ext, int } => {
            if is_internal { int } else { ext }
        }

        BlockAppearance::Model(_) => {
            warn!("Directional texture lookup on a 'model' appearance — \
                   custom models resolve their slots by name, not by face.");
            &MISSING_FACE
        }
    }
}

/// Flatten a `FaceTextures` into the three values the voxel shader wants.
fn resolve_texture_properties(face_texture: &FaceTextures) -> (u32, u32, [f32; 4]) {
    match face_texture {
        FaceTextures::Simple(b) => (*b, 0, [1.0, 1.0, 1.0, 1.0]),
        FaceTextures::Bilayer(b, o) => (*b, *o, [1.0, 1.0, 1.0, 1.0]),
        FaceTextures::Tinted(b, o, color) => (
            *b,
            *o,
            // Convert Bevy Color to a linear [f32; 4] for the shader
            {
                let c = color.to_linear();
                [c.red, c.green, c.blue, c.alpha]
            },
        ),
    }
}

/// The block's paintable-surface table, in the exact order the shape
/// generators stamp slot indices.
///
/// Slot numbering itself lives in `voxel::shape`, which is what lets this
/// (content) and the shape generators (render) agree without either
/// importing the other.
pub fn resolve_slots(
    appearance:    &BlockAppearance,
    shape:         &BlockShape,
    sources:       &ModelSourceRegistry,
    default_class: RenderClass,
) -> Vec<TextureSlot> {

    // Imported models are resolved by name, so they branch before the
    // shape-keyed slot conventions get a chance to apply.
    if let BlockAppearance::Model(surfaces) = appearance {
        return resolve_model_slots(surfaces, shape, sources, default_class);
    }

    match shape {
        // Pipes: 3 slots (core, arm, cap). For now they all take the block's
        // one texture. If you later want a distinct cap, this is the only
        // place that changes.
        BlockShape::Pipe => {
            let ft = resolve_face_texture(appearance, Direction::Up, false);
            let s: TextureSlot = resolve_texture_properties(ft).into();
            vec![s, s, s] // CORE, ARM, CAP  (indices 0,1,2)
        }

        // Everything else: 6 directional slots + 1 interior, in the exact
        // order the shape generators emit (slots::NORTH..DOWN, then INTERIOR).
        _ => {
            let mut out = Vec::with_capacity(slots::PRIMITIVE_SLOT_COUNT);
            for d in ALL_DIRECTIONS {
                let ft = resolve_face_texture(appearance, d, false);
                out.push(resolve_texture_properties(ft).into());
            }
            // Interior slot: is_internal=true so UniformWithInternal picks `int`.
            let ft = resolve_face_texture(appearance, Direction::Up, true);
            out.push(resolve_texture_properties(ft).into());
            out
        }
    }
}

/// One slot per texture the model declares, in the model's own order.
///
/// That order is the contract between the importer (which stamps a face's
/// slot with its texture index) and this table. Nothing else needs to agree
/// on it, which is what lets twelve chess blocks share one baked model.
fn resolve_model_slots(
    surfaces:      &BTreeMap<String, ModelSurface>,
    shape:         &BlockShape,
    sources:       &ModelSourceRegistry,
    default_class: RenderClass,
) -> Vec<TextureSlot> {
    let BlockShape::Custom(path) = shape else {
        warn!("A 'model' appearance on shape {shape:?}, which has no model file. \
               Every surface falls back to 'missing'.");
        return vec![TextureSlot::MISSING; shape.slot_count()];
    };

    let Some(doc) = sources.get(path) else {
        warn!("Block references model '{path}', which is not in the model registry. \
               Check that the file is under assets/models/blocks/.");
        return vec![TextureSlot::MISSING];
    };

    doc.surface_names()
        .map(|name| match surfaces.get(name) {
            Some(surface) => TextureSlot::from(resolve_texture_properties(&surface.textures))
                .with_class(surface.render.unwrap_or(default_class)),
            None => {
                warn!("Model '{path}' declares surface '{name}', which this block's \
                       appearance does not paint — falling back to 'missing'.");
                TextureSlot::MISSING
            }
        })
        .collect()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CUSTOM MODEL RELATED DATA
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


#[derive(Deserialize, Debug, Clone)]
pub struct ModelSurfaceAsset {
    #[serde(flatten)]
    pub textures: FaceTexturesAsset,
    #[serde(default)]
    pub render:   Option<RenderClass>,
}