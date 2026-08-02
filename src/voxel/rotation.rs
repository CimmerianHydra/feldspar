use std::sync::LazyLock;

use bevy::math::{IVec3, Vec3};

use crate::voxel::direction::{dir_from_ivec3, dir_index, Direction, ALL_DIRECTIONS};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE ROTATION GROUP
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const ROTATION_COUNT: usize = 24;

/// A rotation of the cube, stored as the images of the three local basis
/// vectors. Every entry is in {-1, 0, 1} — these are signed permutation
/// matrices, so the arithmetic is exact.
///
/// Exactness is not a nicety. Model geometry lives on a 1/16 grid, and
/// grid alignment is what will let the vertex format compress to integers
/// later. A float rotation matrix would put vertices at 0.4999999 and
/// quietly destroy that property.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RotMatrix {
    /// Images of local +X, +Y, +Z respectively.
    pub cols: [IVec3; 3],
}

impl RotMatrix {
    pub const IDENTITY: Self = Self {
        cols: [IVec3::X, IVec3::Y, IVec3::Z],
    };

    /// Rotate a lattice vector. Pure integer arithmetic.
    #[inline]
    pub fn apply_ivec3(&self, v: IVec3) -> IVec3 {
        self.cols[0] * v.x + self.cols[1] * v.y + self.cols[2] * v.z
    }

    /// Rotate a direction vector (normal, axis). No translation applied.
    #[inline]
    pub fn apply_vec3(&self, v: Vec3) -> Vec3 {
        self.cols[0].as_vec3() * v.x
            + self.cols[1].as_vec3() * v.y
            + self.cols[2].as_vec3() * v.z
    }

    /// Rotate a point in the unit cube, about the cube's centre.
    ///
    /// Model space is `[0,1]³` with the origin at a corner, so a rotation
    /// has to be conjugated by a half-unit translation or the geometry
    /// leaves the voxel.
    #[inline]
    pub fn apply_point(&self, p: Vec3) -> Vec3 {
        self.apply_vec3(p - Vec3::splat(0.5)) + Vec3::splat(0.5)
    }

    /// Determinant. Always +1 for this table — asserted in the tests,
    /// because it is exactly what guarantees winding order survives baking
    /// and therefore that no rotated model needs a backface fixup.
    pub fn determinant(&self) -> i32 {
        let [a, b, c] = self.cols;
        a.x * (b.y * c.z - b.z * c.y) - b.x * (a.y * c.z - a.z * c.y)
            + c.x * (a.y * b.z - a.z * b.y)
    }
}

/// Rotate `v` by 90 degrees about the unit axis `a`, right-hand rule.
///
/// Rodrigues' formula specialised to theta = 90 degrees, where
/// `sin = 1` and `cos = 0`:  `v' = a x v + a (a . v)`.
/// Integer in, integer out.
#[inline]
fn rot90(a: IVec3, v: IVec3) -> IVec3 {
    a.cross(v) + a * a.dot(v)
}

/// The 24 rotations, generated rather than hand-written.
///
/// Enumeration is `(where local +Y lands) x (spin about that axis)`:
/// six choices times four spins. Index 0 is the identity, which matters
/// because `Voxel::AIR` and every unrotated block store rotation 0.
static ROTATIONS: LazyLock<[RotMatrix; ROTATION_COUNT]> = LazyLock::new(|| {
    // Up first, so index 0 comes out as the identity.
    const UP_TARGETS: [Direction; 6] = [
        Direction::Up,
        Direction::Down,
        Direction::North,
        Direction::South,
        Direction::East,
        Direction::West,
    ];

    let mut out = [RotMatrix::IDENTITY; ROTATION_COUNT];

    for (i, up) in UP_TARGETS.iter().enumerate() {
        let ey = up.as_ivec3();

        // Any axis perpendicular to `ey` works as the spin-0 reference;
        // the four spins then sweep out the whole stabiliser regardless.
        let mut ez = if ey.y != 0 { IVec3::Z } else { IVec3::Y };

        for spin in 0..4 {
            // Right-handed basis: ex = ey x ez keeps det = +1.
            let ex = ey.cross(ez);
            out[i * 4 + spin] = RotMatrix { cols: [ex, ey, ez] };
            ez = rot90(ey, ez);
        }
    }

    out
});

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE VOXEL-FACING HANDLE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A cube orientation, packed into the voxel's 5 rotation bits.
///
/// Replaces the old 3-bit `facing`. Six facings could only express "point
/// this shape at that wall", which is why `Stair` and `StairInv` had to be
/// separate shapes. The full group makes orientation a property of the
/// *voxel* rather than a property of the *shape catalogue*, so a single
/// canonical stair covers upright, inverted, and every horizontal variant.
///
/// Raw values 24-31 are unreachable through the constructors and clamp to
/// the identity on the way in, so a corrupted voxel renders wrong rather
/// than panicking in the mesher.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct BlockRotation(u8);

