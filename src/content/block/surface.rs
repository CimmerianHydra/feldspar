use bevy::prelude::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};

use crate::content::block::definition::{FaceTextures, RenderClass, TextureSlot};
use crate::content::model_def::{ModelDef, ModelDefRegistry};
use crate::content::model_source::ModelSourceRegistry;
use crate::voxel::shape::{surface_names, BlockShape, RESERVED_FALLBACK};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SURFACES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// One contract for every block in the game:
//
//   a shape declares an ordered list of surface names;
//   an appearance is a name -> texture map;
//   slot N is whatever the Nth name resolves to.
//
// That replaces five `BlockAppearance` variants, and it replaces them for a
// specific reason: every one of those variants was a *distribution rule* —
// uniform, top/bottom/side, per-face — and distribution rules are exactly
// what named surfaces make unnecessary. `TopBotSide` isn't a kind of
// appearance, it's three keys.
//
// The slot *indices* don't move. `slots::NORTH` is still 0 and
// `pipe_slots::CORE` is still 0; the mesher and the shape generators never
// learn that any of this happened. All that changes is that the resolver
// now knows what to call them.

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – ONE PAINTABLE SURFACE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What a block paints onto one named surface.
///
/// The old `ModelSurface`, promoted: it applies to a cube's north face
/// exactly as well as to an imported model's `bezel`, because after this
/// change there is no difference between the two.
#[derive(Clone, Debug)]
pub struct Surface {
    pub textures: FaceTextures,
    /// `None` inherits the block-wide default.
    pub render: Option<RenderClass>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – LAYOUTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The surface names a shape exposes, in slot order.
///
/// `Cow` because primitives know theirs at compile time and imported models
/// read theirs out of a `.model.json`. The distinction ends here — every
/// consumer past this point sees one list of names and cannot tell which
/// kind of shape produced it.
#[derive(Clone, Debug)]
pub struct SurfaceLayout {
    names: Vec<Cow<'static, str>>,
}

impl SurfaceLayout {
    /// Work out the layout for any shape.
    ///
    /// This is the *only* place a `Custom` shape is treated differently
    /// from a primitive, and even here the difference is just where the
    /// list of names is read from.
    pub fn resolve(
        shape: &BlockShape,
        defs: &ModelDefRegistry,
        sources: &ModelSourceRegistry,
        block_name: &str,
    ) -> Self {
        // Primitives carry their names in the vocabulary layer, next to the
        // slot constants they have to agree with.
        if let Some(names) = surface_names(shape) {
            return Self { names: names.iter().map(|n| Cow::Borrowed(*n)).collect() };
        }

        let BlockShape::Custom(key) = shape else {
            unreachable!("surface_names covers every shape except Custom");
        };

        Self::from_model(key, defs, sources, block_name)
    }

    /// Names declared by a model definition, with two fallbacks.
    ///
    /// A model with no `surfaces` list still has to resolve *something*, or
    /// every block using an unannotated `.bbmodel` breaks at once. Numbered
    /// names reproduce the old positional contract exactly: surface "0" is
    /// the model's first texture, which is what the importer already stamps
    /// as slot 0.
    fn from_model(
        key: &str,
        defs: &ModelDefRegistry,
        sources: &ModelSourceRegistry,
        block_name: &str,
    ) -> Self {
        let declared = defs.get(key).map(|def| def.surfaces.clone()).unwrap_or_default();

        if !declared.is_empty() {
            return Self { names: declared.into_iter().map(Cow::Owned).collect() };
        }

        // No `surfaces` list — fall back to the widest geometry file this
        // definition touches, named positionally.
        let count = match defs.get(key) {
            Some(def) => widest_document(def, sources),
            None => sources.get(key).map_or(0, |doc| doc.slot_count()),
        };

        if count == 0 {
            warn!(
                "Block '{block_name}' uses model '{key}', which declares no surfaces \
                 and whose geometry could not be measured. It will render untextured."
            );
        }

        Self { names: (0..count).map(|i| Cow::Owned(i.to_string())).collect() }
    }

