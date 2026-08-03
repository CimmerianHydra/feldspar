use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::{content::model_source::ModelSourceRegistry, voxel::{Direction, RotationSet}};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MODEL DEFINITIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// Blockbench exports geometry. It has no way to express how the *game*
// varies that geometry — which orientations exist, which state bits swap
// the model out, how a connecting block composes itself from arms. That
// information has to live somewhere, and a sibling JSON is the natural
// place, in the same way a shape set is a named list pointed at by many.
//
// Example — assets/templates/models/extractor.model.json:
// {
//   "model":     "extractor",
//   "geometry":  "models/blocks/extractor.bbmodel",
//   "rotations": "Facing"
// }
//
// Example — a lamp that swaps model on one state bit:
// {
//   "model":  "lamp",
//   "states": { "bits": 1, "variants": ["models/blocks/lamp_off.bbmodel",
//                                       "models/blocks/lamp_on.bbmodel"] }
// }
//
// Example — a pipe, composed per bit. Two files, sixty-four variants:
// {
//   "model":    "steel_pipe",
//   "surfaces": ["core", "arm", "cap"],
//   "states": {
//     "bits": 6,
//     "parts": [
//       { "geometry": "models/blocks/pipe_core.bbmodel" },
//       { "geometry": "models/blocks/pipe_arm.bbmodel", "when_bit": 0, "rotate": "North" },
//       { "geometry": "models/blocks/pipe_arm.bbmodel", "when_bit": 1, "rotate": "South" },
//       { "geometry": "models/blocks/pipe_arm.bbmodel", "when_bit": 2, "rotate": "East"  },
//       { "geometry": "models/blocks/pipe_arm.bbmodel", "when_bit": 3, "rotate": "West"  },
//       { "geometry": "models/blocks/pipe_arm.bbmodel", "when_bit": 4, "rotate": "Up"    },
//       { "geometry": "models/blocks/pipe_arm.bbmodel", "when_bit": 5, "rotate": "Down"  }
//     ]
//   }
// }
//
// The last one is the general case and the reason this file exists: it
// turns `shapes::pipe(mask)` from hardcoded Rust into data. Fences, walls,
// cables and conduits stop needing a generator each.
//
// DEFINITIONS ARE OPTIONAL. A block that names a `.bbmodel` path directly
// still works and means "one model, upright, no states" — exactly today's
// behaviour. Adding a definition is how you opt into more.

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – JSON-SHAPED MIRRORS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One `*.model.json` file.
#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct ModelDefinitionAsset {
    /// The key a block's `Custom(...)` shape names.
    pub model: String,

    /// The geometry, for the simple case. Ignored when `states` is present,
    /// which supplies its own.
    #[serde(default)]
    pub geometry: Option<String>,

    #[serde(default)]
    pub rotations: RotationSet,

    /// Paintable surface names, in slot order.
    ///
    /// Empty means "take the order from the single geometry file", which is
    /// the current contract and keeps existing models working. Declaring
    /// them explicitly is *required* once more than one file contributes
    /// geometry, because otherwise slot 0 means whatever the first file
    /// happened to call it.
    #[serde(default)]
    pub surfaces: Vec<String>,

    #[serde(default)]
    pub states: Option<StateSpec>,
}

/// How state bits select geometry.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum StateSpec {
    /// One whole model per state value. Needs `2^bits` entries.
    Variants { bits: u8, variants: Vec<String> },
    /// Compose from pieces, each present when its bit is set. Needs
    /// `bits + 1` files at most, not `2^bits`.
    Parts { bits: u8, parts: Vec<PartSpec> },
}

impl StateSpec {
    pub fn bits(&self) -> u8 {
        match self {
            Self::Variants { bits, .. } | Self::Parts { bits, .. } => *bits,
        }
    }
}

/// One piece of a composed model.
#[derive(Deserialize, Debug, Clone)]
pub struct PartSpec {
    pub geometry: String,
    /// Which state bit switches this piece on. Absent means always.
    #[serde(default)]
    pub when_bit: Option<u8>,
    /// Rotate the piece before composing, so one arm file serves six
    /// directions.
    #[serde(default)]
    pub rotate: Option<Direction>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – RESOLVED FORM
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What the baker consumes. Same data, but with the "no states" case folded
/// into the same shape as the others so `bake` has one code path.
#[derive(Debug, Clone)]
pub struct ModelDef {
    pub rotations: RotationSet,
    pub surfaces: Vec<String>,
    pub layout: ModelLayout,
}

#[derive(Debug, Clone)]
pub enum ModelLayout {
    Single(String),
    Variants { bits: u8, geometry: Vec<String> },
    Parts { bits: u8, parts: Vec<PartSpec> },
}

impl ModelDef {
    /// The stand-in for a block that names a `.bbmodel` directly.
    pub fn implicit(path: &str) -> Self {
        Self {
            rotations: RotationSet::None,
            surfaces: Vec::new(),
            layout: ModelLayout::Single(path.to_owned()),
        }
    }