impl BlockRotation {
    pub const IDENTITY: Self = Self(0);

    /// From the packed voxel bits. Out-of-range values clamp to identity.
    #[inline]
    pub fn from_raw(raw: u8) -> Self {
        if (raw as usize) < ROTATION_COUNT { Self(raw) } else { Self::IDENTITY }
    }

    #[inline]
    pub fn raw(self) -> u8 { self.0 }

    /// Build from the human-legible pair: where the model's local "up"
    /// should point, and how many quarter-turns to spin about it.
    pub fn from_parts(up: Direction, spin: u8) -> Self {
        let base = match up {
            Direction::Up    => 0,
            Direction::Down  => 1,
            Direction::North => 2,
            Direction::South => 3,
            Direction::East  => 4,
            Direction::West  => 5,
        };
        Self(base * 4 + (spin & 0b11))
    }

    #[inline]
    fn matrix(self) -> &'static RotMatrix { &ROTATIONS[self.0 as usize] }

    #[inline]
    pub fn apply_point(self, p: Vec3) -> Vec3 { self.matrix().apply_point(p) }

    #[inline]
    pub fn apply_normal(self, n: Vec3) -> Vec3 { self.matrix().apply_vec3(n) }

    /// Where a face pointing `d` ends up. Used to rotate cull tags and to
    /// permute a model's occlusion mask without re-rasterising it.
    #[inline]
    pub fn apply_dir(self, d: Direction) -> Direction {
        dir_from_ivec3(self.matrix().apply_ivec3(d.as_ivec3()))
    }

    /// Permute a 6-bit occlusion mask through this rotation.
    pub fn apply_occlusion(self, mask: u8) -> u8 {
        let mut out = 0u8;
        for (i, d) in ALL_DIRECTIONS.iter().enumerate() {
            if mask & (1 << i) != 0 {
                out |= 1 << dir_index(self.apply_dir(*d));
            }
        }
        out
    }

    pub fn all() -> impl Iterator<Item = BlockRotation> {
        (0..ROTATION_COUNT as u8).map(BlockRotation)
    }

    /// Where the model's local +Y ended up. The inverse of the `up` half of
    /// [`BlockRotation::from_parts`].
    #[inline]
    pub fn up(self) -> Direction {
        match self.0 / 4 {
            0 => Direction::Up,
            1 => Direction::Down,
            2 => Direction::North,
            3 => Direction::South,
            4 => Direction::East,
            _ => Direction::West,
        }
    }

    /// Quarter-turns about the `up()` axis.
    #[inline]
    pub fn spin(self) -> u8 {
        self.0 & 0b11
    }

