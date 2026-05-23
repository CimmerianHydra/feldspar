use std::f32::consts::FRAC_1_SQRT_2;
use bevy::math::Vec3;
use crate::plugin::voxel::{BlockShape, Direction};

/// A single renderable quad in local block space [0, 1]³.
///
/// Triangular faces (e.g. the sides of a Slope) use a **degenerate quad**:
/// `verts[3] == verts[2]`. The index pattern (0,1,2),(0,2,3) then produces
/// one real triangle and one zero-area triangle that the GPU discards.
pub struct ShapeQuad {
    /// Vertices in CCW winding order as seen from outside the surface.
    pub verts:           [Vec3; 4],
    pub normal:          Vec3,
    /// Which direction does this face look toward?
    /// `None` = interior face (stair riser, slab inner wall) — always rendered.
    pub face_dir:        Option<Direction>,
    /// If `true`, this quad fully covers its side of the voxel boundary.
    /// Neighbors looking toward this face can cull their own face against it.
    pub covers_neighbor: bool,
}

// ── shorthand constructor ─────────────────────────────────────────────────────

fn q(verts: [Vec3; 4], normal: Vec3, face_dir: Option<Direction>, covers: bool) -> ShapeQuad {
    ShapeQuad { verts, normal, face_dir, covers_neighbor: covers }
}
fn v(x: f32, y: f32, z: f32) -> Vec3 { Vec3::new(x, y, z) }

// ── public API ────────────────────────────────────────────────────────────────

/// Returns all quads needed to render a block with the given shape and facing.
/// The quads are in local block space; the caller adds `pos.as_vec3()` to each vertex.
pub fn shape_quads(shape: BlockShape, facing: Direction) -> Vec<ShapeQuad> {
    match shape {
        BlockShape::Cube  => cube_quads(),
        BlockShape::Slab  => slab_quads(facing),
        BlockShape::Stair => rotate_to(stair_quads_north(), facing),
        BlockShape::StairInv => rotate_to(stair_inv_quads_north(), facing),
        BlockShape::Slope => rotate_to(slope_quads_north(), facing.opposite()),
        BlockShape::Custom(_handle) => cube_quads(),
    }
}

// ── Cube ──────────────────────────────────────────────────────────────────────

fn cube_quads() -> Vec<ShapeQuad> {
    vec![
        q([v(0.,1.,0.),v(0.,1.,1.),v(1.,1.,1.),v(1.,1.,0.)], Vec3::Y,      Some(Direction::Up),    true),
        q([v(0.,0.,1.),v(0.,0.,0.),v(1.,0.,0.),v(1.,0.,1.)], Vec3::NEG_Y,  Some(Direction::Down),  true),
        q([v(1.,0.,0.),v(0.,0.,0.),v(0.,1.,0.),v(1.,1.,0.)], Vec3::NEG_Z,  Some(Direction::North), true),
        q([v(0.,0.,1.),v(1.,0.,1.),v(1.,1.,1.),v(0.,1.,1.)], Vec3::Z,      Some(Direction::South), true),
        q([v(1.,0.,1.),v(1.,0.,0.),v(1.,1.,0.),v(1.,1.,1.)], Vec3::X,      Some(Direction::East),  true),
        q([v(0.,0.,0.),v(0.,0.,1.),v(0.,1.,1.),v(0.,1.,0.)], Vec3::NEG_X,  Some(Direction::West),  true),
    ]
}

// ── Slab ──────────────────────────────────────────────────────────────────────
//
// Facing convention (matches the `occludes` logic in voxel.rs):
//   Up    → bottom slab  y = [0.0, 0.5]   (sits on the floor)
//   Down  → top slab     y = [0.5, 1.0]   (hangs from ceiling)
//   N/S/E/W → vertical slab on that wall  z/x = [0.0, 0.5]

fn slab_quads(facing: Direction) -> Vec<ShapeQuad> {
    match facing {
        Direction::Up   => bottom_slab_quads(),
        Direction::Down => top_slab_quads(),
        _               => rotate_to(vertical_slab_north(), facing.opposite()),
    }
}