    /// How many state bits participate in the model table.
    pub fn state_bits(&self) -> u8 {
        match &self.layout {
            ModelLayout::Single(_) => 0,
            ModelLayout::Variants { bits, .. } | ModelLayout::Parts { bits, .. } => *bits,
        }
    }

    /// Every geometry file this definition touches, deduplicated. The
    /// loader uses it to sanity-check references before bake time.
    pub fn geometry_paths(&self) -> Vec<&str> {
        let mut out: Vec<&str> = match &self.layout {
            ModelLayout::Single(path) => vec![path.as_str()],
            ModelLayout::Variants { geometry, .. } => {
                geometry.iter().map(String::as_str).collect()
            }
            ModelLayout::Parts { parts, .. } => {
                parts.iter().map(|p| p.geometry.as_str()).collect()
            }
        };
        out.dedup();
        out
    }
}

impl From<ModelDefinitionAsset> for ModelDef {
    fn from(src: ModelDefinitionAsset) -> Self {
        let layout = match src.states {
            Some(StateSpec::Variants { bits, variants }) => {
                ModelLayout::Variants { bits, geometry: variants }
            }
            Some(StateSpec::Parts { bits, parts }) => ModelLayout::Parts { bits, parts },
            None => ModelLayout::Single(src.geometry.clone().unwrap_or_default()),
        };

        Self { rotations: src.rotations, surfaces: src.surfaces, layout }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(AssetCollection, Resource)]
pub struct ModelDefinitionAssets {
    #[asset(path = "templates\\models", collection(typed))]
    pub files: Vec<Handle<ModelDefinitionAsset>>,
}

#[derive(Resource, Default)]
pub struct ModelDefRegistry {
    defs: HashMap<String, ModelDef>,
}

impl ModelDefRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&ModelDef> {
        self.defs.get(key)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

/// Must run before `bake_block_geometry_sys`, and after the model sources
/// are parsed so references can be checked while there's still a file name
/// to name in the warning.
pub fn populate_model_def_registry_sys(
    assets: Res<ModelDefinitionAssets>,
    parsed: Res<Assets<ModelDefinitionAsset>>,
    sources: Res<ModelSourceRegistry>,
    mut registry: ResMut<ModelDefRegistry>,
) {
    for handle in &assets.files {
        let Some(file) = parsed.get(handle) else {
            error!("Model definition asset not parsed before resolution — check loading state.");
            continue;
        };

        let key = file.model.clone();
        let def: ModelDef = file.clone().into();

        // ── validate ─────────────────────────────────────────────────────
        if let ModelLayout::Variants { bits, geometry } = &def.layout {
            let expected = 1usize << bits;
            if geometry.len() != expected {
                error!(
                    "Model '{key}': {bits} state bits needs {expected} variants, \
                     found {}. Skipped.",
                    geometry.len()
                );
                continue;
            }
        }

        if let ModelLayout::Parts { bits, parts } = &def.layout {
            if let Some(bad) = parts.iter().filter_map(|p| p.when_bit).find(|b| b >= bits) {
                error!("Model '{key}': part names bit {bad}, but only {bits} declared. Skipped.");
                continue;
            }
        }

        if matches!(&def.layout, ModelLayout::Single(p) if p.is_empty()) {
            error!("Model '{key}' declares neither 'geometry' nor 'states'. Skipped.");
            continue;
        }

        // Reported here rather than at bake time, so a typo names the file
        // that contains it.
        for path in def.geometry_paths() {
            if sources.get(path).is_none() {
                warn!("Model '{key}' references geometry '{path}', which is not in the \
                       model registry. Check it is under assets/models/blocks/.");
            }
        }

        if def.surfaces.is_empty() && def.geometry_paths().len() > 1 {
            warn!("Model '{key}' composes {} geometry files but declares no 'surfaces'. \
                   Slot numbering will follow whichever file loads first, which is not \
                   stable — declare them explicitly.", def.geometry_paths().len());
        }

        if registry.defs.insert(key.clone(), def).is_some() {
            warn!("Model definition '{key}' declared more than once — last wins.");
        }
    }

    info!("ModelDefRegistry: {} definitions.", registry.len());
}