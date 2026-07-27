use bevy::math::{Vec2, Vec3};

use crate::render::mesh::element::{BakedQuad, CullTag, Element, ElementBox, FaceSpec};
use crate::voxel::{dir_index, pipe_slots, slots, Direction, ALL_DIRECTIONS, FACE_SLOTS};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 0 – CONVENTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The authoring grid. Everything here is authored in sixteenths and
/// converted once, which keeps every vertex on the lattice the future
/// integer vertex format will quantise to.
const U: f32 = 1.0 / 16.0;

#[inline]
const fn px(n: f32) -> f32 { n * U }

#[inline]
fn v(x: f32, y: f32, z: f32) -> Vec3 { Vec3::new(x, y, z) }

/// A box whose boundary faces take their matching directional slot and
/// whose interior faces take the interior slot.
///
/// This is the rule that makes every primitive below a one-liner: you say
/// where the box is, and the correct texture slots follow from which of
/// its faces happen to reach the voxel boundary.
fn slotted_box(min: Vec3, max: Vec3) -> ElementBox {
    let mut b = ElementBox::per_face(min, max, FACE_SLOTS);
    for (i, d) in ALL_DIRECTIONS.iter().enumerate() {
        let flush = match d {
            Direction::Up    => (max.y - 1.0).abs() < 1e-4,
            Direction::Down  => min.y.abs() < 1e-4,
            Direction::North => min.z.abs() < 1e-4,
            Direction::South => (max.z - 1.0).abs() < 1e-4,
            Direction::East  => (max.x - 1.0).abs() < 1e-4,
            Direction::West  => min.x.abs() < 1e-4,
        };
        if !flush {
            b.faces[i] = Some(FaceSpec::slot(slots::INTERIOR));
        }
    }
    b
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – PRIMITIVES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// Each shape is authored **once**, in a canonical orientation. Every other
// orientation is `arena.bake(elements, rotation)`.
//
// That is the whole reason `StairInv` no longer needs to exist: an
// upside-down stair is the canonical stair under a rotation that maps +Y
// to -Y, and the baker produces it mechanically.

/// The full cube. One model, shared by every cube-shaped block in the game
/// once the arena deduplicates.
pub fn cube() -> Vec<Element> {
    vec![Element::Box(slotted_box(Vec3::ZERO, Vec3::ONE))]
}

/// Thin panel, default 1/16. Rotations give wall and ceiling panels.
pub fn panel(thickness_px: f32) -> Vec<Element> {
    vec![Element::Box(slotted_box(Vec3::ZERO, v(1.0, px(thickness_px), 1.0)))]
}

/// Stair, canonical: bottom half solid, upper step against the north face.
///
/// Two boxes. The old `Stair`/`StairInv` split is gone — inverted stairs
/// are this model under a rotation.
pub fn stair() -> Vec<Element> {
    vec![
        Element::Box(slotted_box(Vec3::ZERO, v(1.0, 0.5, 1.0))),
        Element::Box(slotted_box(v(0.0, 0.5, 0.0), v(1.0, 1.0, 0.5))),
    ]
}

/// Slope, canonical: rises from the south face to the north face.
///
/// The one primitive that cannot be a box, so it is authored as raw quads.
/// Its two side faces are triangles that sit *exactly on* the east and
/// west voxel boundaries — which is precisely the case the sampling-based
/// occlusion test exists to get right. They must not be reported as
/// covering, and they are not.
pub fn slope() -> Vec<Element> {
    let mut quads = Vec::new();

    // Bottom (full, on the boundary).
    quads.push(BakedQuad {
        verts: [v(0., 0., 1.), v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.)],
        uvs:   [Vec2::new(0., 0.), Vec2::new(0., 1.), Vec2::new(1., 1.), Vec2::new(1., 0.)],
        normal: Vec3::NEG_Y,
        cull:   CullTag::from_dir(Direction::Down),
        slot:   slots::DOWN,
    });

    // Back wall, full height on the north face.
    quads.push(BakedQuad {
        verts: [v(1., 1., 0.), v(1., 0., 0.), v(0., 0., 0.), v(0., 1., 0.)],
        uvs:   [Vec2::new(0., 0.), Vec2::new(0., 1.), Vec2::new(1., 1.), Vec2::new(1., 0.)],
        normal: Vec3::NEG_Z,
        cull:   CullTag::from_dir(Direction::North),
        slot:   slots::NORTH,
    });

    // The ramp itself. Normal is the diagonal, hence not axis-aligned.
    let n = Vec3::new(0.0, 1.0, 1.0).normalize();
    quads.push(BakedQuad {
        verts: [v(1., 1., 0.), v(0., 1., 0.), v(0., 0., 1.), v(1., 0., 1.)],
        uvs:   [Vec2::new(0., 0.), Vec2::new(1., 0.), Vec2::new(1., 1.), Vec2::new(0., 1.)],
        normal: n,
        cull:   CullTag::INTERIOR,
        slot:   slots::UP,
    });

    // Side triangles, degenerate-quad convention (verts[3] == verts[2]).
    quads.push(BakedQuad {
        verts: [v(1., 1., 0.), v(1., 0., 1.), v(1., 0., 0.), v(1., 0., 0.)],
        uvs:   [Vec2::new(0., 0.), Vec2::new(1., 1.), Vec2::new(0., 1.), Vec2::new(0., 1.)],
        normal: Vec3::X,
        cull:   CullTag::from_dir(Direction::East),
        slot:   slots::EAST,
    });
    quads.push(BakedQuad {
        verts: [v(0., 1., 0.), v(0., 0., 0.), v(0., 0., 1.), v(0., 0., 1.)],
        uvs:   [Vec2::new(1., 0.), Vec2::new(1., 1.), Vec2::new(0., 1.), Vec2::new(0., 1.)],
        normal: Vec3::NEG_X,
        cull:   CullTag::from_dir(Direction::West),
        slot:   slots::WEST,
    });

    // Stepped collider approximation. Four steps is plenty for walking on;
    // swap for a Parry convex hull later if it matters.
    let steps = 4;
    let collider = (0..steps)
        .map(|i| {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            [v(0.0, 0.0, 1.0 - t1), v(1.0, t1, 1.0 - t0)]
        })
        .collect();

    vec![Element::Raw { quads, collider }]
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – PIPES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Half-width of an arm, in sixteenths. 4 gives an 8/16 tube.
const ARM_HALF: f32 = 4.0;
/// Half-width of the central junction. Slightly fatter.
const CORE_HALF: f32 = 5.0;

/// Elements for one pipe connection mask.
///
/// `mask` is one bit per direction in `ALL_DIRECTIONS` order — the same
/// six bits the voxel's state field carries, so the variant key is a
/// straight passthrough.
///
/// ## Intra-model culling
///
/// Where an arm meets the core, both surfaces are hidden. That is resolved
/// *here*, by omitting the faces, rather than by any neighbour lookup —
/// the two boxes are part of the same model, so it is a purely local fact
/// and it costs nothing at runtime. Saves two quads per arm, twelve on a
/// fully-connected junction.
///
/// ## Why this is not special
///
/// This function is the entire "pipes are a hard block" problem. Sixty
/// lines, no mesher changes, no runtime cost. Fences, walls, cables and
/// conduits are the same function with different numbers.
pub fn pipe(mask: u8) -> Vec<Element> {
    let mut elements = Vec::with_capacity(7);

    let c0 = px(8.0 - CORE_HALF);
    let c1 = px(8.0 + CORE_HALF);

    // ---- the junction core ----------------------------------------------
    // Faces are dropped wherever an arm covers them.
    let mut core = ElementBox::uniform(v(c0, c0, c0), v(c1, c1, c1), pipe_slots::CORE);
    for i in 0..6 {
        if mask & (1 << i) != 0 {
            core.faces[i] = None;
        }
    }
    elements.push(Element::Box(core));

    // ---- one arm per set bit --------------------------------------------
    let a0 = px(8.0 - ARM_HALF);
    let a1 = px(8.0 + ARM_HALF);

    for (i, d) in ALL_DIRECTIONS.iter().enumerate() {
        if mask & (1 << i) == 0 { continue; }

        // Runs from the core's surface out to the voxel boundary.
        let (min, max) = match d {
            Direction::North => (v(a0, a0, 0.0), v(a1, a1, c0)),
            Direction::South => (v(a0, a0, c1), v(a1, a1, 1.0)),
            Direction::East  => (v(c1, a0, a0), v(1.0, a1, a1)),
            Direction::West  => (v(0.0, a0, a0), v(c0, a1, a1)),
            Direction::Up    => (v(a0, c1, a0), v(a1, 1.0, a1)),
            Direction::Down  => (v(a0, 0.0, a0), v(a1, c0, a1)),
        };

        let mut arm = ElementBox::uniform(min, max, pipe_slots::ARM);

        // The opening at the block boundary gets the cap texture...
        arm.faces[i] = Some(FaceSpec::slot(pipe_slots::CAP));
        // ...and the face buried in the core is not drawn at all.
        arm.faces[dir_index(d.opposite())] = None;

        elements.push(Element::Box(arm));
    }

    elements
}

/// All 64 masks, in variant-table order.
pub fn all_pipe_variants() -> Vec<Vec<Element>> {
    (0u8..64).map(pipe).collect()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconnected_pipe_is_just_the_core() {
        assert_eq!(pipe(0).len(), 1);
    }

    #[test]
    fn fully_connected_pipe_is_core_plus_six_arms() {
        assert_eq!(pipe(0b111111).len(), 7);
    }

    /// The plus sign from above: north, south, east, west.
    #[test]
    fn cross_pipe_has_four_arms() {
        let mask = (1 << dir_index(Direction::North))
            | (1 << dir_index(Direction::South))
            | (1 << dir_index(Direction::East))
            | (1 << dir_index(Direction::West));
        assert_eq!(pipe(mask).len(), 5);
    }

    /// A fully connected core contributes no quads at all — every one of
    /// its faces is covered by an arm.
    #[test]
    fn fully_connected_core_draws_nothing() {
        let elements = pipe(0b111111);
        let Element::Box(core) = &elements[0] else { panic!("core must be a box") };
        assert!(core.faces.iter().all(|f| f.is_none()));
    }

    #[test]
    fn arms_reach_the_boundary() {
        for (i, d) in ALL_DIRECTIONS.iter().enumerate() {
            let elements = pipe(1 << i);
            let Element::Box(arm) = &elements[1] else { panic!() };
            let reaches = match d {
                Direction::Up    => arm.max.y == 1.0,
                Direction::Down  => arm.min.y == 0.0,
                Direction::North => arm.min.z == 0.0,
                Direction::South => arm.max.z == 1.0,
                Direction::East  => arm.max.x == 1.0,
                Direction::West  => arm.min.x == 0.0,
            };
            assert!(reaches, "{d:?} arm stops short of the voxel boundary");
        }
    }
}
