use bevy::prelude::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – VOXEL DATA  (voxel.rs)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single voxel packed into 32 bits.
///
/// Bit layout:
/// ```
///  31       23       20  19    16  15               0
///  ┌────────┬─────────┬──────────┬──────────────────┐
///  │ state  │ facing  │  shape   │     block id     │
///  │  9 b   │   3 b   │   4 b    │      16 b        │
///  └────────┴─────────┴──────────┴──────────────────┘
/// ```
///
/// - **block id** (bits 0-15):  up to 65 535 distinct block types.
/// - **shape**    (bits 16-19): geometry variant (Full, Slab, Stair, Slope …).
/// - **facing**   (bits 20-22): orientation (North/South/East/West/Up/Down).
/// - **state**    (bits 23-31): reserved for extra flags (powered, waterlogged,
///                              light level, etc.) – expand as needed.
///
/// A raw value of `0` is unconditionally **air** (id=0, AIR block, no shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub struct Voxel(u32);

impl Voxel {
    pub const AIR: Self = Self(0);

    // ---- constructors -------------------------------------------------------

    pub fn new(id: u16, shape: BlockShape, facing: Direction) -> Self {
        Self(
            (id as u32)
                | ((shape.as_u32()) << 16)
                | ((facing as u32) << 20),
        )
    }

    /// Shorthand: full-cube block, facing North (default orientation).
    #[inline]
    pub fn full(id: u16) -> Self {
        Self::new(id, BlockShape::Cube, Direction::North)
    }

    // ---- accessors ----------------------------------------------------------

    #[inline] pub fn is_air(self)    -> bool       { self.0 == 0 }
    #[inline] pub fn id(self)  -> u16        { (self.0 & 0x0000_FFFF) as u16 }

    #[inline]
    pub fn shape(self) -> BlockShape {
        let value = (self.0 >> 16) & 0xF;
        BlockShape::from_u32(value)
    }

    #[inline]
    pub fn facing(self) -> Direction {
        match (self.0 >> 20) & 0x7 {
            1 => Direction::South,
            2 => Direction::East,
            3 => Direction::West,
            4 => Direction::Up,
            5 => Direction::Down,
            _ => Direction::North,
        }
    }

    /// Returns `true` if this voxel's face in direction `dir` fully covers
    /// the voxel boundary on that side.
    ///
    /// Used by the mesher: a neighbour's face is culled when
    /// `neighbour.covers_face(face_dir.opposite())` is true.
    pub fn covers_face(self, dir: Direction) -> bool {
        if self.is_air() { return false; }
        match self.shape() {
            BlockShape::Cube => true,

            BlockShape::Slab => matches!(
                (self.facing(), dir),
                (Direction::Up,    Direction::Down)  |   // bottom slab: solid floor
                (Direction::Down,  Direction::Up)    |   // top slab: solid ceiling
                (Direction::North, Direction::North) |   // vertical slabs: solid wall
                (Direction::South, Direction::South) |
                (Direction::East,  Direction::East)  |
                (Direction::West,  Direction::West)
            ),

            BlockShape::Stair => {
                // Always has a full bottom face.
                // Also has a full back wall (opposite the open/walkable side).
                dir == Direction::Down || dir == self.facing().opposite()
            }

            BlockShape::StairInv => {
                // Always has a full top face.
                // Also has a full back wall (opposite the step side).
                dir == Direction::Up || dir == self.facing().opposite()
            }

            BlockShape::Slope => {
                // Full bottom face, and a full vertical wall on the high side.
                dir == Direction::Down || dir == self.facing()
            }

            _ => false,
        }
    }

    /// Calculates whether the specified face occludes the given coverage.
    /// Uses the voxel's own direction and shape where needed.
    pub fn occludes(self, face: Direction, coverage: FaceCoverage) -> bool {
        let coverage_of_this_voxel = self.shape().default_coverage()[0];
        return coverage_of_this_voxel.is_covered(coverage)
    }

    /// Extra state bits (bits 23-31).
    #[inline] pub fn state(self) -> u16 { ((self.0 >> 23) & 0x1FF) as u16 }

    // ---- builders (non-mutating) -------------------------------------------

    pub fn with_facing(self, facing: Direction) -> Self {
        Self((self.0 & !(0x7 << 20)) | ((facing as u32) << 20))
    }

    pub fn with_shape(self, shape: BlockShape) -> Self {
        Self((self.0 & !(0xF << 16)) | ((shape.as_u32()) << 16))
    }

    pub fn with_state(self, state: u16) -> Self {
        Self((self.0 & !(0x1FF << 23)) | (((state as u32) & 0x1FF) << 23))
    }
}

/// FaceCoverage is a bitmask. It represents how a face covers the space for culling
/// purposes; half slabs cover only half the space for example, while full faces cover
/// the entire space.
/// 
/// It enables slicing the face of blocks into 16 pieces which allows for great
/// flexibility in how faces are culled. In the future this may be extended to 64 slices.
/// The slices are ordered so that 0b00000001 means coverage happens on the top left
/// slice, while 0b10000000 on the bottom right slice. This is assuming one sees the face
/// head-on.

#[derive(Copy, Clone, Debug)]
pub struct FaceCoverage(u16);

impl FaceCoverage {
    pub const FULL: Self = FaceCoverage(0b11111111);
    pub const NONE: Self = FaceCoverage(0b00000000);
    pub const HALF_BOT: Self = FaceCoverage(0b11110000);
    pub const HALF_TOP: Self = FaceCoverage(0b00001111);
    pub const HALF_LFT: Self = FaceCoverage(0b00110011);
    pub const HALF_RGT: Self = FaceCoverage(0b11001100);

