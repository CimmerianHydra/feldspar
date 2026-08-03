use serde::Deserialize;

use crate::voxel::direction::{dir_index, Direction};
use crate::voxel::rotation::BlockRotation;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ROTATION SETS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// How many orientations of a model are worth putting in the arena.
//
// This is a *baking* budget, not a gameplay constraint. Which rotations a
// block can actually be placed in is decided by the placement code; if it
// only ever writes three, the other 21 table entries are simply never
// indexed. Keeping the two decisions apart means a log's placement logic
// and a log's model never have to agree about anything.
//
// For the primitive shapes the budget hardly matters — `shapes::cube()`
// takes no per-block input, so all 24 of its bakes are shared by every cube
// block in the game, about 13 KB once, for the whole game. It matters for
// imported models, where each `Custom` has its own element list and 24
// orientations of a 200-quad piece is 4800 quads nothing else uses.

/// The reference face whose image decides "which way this model points".
///
/// Down, to match `ItemExtractor`'s canonical output — an unrotated model
/// faces down, so a freshly-placed sided block behaves like a chute until
/// it's turned. Changing this changes which *spin* each `Facing` member
/// carries, not how many there are.
const FRONT: Direction = Direction::Down;

/// Which orientations of a model to bake.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationSet {
    /// One model, upright. The current behaviour of `Cube` and `Custom`.
    #[default]
    None,
    /// Three: one per axis, ignoring which end points where. Logs, pillars,
    /// anything with two identical ends.
    Axis,
    /// Four: horizontal turns only, `Up` stays up. Chess pieces, chairs,
    /// anything that sits on the floor and faces a compass direction.
    Yaw,
    /// Six: one per face `FRONT` can land on, at a canonical spin. The
    /// common case for machines — extractors, furnaces, pumps.
    Facing,
    /// All 24. Only worth it for a model with no symmetry at all that also
    /// needs to be mountable at arbitrary spin.
    Full,
}

impl RotationSet {
    /// The members, in ascending raw order.
    ///
    /// Defined by *filtering* `BlockRotation::all()` rather than by
    /// constructing rotations from parts, so this file makes no assumptions
    /// about what `from_parts` means. The tests below pin the counts.
    pub fn rotations(self) -> Vec<BlockRotation> {
        match self {
            Self::None => vec![BlockRotation::IDENTITY],

            Self::Full => BlockRotation::all().into_iter().collect(),

            // Everything that leaves the vertical axis alone.
            Self::Yaw => BlockRotation::all()
                .into_iter()
                .filter(|r| r.apply_dir(Direction::Up) == Direction::Up)
                .collect(),

            // One representative per face `FRONT` can point at.
            Self::Facing => first_per_key(|r| dir_index(r.apply_dir(FRONT))),

            // One representative per *axis* `FRONT` can lie along — a
            // direction and its opposite share a key, so 6 collapses to 3.
            Self::Axis => first_per_key(|r| {
                let d = r.apply_dir(FRONT);
                dir_index(d).min(dir_index(d.opposite()))
            }),
        }
    }

    /// How many models this will bake. Cheap enough to just call
    /// `rotations().len()`, but this reads better at a call site deciding
    /// whether a table is worth it.
    pub fn len(self) -> usize {
        match self {
            Self::None => 1,
            Self::Axis => 3,
            Self::Yaw => 4,
            Self::Facing => 6,
            Self::Full => 24,
        }
    }

    /// Whether the model table needs the voxel's rotation bits at all.
    #[inline]
    pub fn uses_rotation(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// First rotation seen for each distinct key, preserving raw order.
///
/// "First" rather than "any" so the chosen representatives are stable
/// across runs and across compiler versions — a model table that shuffles
/// between builds would make save files render differently for no reason.
fn first_per_key(key: impl Fn(BlockRotation) -> usize) -> Vec<BlockRotation> {
    let mut seen = [false; 6];
    let mut out = Vec::new();

    for r in BlockRotation::all().into_iter() {
        let k = key(r);
        if !seen[k] {
            seen[k] = true;
            out.push(r);
        }
    }

    out
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The sets are derived, not enumerated, so this is the thing that
    /// catches a wrong `apply_dir` or `opposite` convention. If one of
    /// these fails, the bug is in the rotation algebra, not here.
    #[test]
    fn set_sizes_match_their_declared_lengths() {
        for set in [
            RotationSet::None,
            RotationSet::Axis,
            RotationSet::Yaw,
            RotationSet::Facing,
            RotationSet::Full,
        ] {
            assert_eq!(
                set.rotations().len(),
                set.len(),
                "{set:?} derived {} members but declares {}",
                set.rotations().len(),
                set.len()
            );
        }
    }

    #[test]
    fn members_are_distinct() {
        for set in [RotationSet::Axis, RotationSet::Yaw, RotationSet::Facing, RotationSet::Full] {
            let members = set.rotations();
            let unique: HashSet<u8> = members.iter().map(|r| r.raw()).collect();
            assert_eq!(unique.len(), members.len(), "{set:?} has duplicate members");
        }
    }

    #[test]
    fn facing_covers_every_direction() {
        let images: HashSet<usize> = RotationSet::Facing
            .rotations()
            .iter()
            .map(|r| dir_index(r.apply_dir(FRONT)))
            .collect();
        assert_eq!(images.len(), 6);
    }

    #[test]
    fn identity_is_always_the_first_member() {
        // `from_rotation_set` uses `models[0]` as the fallback for
        // unreachable table slots, so a corrupted voxel must land upright.
        for set in [RotationSet::Axis, RotationSet::Yaw, RotationSet::Facing, RotationSet::Full] {
            assert_eq!(set.rotations()[0], BlockRotation::IDENTITY, "{set:?}");
        }
    }
}