use std::borrow::Cow;

use bevy::prelude::*;
use serde::Deserialize;

use crate::voxel::direction::{dir_index, Direction, ALL_DIRECTIONS};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – SHAPE FAMILIES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Authoring-time geometry family. **Not** stored in the voxel — it lives on
/// `BlockDefinition` and tells the baker which generator to run. Vocabulary
/// rather than renderer state, because the JSON loader, the texture-slot
/// resolver and the item-icon picker all key off it.
///
/// `StairInv` does not exist: it is `Stair` under a rotation.
#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Deserialize)]
pub enum BlockShape {
    #[default]
    Cube,
    Slab,
    Panel,
    Stair,
    Slope,
    Pipe,
    /// Loaded from a Blockbench file. Baked through the same element IR as
    /// everything above, so the mesher cannot tell the difference.
    Custom(String),
}

impl BlockShape {
    /// The pair of suffixes a generated block inherits from its shape:
    /// `(snake_case, Display Case)`.
    ///
    /// `None` for `Cube`. The cube is the canonical member of a shape
    /// family and keeps the family's bare name — `slate`, not `slate_cube`,
    /// "Slate", not "Slate (Cube)". That also means every block JSON
    /// written before shape families existed keeps the exact name it had,
    /// so `by_name("stone")` in worldgen and recipes never breaks.
    ///
    /// Returning both suffixes from one match makes it impossible for the
    /// name and the label to disagree about whether a shape is suffixed.
    pub fn suffixes(&self) -> Option<(Cow<'_, str>, Cow<'_, str>)> {
        let pair = match self {
            BlockShape::Cube  => return None,
            BlockShape::Slab  => (Cow::Borrowed("slab"),  Cow::Borrowed("Slab")),
            BlockShape::Panel => (Cow::Borrowed("panel"), Cow::Borrowed("Panel")),
            BlockShape::Stair => (Cow::Borrowed("stair"), Cow::Borrowed("Stair")),
            BlockShape::Slope => (Cow::Borrowed("slope"), Cow::Borrowed("Slope")),
            BlockShape::Pipe  => (Cow::Borrowed("pipe"),  Cow::Borrowed("Pipe")),

            // A one-off mesh has no vocabulary of its own, so borrow the
            // model's file stem: "models/blocks/lantern.bbmodel" → "lantern".
            BlockShape::Custom(path) => {
                let stem = asset_stem(path);
                (Cow::Owned(stem.to_owned()), Cow::Owned(title_case(stem)))
            }
        };
        Some(pair)
    }

}

/// Last path segment with the first extension stripped. Handles both
/// separators — asset paths in this project are written with either.
pub fn asset_stem(path: &str) -> &str {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.split('.').next().unwrap_or(file)
}

