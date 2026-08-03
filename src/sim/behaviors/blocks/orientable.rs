use serde::Deserialize;

use crate::content::block::behaviors::{BlockBehavior, BlockBehaviors};
use crate::sim::behaviors::blocks::face_targeted::FaceTargeted;
use crate::voxel::{BlockRotation, Direction};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ORIENTABLE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// The whole behaviour: which orientations are legal, how a target face
// becomes a rotation, and the fact that being orientable implies being
// face-targeted. No entity, no systems — a stair can be wrenched with no
// block entity at all.
//
// The *dispatch* still lives in `player::interaction`, which reads this bag
// when the held item declares `orients_blocks`. Pulling that in here would
// mean this file owned an observer on the interaction event; that is the
// natural next step and nothing outside this file would have to change.

/// Which orientations this block is allowed to take.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrientationMode {
    /// All six. Machines with a single distinguished output face.
    #[default]
    Full,
    /// North/South/East/West. Anything with a fixed top — furnaces,
    /// workbenches, most player-facing UI machines.
    Yaw,
    /// The three axes. A log pointed "down" is a log pointed "up"; this
    /// folds the pair rather than rejecting it, so the click still lands.
    Axis,
}

/// Declares that this block responds to a configuring tool by reorienting.
///
/// Orientation lives in the voxel's rotation bits, not on the block entity.
/// That is deliberate and it is why a stair or a slope can be wrenched with
/// no entity at all, and why orientation survives chunk unload, save/load
/// and a grid splitting in half without anyone writing persistence code.
/// A block entity that caches a facing refreshes it from `BlockEvent::Rotate`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Orientable {
    /// Which of the block's *local* faces the player is aiming. `Up` is the
    /// natural default because `BlockRotation` enumerates by where local +Y
    /// lands, so `reference: Up` makes the maths a straight lookup.
    pub reference: Direction,
    pub mode: OrientationMode,
}

impl Default for Orientable {
    fn default() -> Self {
        Self { reference: Direction::Up, mode: OrientationMode::Full }
    }
}

impl BlockBehavior for Orientable {
    const NAME: &'static str = "orientable";

    /// **This is where the implication belongs.**
    ///
    /// An orientable block is addressed by target face, so it gets the grid.
    /// Resolving that here rather than at every read site means no consumer
    /// has to remember an `||`, and the next implication — whatever it turns
    /// out to be — touches this function and nothing else.
    fn apply(self, into: &mut BlockBehaviors) {
        into.push(FaceTargeted);
        into.push(self);
    }
}

impl Orientable {
    /// Fold a requested target into a legal one, or reject it.
    ///
    /// `None` means "this block cannot point there" — the highlight greys
    /// the cell out and the click is swallowed rather than silently doing
    /// something else.
    pub fn normalize(&self, target: Direction) -> Option<Direction> {
        match self.mode {
            OrientationMode::Full => Some(target),

            OrientationMode::Yaw => match target {
                Direction::Up | Direction::Down => None,
                horizontal => Some(horizontal),
            },

            // Fold each axis onto its positive representative.
            OrientationMode::Axis => Some(match target {
                Direction::Down => Direction::Up,
                Direction::West => Direction::East,
                Direction::North => Direction::South,
                positive => positive,
            }),
        }
    }

    /// The rotation this block should take to aim at `target`, or `None` if
    /// the target is illegal for its mode.
    ///
    /// Returns the *current* rotation unchanged when the block already
    /// points there, which the caller uses to skip a pointless voxel write.
    pub fn rotation_for(
        &self,
        current: BlockRotation,
        target: Direction,
    ) -> Option<BlockRotation> {
        let target = self.normalize(target)?;
        Some(current.point_toward(self.reference, target))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::behavior::BehaviorEntry;
    use crate::content::block::behaviors::BlockBehaviorRegistry;
    use serde_json::Value;

    fn entry(name: &str, data: Option<Value>) -> BehaviorEntry {
        BehaviorEntry { name: name.to_string(), data }
    }

    /// Partial config must fill the rest from `Default`, not from zero.
    #[test]
    fn partial_config_keeps_other_defaults() {
        let json: BehaviorEntry =
            serde_json::from_str(r#"{"orientable": {"mode": "horizontal"}}"#).unwrap();

        let value = json.data.unwrap();
        let orientable = Orientable::deserialize(&value).unwrap();

        assert_eq!(orientable.mode, OrientationMode::Yaw);
        assert_eq!(orientable.reference, Direction::Up);
    }

    /// A typo in a config key must fail loudly rather than be ignored.
    #[test]
    fn unknown_config_key_is_rejected() {
        let value: Value = serde_json::from_str(r#"{"referance": "North"}"#).unwrap();
        assert!(Orientable::deserialize(&value).is_err());
    }

    /// The implication lives in `apply`, so it must survive resolution.
    #[test]
    fn orientable_implies_face_targeted() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<Orientable>();

        let bag = registry.resolve(&[entry("orientable", None)], (), "test_block");

        assert!(bag.has::<Orientable>());
        assert!(bag.has::<FaceTargeted>());
    }

    /// …but not the other way around.
    #[test]
    fn face_targeted_does_not_imply_orientable() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<FaceTargeted>();

        let bag = registry.resolve(&[entry("face_targeted", None)], (), "test_block");

        assert!(bag.has::<FaceTargeted>());
        assert!(!bag.has::<Orientable>());
    }

    #[test]
    fn horizontal_mode_rejects_vertical_targets() {
        let orientable = Orientable {
            reference: Direction::Up,
            mode: OrientationMode::Yaw,
        };

        assert!(orientable.normalize(Direction::Up).is_none());
        assert!(orientable.normalize(Direction::Down).is_none());
        assert_eq!(orientable.normalize(Direction::North), Some(Direction::North));
    }

    #[test]
    fn axis_mode_folds_onto_positive_representatives() {
        let orientable = Orientable {
            reference: Direction::Up,
            mode: OrientationMode::Axis,
        };

        assert_eq!(orientable.normalize(Direction::Down), Some(Direction::Up));
        assert_eq!(orientable.normalize(Direction::West), Some(Direction::East));
        assert_eq!(orientable.normalize(Direction::North), Some(Direction::South));
    }
}
