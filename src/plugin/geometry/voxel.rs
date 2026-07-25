use bevy::math::IVec3;
use bevy::prelude::*;
use serde::Deserialize;

use crate::plugin::geometry::rotation::BlockRotation;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – VOXEL DATA
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single voxel packed into 32 bits.
///
/// ```text
///  31              21 20    16 15               0
///  ┌─────────────────┬────────┬──────────────────┐
///  │      state      │ rotate │     block id     │
///  │      11 b       │  5 b   │       16 b       │
///  └─────────────────┴────────┴──────────────────┘
/// ```
///
/// ## What changed, and why
///
/// **`shape` is gone.** It used to occupy four bits so the mesher could
/// pick geometry without consulting the registry. That saved nothing: the
/// mesher already does `registry.get(block_id)` for every non-air voxel to
/// fetch appearance and collision, so the definition is in cache by the
/// time geometry is needed. All the bits bought was the possibility of a
/// voxel and its definition disagreeing about what block it is. Shape is
/// now purely an authoring-time input to the model baker, and a half-slab
/// is its own `BlockID`.
///
/// **`facing` (3 b) became `rotation` (5 b).** Six facings could only say
/// "point this shape at that wall", which is why `Stair` and `StairInv`
/// had to be separate shapes. The full 24-element rotation group of the
/// cube makes orientation a property of the voxel rather than of the shape
/// catalogue, so upside-down stairs, horizontal slopes and sideways panels
/// all come out of one canonical model for free.
///
/// **`state` grew from 9 to 11 bits.** A pipe spends six on its connection
/// mask and has five left over.
///
/// A raw value of `0` is still unconditionally **air**, so empty chunks
/// stay trivially cheap and rotation 0 is the identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub struct Voxel(u32);

pub const ID_BITS:       u32 = 16;
pub const ROTATION_BITS: u32 = 5;
pub const STATE_BITS:    u32 = 11;

const ROTATION_SHIFT: u32 = ID_BITS;                    // 16
const STATE_SHIFT:    u32 = ID_BITS + ROTATION_BITS;    // 21

const ID_MASK:       u32 = (1 << ID_BITS) - 1;          // 0x0000_FFFF
const ROTATION_MASK: u32 = (1 << ROTATION_BITS) - 1;    // 0x1F
const STATE_MASK:    u32 = (1 << STATE_BITS) - 1;       // 0x7FF

impl Voxel {
    pub const AIR: Self = Self(0);

    // ---- constructors ---------------------------------------------------

    #[inline]
    pub fn new(id: u16, rotation: BlockRotation, state: u16) -> Self {
        Self(
            (id as u32)
                | ((rotation.raw() as u32 & ROTATION_MASK) << ROTATION_SHIFT)
                | ((state as u32 & STATE_MASK) << STATE_SHIFT),
        )
    }

    /// Shorthand: unrotated, stateless. The overwhelming majority of the
    /// world.
    #[inline]
    pub fn full(id: u16) -> Self { Self(id as u32) }

    #[inline]
    pub fn from_raw(raw: u32) -> Self { Self(raw) }

    #[inline]
    pub fn raw(self) -> u32 { self.0 }

    // ---- accessors ------------------------------------------------------

    #[inline] pub fn is_air(self) -> bool { self.0 == 0 }
    #[inline] pub fn id(self)     -> u16  { (self.0 & ID_MASK) as u16 }

    #[inline]
    pub fn rotation(self) -> BlockRotation {
        BlockRotation::from_raw(((self.0 >> ROTATION_SHIFT) & ROTATION_MASK) as u8)
    }

    #[inline]
    pub fn state(self) -> u16 {
        ((self.0 >> STATE_SHIFT) & STATE_MASK) as u16
    }

    /// Everything the model table keys off, in one read. Used by the
    /// mesher's dirty check: if this is unchanged, geometry is unchanged.
    #[inline]
    pub fn variant_bits(self) -> u32 { self.0 >> ROTATION_SHIFT }

    // ---- builders (non-mutating) ----------------------------------------

    #[inline]
    pub fn with_rotation(self, r: BlockRotation) -> Self {
        Self(
            (self.0 & !(ROTATION_MASK << ROTATION_SHIFT))
                | ((r.raw() as u32 & ROTATION_MASK) << ROTATION_SHIFT),
        )
    }

