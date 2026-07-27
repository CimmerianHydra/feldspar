use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::content::item::asset::parse_hex_color;
use crate::content::item::components::ItemComponents;
use crate::content::item::display::{DisplayLayer, ItemDisplay};
use crate::content::item::registry::{ItemDefinition, ItemKind, ItemRegistry};
use crate::content::item::MAX_STACK;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – BASIC DEFINITIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Three layers of data, from concrete to abstract:
//
//   PartTemplate — one SHAPE of thing (plate, rod, gem, plank...). Global
//                  namespace, single PartID space, deduplicated by name.
//   PartProfile  — a NAMED SET of part templates ("metal", "gem", "wooden",
//                  and later "tool", "weapon", "armor"...). One *.part.json
//                  file per profile; the parts are defined inline in it.
//   Substance    — points at one or more PROFILES (never at individual
//                  parts). The generator emits one item per part in the
//                  UNION of the substance's profiles.
//
// So "what can diamond become?" is answered by the gem profile, written
// once and shared by every gem. Making iron tool-grade later is one word
// in its profiles list, not a parts list edited on fifty metals.
//
// Example — assets/templates/parts/metal.part.json:
// {
//   "profile": "metal",
//   "parts": [
//     { "name": "ingot", "pattern": "{} Ingot", "sprite": "icons/parts/ingot.png" },
//     { "name": "plate", "pattern": "{} Plate", "sprite": "icons/parts/plate.png",
//       "overlay": "icons/parts/plate_shine.png" },
//     { "name": "gear",  "pattern": "{} Gear",  "sprite": "icons/parts/gear.png"  },
//     { "name": "rod",   "pattern": "{} Rod",   "sprite": "icons/parts/rod.png"   },
//     { "name": "dust",  "pattern": "{} Dust",  "sprite": "icons/parts/dust.png"  }
//   ]
// }
//
// assets/templates/substances/base.substance.json:
// [
//   { "name": "iron",    "display_name": "Iron",    "color": "#D8D8D8",
//     "tier": 2, "durability": 250, "mining_speed": 6.0, "damage": 2.0,
//     "profiles": ["metal"] },                    // later: ["metal", "tool", "weapon", "armor"]
//   { "name": "diamond", "display_name": "Diamond", "color": "#4AEDD9",
//     "tier": 3, "durability": 1500, "mining_speed": 8.0, "damage": 3.0,
//     "profiles": ["gem"] }
// ]
//
// DEDUPLICATION SEMANTICS (the fine print that matters):
//   * Parts are deduplicated BY NAME across profiles. "rod" in gem.part.json
//     and "rod" in metal.part.json resolve to the SAME PartID — which is what
//     recipe logic wants ("any rod" is one category). The first definition
//     encountered wins; a redefinition with different data logs a warning.
//     If a gem rod should LOOK different from a metal rod, that's a
//     different part — name it "gem_rod".
//   * A substance whose profiles overlap (metal + tool might both include
//     "rod") generates each part ONCE — the union, not the concatenation.
//   * Two files declaring the same profile name MERGE their parts into it.
//     Deliberate: an addon file can extend "metal" with exotic parts
//     without touching the base file.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SubstanceID(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PartID(pub u16);

/// A substance: the shared identity behind many generated items (and,
/// later, tool parts). Stats live HERE, not on the generated items —
/// rebalance a substance and every plate, gear, and tool made of it follows.
pub struct SubstanceDefinition {
    pub id:           SubstanceID,
    pub name:         String,       // "iron"
    pub display_name: String,       // "Iron"
    pub color:        Color,        // the tint applied to grayscale sprites
    pub tier:         u8,           // gates what a tool of this substance can mine
    pub durability:   u32,
    pub mining_speed: f32,
    pub damage:       f32,
    /// Part profile names ("metal", "gem", ...) whose union of parts is
    /// generated for this substance.
    pub profiles:     Vec<String>,
}

/// A part template: one entry per SHAPE of thing (plate, gear, rod...).
/// The sprite is authored in grayscale; the substance's color tints it.
/// Defined inline inside profile files, but lives in ONE global namespace.
pub struct PartTemplate {
    pub id:           PartID,
    pub name:         String,          // "plate"
    pub name_pattern: String,          // "{} Plate" → "Iron Plate"
    pub sprite:       String,          // grayscale base, tinted per substance
    /// Optional untinted layer drawn on top (specular highlights, outlines).
    pub overlay:      Option<String>,
    pub max_stack:    u16,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – REGISTRIES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Resource, Default)]
pub struct SubstanceRegistry {
    substances: Vec<SubstanceDefinition>,
    name_to_id: HashMap<String, SubstanceID>,
}

