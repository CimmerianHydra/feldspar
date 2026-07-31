use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use bevy::math::{Vec2, Vec3};
use bevy::prelude::*;

use crate::render::mesh::element::{box_quads, BakedQuad, CullTag, Element};
use crate::voxel::{dir_index, BlockRotation, Direction, ModelID, ALL_DIRECTIONS};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – MODELS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Small bit set. Hand-rolled rather than pulling in `bitflags`, since it
/// is two flags and the crate is not already a direct dependency.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ModelFlags(u8);

impl ModelFlags {
    /// Occludes on all six sides and has no interior faces. The
    /// precondition for greedy meshing, computed now rather than
    /// rediscovered later.
    pub const FULL_CUBE: Self = Self(1 << 0);
    /// Has at least one interior (non-boundary) face.
    pub const HAS_INTERIOR: Self = Self(1 << 1);

    pub const fn empty() -> Self { Self(0) }
    #[inline]
    pub fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }
}

impl std::ops::BitOrAssign for ModelFlags {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

/// A finished model: a slice of the arena's quad array, plus everything
/// the mesher needs to use it without ever looking at how it was authored.
///
/// Note what is *not* here: textures, materials, block identity. A model
/// is geometry. That is the whole reason 64 pipe variants can be shared by
/// every pipe material in the game.
#[derive(Clone, Debug)]
pub struct BakedModel {
    pub quads:     Range<u32>,
    /// Collider boxes, as `[min, max]` pairs, in model space.
    pub colliders: Range<u32>,
    /// Bit per direction, in `ALL_DIRECTIONS` order. Set means this model
    /// fully covers that voxel boundary, so a neighbour looking at it can
    /// drop its own face.
    pub occlusion: u8,
    pub flags:     ModelFlags,
}

impl BakedModel {
    #[inline]
    pub fn occludes(&self, d: Direction) -> bool {
        self.occlusion & (1 << dir_index(d)) != 0
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE ARENA
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Every model in the game, in two flat arrays.
///
/// Built once at startup and never mutated afterwards, so the mesher's
/// per-voxel work is: resolve a `ModelID`, take a slice, append. No
/// allocation, no hashing, no `Vec<Quad>` churn.
#[derive(Resource, Default)]
pub struct ModelArena {
    quads:     Vec<BakedQuad>,
    colliders: Vec<[Vec3; 2]>,
    models:    Vec<BakedModel>,
    /// Structural hash → id. Collapses duplicate bakes, which is how a
    /// log's 32-entry rotation table ends up pointing at 3 real models.
    dedup:     HashMap<u64, ModelID>,
}

impl ModelArena {
    pub fn new() -> Self { Self::default() }

    #[inline]
    pub fn model(&self, id: ModelID) -> &BakedModel {
        &self.models[id.0 as usize]
    }

    /// The hot accessor. One bounds check, one slice.
    #[inline]
    pub fn quads(&self, id: ModelID) -> &[BakedQuad] {
        let m = &self.models[id.0 as usize];
        &self.quads[m.quads.start as usize..m.quads.end as usize]
    }

    #[inline]
    pub fn colliders(&self, id: ModelID) -> &[[Vec3; 2]] {
        let m = &self.models[id.0 as usize];
        &self.colliders[m.colliders.start as usize..m.colliders.end as usize]
    }

    pub fn model_count(&self) -> usize { self.models.len() }
    pub fn quad_count(&self)  -> usize { self.quads.len() }

    // ---- baking ---------------------------------------------------------

    /// Bake one orientation of an element list.
    ///
    /// Rotation is applied here, at startup, exactly once per variant —
    /// never per remesh.
    pub fn bake(&mut self, elements: &[Element], rotation: BlockRotation) -> ModelID {
        let mut quads: Vec<BakedQuad> = Vec::new();
        let mut colliders: Vec<[Vec3; 2]> = Vec::new();

        for element in elements {
            match element {
                Element::Box(b) => {
                    box_quads(b, &mut quads);
                    colliders.push([b.min, b.max]);
                }
                Element::Raw { quads: raw, collider } => {
                    quads.extend_from_slice(raw);
                    colliders.extend_from_slice(collider);
                }
            }
        }

        for q in &mut quads {
            rotate_quad(q, rotation);
        }
        for c in &mut colliders {
            let (a, b) = (rotation.apply_point(c[0]), rotation.apply_point(c[1]));
            *c = [a.min(b), a.max(b)];
        }

        self.intern(quads, colliders)
    }

    /// Bake every rotation named by `rotations`, returning ids in the same
    /// order. Duplicates collapse in the arena, so passing all 24 for a
    /// shape with rotational symmetry costs table entries, not memory.
    pub fn bake_rotations(
        &mut self,
        elements: &[Element],
        rotations: impl IntoIterator<Item = BlockRotation>,
    ) -> Vec<ModelID> {
        rotations.into_iter().map(|r| self.bake(elements, r)).collect()
    }

    /// Push (or reuse) a finished quad list.
    fn intern(&mut self, quads: Vec<BakedQuad>, colliders: Vec<[Vec3; 2]>) -> ModelID {
        let key = structural_hash(&quads, &colliders);
        if let Some(existing) = self.dedup.get(&key) {
            return *existing;
        }

        let occlusion = compute_occlusion(&quads);

        let mut flags = ModelFlags::empty();
        if quads.iter().any(|q| q.cull.is_interior()) {
            flags |= ModelFlags::HAS_INTERIOR;
        }
        if occlusion == 0b0011_1111 && !flags.contains(ModelFlags::HAS_INTERIOR) {
            flags |= ModelFlags::FULL_CUBE;
        }

        let quad_start = self.quads.len() as u32;
        self.quads.extend(quads);
        let coll_start = self.colliders.len() as u32;
        self.colliders.extend(colliders);

        let id = ModelID(self.models.len() as u32);
        self.models.push(BakedModel {
            quads:     quad_start..self.quads.len() as u32,
            colliders: coll_start..self.colliders.len() as u32,
            occlusion,
            flags,
        });
        self.dedup.insert(key, id);
        id
    }
}

fn rotate_quad(q: &mut BakedQuad, r: BlockRotation) {
    if r == BlockRotation::IDENTITY { return; }
    for v in &mut q.verts {
        *v = r.apply_point(*v);
    }
    q.normal = r.apply_normal(q.normal);
    if let Some(d) = q.cull.dir() {
        q.cull = CullTag::from_dir(r.apply_dir(d));
    }
    // UVs and slots ride along with their vertices untouched, which is
    // exactly right: rotating a log rotates its bark texture with it, and
    // a cube's "up" slot follows the face it landed on.
}

fn structural_hash(quads: &[BakedQuad], colliders: &[[Vec3; 2]]) -> u64 {
    let mut h = DefaultHasher::new();
    quads.len().hash(&mut h);
    for q in quads {
        q.hash_key().hash(&mut h);
    }
    colliders.len().hash(&mut h);
    for c in colliders {
        for v in c {
            for f in v.to_array() { f.to_bits().hash(&mut h); }
        }
    }
    h.finish()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – OCCLUSION BY RASTERISATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Sampling resolution for the coverage test. 16 matches the authoring
/// grid, so a face made of grid-aligned boxes is decided exactly.
const COVERAGE_RES: usize = 16;

/// Derive the 6-bit occlusion mask.
///
/// This is the reason `Voxel` has no `covers_face`. A hand-written match
/// per `BlockShape` does not survive contact with imported models — nobody
/// can eyeball whether a Blockbench mesh happens to tile its north
/// boundary. So: rasterise instead.
///
/// Sampling rather than analytic union because a boundary face is not
/// always a rectangle. Slope sides are triangles sitting exactly on the
/// east and west boundaries, and they must come out *not* covering.
///
/// Cost is 6 faces x 256 samples x a handful of quads, once per model at
/// startup. Immeasurable.
fn compute_occlusion(quads: &[BakedQuad]) -> u8 {
    let mut mask = 0u8;

    for (i, dir) in ALL_DIRECTIONS.iter().enumerate() {
        let face: Vec<&BakedQuad> = quads
            .iter()
            .filter(|q| q.cull.dir() == Some(*dir))
            .collect();

        if face.is_empty() { continue; }
        if face_fully_covered(&face, *dir) {
            mask |= 1 << i;
        }
    }

    mask
}

/// Project a model-space point onto the 2D plane of a boundary face.
#[inline]
fn project(p: Vec3, dir: Direction) -> Vec2 {
    match dir {
        Direction::Up | Direction::Down       => Vec2::new(p.x, p.z),
        Direction::North | Direction::South   => Vec2::new(p.x, p.y),
        Direction::East | Direction::West     => Vec2::new(p.z, p.y),
    }
}

fn face_fully_covered(quads: &[&BakedQuad], dir: Direction) -> bool {
    // Pre-project once; the inner loop runs 256 times.
    let tris: Vec<[Vec2; 3]> = quads
        .iter()
        .flat_map(|q| {
            let p: Vec<Vec2> = q.verts.iter().map(|v| project(*v, dir)).collect();
            // Degenerate quads (verts[3] == verts[2]) yield one real
            // triangle and one zero-area triangle, which never contains a
            // sample point, so no special case is needed.
            [[p[0], p[1], p[2]], [p[0], p[2], p[3]]]
        })
        .collect();

    let step = 1.0 / COVERAGE_RES as f32;
    for iy in 0..COVERAGE_RES {
        for ix in 0..COVERAGE_RES {
            let sample = Vec2::new(
                (ix as f32 + 0.5) * step,
                (iy as f32 + 0.5) * step,
            );
            if !tris.iter().any(|t| point_in_triangle(sample, t)) {
                return false;
            }
        }
    }
    true
}

fn point_in_triangle(p: Vec2, t: &[Vec2; 3]) -> bool {
    const EPS: f32 = 1e-6;
    let sign = |a: Vec2, b: Vec2, c: Vec2| (a.x - c.x) * (b.y - c.y) - (b.x - c.x) * (a.y - c.y);

    let d1 = sign(p, t[0], t[1]);
    let d2 = sign(p, t[1], t[2]);
    let d3 = sign(p, t[2], t[0]);

    let has_neg = d1 < -EPS || d2 < -EPS || d3 < -EPS;
    let has_pos = d1 > EPS || d2 > EPS || d3 > EPS;
    !(has_neg && has_pos)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::mesh::shapes;

    #[test]
    fn cube_occludes_everything() {
        let mut arena = ModelArena::new();
        let id = arena.bake(&shapes::cube(), BlockRotation::IDENTITY);
        let m = arena.model(id);
        assert_eq!(m.occlusion, 0b0011_1111);
        assert!(m.flags.contains(ModelFlags::FULL_CUBE));
    }

    /// The result the old hand-written `covers_face` produced for slabs,
    /// now derived.
    #[test]
    fn bottom_slab_occludes_only_down() {
        let mut arena = ModelArena::new();
        let id = arena.bake(&shapes::panel(8.0), BlockRotation::IDENTITY);
        let m = arena.model(id);
        assert!(m.occludes(Direction::Down));
        assert!(!m.occludes(Direction::Up));
        assert!(!m.occludes(Direction::North));
        assert!(!m.flags.contains(ModelFlags::FULL_CUBE));
    }

    /// Rotating a slab so its solid face points north must move the
    /// occlusion bit with it — this is the check that catches a broken
    /// cull-tag rotation, which would otherwise show up as holes in the
    /// world much later.
    #[test]
    fn rotating_a_slab_moves_its_occlusion() {
        let mut arena = ModelArena::new();
        let r = BlockRotation::from_parts(Direction::North, 0);
        let id = arena.bake(&shapes::panel(8.0), r);
        let m = arena.model(id);
        assert_eq!(m.occlusion.count_ones(), 1);
        assert!(m.occludes(r.apply_dir(Direction::Down)));
    }

    /// Pipes are thin: they must neither cull nor be culled, whatever
    /// their connection mask.
    #[test]
    fn pipes_never_occlude() {
        let mut arena = ModelArena::new();
        for mask in 0u8..64 {
            let id = arena.bake(&shapes::pipe(mask), BlockRotation::IDENTITY);
            assert_eq!(arena.model(id).occlusion, 0, "mask {mask:06b} occludes");
        }
    }

    /// Identical geometry must intern to one model. This is what keeps the
    /// arena small when a block declares more rotation bits than it has
    /// distinct orientations.
    #[test]
    fn identical_bakes_dedup() {
        let mut arena = ModelArena::new();
        let a = arena.bake(&shapes::cube(), BlockRotation::IDENTITY);
        let b = arena.bake(&shapes::cube(), BlockRotation::IDENTITY);
        assert_eq!(a, b);
        assert_eq!(arena.model_count(), 1);
    }

    /// A uniformly-slotted cube is symmetric, so all 24 rotations collapse.
    #[test]
    fn uniform_cube_collapses_across_rotations() {
        let mut arena = ModelArena::new();
        let ids = arena.bake_rotations(&shapes::cube(), BlockRotation::all());
        assert_eq!(ids.len(), 24);
        assert_eq!(arena.model_count(), 1);
    }

    /// The full pipe set is small enough to bake unconditionally.
    #[test]
    fn pipe_arena_is_small() {
        let mut arena = ModelArena::new();
        for mask in 0u8..64 {
            arena.bake(&shapes::pipe(mask), BlockRotation::IDENTITY);
        }
        assert!(arena.quad_count() < 3000, "pipes baked {} quads", arena.quad_count());
    }
}
