use std::{f32::consts::FRAC_1_SQRT_2, vec};
use bevy::math::Vec3;
use bevy::math::Vec2;
use crate::plugin::voxel::{BlockShape, Direction};

/// A single renderable quad in local block space [0, 1]³.
///
/// Triangular faces (e.g. the sides of a Slope) use a **degenerate quad**:
/// `verts[3] == verts[2]`. The index pattern (0,1,2),(0,2,3) then produces
/// one real triangle and one zero-area triangle that the GPU discards.
pub struct Quad {
    /// Vertices. They should be ordered top left to bottom right.
    pub verts:                  [Vec3; 4],
    /// UVs. They should be ordered top left to bottom right.
    pub uvs:                    [Vec2; 4],
    /// Normal direction
    pub normal:                 Vec3,
    /// Which direction does this face look toward?
    /// `Some(Direction)` = face sits on the boundary between this block and the neighbor.
    /// `None` = interior face (stair riser, slab inner wall) — always rendered.
    pub culling_direction:      Option<Direction>,

    /// Tells the renderer which direction this face is pointing into so that
    /// the correct FaceTexture can be applied to it.
    /// If the face is internal, we know it from culling_face being None. This
    /// enables us to use different texture sets between internal and external
    /// faces.
    pub texture_direction:      Direction,
}

// ── shorthand constructor ─────────────────────────────────────────────────────

fn q(
    verts:              [Vec3; 4],
    uvs:                [Vec2; 4],
    normal:             Vec3,
    culling_direction:  Option<Direction>,
    texture_direction:  Direction,
) -> Quad {
    Quad { verts, uvs, normal, culling_direction, texture_direction }
}
fn v(x: f32, y: f32, z: f32) -> Vec3 { Vec3::new(x, y, z) }
fn v_uv(x: f32, y: f32) -> Vec2 { Vec2::new(x, y) }

// ── public API ────────────────────────────────────────────────────────────────

/// Returns all quads needed to render a block with the given shape and facing.
/// The quads are in local block space; the caller adds `pos.as_vec3()` to each vertex.
pub fn shape_quads(shape: BlockShape, facing: Direction) -> Vec<Quad> {
    match shape {
        BlockShape::Cube    =>      cube_quads(),
        BlockShape::Slab    =>      slab_quads(facing),
        _ => Vec::new(),
    }
}

// ── Cube ──────────────────────────────────────────────────────────────────────

