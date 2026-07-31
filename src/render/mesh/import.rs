//! Blockbench documents → the element IR.
//!
//! Goes in `src/render/mesh/import.rs`.
//!
//! This is a *translation*, not a second geometry pipeline. Everything a
//! primitive gets — cull tags, winding, normals, occlusion, colliders,
//! rotation baking, arena deduplication — an imported model gets too,
//! because both come out the far end as the same `Vec<Element>`.
//!
//! ## The two paths
//!
//! **Axis-aligned elements** become `Element::Box`. `box_quads` then derives
//! their cull tags for free, so an imported pedestal sitting flush on the
//! voxel floor correctly reports its underside as cullable without anyone
//! saying so.
//!
//! **Rotated or zero-thickness elements** become `Element::Raw`, because
//! `ElementBox` is axis-aligned by construction and a 45° blade is not. They
//! still go *through* `box_quads` first and are transformed afterwards, which
//! is why this file contains no vertex ordering or winding logic of its own.

use bevy::math::{EulerRot, Quat, Vec2, Vec3};
use bevy::prelude::*;

use crate::content::model_source::{BbElement, BbFace, BbModel};
use crate::render::mesh::element::{box_quads, Element, ElementBox, FaceSpec};
use crate::render::mesh::element::CullTag;
use crate::voxel::ALL_DIRECTIONS;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 0 – CONVENTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Blockbench authors on a 16-unit grid, one unit per texel of a 16×16 block
/// texture. Same lattice `shapes::px` uses, which is what keeps imported and
/// hand-authored geometry on the same vertex grid.
const UNITS_PER_BLOCK: f32 = 16.0;

/// Where a document's origin sits relative to the voxel.
///
/// Blockbench's Minecraft formats put it at a corner and run 0..16.  The
/// `free` format — which the chess piece uses — centres X and Z on zero and
/// rises from the floor, so the horizontal axes need shifting half a block
/// and Y does not.
///
/// This is the one implicit convention in the whole importer. If a model
/// comes in offset by half a block, this constant is why, and it is the only
/// place to fix it.
const ORIGIN_OFFSET: Vec3 = Vec3::new(8.0, 0.0, 8.0);

/// How far outside `[0,1]³` geometry may stray before it is worth a warning.
/// A model that escapes its own voxel renders into its neighbour and breaks
/// the assumptions the occlusion rasteriser samples under.
const FIT_EPS: f32 = 1e-3;

