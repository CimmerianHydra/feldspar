use bevy::audio::AudioSource;
use bevy::prelude::*;

use crate::content::block::behaviors::BlockBehaviors;
use crate::voxel::{BlockShape, ModelTable};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE DEFINITION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A fully resolved, immutable description of one block type.
/// Created once at startup, lives in the global `BlockRegistry`.
///
/// Note what the fields point at: `ModelTable` is a table of `ModelID`s, and
/// `ModelID` is a vocabulary index. The definition names its geometry without
/// content ever depending on the renderer that bakes it — `render::mesh::bake`
/// fills `models` in as a later pass over this same registry.
pub struct BlockDefinition {
    pub name:           String,             // e.g. "ore_iron_andesite"
    pub display_name:   String,             // e.g. "Andesite Iron Ore"
    pub shape:          BlockShape,
    pub appearance:     BlockAppearance,
    /// Resolved and baked models, one per geometric variant.
    pub models:         ModelTable,
    /// Resolved texture palette, indexed by a quad's slot number.
    pub texture_slots:  Vec<TextureSlot>,
    pub has_collision:  bool,
    pub material:       BlockMaterial,
    pub sound_profile:  SoundProfile,
    pub behaviors: BlockBehaviors,
}

impl BlockDefinition {
    pub fn air() -> BlockDefinition {
        BlockDefinition {
            name: "air".to_string(),
            display_name: "Air".to_string(),
            ..default()
        }
    }
}

impl Default for BlockDefinition {
    fn default() -> Self {
        BlockDefinition {
            name: "default_cube".to_string(),
            display_name: "Default Cube".to_string(),
            shape: BlockShape::default(),
            appearance: BlockAppearance::default(),
            models: ModelTable::default(),
            texture_slots: Vec::new(),
            has_collision: true,
            material: BlockMaterial::default(),
            sound_profile: SoundProfile::default(),
            behaviors: BlockBehaviors::default(),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – APPEARANCE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug)]
pub enum FaceTextures {
    /// Index into the base texture array.
    Simple(u32),
    /// Has both texture and (non-tinted) overlay.
    Bilayer(u32, u32),
    /// First index is into base texture array, second into overlay array.
    /// Color is used to tint the overlay texture.
    Tinted(u32, u32, Color),
}

/// How a block distributes textures across its faces. Authoring-side; the
/// renderer never sees it, because `texture_slots` is resolved from it once
/// at load.
pub enum BlockAppearance {
    /// All six faces use the same texture. Default choice.
    Uniform(FaceTextures),
    /// Top/bottom differ from sides.
    TopBotSide {
        up:    FaceTextures,
        down:  FaceTextures,
        side:  FaceTextures,
    },
    PerFace {
        up:    FaceTextures,
        down:  FaceTextures,
        north: FaceTextures,
        south: FaceTextures,
        east:  FaceTextures,
        west:  FaceTextures,
    },
    /// All six faces use the same texture, but an "internal" texture is
    /// defined for all those faces that don't sit on the boundary of the
    /// voxel.
    UniformWithInternal {
        ext:    FaceTextures,
        int:    FaceTextures,
    },
    /// Surfaces of an imported model, keyed by the name the model gives
    /// them. Resolution order comes from the model file, not from here.
    Model(std::collections::BTreeMap<String, ModelSurface>),
}

impl Default for BlockAppearance {
    fn default() -> Self {
        BlockAppearance::Uniform(FaceTextures::Simple(1))
    }
}

/// One paintable surface of an imported model.
#[derive(Clone, Debug)]
pub struct ModelSurface {
    pub textures: FaceTextures,
    /// `None` inherits the block-wide default.
    pub render:   Option<RenderClass>,
}

/// One paintable surface, resolved down to what the shader wants.
///
/// A quad carries a slot index; this is what that index points at. The
/// indirection is what makes materials free: iron, bronze and wood pipes are
/// three definitions with three slot tables sharing one set of baked models.
#[derive(Clone, Copy)]
pub struct TextureSlot {
    pub base_layer:    u32,
    pub overlay_layer: u32,
    pub tint:          [f32; 4],
    pub class:         RenderClass,
}

impl TextureSlot {
    /// The fallback for a surface nobody painted.
    pub const MISSING: Self = Self {
        base_layer: 0,
        overlay_layer: 0,
        tint: [1.0, 1.0, 1.0, 1.0],
        class: RenderClass::Opaque,
    };

    pub fn with_class(mut self, class: RenderClass) -> Self {
        self.class = class;
        self
    }
}

impl From<(u32, u32, [f32; 4])> for TextureSlot {
    fn from((base_layer, overlay_layer, tint): (u32, u32, [f32; 4])) -> Self {
        Self { base_layer, overlay_layer, tint, class: RenderClass::default() }
    }
}

use serde::Deserialize;

/// Which pipeline a surface draws through.
///
/// Alpha mode is pipeline state, not fragment state — opaque and alpha-tested
/// geometry are queued into different phases with different depth behaviour,
/// so no single draw call can be both. This enum is where that split is
/// named; the mesher buckets on it and emits one mesh per class.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderClass {
    #[default]
    Opaque,
    /// Alpha-tested. Still writes depth, so unlike blending it needs no
    /// back-to-front sorting — which is why cutouts belong here and not in a
    /// future `Blend` variant.
    Mask,
}

pub const RENDER_CLASS_COUNT: usize = 2;

impl RenderClass {
    pub const ALL: [RenderClass; RENDER_CLASS_COUNT] =
        [RenderClass::Opaque, RenderClass::Mask];

    #[inline]
    pub fn index(self) -> usize { self as usize }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – MATERIAL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Default)]
pub enum ToolType {
    #[default]
    Pick,
    Hammer,
    Wrench,
    Shovel,
    Weapon,
}

#[derive(Default)]
pub struct BlockMaterial {
    pub name:           String,
    /// How long it takes to break.
    pub hardness:       f32,
    /// Mass of a full block of this material, useful for physics.
    pub mass_of_block:  f32,
    /// Required tool type to initiate breaking the material.
    pub required_tool:  Option<ToolType>,
    /// Tier-gating so that only certain materials can break certain others.
    pub hardness_tier:  u32,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – SOUND
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Per-block sound bundle. `None` means "this block makes no sound for this
/// action" — air uses the default and is silent everywhere.
///
/// The handles live on the definition; playing them is
/// [`crate::audio`]'s job, and it observes `BlockEvent` to do it.
#[derive(Clone, Default)]
pub struct SoundProfile {
    pub on_place: Option<Handle<AudioSource>>,
    pub on_break: Option<Handle<AudioSource>>,
}