fn bottom_slab_quads() -> Vec<ShapeQuad> {
    // Occupies y = [0, 0.5].  Only the bottom face is a full boundary face.
    vec![
        q([v(0.,0.,1.),v(0.,0.,0.),v(1.,0.,0.),v(1.,0.,1.)], Vec3::NEG_Y, Some(Direction::Down),  true),
        // Inner face (y = 0.5) — always rendered, no neighbor to cull against.
        q([v(0.,0.5,0.),v(0.,0.5,1.),v(1.,0.5,1.),v(1.,0.5,0.)], Vec3::Y, None,   false),
        q([v(1.,0.,0.),v(0.,0.,0.),v(0.,0.5,0.),v(1.,0.5,0.)],    Vec3::NEG_Z, Some(Direction::North), false),
        q([v(0.,0.,1.),v(1.,0.,1.),v(1.,0.5,1.),v(0.,0.5,1.)],    Vec3::Z,     Some(Direction::South), false),
        q([v(1.,0.,1.),v(1.,0.,0.),v(1.,0.5,0.),v(1.,0.5,1.)],    Vec3::X,     Some(Direction::East),  false),
        q([v(0.,0.,0.),v(0.,0.,1.),v(0.,0.5,1.),v(0.,0.5,0.)],    Vec3::NEG_X, Some(Direction::West),  false),
    ]
}

fn top_slab_quads() -> Vec<ShapeQuad> {
    // Occupies y = [0.5, 1].  Only the top face is a full boundary face.
    vec![
        q([v(0.,1.,0.),v(0.,1.,1.),v(1.,1.,1.),v(1.,1.,0.)],       Vec3::Y,      Some(Direction::Up),   true),
        // Inner face (y = 0.5) — always rendered, no neighbor to cull against.
        q([v(0.,0.5,1.),v(0.,0.5,0.),v(1.,0.5,0.),v(1.,0.5,1.)],   Vec3::NEG_Y,  None, false),
        q([v(1.,0.5,0.),v(0.,0.5,0.),v(0.,1.,0.),v(1.,1.,0.)],      Vec3::NEG_Z, Some(Direction::North), false),
        q([v(0.,0.5,1.),v(1.,0.5,1.),v(1.,1.,1.),v(0.,1.,1.)],      Vec3::Z,     Some(Direction::South), false),
        q([v(1.,0.5,1.),v(1.,0.5,0.),v(1.,1.,0.),v(1.,1.,1.)],      Vec3::X,     Some(Direction::East),  false),
        q([v(0.,0.5,0.),v(0.,0.5,1.),v(0.,1.,1.),v(0.,1.,0.)],      Vec3::NEG_X, Some(Direction::West),  false),
    ]
}

fn vertical_slab_north() -> Vec<ShapeQuad> {
    // Canonical vertical slab: flush against the North wall, z = [0, 0.5].
    // Covers only the North boundary face.  Rotated for S/E/W variants.
    vec![
        q([v(1.,0.,0.),v(0.,0.,0.),v(0.,1.,0.),v(1.,1.,0.)],    Vec3::NEG_Z,  Some(Direction::North), true),
        // Inner face (z = 0.5) — always rendered, no neighbor to cull against.
        q([v(0.,0.,0.5),v(1.,0.,0.5),v(1.,1.,0.5),v(0.,1.,0.5)], Vec3::Z,     None,                   false),
        q([v(0.,1.,0.),v(0.,1.,0.5),v(1.,1.,0.5),v(1.,1.,0.)],   Vec3::Y,     Some(Direction::Up),    false),
        q([v(0.,0.,0.5),v(0.,0.,0.),v(1.,0.,0.),v(1.,0.,0.5)],   Vec3::NEG_Y, Some(Direction::Down),  false),
        q([v(1.,0.,0.5),v(1.,0.,0.),v(1.,1.,0.),v(1.,1.,0.5)],   Vec3::X,     Some(Direction::East),  false),
        q([v(0.,0.,0.),v(0.,0.,0.5),v(0.,1.,0.5),v(0.,1.,0.)],   Vec3::NEG_X, Some(Direction::West),  false),
    ]
}

// ── Stair ─────────────────────────────────────────────────────────────────────
//
// Facing = the open/walkable side (approach direction, bottom of the step).
// Canonical geometry is facing North: you walk in from -Z and step up toward +Z.
//
//  Side cross-section (viewed from East):
//
//   y=1        ┌───┐
//              │   │
//   y=0.5  ┌───┴───│
//          │       │
//   y=0    ┴───────┘
//         z=0 z=0.5 z=1
//
// Covers: Down (always) + South/back wall (full height).
// Rotated 90°/180°/270° for East/South/West.
//
// ── Stair, Inverted ───────────────────────────────────────────────────────────
//  Side cross-section (viewed from East):
//
//   y=1    ┌───────┐
//          │       │
//   y=0.5  ┴───┐───│
//              │   │
//   y=0        ┴───┘
//         z=0 z=0.5 z=1
//
// Covers: Down (always) + South/back wall (full height).
// Rotated 90°/180°/270° for East/South/West.