    /// The rotation that points local face `reference` at world-local
    /// `target`, changing as little else as possible.
    ///
    /// Four rotations satisfy the constraint — the spin about the target
    /// axis is free — so "as little else as possible" is the tie-break:
    /// pick the candidate that agrees with `self` on the most face images.
    /// A machine wrenched from north to east keeps its top on top instead
    /// of arriving in an arbitrary roll.
    ///
    /// Ties resolve to the lowest raw value, so this is deterministic and
    /// safe to run on both ends of a future network connection.
    ///
    /// Returns `self` unchanged if no candidate exists, which cannot happen
    /// for a proper rotation group but keeps the signature total.
    pub fn point_toward(self, reference: Direction, target: Direction) -> Self {
        let current = ALL_DIRECTIONS.map(|d| self.apply_dir(d));
        let mut best: Option<(usize, BlockRotation)> = None;

        for candidate in Self::all() {
            if candidate.apply_dir(reference) != target {
                continue;
            }

            let agreement = ALL_DIRECTIONS
                .iter()
                .enumerate()
                .filter(|(i, d)| candidate.apply_dir(**d) == current[*i])
                .count();

            // Strict `>` keeps the first (lowest raw) candidate on a tie.
            if best.is_none_or(|(score, _)| agreement > score) {
                best = Some((agreement, candidate));
            }
        }

        best.map_or(self, |(_, r)| r)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn identity_is_index_zero() {
        assert_eq!(*BlockRotation::IDENTITY.matrix(), RotMatrix::IDENTITY);
    }

    /// If two entries collide the group is malformed and some orientations
    /// would be unreachable.
    #[test]
    fn all_rotations_distinct() {
        let set: HashSet<_> = ROTATIONS.iter().map(|m| m.cols).collect();
        assert_eq!(set.len(), ROTATION_COUNT);
    }

    /// Determinant +1 everywhere is what lets the baker skip winding fixups.
    #[test]
    fn all_rotations_proper() {
        for (i, m) in ROTATIONS.iter().enumerate() {
            assert_eq!(m.determinant(), 1, "rotation {i} is improper");
        }
    }

    /// Signed permutation matrices only: no entry outside {-1, 0, 1}, and
    /// exactly one non-zero per column.
    #[test]
    fn all_rotations_are_signed_permutations() {
        for m in ROTATIONS.iter() {
            for c in m.cols {
                let nz = [c.x, c.y, c.z].iter().filter(|v| **v != 0).count();
                assert_eq!(nz, 1);
                assert!([c.x, c.y, c.z].iter().all(|v| (-1..=1).contains(v)));
            }
        }
    }

    /// Rotations must map the six axis directions onto themselves,
    /// otherwise `apply_dir` would hit its debug assertion.
    #[test]
    fn directions_are_closed_under_rotation() {
        for r in BlockRotation::all() {
            for d in ALL_DIRECTIONS {
                let _ = r.apply_dir(d);
            }
        }
    }

    /// A point in the unit cube must stay in the unit cube.
    #[test]
    fn unit_cube_is_preserved() {
        for r in BlockRotation::all() {
            for p in [Vec3::ZERO, Vec3::ONE, Vec3::new(0.5, 0.0, 1.0)] {
                let q = r.apply_point(p);
                assert!(q.cmpge(Vec3::ZERO).all() && q.cmple(Vec3::ONE).all());
            }
        }
    }

    #[test]
    fn up_and_spin_invert_from_parts() {
        for up in ALL_DIRECTIONS {
            for spin in 0..4 {
                let r = BlockRotation::from_parts(up, spin);
                assert_eq!(r.up(), up);
                assert_eq!(r.spin(), spin);
            }
        }
    }

    /// The guarantee the wrench depends on.
    #[test]
    fn point_toward_actually_points() {
        for start in BlockRotation::all() {
            for reference in ALL_DIRECTIONS {
                for target in ALL_DIRECTIONS {
                    let r = start.point_toward(reference, target);
                    assert_eq!(
                        r.apply_dir(reference),
                        target,
                        "from {start:?}, {reference:?} → {target:?}"
                    );
                }
            }
        }
    }

    /// Re-aiming at the face you are already aimed at must be a no-op, or
    /// every wrench click would emit a spurious voxel write.
    #[test]
    fn point_toward_is_idempotent() {
        for start in BlockRotation::all() {
            for reference in ALL_DIRECTIONS {
                let target = start.apply_dir(reference);
                assert_eq!(start.point_toward(reference, target), start);
            }
        }
    }

    /// A horizontal re-aim keeps the block's top on top.
    #[test]
    fn point_toward_preserves_the_unaffected_axis() {
        let start = BlockRotation::IDENTITY;
        let r = start.point_toward(Direction::North, Direction::East);
        assert_eq!(r.apply_dir(Direction::North), Direction::East);
        assert_eq!(r.apply_dir(Direction::Up), Direction::Up);
    }
}