    #[inline]
    pub fn names(&self) -> &[Cow<'static, str>] {
        &self.names
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

fn widest_document(def: &ModelDef, sources: &ModelSourceRegistry) -> usize {
    def.geometry_paths()
        .iter()
        .filter_map(|path| sources.get(path))
        .map(|doc| doc.slot_count())
        .max()
        .unwrap_or(0)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE FALLBACK CHAIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The keys a surface consults, in order. First hit wins.
///
/// Note what this is *not*: an expansion pass. `"all"` is never rewritten
/// into six entries and `"side"` is never rewritten into four. Each surface
/// asks for itself, then for the groups it belongs to, then for the
/// catch-all. Precedence therefore falls out of the ordering rather than
/// out of who wrote to the map last, which is why
///
///     { "up": …, "down": …, "all": … }
///
/// paints four sides from `all` without `all` ever having overwritten
/// anything.
///
/// The chain is a property of **the name**, not of the shape. An imported
/// model that calls its faces `north` and `up` gets `side` and `all`
/// shorthand for free, and its `.model.json` never had to say so.
pub fn fallback_chain(name: &str) -> impl Iterator<Item = &str> {
    let groups: &'static [&'static str] = match name {
        "north" | "south" | "east" | "west" => &["side", "sides", RESERVED_FALLBACK],
        "up" => &["top", RESERVED_FALLBACK],
        "down" => &["bottom", RESERVED_FALLBACK],
        "interior" => &["inside", RESERVED_FALLBACK],
        // Model surfaces, pipe parts, anything bespoke. `all` still
        // applies, so a one-texture model needs one key.
        _ => &[RESERVED_FALLBACK],
    };

    std::iter::once(name).chain(groups.iter().copied())
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – RESOLUTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Build the block's slot table: one entry per name in the layout.
///
/// Replaces the old `resolve_slots` *and* `resolve_model_slots`, which
/// existed as two functions only because appearances and models disagreed
/// about whether a surface was a direction or a name. They don't any more.
pub fn resolve_slots(
    layout: &SurfaceLayout,
    surfaces: &BTreeMap<String, Surface>,
    default_class: RenderClass,
    block_name: &str,
) -> Vec<TextureSlot> {
    let mut used: HashSet<&str> = HashSet::new();
    let mut unpainted: Vec<&str> = Vec::new();

    let slots = layout
        .names()
        .iter()
        .map(|name| {
            match lookup(surfaces, name) {
                Some((key, surface)) => {
                    used.insert(key);
                    to_slot(surface, default_class)
                }
                None => {
                    unpainted.push(name.as_ref());
                    TextureSlot::MISSING
                }
            }
        })
        .collect();

    report(block_name, layout, surfaces, &used, &unpainted);
    slots
}

fn lookup<'a>(
    surfaces: &'a BTreeMap<String, Surface>,
    name: &str,
) -> Option<(&'a str, &'a Surface)> {
    fallback_chain(name).find_map(|key| {
        surfaces.get_key_value(key).map(|(k, v)| (k.as_str(), v))
    })
}

fn to_slot(surface: &Surface, default_class: RenderClass) -> TextureSlot {
    let (base_layer, overlay_layer, tint) = match &surface.textures {
        FaceTextures::Simple(b) => (*b, 0, [1.0, 1.0, 1.0, 1.0]),
        FaceTextures::Bilayer(b, o) => (*b, *o, [1.0, 1.0, 1.0, 1.0]),
        FaceTextures::Tinted(b, o, color) => {
            let c = color.to_linear();
            (*b, *o, [c.red, c.green, c.blue, c.alpha])
        }
    };

    TextureSlot {
        base_layer,
        overlay_layer,
        tint,
        class: surface.render.unwrap_or(default_class),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – DIAGNOSTICS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Complain about keys nobody read and surfaces nobody painted.
///
/// The cheapest genuinely valuable part of this change. Under the old
/// scheme a mistyped `"nroth"` or a Blockbench surface renamed without
/// updating the block JSON produced a silent missing texture, discovered
/// visually, three files away from the mistake. Both are now named at load
/// with the block that caused them.
///
/// A fully shadowed key gets flagged too — `"side"` alongside four explicit
/// faces is dead configuration, and being told is better than wondering
/// which one wins.
fn report(
    block_name: &str,
    layout: &SurfaceLayout,
    surfaces: &BTreeMap<String, Surface>,
    used: &HashSet<&str>,
    unpainted: &[&str],
) {
    let ignored: Vec<&str> = surfaces
        .keys()
        .map(String::as_str)
        .filter(|key| !used.contains(key))
        .collect();

    if !ignored.is_empty() {
        warn!(
            "Block '{block_name}' declares surface key(s) {ignored:?} that nothing \
             reads. Its shape exposes {:?}. Either a typo, or a key shadowed by \
             more specific ones.",
            layout.names()
        );
    }

    // An empty map is a block that simply hasn't been painted yet, which is
    // worth one line rather than one line per surface.
    if !unpainted.is_empty() && !surfaces.is_empty() {
        warn!(
            "Block '{block_name}' leaves surface(s) {unpainted:?} unpainted — \
             add a key for each, or an '{RESERVED_FALLBACK}' key to cover them."
        );
    } else if surfaces.is_empty() {
        warn!("Block '{block_name}' has no surfaces at all and will render untextured.");
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::shape::slots;

    fn layout(names: &[&'static str]) -> SurfaceLayout {
        SurfaceLayout { names: names.iter().map(|n| Cow::Borrowed(*n)).collect() }
    }

    fn simple(layer: u32) -> Surface {
        Surface { textures: FaceTextures::Simple(layer), render: None }
    }

    fn map(pairs: &[(&str, u32)]) -> BTreeMap<String, Surface> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), simple(*v))).collect()
    }

    /// The basalt case from the design: two explicit faces, everything else
    /// from `all`.
    #[test]
    fn all_fills_only_what_is_left() {
        let l = SurfaceLayout::resolve_primitive_for_test();
        let s = map(&[("up", 10), ("down", 10), ("all", 20)]);
        let slots = resolve_slots(&l, &s, RenderClass::Opaque, "basalt");

        assert_eq!(slots[slots::UP as usize].base_layer, 10);
        assert_eq!(slots[slots::DOWN as usize].base_layer, 10);
        assert_eq!(slots[slots::NORTH as usize].base_layer, 20);
        assert_eq!(slots[slots::WEST as usize].base_layer, 20);
    }

    #[test]
    fn side_beats_all_and_loses_to_an_explicit_face() {
        let l = SurfaceLayout::resolve_primitive_for_test();
        let s = map(&[("north", 1), ("side", 2), ("all", 3)]);
        let slots = resolve_slots(&l, &s, RenderClass::Opaque, "t");

        assert_eq!(slots[slots::NORTH as usize].base_layer, 1);
        assert_eq!(slots[slots::SOUTH as usize].base_layer, 2);
        assert_eq!(slots[slots::UP as usize].base_layer, 3);
    }

    /// The old `TopBotSide` sent interior to `side`; the chain sends it to
    /// `all`. Both reproduce "interior looks like the rest of the block"
    /// for every appearance that existed, but the new one is explicit.
    #[test]
    fn interior_falls_through_to_all() {
        let l = SurfaceLayout::resolve_primitive_for_test();
        let s = map(&[("up", 1), ("side", 2), ("all", 3)]);
        let slots = resolve_slots(&l, &s, RenderClass::Opaque, "t");

        assert_eq!(slots[slots::INTERIOR as usize].base_layer, 3);
    }

    #[test]
    fn aliases_resolve() {
        let l = SurfaceLayout::resolve_primitive_for_test();
        let s = map(&[("top", 1), ("bottom", 2), ("sides", 3)]);
        let slots = resolve_slots(&l, &s, RenderClass::Opaque, "t");

        assert_eq!(slots[slots::UP as usize].base_layer, 1);
        assert_eq!(slots[slots::DOWN as usize].base_layer, 2);
        assert_eq!(slots[slots::EAST as usize].base_layer, 3);
    }

    /// A model surface named `side` is its own name, not the directional
    /// group — the extractor depends on this.
    #[test]
    fn model_surfaces_match_by_their_own_name() {
        let l = layout(&["side", "top", "bottom"]);
        let s = map(&[("side", 1), ("top", 2), ("bottom", 3)]);
        let slots = resolve_slots(&l, &s, RenderClass::Opaque, "item_extractor");

        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0].base_layer, 1);
        assert_eq!(slots[2].base_layer, 3);
    }

    #[test]
    fn one_key_paints_a_pipe() {
        let l = layout(&["core", "arm", "cap"]);
        let s = map(&[("all", 7)]);
        let slots = resolve_slots(&l, &s, RenderClass::Opaque, "pipe");

        assert!(slots.iter().all(|s| s.base_layer == 7));
    }

    #[test]
    fn unmatched_surfaces_fall_back_to_missing() {
        let l = SurfaceLayout::resolve_primitive_for_test();
        let s = map(&[("up", 1)]);
        let slots = resolve_slots(&l, &s, RenderClass::Opaque, "t");

        assert_eq!(slots[slots::NORTH as usize].base_layer, TextureSlot::MISSING.base_layer);
    }

    impl SurfaceLayout {
        fn resolve_primitive_for_test() -> Self {
            layout(&["north", "south", "east", "west", "up", "down", "interior"])
        }
    }
}