    pub fn from_shape(shape: BlockShape, facing: Direction, face_dir: Direction) -> Self {
        match shape {
            BlockShape::Cube        =>      FaceCoverage::FULL,
            _                       =>      FaceCoverage::FULL,
        }
    }

    /// Calculates coverage: true when the first quad covers the second.
    pub fn covers(self, coverage: Self) -> bool {
        return (self.0 | coverage.0) == self.0;
    }

    /// Calculates coverage: true when the second quad covers the first.
    pub fn is_covered(self, coverage: Self) -> bool {
        return (self.0 | coverage.0) == coverage.0;
    }
}

// ---------------------------------------------------------------------------

/// Geometry variant of a block.
///
/// `shape` + `facing` together fully determine the rendered geometry:
///
/// | Shape | Facing Up/Down | Facing N/S/E/W |
/// |-------|----------------|----------------|
/// | Slab  | Horizontal top/bottom slab | Vertical slab on that wall |
/// | Stair | (unused)       | Open side faces that direction |
/// | Slope | Uphill toward Up/Down | Uphill toward that cardinal |

#[repr(u32)]
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum BlockShape {
    /// Fills the entire voxel cube.
    Cube,
    /// Half-block slab. Facing determines placement.
    Slab,
    /// L-shaped stair.  Facing = the direction opposite to the open/walkable side faces.
    Stair,
    /// L-shaped stair upside down.
    StairInv,
    /// 45 ° ramp / slope. Facing = direction of the uphill edge.
    Slope,
    /// Custom
    Custom(Handle<Mesh>),
}

impl BlockShape {
    fn as_u32(self) -> u32 {
        match self {
            BlockShape::Cube        => 0,
            BlockShape::Slab        => 1,
            BlockShape::Stair       => 2,
            BlockShape::StairInv    => 3,
            BlockShape::Slope       => 4,
            _ => 0,
        }
    }

    fn from_u32(value: u32) -> BlockShape {
        match value {
            0 => BlockShape::Cube,
            1 => BlockShape::Slab,
            2 => BlockShape::Stair,
            3 => BlockShape::StairInv,
            4 => BlockShape::Slope,
            _ => BlockShape::Cube,
        }
    }

    /// Returns the default (facing North) coverage of each shape.
    fn default_coverage(self) -> [FaceCoverage; 6] {
        match self {
            BlockShape::Cube        =>      [FaceCoverage::FULL; 6],
            BlockShape::Slab        =>      [FaceCoverage::NONE,
                                             FaceCoverage::FULL,
                                             FaceCoverage::HALF_RGT,
                                             FaceCoverage::HALF_LFT,
                                             FaceCoverage::HALF_TOP,
                                             FaceCoverage::HALF_TOP,],
            _                       =>      [FaceCoverage::NONE; 6],
        }
    }
}

impl Default for BlockShape {
    fn default() -> Self {
        BlockShape::Cube
    }
}

// ---------------------------------------------------------------------------

/// Cardinal + vertical directions used for block orientation.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum Direction {
    North = 0, // −Z
    South = 1, // +Z
    East  = 2, // +X
    West  = 3, // −X
    Up    = 4, // +Y
    Down  = 5, // −Y
}

impl Direction {
    /// Unit vector in Bevy's right-hand Y-up coordinate space.
    pub fn as_ivec3(self) -> IVec3 {
        match self {
            Self::North => IVec3::NEG_Z,
            Self::South => IVec3::Z,
            Self::East  => IVec3::X,
            Self::West  => IVec3::NEG_X,
            Self::Up    => IVec3::Y,
            Self::Down  => IVec3::NEG_Y,
        }
    }

    /// Unit vector in Bevy's right-hand Y-up coordinate space.
    pub fn as_vec3(self) -> Vec3 {
        match self {
            Self::North => Vec3::NEG_Z,
            Self::South => Vec3::Z,
            Self::East  => Vec3::X,
            Self::West  => Vec3::NEG_X,
            Self::Up    => Vec3::Y,
            Self::Down  => Vec3::NEG_Y,
        }
    }

    /// Index associated to each direction.
    pub fn as_u32(self) -> u32 {
        match self {
            Self::North => 0,
            Self::South => 1,
            Self::East  => 2,
            Self::West  => 3,
            Self::Up    => 4,
            Self::Down  => 5,
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::North => Self::South, Self::South => Self::North,
            Self::East  => Self::West,  Self::West  => Self::East,
            Self::Up    => Self::Down,  Self::Down  => Self::Up,
        }
    }

    pub fn closest(direction: Vec3) -> Self {
        let abs = direction.abs();
        if abs.x > abs.y && abs.x > abs.z { if direction.x > 0.0 { Self::East } else { Self::West }}
        else if abs.y > abs.x && abs.y > abs.z { if direction.y > 0.0 { Self::Up } else { Self::Down }}
        else { if direction.z > 0.0 { Self::South } else { Self::North }}
    } 

    pub const ALL: [Self; 6] = [
        Self::North, Self::South, Self::East, Self::West, Self::Up, Self::Down,
    ];
}




// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 8 – PLUGIN  (plugin.rs)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct VoxelPlugin;

impl Plugin for VoxelPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<Voxel>()
            .register_type::<BlockShape>()
            .register_type::<Direction>();
    }
}