/// Corner parameters in the exact order `face_geometry` emits verts:
/// (0,0), (0,1), (1,1), (1,0), counter-clockwise seen from outside.
const CORNERS: [Vec2; 4] = [
    Vec2::new(0.0, 0.0),
    Vec2::new(0.0, 1.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(1.0, 0.0),
];

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – ENTRY POINT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Translate a whole document. `name` is only ever used for diagnostics.
///
/// Returns an empty vec if the document yields nothing usable; the caller
/// decides what to fall back to, because "no geometry" is a content error and
/// content errors belong in the baker's log next to the block that caused
/// them.
pub fn import(doc: &BbModel, name: &str) -> Vec<Element> {
    let mut out = Vec::with_capacity(doc.elements.len());

    for el in &doc.elements {
        if !el.visibility {
            continue;
        }
        if el.kind != "cube" {
            warn!(
                "Model '{name}': element '{}' has type '{}', not 'cube' — skipped. \
                 Only box elements are supported; convert meshes to boxes in Blockbench.",
                el.name, el.kind
            );
            continue;
        }
        if let Some(element) = import_element(doc, el, name) {
            out.push(element);
        }
    }

    warn_if_outside_unit_cube(doc, name);
    out
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – ONE ELEMENT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn import_element(doc: &BbModel, el: &BbElement, name: &str) -> Option<Element> {
    let inflate = Vec3::splat(el.inflate);
    let a = Vec3::from(el.from) - inflate;
    let b = Vec3::from(el.to) + inflate;

    // Blockbench happily lets `from` exceed `to` on an axis; the box means
    // the same thing either way, and normalising here keeps every downstream
    // `min`/`max` assumption true.
    let min = to_model_space(a.min(b));
    let max = to_model_space(a.max(b));

    // ── Faces ──────────────────────────────────────────────────────────
    let mut boxed = ElementBox { min, max, faces: [None; 6] };
    let mut drawn = 0usize;

    for (i, dir) in ALL_DIRECTIONS.iter().enumerate() {
        let Some(face) = el.faces.get(*dir) else { continue };

        // No texture means Blockbench is telling us not to draw it.
        let Some(texture) = face.texture else { continue };

        if texture >= doc.textures.len() {
            warn!(
                "Model '{name}': element '{}' face {dir:?} names texture {texture}, \
                 but the model declares only {}. Face skipped.",
                el.name,
                doc.textures.len()
            );
            continue;
        }

        boxed.faces[i] = Some(FaceSpec {
            // The slot *is* the texture index. A model declares N paintable
            // surfaces; the block decides what goes on them.
            slot: texture as u8,
            uv: Some(face_uvs(doc, face)),
        });
        drawn += 1;
    }

    if drawn == 0 {
        return None;
    }

    // ── Which path ─────────────────────────────────────────────────────
    let rotation = el
        .rotation
        .map(Vec3::from)
        .filter(|r| *r != Vec3::ZERO);

    // A plane has no volume, so `bake`'s automatic `[min, max]` collider
    // would be degenerate. Route it through `Raw` and give it none.
    let flat = (max - min).min_element() <= f32::EPSILON;

    if rotation.is_none() && !flat {
        return Some(Element::Box(boxed));
    }

    let mut quads = Vec::new();
    box_quads(&boxed, &mut quads);

    if let Some(euler) = rotation {
        let pivot = to_model_space(Vec3::from(el.origin.unwrap_or_default()));

        // Single-axis rotations — which is all this model and nearly every
        // hand-built block model uses — are order-independent. If you ever
        // author a multi-axis element and it comes out twisted, this is the
        // line to revisit; Blockbench's compose order is the thing to match.
        let q = Quat::from_euler(
            EulerRot::XYZ,
            euler.x.to_radians(),
            euler.y.to_radians(),
            euler.z.to_radians(),
        );

        for quad in &mut quads {
            for v in &mut quad.verts {
                *v = pivot + q * (*v - pivot);
            }
            quad.normal = (q * quad.normal).normalize();

            // A rotated face is no longer coplanar with any voxel boundary,
            // so it must neither be culled nor cull anything. Conservative,
            // and the failure mode of being wrong here is holes in the world.
            quad.cull = CullTag::INTERIOR;
        }
    }

    // Deliberately no collider. The axis-aligned bounds of a rotated blade
    // are several times its own volume.
    // Solid elements keep the free collider they get from `Element::Box`.
    Some(Element::Raw { quads, collider: Vec::new() })
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – UVs
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The face's four corner UVs, in vert order.
///
/// Three conventions collapse into this one function:
///
/// - **Normalisation.** Pixels → `[0,1]²` through the project resolution.
/// - **Mirroring.** The rectangle's corners are used unsorted, so a reversed
///   `[x1 > x2]` reproduces the flip the artist authored rather than
///   quietly straightening it out.
/// - **Rotation.** Blockbench's per-face 0/90/180/270 is a cyclic shift of
///   which corner the rectangle's first point lands on.
///
/// Blockbench's V runs downward from the top of the image, which is the same
/// convention `face_geometry` already derives (v = 0 at the top of a block).
/// So no flip. If an imported texture comes out upside down, fix it in
/// `face_geometry` — one place, every model — not here.
fn face_uvs(doc: &BbModel, face: &BbFace) -> [Vec2; 4] {
    let scale = Vec2::new(doc.resolution.width, doc.resolution.height);
    let start = Vec2::new(face.uv[0], face.uv[1]) / scale;
    let end = Vec2::new(face.uv[2], face.uv[3]) / scale;

    // If a rotated face comes out turning the wrong way, this is the line:
    // swap to `(k + 4 - steps) % 4`.
    let steps = ((face.rotation / 90) % 4) as usize;

    std::array::from_fn(|k| {
        let t = CORNERS[(k + steps) % 4];
        start + (end - start) * t
    })
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – HELPERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[inline]
fn to_model_space(p: Vec3) -> Vec3 {
    (p + ORIGIN_OFFSET) / UNITS_PER_BLOCK
}

/// Warn once per model if any element's *unrotated* bounds escape the unit
/// cube. Rotation can only pull corners further out, so this is a lower
/// bound — but it catches the common case of a document authored against a
/// different origin convention, which otherwise shows up much later as
/// geometry bleeding into the neighbouring voxel.
fn warn_if_outside_unit_cube(doc: &BbModel, name: &str) {
    let escapes = doc.elements.iter().filter(|el| el.visibility).any(|el| {
        let a = to_model_space(Vec3::from(el.from));
        let b = to_model_space(Vec3::from(el.to));
        a.min(b).min_element() < -FIT_EPS || a.max(b).max_element() > 1.0 + FIT_EPS
    });

    if escapes {
        warn!(
            "Model '{name}' has geometry outside [0,1]³ once mapped into voxel space. \
             It will render into neighbouring voxels and its occlusion mask is unreliable. \
             Check that it is authored against the origin convention in `import::ORIGIN_OFFSET`."
        );
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::mesh::model::ModelArena;
    use crate::voxel::{BlockRotation, Direction};

    /// The chess piece, trimmed to the fields the importer reads.
    const CHESS_PIECE: &str = r#"{
      "resolution": { "width": 16, "height": 16 },
      "textures": [ { "name": "base.png" }, { "name": "top.png" } ],
      "elements": [
        { "type": "cube", "name": "pedestal",
          "from": [-3, 0, -3], "to": [3, 6, 3],
          "faces": {
            "north": { "uv": [0,0,6,6],  "texture": 0 },
            "east":  { "uv": [0,0,6,6],  "texture": 0 },
            "south": { "uv": [0,0,6,6],  "texture": 0 },
            "west":  { "uv": [0,0,6,6],  "texture": 0 },
            "up":    { "uv": [6,12,0,6], "texture": 0 },
            "down":  { "uv": [12,0,6,6], "texture": 0 }
          } },
        { "type": "cube", "name": "crown_a",
          "from": [0, 6, -4], "to": [0, 16, 4],
          "rotation": [0, 45, 0], "origin": [0, 6, 0],
          "faces": {
            "north": { "uv": [0,0,0,10] },
            "east":  { "uv": [0,0,8,10], "texture": 1 },
            "south": { "uv": [0,0,0,10] },
            "west":  { "uv": [0,0,8,10], "texture": 1 },
            "up":    { "uv": [0,0,0,8] },
            "down":  { "uv": [0,0,0,8] }
          } },
        { "type": "cube", "name": "crown_b",
          "from": [0, 6, -4], "to": [0, 16, 4],
          "rotation": [0, -45, 0], "origin": [0, 6, 0],
          "faces": {
            "east":  { "uv": [0,0,8,10], "texture": 1 },
            "west":  { "uv": [0,0,8,10], "texture": 1 }
          } }
      ]
    }"#;

    fn chess_piece() -> BbModel {
        serde_json::from_str(CHESS_PIECE).expect("fixture should parse")
    }

    #[test]
    fn pedestal_is_a_box_and_crowns_are_raw() {
        let els = import(&chess_piece(), "chess_piece");
        assert_eq!(els.len(), 3);
        assert!(matches!(els[0], Element::Box(_)));
        assert!(matches!(els[1], Element::Raw { .. }));
        assert!(matches!(els[2], Element::Raw { .. }));
    }

    /// Four of six faces on each blade are untextured. Dropping them is what
    /// makes the X-cross two-sided rather than a degenerate solid.
    #[test]
    fn untextured_faces_are_dropped() {
        let els = import(&chess_piece(), "chess_piece");
        let Element::Raw { quads, collider } = &els[1] else { panic!("expected raw") };
        assert_eq!(quads.len(), 2);
        assert!(collider.is_empty(), "a zero-thickness blade must not carry a collider");
    }

    /// The whole point of the exercise: this must fit inside its own voxel.
    #[test]
    fn geometry_stays_inside_the_unit_cube() {
        let mut arena = ModelArena::new();
        let id = arena.bake(&import(&chess_piece(), "chess_piece"), BlockRotation::IDENTITY);
        for q in arena.quads(id) {
            for v in q.verts {
                assert!(
                    v.min_element() >= -1e-4 && v.max_element() <= 1.0 + 1e-4,
                    "vertex {v:?} escaped the unit cube"
                );
            }
        }
    }

    /// A chess piece is a thin thing in the middle of a voxel: it covers no
    /// boundary, so it must never let a neighbour drop a face. This falls out
    /// of the occlusion rasteriser rather than being asserted anywhere.
    #[test]
    fn chess_piece_occludes_nothing() {
        let mut arena = ModelArena::new();
        let id = arena.bake(&import(&chess_piece(), "chess_piece"), BlockRotation::IDENTITY);
        assert_eq!(arena.model(id).occlusion, 0);
    }

    /// The pedestal sits flush on the floor, so its underside is cullable
    /// even though the model as a whole occludes nothing. Derived by
    /// `box_quads`, not declared here.
    #[test]
    fn pedestal_underside_is_tagged_cullable() {
        let els = import(&chess_piece(), "chess_piece");
        let Element::Box(b) = &els[0] else { panic!("expected box") };
        let mut quads = Vec::new();
        box_quads(b, &mut quads);
        let down = quads.iter().find(|q| q.normal == Vec3::NEG_Y).unwrap();
        assert_eq!(down.cull.dir(), Some(Direction::Down));
    }

    /// Reversed rectangles are mirroring, not sloppiness. The pedestal's top
    /// face reverses both axes, and the imported UVs must reverse with it.
    #[test]
    fn reversed_uv_rectangles_stay_reversed() {
        let doc = chess_piece();
        let face = doc.elements[0].faces.up.as_ref().unwrap();
        let uvs = face_uvs(&doc, face);
        // uv rect is [6,12] -> [0,6]: u decreases, v decreases.
        assert!(uvs[2].x < uvs[0].x, "u should run backwards");
        assert!(uvs[1].y > uvs[3].y, "v should run backwards");
    }

    /// Two elements differing only by the sign of their yaw are distinct
    /// geometry, so the arena must not collapse them.
    #[test]
    fn mirrored_blades_are_distinct_models() {
        let mut arena = ModelArena::new();
        let els = import(&chess_piece(), "chess_piece");
        let a = arena.bake(std::slice::from_ref(&els[1]), BlockRotation::IDENTITY);
        let b = arena.bake(std::slice::from_ref(&els[2]), BlockRotation::IDENTITY);
        assert_ne!(a, b);
    }
}