fn cube_quads() -> Vec<Quad> {
    vec![
        q(  [v(1.,1.,1.),v(1.,1.,0.),v(0.,1.,0.),v(0.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::Y,        Some(Direction::Up),        Direction::Up),
        q(  [v(0.,0.,1.),v(0.,0.,0.),v(1.,0.,0.),v(1.,0.,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::NEG_Y,    Some(Direction::Down),      Direction::Down),
        q(  [v(1.,1.,0.),v(1.,0.,0.),v(0.,0.,0.),v(0.,1.,0.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::NEG_Z,    Some(Direction::North),     Direction::North),
        q(  [v(0.,1.,1.),v(0.,0.,1.),v(1.,0.,1.),v(1.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::Z,        Some(Direction::South),     Direction::South),
        q(  [v(0.,1.,0.),v(0.,0.,0.),v(0.,0.,1.),v(0.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::NEG_X,    Some(Direction::West),      Direction::West),
        q(  [v(1.,1.,1.),v(1.,0.,1.),v(1.,0.,0.),v(1.,1.,0.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::X,        Some(Direction::East),      Direction::East),
    ]
}

// ── Slab ──────────────────────────────────────────────────────────────────────
//
// Facing convention:
//   Up    → bottom slab  y = [0.0, 0.5]   (sits on the floor)
//   Down  → top slab     y = [0.5, 1.0]   (hangs from ceiling)
//   N/S/E/W → vertical slab on that wall  z/x = [0.0, 0.5]


// NEED TO BE RE-CHECKED!!!

fn slab_quads(facing: Direction) -> Vec<Quad> {
    match facing {
        Direction::Up   => bottom_slab_quads(),
        Direction::Down => top_slab_quads(),
        _               => rotate_to(vertical_slab_north(), facing.opposite()),
    }
}

fn bottom_slab_quads() -> Vec<Quad> {
    // Occupies y = [0, 0.5].  Only the bottom face is a full boundary face.
    vec![
        q(  [v(1.,0.5,1.),v(1.,0.5,0.),v(0.,0.5,0.),v(0.,0.5,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::Y,        None, /* internal face */   Direction::Up),
        q(  [v(0.,0.,1.),v(0.,0.,0.),v(1.,0.,0.),v(1.,0.,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::NEG_Y,    Some(Direction::Down),      Direction::Down),
        q(  [v(1.,0.5,0.),v(1.,0.,0.),v(0.,0.,0.),v(0.,0.5,0.)],
            [v_uv(0.,0.5),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.5)],
            Vec3::NEG_Z,    Some(Direction::North),     Direction::North),
        q(  [v(0.,0.5,1.),v(0.,0.,1.),v(1.,0.,1.),v(1.,0.5,1.)],
            [v_uv(0.,0.5),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.5)],
            Vec3::Z,        Some(Direction::South),     Direction::South),
        q(  [v(0.,0.5,0.),v(0.,0.,0.),v(0.,0.,1.),v(0.,0.5,1.)],
            [v_uv(0.,0.5),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.5)],
            Vec3::X,        Some(Direction::East),      Direction::East),
        q(  [v(1.,0.5,1.),v(1.,0.,1.),v(1.,0.,0.),v(1.,0.5,0.)],
            [v_uv(0.,0.5),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.5)],
            Vec3::NEG_X,    Some(Direction::West),      Direction::West),
    ]
}

fn top_slab_quads() -> Vec<Quad> {
    // Occupies y = [0.5, 1].  Only the top face is a full boundary face.
    vec![
        q(  [v(1.,1.,1.),v(1.,1.,0.),v(0.,1.,0.),v(0.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::Y,        Some(Direction::Up),        Direction::Up),
        q(  [v(0.,0.5,1.),v(0.,0.5,0.),v(1.,0.5,0.),v(1.,0.5,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::NEG_Y,    None, /* internal face */   Direction::Down),
        q(  [v(1.,1.,0.),v(1.,0.5,0.),v(0.,0.5,0.),v(0.,1.,0.)],
            [v_uv(0.,0.),v_uv(0.,0.5),v_uv(1.,0.5),v_uv(1.,0.)],
            Vec3::NEG_Z,    Some(Direction::North),     Direction::North),
        q(  [v(0.,1.,1.),v(0.,0.5,1.),v(1.,0.5,1.),v(1.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,0.5),v_uv(1.,0.5),v_uv(1.,0.)],
            Vec3::Z,        Some(Direction::South),     Direction::South),
        q(  [v(0.,1.,0.),v(0.,0.5,0.),v(0.,0.5,1.),v(0.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,0.5),v_uv(1.,0.5),v_uv(1.,0.)],
            Vec3::X,        Some(Direction::East),      Direction::East),
        q(  [v(1.,1.,1.),v(1.,0.5,1.),v(1.,0.5,0.),v(1.,1.,0.)],
            [v_uv(0.,0.),v_uv(0.,0.5),v_uv(1.,0.5),v_uv(1.,0.)],
            Vec3::NEG_X,    Some(Direction::West),      Direction::West),
    ]
}

fn vertical_slab_north() -> Vec<Quad> {
    // Canonical vertical slab: flush against the South wall, z = [0.5, 1].
    // Covers fully the South boundary face.  Rotated for S/E/W variants using the rotate_around_y helper.
    vec![
        q(  [v(1.,1.,1.),v(1.,1.,0.5),v(0.,1.,0.5),v(0.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,0.5),v_uv(1.,0.5),v_uv(1.,0.)],
            Vec3::Y,        Some(Direction::Up),        Direction::Up),
        q(  [v(0.,0.,1.),v(0.,0.,0.5),v(1.,0.,0.5),v(1.,0.,1.)],
            [v_uv(0.,0.),v_uv(0.,0.5),v_uv(1.,0.5),v_uv(1.,0.)],
            Vec3::NEG_Y,    Some(Direction::Down),      Direction::Down),
        q(  [v(1.,1.,0.5),v(1.,0.,0.5),v(0.,0.,0.5),v(0.,1.,0.5)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::NEG_Z,    None, /* internal face */   Direction::North),
        q(  [v(0.,1.,1.),v(0.,0.,1.),v(1.,0.,1.),v(1.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,1.),v_uv(1.,1.),v_uv(1.,0.)],
            Vec3::Z,        Some(Direction::South),     Direction::South),
        q(  [v(0.,1.,0.),v(0.,0.5,0.),v(0.,0.5,1.),v(0.,1.,1.)],
            [v_uv(0.,0.),v_uv(0.,0.5),v_uv(1.,0.5),v_uv(1.,0.)],
            Vec3::X,        Some(Direction::East),      Direction::East),
        q(  [v(1.,1.,1.),v(1.,0.5,1.),v(1.,0.5,0.),v(1.,1.,0.)],
            [v_uv(0.,0.),v_uv(0.,0.5),v_uv(1.,0.5),v_uv(1.,0.)],
            Vec3::NEG_X,    Some(Direction::West),      Direction::West),
    ]
}


// ── Rotation around Y ──────────────────────────────────────────────────────────
//
// All rotations are 90°/180° steps around the Y axis, mapping North to the target.
// Vertex positions are rotated around the block centre (0.5, 0.5, 0.5).
//
// This section is used so that we don't have to manually write out every possible
// mesh rotation.

fn rotate_vec3_around_y(p: Vec3, quarter_turns: u8) -> Vec3 {
    match quarter_turns {
        0 => p,
        1 => Vec3 { x: p.z,         y: p.y,     z: 1.0-p.x  },
        2 => Vec3 { x: 1.0-p.x,     y: p.y,     z: 1.0-p.z  },
        3 => Vec3 { x: 1.0-p.z,     y: p.y,     z: p.x      },
        _ => rotate_vec3_around_y(p, quarter_turns % 4)
    }
}

fn rotate_dir_around_y(d: Direction, quarter_turns: u8) -> Direction {
    use Direction::*;
    match d {
        Up | Down => return d,
        dir => {
            let index = match dir {
                North => 0u8,
                West => 1,
                South => 2,
                _ => 3,
            };
            match (index + quarter_turns) % 4 {
                0 => North,
                1 => West,
                2 => South,
                _ => East,
            }
        }
    }
}

fn rotate_uv_around_y(p: Vec2, facing: Direction, quarter_turns: u8) -> Vec2 {
    use Direction::*;
    match facing {
        North | West | South | East => return p,
        Up => {
                return match quarter_turns {
                0 => p,
                1 => Vec2 { x: p.y,         y: 1.0-p.x, },
                2 => Vec2 { x: 1.0-p.x,     y: 1.0-p.y, },
                3 => Vec2 { x: 1.0-p.y,     y: p.x,     },
                _ => rotate_uv_around_y(p, facing, quarter_turns % 4),
            }
        }
        Down => {
                return match quarter_turns {
                0 => p,
                1 => Vec2 { x: 1.0-p.y,     y: p.x,     },
                2 => Vec2 { x: 1.0-p.x,     y: 1.0-p.y, },
                3 => Vec2 { x: p.y,         y: 1.0-p.x, },
                _ => rotate_uv_around_y(p, facing, quarter_turns % 4),
            }
        }
    }
}

fn rotate_to(quads: Vec<Quad>, to: Direction) -> Vec<Quad> {
    let quarter_turns: u8 = match to {
        Direction::West => 1,
        Direction::South => 2,
        Direction::East => 3,
        _ => return quads,
    };

    quads.into_iter().map(|q| Quad {
        verts:                  q.verts.map(|p| rotate_vec3_around_y(p, quarter_turns)),
        uvs:                    q.uvs.map(|p| rotate_uv_around_y(p, q.texture_direction, quarter_turns)),
        normal:                 rotate_vec3_around_y(q.normal, quarter_turns),
        culling_direction:      q.culling_direction.map(|d| rotate_dir_around_y(d, quarter_turns)),
        texture_direction:      rotate_dir_around_y(q.texture_direction, quarter_turns),
    }).collect()
}