fn stair_quads_north() -> Vec<ShapeQuad> {
    vec![
        // Bottom — full face
        q([v(0.,0.,1.),v(0.,0.,0.),v(1.,0.,0.),v(1.,0.,1.)],       Vec3::NEG_Y, Some(Direction::Down),  true),
        // Front (z=0) — half height, faces North
        q([v(1.,0.,0.),v(0.,0.,0.),v(0.,0.5,0.),v(1.,0.5,0.)],     Vec3::NEG_Z, Some(Direction::North), false),
        // Step tread (y=0.5, z=[0,0.5])
        q([v(0.,0.5,0.),v(0.,0.5,0.5),v(1.,0.5,0.5),v(1.,0.5,0.)], Vec3::Y,     None,                   false),
        // Riser (z=0.5, y=[0.5,1]) — interior face, always rendered
        q([v(1.,0.5,0.5),v(0.,0.5,0.5),v(0.,1.,0.5),v(1.,1.,0.5)], Vec3::NEG_Z, None,                   false),
        // Upper top (y=1, z=[0.5,1])
        q([v(0.,1.,0.5),v(0.,1.,1.),v(1.,1.,1.),v(1.,1.,0.5)],     Vec3::Y,     Some(Direction::Up),    false),
        // Back (z=1) — full height, covers South neighbor
        q([v(0.,0.,1.),v(1.,0.,1.),v(1.,1.,1.),v(0.,1.,1.)],       Vec3::Z,     Some(Direction::South), true),
        // East — lower quad (z=[0,1], y=[0,0.5])
        q([v(1.,0.,1.),v(1.,0.,0.),v(1.,0.5,0.),v(1.,0.5,1.)],     Vec3::X,     Some(Direction::East),  false),
        // East — upper quad (z=[0.5,1], y=[0.5,1])
        q([v(1.,0.5,1.),v(1.,0.5,0.5),v(1.,1.,0.5),v(1.,1.,1.)],   Vec3::X,     Some(Direction::East),  false),
        // West — lower quad
        q([v(0.,0.,0.),v(0.,0.,1.),v(0.,0.5,1.),v(0.,0.5,0.)],     Vec3::NEG_X, Some(Direction::West),  false),
        // West — upper quad
        q([v(0.,0.5,0.5),v(0.,0.5,1.),v(0.,1.,1.),v(0.,1.,0.5)],   Vec3::NEG_X, Some(Direction::West),  false),
    ]
}

fn stair_inv_quads_north() -> Vec<ShapeQuad> {
    vec![
        // Top — full face
        q([v(0.,1.,1.),v(0.,1.,0.),v(1.,1.,0.),v(1.,1.,1.)],       Vec3::Y,     Some(Direction::Up),    true),
        // Front (z=0) — half height, faces North
        q([v(1.,0.5,0.),v(0.,0.5,0.),v(0.,1.,0.),v(1.,1.,0.)],     Vec3::NEG_Z, Some(Direction::North), false),
        // Step tread (y=0.5, z=[0,0.5])
        q([v(0.,0.5,0.),v(0.,0.5,0.5),v(1.,0.5,0.5),v(1.,0.5,0.)], Vec3::NEG_Y, None,                   false),
        // Riser (z=0.5, y=[0,0.5]) — interior face, always rendered
        q([v(1.,0.,0.5),v(0.,0.,0.5),v(0.,0.5,0.5),v(1.,0.5,0.5)], Vec3::NEG_Z, None,                   false),
        // Lower bottom (y=0, z=[0.5,1])
        q([v(0.,0.,0.5),v(0.,0.,1.),v(1.,0.,1.),v(1.,0.,0.5)],     Vec3::NEG_Y, Some(Direction::Down),  false),
        // Back (z=1) — full height, covers South neighbor
        q([v(0.,0.,1.),v(1.,0.,1.),v(1.,1.,1.),v(0.,1.,1.)],       Vec3::Z,     Some(Direction::South), true),
        // East — upper quad (z=[0,1], y=[0.5,1])
        q([v(1.,0.5,1.),v(1.,0.5,0.),v(1.,1.,0.),v(1.,1.,1.)],     Vec3::X,     Some(Direction::East),  false),
        // East — lower quad (z=[0.5,1], y=[0.,0.5])
        q([v(1.,0.,1.),v(1.,0.,0.5),v(1.,0.5,0.5),v(1.,0.5,1.)],   Vec3::X,     Some(Direction::East),  false),
        // West — upper quad
        q([v(0.,0.5,0.),v(0.,0.5,1.),v(0.,1.,1.),v(0.,1.,0.)],     Vec3::NEG_X, Some(Direction::West),  false),
        // West — lower quad
        q([v(0.,0.,0.5),v(0.,0.,1.),v(0.,0.5,1.),v(0.,0.5,0.5)],   Vec3::NEG_X, Some(Direction::West),  false),
    ]
}

