//! The 3×3 interaction grid on a block face.
//!
//! Clicking a face's centre addresses that face; clicking an edge cell
//! addresses the neighbouring face in that direction; clicking a corner
//! addresses the opposite face. Nine cells, six outcomes — which is the
//! whole point: every face of a block is reachable from any single exposed
//! face, so an extractor buried against a barrel can still be pointed into
//! it.
//!
//! ## Why this is vocabulary and not input
//!
//! Two very different consumers need it: `player::interaction` to decide
//! what a click means, and `render::highlight` to draw what the click will
//! do. If either derived its own in-plane basis they would eventually
//! disagree by a quarter turn and the overlay would lie. One definition,
//! two callers.
//!
//! ## The basis is space-local, never view-relative
//!
//! `FaceBasis` is a pure function of `Direction`. It does not know where
//! the player is standing, which means the grid does not flip as they walk
//! around a block, and — more importantly — a machine bolted to a rolling
//! ship resolves the same cell to the same space-local face regardless of
//! the ship's attitude. GregTech makes the same choice and players learn
//! it in about four clicks.
//!
//! The `right`/`up` axes below are derived from the mesher's UV convention
//! in `render::mesh::element::face_geometry`: `u` runs along `right`, and
//! `v` runs along `-up` (v = 0 at the top, as textures do). Keeping them
//! aligned means a textured overlay can replace the wireframe later without
//! any mirroring fixups.

use bevy::math::{Vec2, Vec3};

use crate::voxel::direction::Direction;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE IN-PLANE BASIS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The two in-plane axes of a face, oriented as the player reads them.
///
/// `right × up == face.as_vec3()` for all six faces — asserted in the
/// tests, because that is what lets the highlight build a rotation straight
/// out of `Mat3::from_cols(right, up, normal)` with no handedness fixup.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FaceBasis {
    /// Increasing `u`.
    pub right: Direction,
    /// Decreasing `v` — "up" as seen on the face.
    pub up: Direction,
}

