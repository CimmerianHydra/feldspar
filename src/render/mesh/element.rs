use bevy::math::{Vec2, Vec3};

use crate::voxel::{dir_index, Direction, ALL_DIRECTIONS};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – CULL TAGS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Which voxel boundary a quad sits on, or "none — interior geometry".
///
/// Replaces `Option<Direction>`. Same information, but a plain `u8` the
/// mesher can compare and index without matching an enum inside an option,
/// which matters because this is read once per quad per remesh.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CullTag(u8);

impl CullTag {
    pub const INTERIOR: Self = Self(6);

    #[inline]
    pub fn from_dir(d: Direction) -> Self { Self(dir_index(d) as u8) }

    #[inline]
    pub fn is_interior(self) -> bool { self.0 >= 6 }

    #[inline]
    pub fn index(self) -> u8 { self.0 }

    #[inline]
    pub fn dir(self) -> Option<Direction> {
        (!self.is_interior()).then(|| ALL_DIRECTIONS[self.0 as usize])
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE BAKED QUAD
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One quad, in model space `[0,1]³`, fully resolved and never touched
/// again at runtime.
///
/// A model knows nothing about textures — it knows it has N paintable
/// surfaces, named by `slot`. The `BlockDefinition` decides what goes on
/// them.
///
/// That indirection is what makes materials free. Iron, bronze and wood
/// pipes are three block definitions with three slot tables sharing one
/// set of 64 baked models.
///
/// Triangular faces (slope sides) use the degenerate-quad convention:
/// `verts[3] == verts[2]`, producing one real triangle and one zero-area
/// triangle the GPU discards.
#[derive(Clone, Copy, Debug)]
pub struct BakedQuad {
    /// Ordered to match `uvs`: (0,0), (0,1), (1,1), (1,0).
    /// Winding is CCW seen from outside the surface.
    pub verts:  [Vec3; 4],
    pub uvs:    [Vec2; 4],
    pub normal: Vec3,
    pub cull:   CullTag,
    /// Index into the block definition's resolved slot table.
    pub slot:   u8,
}

impl BakedQuad {
    /// Bit-exact hash key. `f32` is not `Hash`, and we want structural
    /// identity anyway so the arena can deduplicate models that bake to
    /// the same geometry (which is how 32 rotation table entries collapse
    /// to the 3 distinct models a log actually needs).
    pub fn hash_key(&self) -> impl std::hash::Hash {
        let mut out = [0u32; 4 * 3 + 4 * 2 + 3 + 1];
        let mut i = 0;
        for v in self.verts { for c in v.to_array() { out[i] = c.to_bits(); i += 1; } }
        for v in self.uvs   { for c in v.to_array() { out[i] = c.to_bits(); i += 1; } }
        for c in self.normal.to_array() { out[i] = c.to_bits(); i += 1; }
        out[i] = ((self.cull.index() as u32) << 8) | self.slot as u32;
        out
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE ELEMENT IR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What a face of a box looks like. `None` in a box's face array means
/// "don't emit this face at all" — that is how intra-model culling is
/// expressed, e.g. a pipe core omitting the faces its arms cover.
#[derive(Clone, Copy, Debug)]
pub struct FaceSpec {
    /// Which of the model's paintable surfaces this face uses.
    pub slot: u8,
    /// Explicit per-corner UVs, in the same order as the face's verts, for
    /// imported models that carry their own mapping. `None` derives UVs from
    /// the box extents, which gives the "cut out of the full-block texture"
    /// behaviour we want for slabs, stairs and panels.
    pub uv: Option<[Vec2; 4]>,
}

impl FaceSpec {
    pub const fn slot(slot: u8) -> Self { Self { slot, uv: None } }
}

/// An axis-aligned box in model space, with a spec per face.
///
/// This is deliberately the same primitive Blockbench and Minecraft's
/// model JSON use, because it means importing an external model is a
/// parser, not a second geometry pipeline.
#[derive(Clone, Debug)]
pub struct ElementBox {
    pub min:   Vec3,
    pub max:   Vec3,
    /// Indexed by `dir_index`. `None` omits the face.
    pub faces: [Option<FaceSpec>; 6],
}

impl ElementBox {
    /// Box with every face present on the same slot.
    pub fn uniform(min: Vec3, max: Vec3, slot: u8) -> Self {
        Self { min, max, faces: [Some(FaceSpec::slot(slot)); 6] }
    }

    /// Box with one slot per face, in `ALL_DIRECTIONS` order. This is what
    /// a cube uses, so `PerFace` appearances resolve correctly.
    pub fn per_face(min: Vec3, max: Vec3, slots: [u8; 6]) -> Self {
        Self {
            min,
            max,
            faces: std::array::from_fn(|i| Some(FaceSpec::slot(slots[i]))),
        }
    }

    /// Drop a face. Chainable, and the workhorse of intra-model culling.
    pub fn without(mut self, d: Direction) -> Self {
        self.faces[dir_index(d)] = None;
        self
    }

    pub fn with_face(mut self, d: Direction, spec: FaceSpec) -> Self {
        self.faces[dir_index(d)] = Some(spec);
        self
    }
}

/// One piece of a model.
///
/// Boxes cover everything except slopes, which carry a diagonal and
/// therefore need an escape hatch. Keeping `Raw` narrow is deliberate: the
/// more geometry that goes through `Box`, the more of it gets free
/// colliders, free UVs and free cull tags.
#[derive(Clone, Debug)]
pub enum Element {
    Box(ElementBox),
    /// Hand-authored geometry. Colliders must be supplied explicitly since
    /// they cannot be inferred from a quad soup.
    Raw {
        quads:    Vec<BakedQuad>,
        collider: Vec<[Vec3; 2]>,
    },
}

impl Element {
    pub fn boxed(min: Vec3, max: Vec3, slot: u8) -> Self {
        Element::Box(ElementBox::uniform(min, max, slot))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – BOX → QUADS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Tolerance for "this face lies on the voxel boundary". Model geometry is
/// authored on a 1/16 grid, so anything this close is exact in intent.
const BOUNDARY_EPS: f32 = 1e-4;

/// Emit the quads for one box.
///
/// Two things are *derived* here that used to be hand-authored:
///
/// - **Cull tags.** A face is cullable against a neighbour precisely when
///   its plane is coplanar with the unit cube boundary. A bottom slab's
///   underside (y=0) gets `Down`; its top (y=0.5) is interior. Nobody has
///   to remember to mark it.
/// - **UVs.** Derived from the box extents so a sub-unit box shows the
///   corresponding sub-rectangle of the block texture.
pub fn box_quads(b: &ElementBox, out: &mut Vec<BakedQuad>) {
    for (i, face) in b.faces.iter().enumerate() {
        let Some(spec) = face else { continue };
        let dir = ALL_DIRECTIONS[i];

        let (verts, uv_pairs, normal) = face_geometry(b.min, b.max, dir);
        let uvs = spec.uv.unwrap_or(uv_pairs);

        out.push(BakedQuad {
            verts,
            uvs,
            normal,
            cull: boundary_tag(b.min, b.max, dir),
            slot: spec.slot,
        });
    }
}

/// A boundary tag iff this face is flush with the voxel boundary.
fn boundary_tag(min: Vec3, max: Vec3, dir: Direction) -> CullTag {
    let flush = match dir {
        Direction::Up    => (max.y - 1.0).abs() < BOUNDARY_EPS,
        Direction::Down  => min.y.abs()        < BOUNDARY_EPS,
        Direction::North => min.z.abs()        < BOUNDARY_EPS,
        Direction::South => (max.z - 1.0).abs() < BOUNDARY_EPS,
        Direction::East  => (max.x - 1.0).abs() < BOUNDARY_EPS,
        Direction::West  => min.x.abs()        < BOUNDARY_EPS,
    };
    if flush { CullTag::from_dir(dir) } else { CullTag::INTERIOR }
}

/// Corner positions, corner UV parameters, and the outward normal for one
/// face of a box.
///
/// The four corners are ordered to match UV corners (0,0), (0,1), (1,1),
/// (1,0), with CCW winding seen from outside.
///
/// ## If a texture comes out mirrored or upside down
///
/// Fix it here and only here. Each arm below picks how the two in-plane
/// axes map to (u, v); flipping one of them is a one-character change and
/// affects every model in the game consistently. Resist the urge to
/// compensate in a shape generator.
fn face_geometry(min: Vec3, max: Vec3, dir: Direction) -> ([Vec3; 4], [Vec2; 4], Vec3) {
    let (lo, hi) = (min, max);

    // Helper: build the four (position, uv) pairs from two in-plane
    // parameters expressed as functions of the box corners.
    macro_rules! face {
        ($normal:expr; $( ($x:expr, $y:expr, $z:expr) => ($u:expr, $v:expr) ),+ $(,)?) => {
            (
                [ $( Vec3::new($x, $y, $z) ),+ ],
                [ $( Vec2::new($u, $v) ),+ ],
                $normal,
            )
        };
    }

    match dir {
        // +Y. u runs with -x, v runs with -z.
        Direction::Up => face![Vec3::Y;
            (hi.x, hi.y, hi.z) => (1.0 - hi.x, 1.0 - hi.z),
            (hi.x, hi.y, lo.z) => (1.0 - hi.x, 1.0 - lo.z),
            (lo.x, hi.y, lo.z) => (1.0 - lo.x, 1.0 - lo.z),
            (lo.x, hi.y, hi.z) => (1.0 - lo.x, 1.0 - hi.z),
        ],

        // -Y. u runs with +x, v runs with -z.
        Direction::Down => face![Vec3::NEG_Y;
            (lo.x, lo.y, hi.z) => (lo.x, 1.0 - hi.z),
            (lo.x, lo.y, lo.z) => (lo.x, 1.0 - lo.z),
            (hi.x, lo.y, lo.z) => (hi.x, 1.0 - lo.z),
            (hi.x, lo.y, hi.z) => (hi.x, 1.0 - hi.z),
        ],

        // -Z. u runs with -x, v runs with -y.
        Direction::North => face![Vec3::NEG_Z;
            (hi.x, hi.y, lo.z) => (1.0 - hi.x, 1.0 - hi.y),
            (hi.x, lo.y, lo.z) => (1.0 - hi.x, 1.0 - lo.y),
            (lo.x, lo.y, lo.z) => (1.0 - lo.x, 1.0 - lo.y),
            (lo.x, hi.y, lo.z) => (1.0 - lo.x, 1.0 - hi.y),
        ],

        // +Z. u runs with +x, v runs with -y.
        Direction::South => face![Vec3::Z;
            (lo.x, hi.y, hi.z) => (lo.x, 1.0 - hi.y),
            (lo.x, lo.y, hi.z) => (lo.x, 1.0 - lo.y),
            (hi.x, lo.y, hi.z) => (hi.x, 1.0 - lo.y),
            (hi.x, hi.y, hi.z) => (hi.x, 1.0 - hi.y),
        ],

        // +X. u runs with -z, v runs with -y.
        Direction::East => face![Vec3::X;
            (hi.x, hi.y, hi.z) => (1.0 - hi.z, 1.0 - hi.y),
            (hi.x, lo.y, hi.z) => (1.0 - hi.z, 1.0 - lo.y),
            (hi.x, lo.y, lo.z) => (1.0 - lo.z, 1.0 - lo.y),
            (hi.x, hi.y, lo.z) => (1.0 - lo.z, 1.0 - hi.y),
        ],

        // -X. u runs with +z, v runs with -y.
        Direction::West => face![Vec3::NEG_X;
            (lo.x, hi.y, lo.z) => (lo.z, 1.0 - hi.y),
            (lo.x, lo.y, lo.z) => (lo.z, 1.0 - lo.y),
            (lo.x, lo.y, hi.z) => (hi.z, 1.0 - lo.y),
            (lo.x, hi.y, hi.z) => (hi.z, 1.0 - hi.y),
        ],
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_cube() -> ElementBox {
        ElementBox::uniform(Vec3::ZERO, Vec3::ONE, 0)
    }

    #[test]
    fn cube_emits_six_quads_all_cullable() {
        let mut out = Vec::new();
        box_quads(&unit_cube(), &mut out);
        assert_eq!(out.len(), 6);
        assert!(out.iter().all(|q| !q.cull.is_interior()));
    }

    /// The whole point of deriving cull tags: a slab's top face must come
    /// out interior without anyone saying so.
    #[test]
    fn slab_top_face_is_interior() {
        let slab = ElementBox::uniform(Vec3::ZERO, Vec3::new(1.0, 0.5, 1.0), 0);
        let mut out = Vec::new();
        box_quads(&slab, &mut out);

        let up = out.iter().find(|q| q.normal == Vec3::Y).unwrap();
        let down = out.iter().find(|q| q.normal == Vec3::NEG_Y).unwrap();
        assert!(up.cull.is_interior());
        assert_eq!(down.cull.dir(), Some(Direction::Down));
    }

    /// Every quad must wind counter-clockwise seen from outside, i.e. the
    /// geometric normal from the first three verts must agree with the
    /// declared normal.
    #[test]
    fn winding_matches_normals() {
        let mut out = Vec::new();
        box_quads(&unit_cube(), &mut out);
        for q in &out {
            let geo = (q.verts[1] - q.verts[0]).cross(q.verts[2] - q.verts[0]);
            assert!(geo.dot(q.normal) > 0.0, "bad winding on normal {:?}", q.normal);
        }
    }

    #[test]
    fn omitted_faces_are_not_emitted() {
        let mut out = Vec::new();
        box_quads(&unit_cube().without(Direction::Up), &mut out);
        assert_eq!(out.len(), 5);
        assert!(out.iter().all(|q| q.normal != Vec3::Y));
    }
}