    #[inline]
    pub fn with_state(self, state: u16) -> Self {
        Self(
            (self.0 & !(STATE_MASK << STATE_SHIFT))
                | ((state as u32 & STATE_MASK) << STATE_SHIFT),
        )
    }

    /// Same block, different appearance. The pipe reconnection path.
    ///
    /// Worth its own name because `voxel_write_obs` needs to distinguish
    /// it: a state-only edit must dirty the mesh **without** decomposing
    /// into Break + Place, or reconnecting a pipe would tear down its
    /// block entity and take its network membership with it.
    #[inline]
    pub fn is_same_block(self, other: Self) -> bool {
        self.id() == other.id()
    }
}

// ---------------------------------------------------------------------------
// NOTE: `covers_face` is deliberately gone.
//
// Occlusion is now a property of the *model*, derived at bake time by
// rasterising each boundary face, and read as `arena.model(id).occludes(d)`.
// A hand-written match per shape cannot survive imported geometry — nobody
// can eyeball whether a Blockbench mesh tiles its north boundary.
// ---------------------------------------------------------------------------

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – DIRECTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Unchanged from the current definition, and still the vocabulary for
/// neighbours, cull tags and connection masks.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Deserialize)]
pub enum Direction {
    North = 0,   // -Z
    South = 1,   // +Z
    East  = 2,   // +X
    West  = 3,   // -X
    Up    = 4,   // +Y
    Down  = 5,   // -Y
}

impl Direction {
    #[inline]
    pub fn as_ivec3(self) -> IVec3 {
        match self {
            Direction::North => IVec3::new(0, 0, -1),
            Direction::South => IVec3::new(0, 0, 1),
            Direction::East  => IVec3::new(1, 0, 0),
            Direction::West  => IVec3::new(-1, 0, 0),
            Direction::Up    => IVec3::new(0, 1, 0),
            Direction::Down  => IVec3::new(0, -1, 0),
        }
    }

    #[inline]
    pub fn as_vec3(self) -> Vec3 {
        match self {
            Direction::North => Vec3::new(0., 0., -1.),
            Direction::South => Vec3::new(0., 0., 1.),
            Direction::East  => Vec3::new(1., 0., 0.),
            Direction::West  => Vec3::new(-1., 0., 0.),
            Direction::Up    => Vec3::new(0., 1., 0.),
            Direction::Down  => Vec3::new(0., -1., 0.),
        }
    }

    #[inline]
    pub fn opposite(self) -> Self {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East  => Direction::West,
            Direction::West  => Direction::East,
            Direction::Up    => Direction::Down,
            Direction::Down  => Direction::Up,
        }
    }
}

/// Authoring-time geometry family. **Not** stored in the voxel any more —
/// it lives on `BlockDefinition` and tells the baker which generator to
/// run. Kept because the JSON loader and the item-icon picker both key off
/// it.
///
/// `StairInv` is gone: it is `Stair` under a rotation.
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_budget_is_exact() {
        assert_eq!(ID_BITS + ROTATION_BITS + STATE_BITS, 32);
    }

    #[test]
    fn air_is_zero() {
        assert!(Voxel::AIR.is_air());
        assert!(!Voxel::full(1).is_air());
        assert_eq!(Voxel::AIR.rotation(), BlockRotation::IDENTITY);
    }

    #[test]
    fn fields_are_independent() {
        let r = BlockRotation::from_parts(Direction::East, 2);
        let v = Voxel::new(0xBEEF, r, 0x5A5);
        assert_eq!(v.id(), 0xBEEF);
        assert_eq!(v.rotation(), r);
        assert_eq!(v.state(), 0x5A5);
    }

    #[test]
    fn builders_do_not_disturb_neighbours() {
        let v = Voxel::full(42).with_state(0x3FF);
        let r = BlockRotation::from_parts(Direction::Down, 1);
        let w = v.with_rotation(r);
        assert_eq!(w.id(), 42);
        assert_eq!(w.state(), 0x3FF);
        assert_eq!(w.rotation(), r);
    }

    #[test]
    fn state_saturates_at_eleven_bits() {
        let v = Voxel::full(1).with_state(0xFFFF);
        assert_eq!(v.state(), STATE_MASK as u16);
        assert_eq!(v.id(), 1);
    }
}