impl SubstanceRegistry {
    pub fn new() -> Self {
        Self { substances: Vec::new(), name_to_id: HashMap::new() }
    }

    pub fn get(&self, id: SubstanceID) -> &SubstanceDefinition {
        &self.substances[id.0 as usize]
    }

    pub fn by_name(&self, name: &str) -> Option<SubstanceID> {
        self.name_to_id.get(name).copied()
    }

    pub fn register(&mut self, def: SubstanceDefinition) -> SubstanceID {
        let id = SubstanceID(self.substances.len() as u16);
        let name = def.name.clone();
        self.substances.push(SubstanceDefinition { id, ..def });
        self.name_to_id.insert(name, id);
        id
    }

    pub fn iter(&self) -> impl Iterator<Item = &SubstanceDefinition> {
        self.substances.iter()
    }

    pub fn len(&self) -> usize { self.substances.len() }
    pub fn is_empty(&self) -> bool { self.substances.is_empty() }
}

/// All part templates (single PartID namespace) plus the named profiles
/// grouping them. Kept in one registry because a profile is nothing but a
/// named subset of the part list.
#[derive(Resource, Default)]
pub struct PartRegistry {
    parts:      Vec<PartTemplate>,
    name_to_id: HashMap<String, PartID>,
    /// Profile name → member parts, in declaration order.
    profiles:   HashMap<String, Vec<PartID>>,
}

impl PartRegistry {
    pub fn new() -> Self {
        Self {
            parts:      Vec::new(),
            name_to_id: HashMap::new(),
            profiles:   HashMap::new(),
        }
    }

    pub fn get(&self, id: PartID) -> &PartTemplate {
        &self.parts[id.0 as usize]
    }

    pub fn by_name(&self, name: &str) -> Option<PartID> {
        self.name_to_id.get(name).copied()
    }

    /// The members of a profile, or None if no file declared it.
    pub fn profile(&self, name: &str) -> Option<&[PartID]> {
        self.profiles.get(name).map(Vec::as_slice)
    }

    /// Get-or-create by name — THE entry point for profile loading, so the
    /// same part declared in several profiles collapses to one PartID.
    /// A redefinition with different data keeps the first and warns.
    pub fn register_or_get(&mut self, def: PartTemplate) -> PartID {
        if let Some(&id) = self.name_to_id.get(&def.name) {
            let existing = &self.parts[id.0 as usize];
            if existing.sprite != def.sprite
                || existing.name_pattern != def.name_pattern
                || existing.overlay != def.overlay
                || existing.max_stack != def.max_stack
            {
                warn!(
                    "Part '{}' redefined with different data — keeping the first \
                     definition. A part that should differ per profile needs a \
                     distinct name (e.g. 'gem_rod').",
                    def.name
                );
            }
            return id;
        }

        let id = PartID(self.parts.len() as u16);
        let name = def.name.clone();
        self.parts.push(PartTemplate { id, ..def });
        self.name_to_id.insert(name, id);
        id
    }

    /// Append members to a profile, creating it on first sight. Called once
    /// per file; the same profile name across files merges (addon files can
    /// extend "metal" without touching the base file).
    pub fn extend_profile(&mut self, profile: &str, members: impl IntoIterator<Item = PartID>) {
        let list = self.profiles.entry(profile.to_string()).or_default();
        for id in members {
            if !list.contains(&id) {
                list.push(id);
            }
        }
    }

    pub fn parts_len(&self) -> usize { self.parts.len() }
    pub fn profiles_len(&self) -> usize { self.profiles.len() }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – JSON SCHEMA & ASSET COLLECTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Deserialize, Asset, TypePath)]
pub struct SubstanceFileAsset(pub Vec<SubstanceJson>);

#[derive(Deserialize)]
pub struct SubstanceJson {
    pub name:         String,
    pub display_name: String,
    pub color:        String,       // "#RRGGBB"
    #[serde(default)] pub tier:         u8,
    #[serde(default)] pub durability:   u32,
    #[serde(default)] pub mining_speed: f32,
    #[serde(default)] pub damage:       f32,
    /// Part profile names, e.g. ["metal", "tool"].
    #[serde(default)] pub profiles:     Vec<String>,
}

/// One `*.part.json` file: one named profile with its parts defined inline.
#[derive(Deserialize, Asset, TypePath)]
pub struct PartProfileAsset {
    pub profile: String,
    pub parts:   Vec<PartJson>,
}

#[derive(Deserialize)]
pub struct PartJson {
    pub name:    String,
    pub pattern: String,
    pub sprite:  String,
    #[serde(default)]
    pub overlay: Option<String>,
    #[serde(default = "default_part_stack")]
    pub max_stack: u16,
}

fn default_part_stack() -> u16 { MAX_STACK }