/// "lantern_post" → "Lantern Post".
fn title_case(slug: &str) -> String {
    slug.split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – TEXTURE SLOT NUMBERING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Reserved terminal key of every fallback chain. A model may not name a
/// surface this, or it would shadow itself.
pub const RESERVED_FALLBACK: &str = "all";

pub mod slots {
    pub const NORTH: u8 = 0;
    pub const SOUTH: u8 = 1;
    pub const EAST:  u8 = 2;
    pub const WEST:  u8 = 3;
    pub const UP:    u8 = 4;
    pub const DOWN:  u8 = 5;
    /// Interior faces — the old `UniformWithInternal::int`. One extra slot
    /// rather than a special case in the texture resolver.
    pub const INTERIOR: u8 = 6;

    pub const PRIMITIVE_SLOT_COUNT: usize = 7;
    /// The primitive surface names, in slot order.
    ///
    /// This array and the constants above are the same fact written twice,
    /// which is why the test below asserts they agree. Adding a slot means
    /// adding a name.
    pub const NAMES: &[&str] = &[
        "north", "south", "east", "west", "up", "down", "interior",
    ];
}

/// Slots a pipe paints. Three, not six — a pipe has no meaningful "north
/// texture", it has a body and an opening.
pub mod pipe_slots {
    pub const CORE: u8 = 0;
    pub const ARM:  u8 = 1;
    /// The annular opening at the block boundary.
    pub const CAP:  u8 = 2;

    pub const PIPE_SLOT_COUNT: usize = 3;
    pub const NAMES: &[&str] = &["core", "arm", "cap"];
}

/// The six directional slots, in `ALL_DIRECTIONS` order.
pub const FACE_SLOTS: [u8; 6] = [
    slots::NORTH, slots::SOUTH, slots::EAST, slots::WEST, slots::UP, slots::DOWN,
];

/// The surfaces a shape exposes, in slot order.
///
/// `None` for `Custom`, whose names live in its `.model.json` — the
/// vocabulary layer can't see the model registry, and shouldn't.
pub fn surface_names(shape: &BlockShape) -> Option<&'static [&'static str]> {
    match shape {
        BlockShape::Pipe      => Some(pipe_slots::NAMES),
        BlockShape::Custom(_) => None,
        _                     => Some(slots::NAMES),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – CONNECTION MASKS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The pipe connection mask, as stored in the voxel's low six state bits.
///
/// Authored, never derived from neighbours — which is what keeps a pipe's
/// geometry a pure function of its own 32 bits, with no cross-chunk
/// dependency and no cascade when a neighbour changes.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct ConnectionMask(pub u8);

impl ConnectionMask {
    pub const NONE: Self = Self(0);

    #[inline]
    pub fn has(self, d: Direction) -> bool {
        self.0 & (1 << dir_index(d)) != 0
    }

    #[inline]
    pub fn with(self, d: Direction, on: bool) -> Self {
        let bit = 1 << dir_index(d);
        Self(if on { self.0 | bit } else { self.0 & !bit })
    }

    #[inline]
    pub fn toggled(self, d: Direction) -> Self {
        Self(self.0 ^ (1 << dir_index(d)))
    }

    #[inline]
    pub fn count(self) -> u32 { self.0.count_ones() }

    /// True when this pipe is a junction rather than a straight run or an
    /// endpoint. The pipe network's reduced graph keys off exactly this.
    #[inline]
    pub fn is_junction(self) -> bool { self.count() >= 3 }

    pub fn directions(self) -> impl Iterator<Item = Direction> {
        ALL_DIRECTIONS
            .into_iter()
            .enumerate()
            .filter_map(move |(i, d)| (self.0 & (1 << i) != 0).then_some(d))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_mask_roundtrips() {
        let m = ConnectionMask::NONE
            .with(Direction::North, true)
            .with(Direction::Up, true);
        assert!(m.has(Direction::North) && m.has(Direction::Up));
        assert!(!m.has(Direction::South));
        assert_eq!(m.count(), 2);
        assert!(!m.is_junction());
        assert!(m.toggled(Direction::North).count() == 1);
        assert_eq!(m.directions().count(), 2);
    }

    #[test]
    fn cube_keeps_the_family_name() {
        assert!(BlockShape::Cube.suffixes().is_none());
        let (slug, label) = BlockShape::Slab.suffixes().unwrap();
        assert_eq!(&*slug, "slab");
        assert_eq!(&*label, "Slab");
    }

    #[test]
    fn custom_shapes_borrow_their_file_stem() {
        let shape = BlockShape::Custom("models/blocks/lantern_post.bbmodel".into());
        let (slug, label) = shape.suffixes().unwrap();
        assert_eq!(&*slug, "lantern_post");
        assert_eq!(&*label, "Lantern Post");
    }

    #[test]
    fn slot_constants_agree_with_surface_names() {
        assert_eq!(slots::NAMES.len(), slots::PRIMITIVE_SLOT_COUNT);
        assert_eq!(slots::NAMES[slots::NORTH as usize], "north");
        assert_eq!(slots::NAMES[slots::UP as usize], "up");
        assert_eq!(slots::NAMES[slots::INTERIOR as usize], "interior");

        assert_eq!(pipe_slots::NAMES.len(), pipe_slots::PIPE_SLOT_COUNT);
        assert_eq!(pipe_slots::NAMES[pipe_slots::CAP as usize], "cap");
    }
}
