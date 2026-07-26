use bevy::prelude::*;
use serde::Deserialize;
use bevy_asset_loader::prelude::*;

use crate::plugin::block::behavior::{BlockBehaviorRegistry, BlockComponents, InteractsOnSecondary};


use crate::plugin::graphics::block_textures::{BlockAppearance, FaceTextures};
use crate::plugin::block::material::BlockMaterial;
use crate::plugin::audio::block::SoundProfile;

use crate::plugin::geometry::variants::ModelTable;
use crate::plugin::geometry::model::ModelArena;
use crate::plugin::geometry::shapes;
use crate::plugin::geometry::rotation::BlockRotation;
use crate::plugin::geometry::variants::VariantKey;
use crate::plugin::geometry::voxel::BlockShape;
use crate::plugin::geometry::{shapes::slots, rotation::ALL_DIRECTIONS};
use crate::plugin::geometry::voxel::Direction;

use crate::plugin::loader::texture_registry::TextureRegistry;
use crate::plugin::loader::block_registry::{BlockDefinition, BlockRegistry};
use crate::plugin::loader::shape_sets::ShapeSetRegistry;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ASSET COLLECTION  (bevy_asset_loader)
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

#[derive(Clone, Copy)]
pub struct TexureSlot {
    pub base_layer:    u32,
    pub overlay_layer: u32,
    pub tint:          [f32; 4],
}

impl From<(u32, u32, [f32; 4])> for TexureSlot {
    fn from((base_layer, overlay_layer, tint): (u32, u32, [f32; 4])) -> Self {
        Self { base_layer, overlay_layer, tint }
    }
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
// JSON-SHAPED MIRRORS  (bevy_common_assets reads these)
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
    block_assets: Res<BlockDefinitionAssets>,
    assets:       Res<Assets<BlockDefinitionAsset>>,
    asset_server: Res<AssetServer>,
    mut registry: ResMut<BlockRegistry>,
    tex:          Res<TextureRegistry>,
    behaviors:    Res<BlockBehaviorRegistry>,
    shape_sets:   Res<ShapeSetRegistry>,
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

        registry.register_block(BlockDefinition {
            name:          block.name,
            display_name:  block.display_name,
            shape:         block.shape,
            models:        ModelTable::default(),
            texture_slots: Vec::new(),
            appearance:    resolve_appearance(&block.src.appearance, &tex),
            has_collision: block.src.has_collision,
            material:      resolve_material(&block.src.material),
            sound_profile: resolve_sound_profile(&block.src.sound_profile, &asset_server),
            components,
        });
    }

    info!("BlockRegistry populated from JSON: {} entries.", registry.size());
}

pub fn bake_block_geometry(
    mut commands: Commands,
    mut registry: ResMut<BlockRegistry>,
) {
    let mut arena = ModelArena::new();
    bake_all(&mut registry, &mut arena);
    info!("Model arena baked: {} models, {} quads",
          arena.model_count(), arena.quad_count());
    commands.insert_resource(arena);
}

pub fn bake_all(registry: &mut BlockRegistry, arena: &mut ModelArena) {
    for i in 1..registry.definitions.len() {
        let shape = registry.definitions[i].shape.clone();
        let name  = registry.definitions[i].name.clone();

        let table = match &shape {
            BlockShape::Cube =>
                ModelTable::single(arena.bake(&shapes::cube(), BlockRotation::IDENTITY)),

            BlockShape::Slab =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::slab(), BlockRotation::all())),

            BlockShape::Panel =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::panel(1.0), BlockRotation::all())),

            BlockShape::Stair =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::stair(), BlockRotation::all())),

            BlockShape::Slope =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::slope(), BlockRotation::all())),

            // The 64 masks, baked in order, indexed by 6 connection bits.
            BlockShape::Pipe => {
                let ids = shapes::all_pipe_variants()
                    .iter()
                    .map(|els| arena.bake(els, BlockRotation::IDENTITY))
                    .collect();
                ModelTable::new(VariantKey::stateful(6), ids)
            }

            // Blockbench import — parser is future work; fall back to a
            // cube so the match stays total and nothing panics.
            BlockShape::Custom(_path) =>
                ModelTable::single(arena.bake(&shapes::cube(), BlockRotation::IDENTITY)),
        };

        table.key.validate(&name);

        let slots = resolve_slots(&registry.definitions[i].appearance, &shape);

        registry.definitions[i].models = table;
        registry.definitions[i].texture_slots  = slots;
    }
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
    let mut push = |shape: &BlockShape, out: &mut Vec<BlockShape>| {
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

/// Helper function to figure out the correct texture to give each face.
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
            _               => side, // sides AND interior (None) faces
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
        BlockAppearance::UniformWithInternal{ ext, int} => {
            if is_internal { int } else { ext }
        }
    }
}

/// Helper function to figure out the correct texture data to provide to the voxel terrain shader.
fn resolve_texture_properties(face_texture: &FaceTextures) -> (u32, u32, [f32; 4]) {
    match face_texture {
        FaceTextures::Simple(b) => (
            *b, 0,
            [1.0, 1.0, 1.0, 1.0],
        ),
        FaceTextures::Bilayer(b, o) => (
            *b, *o,
            [1.0, 1.0, 1.0, 1.0],
        ),
        FaceTextures::Tinted(b, o, color) => (
            *b, *o,
            // Convert Bevy Color to a linear [f32; 4] for the shader
            {
                let c = color.to_linear();
                [c.red, c.green, c.blue, c.alpha]
            },
        ),
    }
}

pub fn resolve_slots(appearance: &BlockAppearance, shape: &BlockShape) -> Vec<TexureSlot> {
    match shape {
        // Pipes: 3 slots (core, arm, cap). For now they all take the block's
        // one texture. If you later want a distinct cap, this is the only
        // place that changes.
        BlockShape::Pipe => {
            let ft = resolve_face_texture(appearance, Direction::Up, false);
            let s: TexureSlot = resolve_texture_properties(ft).into();
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