#[derive(AssetCollection, Resource)]
pub struct SubstanceDefinitionAssets {
    #[asset(path = "templates/substances", collection(typed))]
    pub files: Vec<Handle<SubstanceFileAsset>>,
}

#[derive(AssetCollection, Resource)]
pub struct PartProfileAssets {
    #[asset(path = "templates/parts", collection(typed))]
    pub files: Vec<Handle<PartProfileAsset>>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – LOADING SYSTEMS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Parse profile files (parts + profile membership) and substance files
/// into the registries. Parts first, so nothing else here depends on file
/// order. Must run before item generation.
pub fn populate_substance_registries_sys(
    sub_assets:     Res<SubstanceDefinitionAssets>,
    sub_parsed:     Res<Assets<SubstanceFileAsset>>,
    profile_assets: Res<PartProfileAssets>,
    profile_parsed: Res<Assets<PartProfileAsset>>,
    mut substances: ResMut<SubstanceRegistry>,
    mut parts:      ResMut<PartRegistry>,
) {
    // ── Part profiles ──────────────────────────────────────────────────
    for handle in &profile_assets.files {
        let Some(file) = profile_parsed.get(handle) else { continue };

        let members: Vec<PartID> = file.parts.iter().map(|p| {
            parts.register_or_get(PartTemplate {
                id:           PartID(0),
                name:         p.name.clone(),
                name_pattern: p.pattern.clone(),
                sprite:       p.sprite.clone(),
                overlay:      p.overlay.clone(),
                max_stack:    p.max_stack,
            })
        }).collect();

        parts.extend_profile(&file.profile, members);
    }

    // ── Substances ─────────────────────────────────────────────────────
    for handle in &sub_assets.files {
        let Some(file) = sub_parsed.get(handle) else { continue };
        for s in &file.0 {
            if substances.by_name(&s.name).is_some() {
                warn!("Duplicate substance '{}' skipped.", s.name);
                continue;
            }
            // Unknown profiles are reported here, at load time, not at
            // generation time — a typo should scream early.
            for profile in &s.profiles {
                if parts.profile(profile).is_none() {
                    warn!("Substance '{}' references unknown part profile '{}'.",
                          s.name, profile);
                }
            }
            substances.register(SubstanceDefinition {
                id:           SubstanceID(0),
                name:         s.name.clone(),
                display_name: s.display_name.clone(),
                color:        parse_hex_color(&s.color),
                tier:         s.tier,
                durability:   s.durability,
                mining_speed: s.mining_speed,
                damage:       s.damage,
                profiles:     s.profiles.clone(),
            });
        }
    }

    info!("SubstanceRegistry: {} substances, {} parts across {} profiles.",
          substances.len(), parts.parts_len(), parts.profiles_len());
}

/// THE cross product: for each substance, take the UNION of its profiles'
/// parts and register one ordinary item per part. Runs after
/// `populate_substance_registries_sys` and the block→item pass, before
/// explicit JSON items.
pub fn generate_substance_items_sys(
    substances:   Res<SubstanceRegistry>,
    parts:        Res<PartRegistry>,
    mut items:    ResMut<ItemRegistry>,
    asset_server: Res<AssetServer>,
) {
    let before = items.len();

    for sub in substances.iter() {
        // Union of the substance's profiles, order-preserving: profiles may
        // overlap ("rod" in both metal and tool) — each part generates once.
        let mut seen: HashSet<PartID> = HashSet::new();
        let expanded = sub.profiles.iter()
            .filter_map(|profile| parts.profile(profile))   // typos already warned
            .flatten()
            .copied()
            .filter(|id| seen.insert(*id));

        for part_id in expanded {
            let part = parts.get(part_id);

            // "iron_plate" — deterministic, save-file-stable naming.
            let name = format!("{}_{}", sub.name, part.name);
            if items.by_name(name.clone()).is_some() {
                warn!("Generated item '{name}' collides with an existing item — skipped.");
                continue;
            }

            // Grayscale base × substance color, plus optional untinted overlay.
            let mut layers = vec![DisplayLayer {
                image: asset_server.load(part.sprite.clone()),
                tint:  sub.color,
            }];
            if let Some(overlay) = &part.overlay {
                layers.push(DisplayLayer::plain(asset_server.load(overlay.clone())));
            }

            items.register(ItemDefinition {
                name,
                display_name: part.name_pattern.replace("{}", &sub.display_name),
                max_stack:    part.max_stack,
                kind:         ItemKind::SubstancePart { part: part_id, substance: sub.id },
                display:      ItemDisplay::Layered { layers },
                components:   ItemComponents::default(),
            });
        }
    }

    info!("ItemRegistry: generated {} substance-part items ({} total).",
          items.len() - before, items.len());
}