// ── Slope ─────────────────────────────────────────────────────────────────────
//
// Facing = direction of the HIGH wall.
// Canonical geometry is facing North: full height at z=0, tapers to y=0 at z=1.
//
//  Side cross-section (viewed from East):
//
//   y=1  ╲
//          ╲
//   y=0     ────
//        z=0   z=1
//
// East/West sides are right triangles, stored as degenerate quads (verts[2]==verts[3]).
// Covers: Down (always) + North wall (full height, the "facing" side).

fn slope_quads_north() -> Vec<ShapeQuad> {
    // The ramp surface normal: slope goes from (x,1,0) down to (x,0,1).
    // Surface vectors: (1,0,0) and (0,-1,1). Normal = cross = (0,1,1) → normalize.
    let ramp_n = Vec3::new(0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2);

    vec![
        // Bottom — full face
        q([v(0.,0.,1.),v(0.,0.,0.),v(1.,0.,0.),v(1.,0.,1.)],    Vec3::NEG_Y, Some(Direction::Down),  true),
        // North wall — full height, covers North neighbor
        q([v(1.,0.,0.),v(0.,0.,0.),v(0.,1.,0.),v(1.,1.,0.)],    Vec3::NEG_Z, Some(Direction::North), true),
        // Ramp surface — diagonal; no neighbor to cull against
        q([v(0.,0.,1.),v(1.,0.,1.),v(1.,1.,0.),v(0.,1.,0.)],    ramp_n,      None,                   false),
        // East — right triangle stored as degenerate quad
        q([v(1.,0.,0.),v(1.,1.,0.),v(1.,0.,1.),v(1.,0.,1.)],    Vec3::X,     Some(Direction::East),  false),
        // West — right triangle stored as degenerate quad
        q([v(0.,0.,0.),v(0.,0.,1.),v(0.,1.,0.),v(0.,1.,0.)],    Vec3::NEG_X, Some(Direction::West),  false),
    ]
}

// ── Rotation helpers ──────────────────────────────────────────────────────────
//
// All rotations are 90°/180° steps around the Y axis, mapping North to the target.
// Vertex positions are rotated around the block centre (0.5, 0.5, 0.5).
//
//   North → identity
//   South → 180°   (x,y,z) → (1-x, y, 1-z)
//   East  →  90° CCW around Y: (x,y,z) → (1-z, y,   x)
//   West  →  90° CW  around Y: (x,y,z) → (  z, y, 1-x)
//
// Up/Down are not valid facing values for Stair/Slope; treated as North.

fn rotate_vert(p: Vec3, facing: Direction) -> Vec3 {
    match facing {
        Direction::North | Direction::Up | Direction::Down => p,
        Direction::South => v(1.-p.x, p.y, 1.-p.z),
        Direction::East  => v(1.-p.z, p.y,    p.x),
        Direction::West  => v(   p.z, p.y, 1.-p.x),
    }
}

fn rotate_normal(n: Vec3, facing: Direction) -> Vec3 {
    match facing {
        Direction::North | Direction::Up | Direction::Down => n,
        Direction::South => v(-n.x, n.y, -n.z),
        Direction::East  => v(-n.z, n.y,  n.x),
        Direction::West  => v( n.z, n.y, -n.x),
    }
}

fn rotate_dir(d: Direction, facing: Direction) -> Direction {
    use Direction::*;
    match facing {
        North | Up | Down => d,
        South => match d { North=>South, South=>North, East=>West,  West=>East,  o=>o },
        East  => match d { North=>East,  East=>South,  South=>West, West=>North, o=>o },
        West  => match d { North=>West,  West=>South,  South=>East, East=>North, o=>o },
    }
}

fn rotate_to(quads: Vec<ShapeQuad>, facing: Direction) -> Vec<ShapeQuad> {
    if matches!(facing, Direction::North) { return quads; }
    quads.into_iter().map(|q| ShapeQuad {
        verts:           q.verts.map(|p| rotate_vert(p, facing)),
        normal:          rotate_normal(q.normal, facing),
        face_dir:        q.face_dir.map(|d| rotate_dir(d, facing)),
        covers_neighbor: q.covers_neighbor,
    }).collect()
}