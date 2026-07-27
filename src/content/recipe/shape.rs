//! Spatial recipe topology: which arrangements of items count as a shape,
//! and how a point cloud is classified into one.
//!
//! Recipes are determined by the number of items in the crafting space and
//! their spatial distribution — a triangle recipe and a line recipe differ.
//! All predicates below are functions of distances and angles only, so the
//! recognizer is blind to translation, rotation, reflection, and scale.
//!
//! This is vocabulary for recipes, so it sits with the recipe registry rather
//! than with the crafting machine that consumes it — the registry keys every
//! entry on a `CraftShape`, and `sim::crafting` depends on the registry rather
//! than the other way round.

use bevy::prelude::*;
use serde::Deserialize;

/// Relative tolerances (dimensionless), tunable in one place.
const LINE_TOLERANCE:     f32 = 0.20; // perpendicular deviation / span
const TRIANGLE_TOLERANCE: f32 = 0.20; // (max_side - min_side) / max_side
#[allow(dead_code)]
const ANGLE_TOLERANCE:    f32 = 0.20; // radians; reserved for L4 / Y4

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftShape {
    Dot,
    Line2,
    Line3,
    Triangle,
    // Reserved for the next pass:
    Line4,
    L4,
    Y4,
    Square,
}

impl CraftShape {
    /// How many vertices this shape has.
    pub fn arity(self) -> usize {
        match self {
            CraftShape::Dot => 1,
            CraftShape::Line2 => 2,
            CraftShape::Line3 | CraftShape::Triangle => 3,
            CraftShape::Line4 | CraftShape::L4 | CraftShape::Y4 | CraftShape::Square => 4,
        }
    }

    /// Edges between conventional vertex indices — pure UI topology.
    /// Extend together with `arity` and `symmetry_perms` when adding
    /// the 4-point shapes.
    pub fn edges(self) -> &'static [(usize, usize)] {
        match self {
            CraftShape::Dot      => &[],
            CraftShape::Line2    => &[(0, 1)],
            CraftShape::Line3    => &[(0, 1), (1, 2)],
            CraftShape::Triangle => &[(0, 1), (1, 2), (2, 0)],
            _ => &[], // 4-point shapes: next pass
        }
    }

    /// The shape's symmetry group as permutations of vertex indices in
    /// conventional vertex order. Used identically at recipe registration
    /// and at match time — sharing these tables is what guarantees rotated
    /// / mirrored arrangements resolve to the same recipe.
    pub fn symmetry_perms(self) -> &'static [&'static [usize]] {
        match self {
            CraftShape::Dot   => &[&[0]],
            CraftShape::Line2 => &[&[0, 1], &[1, 0]],
            // Convention: [end, middle, end]. Symmetry = reversal.
            CraftShape::Line3 => &[&[0, 1, 2], &[2, 1, 0]],
            // Equilateral: full dihedral group = every permutation.
            CraftShape::Triangle => &[
                &[0, 1, 2], &[0, 2, 1], &[1, 0, 2],
                &[1, 2, 0], &[2, 0, 1], &[2, 1, 0],
            ],
            _ => &[], // registration rejects shapes without tables
        }
    }
}

/// Classify a point cloud. On success returns the shape plus the indices of
/// `points` in the shape's conventional vertex order (e.g. Line3 comes back
/// as [end, middle, end]) — the canonicalizer consumes that ordering.
pub fn recognize(points: &[Vec2]) -> Option<(CraftShape, Vec<usize>)> {
    match points.len() {
        1 => Some((CraftShape::Dot, vec![0])),
        2 => Some((CraftShape::Line2, vec![0, 1])),
        3 => recognize_three(points),
        _ => None, // 4-point shapes: next pass
    }
}

fn recognize_three(p: &[Vec2]) -> Option<(CraftShape, Vec<usize>)> {
    let d01 = p[0].distance(p[1]);
    let d12 = p[1].distance(p[2]);
    let d02 = p[0].distance(p[2]);

    // The farthest pair are the candidate line ends; the leftover is the
    // candidate middle.
    let (a, b, m, span) =
        if d01 >= d12 && d01 >= d02 { (0, 1, 2, d01) }
        else if d12 >= d01 && d12 >= d02 { (1, 2, 0, d12) }
        else { (0, 2, 1, d02) };

    // Degenerate: everything on top of everything.
    if span <= f32::EPSILON { return None; }

    // Test 1: collinearity — perpendicular deviation of the middle point
    // from the end-to-end line, relative to the span.
    let line_vec  = p[b] - p[a];
    let point_vec = p[m] - p[a];
    let deviation = line_vec.perp_dot(point_vec).abs() / (span * span);

    if deviation < LINE_TOLERANCE {
        return Some((CraftShape::Line3, vec![a, m, b]));
    }

    // Test 2: equilateral triangle — all sides equal, relatively.
    let max = d01.max(d12).max(d02);
    let min = d01.min(d12).min(d02);
    if (max - min) / max <= TRIANGLE_TOLERANCE {
        // Full symmetry: any order is a valid conventional order.
        return Some((CraftShape::Triangle, vec![0, 1, 2]));
    }

    None
}