impl Direction {
    /// The in-plane axes of this face.
    pub const fn face_basis(self) -> FaceBasis {
        match self {
            // Side faces: v runs with -y, so "up on the face" is world up.
            Direction::North => FaceBasis { right: Direction::West,  up: Direction::Up },
            Direction::South => FaceBasis { right: Direction::East,  up: Direction::Up },
            Direction::East  => FaceBasis { right: Direction::North, up: Direction::Up },
            Direction::West  => FaceBasis { right: Direction::South, up: Direction::Up },

            // Top and bottom have no natural up, so the convention is fixed:
            // +Z ("south") points toward the top of the grid on both.
            Direction::Up    => FaceBasis { right: Direction::West, up: Direction::South },
            Direction::Down  => FaceBasis { right: Direction::East, up: Direction::South },
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE CELL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Which ninth of a face was hit. `col` runs left→right, `row` top→bottom,
/// both in `0..3`.
///
/// Construct only through [`FaceCell::new`] or [`FaceCell::from_uv`]; both
/// clamp, because a hit exactly on the far edge multiplies to 3.0 and would
/// otherwise index off the end of the grid.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FaceCell {
    pub col: u8,
    pub row: u8,
}

/// The face-local centre of a cell, in `[-0.5, 0.5]²`. What the highlight
/// translates its indicator by.
pub const CELL_SIZE: f32 = 1.0/3.0;

impl FaceCell {
    /// Dead centre. The "no subdivision" answer.
    pub const CENTRE: Self = Self { col: 1, row: 1 };

    #[inline]
    pub fn new(col: u8, row: u8) -> Self {
        Self { col: col.min(2), row: row.min(2) }
    }

    /// From face UV in `[0,1]²`, v measured downward.
    #[inline]
    pub fn from_uv(uv: Vec2) -> Self {
        Self::new(
            (uv.x * 3.0).floor().clamp(0.0, 2.0) as u8,
            (uv.y * 3.0).floor().clamp(0.0, 2.0) as u8,
        )
    }

    /// From a hit point expressed in the *voxel's own* `[0,1]³` frame — i.e.
    /// the space-local hit point minus the block's integer coordinate.
    ///
    /// The projection is generic rather than a six-arm match: `u` is the
    /// signed offset along `right`, `v` the negated offset along `up`, both
    /// recentred into `[0,1]`. Add a face and nothing here changes.
    pub fn from_local_hit(point_in_voxel: Vec3, face: Direction) -> Self {
        Self::from_uv(face_uv(point_in_voxel, face))
    }

    #[inline]
    pub fn is_centre(self) -> bool {
        self.col == 1 && self.row == 1
    }

    #[inline]
    pub fn is_corner(self) -> bool {
        self.col != 1 && self.row != 1
    }

    /// Face-local centre of this cell in `[-0.5, 0.5]²`, `y` up. Feeds the
    /// highlight's cell indicator transform directly.
    #[inline]
    pub fn centre_offset(self) -> Vec2 {
        Vec2::new(
            (self.col as f32 - 1.0) * CELL_SIZE,
            (1.0 - self.row as f32) * CELL_SIZE,
        )
    }

    /// **The rule.** Which face this cell addresses, given the face that was
    /// actually struck.
    ///
    /// - centre → the struck face
    /// - edge   → the neighbour in that direction
    /// - corner → the opposite face
    pub fn resolve(self, face: Direction) -> Direction {
        let basis = face.face_basis();

        match (self.col, self.row) {
            (1, 1) => face,
            (1, 0) => basis.up,
            (1, 2) => basis.up.opposite(),
            (0, 1) => basis.right.opposite(),
            (2, 1) => basis.right,
            // All four corners.
            _ => face.opposite(),
        }
    }
}

/// Face UV of a point given in the voxel's own `[0,1]³` frame. `v` runs
/// downward, matching the mesher.
pub fn face_uv(point_in_voxel: Vec3, face: Direction) -> Vec2 {
    let basis = face.face_basis();
    let centred = point_in_voxel - Vec3::splat(0.5);

    Vec2::new(
        centred.dot(basis.right.as_vec3()) + 0.5,
        0.5 - centred.dot(basis.up.as_vec3()),
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use crate::voxel::direction::ALL_DIRECTIONS;

    /// The property the whole feature rests on: from any single exposed
    /// face, all six faces are addressable.
    #[test]
    fn every_face_reaches_every_direction() {
        for face in ALL_DIRECTIONS {
            let reached: HashSet<Direction> = (0..3)
                .flat_map(|col| (0..3).map(move |row| FaceCell::new(col, row)))
                .map(|cell| cell.resolve(face))
                .collect();

            assert_eq!(
                reached.len(),
                6,
                "face {face:?} only reaches {reached:?}"
            );
        }
    }

    /// Right-handedness is what lets the highlight build a `Quat` straight
    /// from the basis with no fixup.
    #[test]
    fn basis_is_right_handed() {
        for face in ALL_DIRECTIONS {
            let b = face.face_basis();
            let cross = b.right.as_vec3().cross(b.up.as_vec3());
            assert_eq!(
                cross,
                face.as_vec3(),
                "face {face:?}: right × up = {cross:?}, expected {:?}",
                face.as_vec3()
            );
        }
    }

    /// The basis axes must be perpendicular to the face itself, or an edge
    /// cell would resolve to the face it was clicked on.
    #[test]
    fn basis_axes_are_in_plane() {
        for face in ALL_DIRECTIONS {
            let b = face.face_basis();
            assert_ne!(b.right, face);
            assert_ne!(b.right, face.opposite());
            assert_ne!(b.up, face);
            assert_ne!(b.up, face.opposite());
            assert_ne!(b.up, b.right);
            assert_ne!(b.up, b.right.opposite());
        }
    }

    #[test]
    fn centre_resolves_to_the_struck_face() {
        for face in ALL_DIRECTIONS {
            assert_eq!(FaceCell::CENTRE.resolve(face), face);
        }
    }

    #[test]
    fn corners_resolve_to_the_opposite_face() {
        for face in ALL_DIRECTIONS {
            for (col, row) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
                assert_eq!(FaceCell::new(col, row).resolve(face), face.opposite());
            }
        }
    }

    /// A hit exactly on the far edge must not produce cell 3.
    #[test]
    fn uv_is_clamped_at_both_ends() {
        assert_eq!(FaceCell::from_uv(Vec2::new(1.0, 1.0)), FaceCell::new(2, 2));
        assert_eq!(FaceCell::from_uv(Vec2::new(0.0, 0.0)), FaceCell::new(0, 0));
        assert_eq!(FaceCell::from_uv(Vec2::new(-0.01, 1.01)), FaceCell::new(0, 2));
    }

    /// Round-trip: a point in the middle of the top-centre cell of the
    /// south face must resolve to Up.
    #[test]
    fn local_hit_round_trips_through_the_rule() {
        // South face is +Z, so z = 1. High on the face, horizontally centred.
        let point = Vec3::new(0.5, 5.0 / 6.0, 1.0);
        let cell = FaceCell::from_local_hit(point, Direction::South);
        assert_eq!(cell, FaceCell::new(1, 0));
        assert_eq!(cell.resolve(Direction::South), Direction::Up);
    }

    /// And the geometrically opposite one, to catch a v-flip.
    #[test]
    fn low_on_a_side_face_resolves_downward() {
        let point = Vec3::new(0.5, 1.0 / 6.0, 1.0);
        let cell = FaceCell::from_local_hit(point, Direction::South);
        assert_eq!(cell, FaceCell::new(1, 2));
        assert_eq!(cell.resolve(Direction::South), Direction::Down);
    }

    #[test]
    fn cell_centre_offsets_are_symmetric() {
        assert_eq!(FaceCell::CENTRE.centre_offset(), Vec2::ZERO);
        assert_eq!(
            FaceCell::new(0, 0).centre_offset(),
            -FaceCell::new(2, 2).centre_offset()
        );
    }
}