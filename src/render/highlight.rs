use bevy::asset::RenderAssetUsages;
use bevy::math::Mat3;
use bevy::mesh::{Mesh, PrimitiveTopology};
use bevy::{prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef};

use crate::content::block::BlockRegistry;
use crate::sim::behaviors::blocks::face_targeted::FaceTargeted;
use crate::sim::player::{LookTarget, PlayerLookTarget};
use crate::space::VoxelWorld;
use crate::voxel::{
    dir_index, BlockID, Direction, FaceCell, ALL_DIRECTIONS, BLOCK_SIZE, CELL_SIZE,
};
use crate::GameplaySet;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – LINE MATERIAL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Keeping the highlight slightly larger than the block avoids z-fighting.
pub const HIGHLIGHT_EPSILON: f32 = 1.01;

/// How far off the face the interaction grid floats, in block units. Larger
/// than the cube's epsilon so the grid always reads as being in front of the
/// wireframe rather than tangled in it.
pub const FACE_OVERLAY_LIFT: f32 = 0.01;

const LINE_SHADER_PATH: &str = "shaders\\line_material.wgsl";

#[derive(Asset, TypePath, Default, AsBindGroup, Debug, Clone)]
pub struct LineMaterial {
    #[uniform(0)]
    color: LinearRgba,
}

impl Material for LineMaterial {
    fn fragment_shader() -> ShaderRef {
        LINE_SHADER_PATH.into()
    }
}

/// A list of lines with a start and end position.
#[derive(Debug, Clone)]
struct LineList {
    lines: Vec<(Vec3, Vec3)>,
}

impl From<LineList> for Mesh {
    fn from(line: LineList) -> Self {
        let vertices: Vec<_> = line.lines.into_iter().flat_map(|(a, b)| [a, b]).collect();

        Mesh::new(
            // This tells wgpu that the positions are a list of lines
            // where every pair is a start and end point.
            PrimitiveTopology::LineList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    }
}

fn build_cuboid_of_lines(side_length: f32) -> LineList {
    let s_l = side_length;
    let array = [
        // Top face (CCW)
        (Vec3::new(0.0, s_l, 0.0), Vec3::new(0.0, s_l, s_l)),
        (Vec3::new(0.0, s_l, s_l), Vec3::new(s_l, s_l, s_l)),
        (Vec3::new(s_l, s_l, s_l), Vec3::new(s_l, s_l, 0.0)),
        (Vec3::new(s_l, s_l, 0.0), Vec3::new(0.0, s_l, 0.0)),
        // 4 Edges on the Sides (CCW)
        (Vec3::new(0.0, s_l, 0.0), Vec3::new(0.0, 0.0, 0.0)),
        (Vec3::new(0.0, s_l, s_l), Vec3::new(0.0, 0.0, s_l)),
        (Vec3::new(s_l, s_l, s_l), Vec3::new(s_l, 0.0, s_l)),
        (Vec3::new(s_l, s_l, 0.0), Vec3::new(s_l, 0.0, 0.0)),
        // Bottom face (CCW)
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, s_l)),
        (Vec3::new(0.0, 0.0, s_l), Vec3::new(s_l, 0.0, s_l)),
        (Vec3::new(s_l, 0.0, s_l), Vec3::new(s_l, 0.0, 0.0)),
        (Vec3::new(s_l, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)),
    ];

    LineList { lines: array.into() }
}

/// The four interior lines of a 3×3 grid on a unit square centred at the
/// origin, in the XY plane. The border is not drawn — the cube wireframe
/// already provides it.
fn build_face_grid_of_lines() -> LineList {
    let third = 1.0 / 6.0; // ±1/6 from centre = the two interior boundaries.
    let half = 0.5;

    LineList {
        lines: vec![
            // Vertical.
            (Vec3::new(-third, -half, 0.0), Vec3::new(-third, half, 0.0)),
            (Vec3::new(third, -half, 0.0), Vec3::new(third, half, 0.0)),
            // Horizontal.
            (Vec3::new(-half, -third, 0.0), Vec3::new(half, -third, 0.0)),
            (Vec3::new(-half, third, 0.0), Vec3::new(half, third, 0.0)),
        ],
    }
}

/// A unit square outline centred at the origin, in the XY plane. Scaled to
/// a third and translated to mark the hovered cell.
fn build_square_of_lines() -> LineList {
    let h = 0.5;
    let corners = [
        Vec3::new(-h, -h, 0.0),
        Vec3::new(-h, h, 0.0),
        Vec3::new(h, h, 0.0),
        Vec3::new(h, -h, 0.0),
    ];

    LineList {
        lines: (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect(),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – MATERIALS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Every colour the highlight can wear, built once.
///
/// Swapping a `MeshMaterial3d` handle is cheap and lets the cell indicator
/// state which face a click will address — turning "which corner does what"
/// from folklore into something visible. The palette follows the gizmo
/// convention (X red, Y green, Z blue), light for the positive direction and
/// dark for its opposite, so the mapping is guessable before it is learned.
#[derive(Resource)]
struct HighlightMaterials {
    plain: Handle<LineMaterial>,
    /// Indexed by `dir_index`.
    per_face: [Handle<LineMaterial>; 6],
    /// The target is illegal for this block's orientation mode.
    rejected: Handle<LineMaterial>,
}

impl HighlightMaterials {
    fn for_face(&self, face: Direction) -> Handle<LineMaterial> {
        self.per_face[dir_index(face)].clone()
    }
}

fn face_colour(face: Direction) -> LinearRgba {
    match face {
        Direction::East  => LinearRgba::rgb(1.00, 0.25, 0.25),
        Direction::West  => LinearRgba::rgb(0.45, 0.10, 0.10),
        Direction::Up    => LinearRgba::rgb(0.35, 1.00, 0.35),
        Direction::Down  => LinearRgba::rgb(0.12, 0.45, 0.12),
        Direction::South => LinearRgba::rgb(0.35, 0.55, 1.00),
        Direction::North => LinearRgba::rgb(0.12, 0.20, 0.50),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE HIGHLIGHT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct BlockHighlight;

/// The 3×3 subdivision drawn on the struck face of a `FaceTargeted` block.
#[derive(Component)]
pub struct FaceGridOverlay;

/// The outline around the cell the crosshair is currently over.
#[derive(Component)]
pub struct FaceCellIndicator;

fn spawn_block_highlight_sys(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LineMaterial>>,
) {
    let palette = HighlightMaterials {
        plain: materials.add(LineMaterial { color: LinearRgba::WHITE }),
        per_face: ALL_DIRECTIONS
            .map(|d| materials.add(LineMaterial { color: face_colour(d) })),
        rejected: materials.add(LineMaterial {
            color: LinearRgba::rgb(0.30, 0.30, 0.30),
        }),
    };

    let cube = build_cuboid_of_lines(BLOCK_SIZE * HIGHLIGHT_EPSILON);

    commands.spawn((
        Mesh3d(meshes.add(cube)),
        MeshMaterial3d(palette.plain.clone()),
        Transform::default(),
        Visibility::Hidden, // Start hidden until we have a block to highlight.
        BlockHighlight,
        bevy::light::NotShadowCaster,
    ));

    // The grid owns the face frame. Everything below is positioned relative
    // to it, which is why the indicator is a child: one transform is
    // computed per frame, not two, and hiding the parent hides both.
    let grid = commands
        .spawn((
            Mesh3d(meshes.add(build_face_grid_of_lines())),
            MeshMaterial3d(palette.plain.clone()),
            Transform::default(),
            Visibility::Hidden,
            FaceGridOverlay,
            bevy::light::NotShadowCaster,
        ))
        .id();

    // `Visibility::Inherited` and never touched again — the parent's state
    // is the only source of truth for whether the overlay is showing.
    commands.spawn((
        Mesh3d(meshes.add(build_square_of_lines())),
        MeshMaterial3d(palette.plain.clone()),
        Transform::default(),
        Visibility::Inherited,
        ChildOf(grid),
        FaceCellIndicator,
        bevy::light::NotShadowCaster,
    ));

    commands.insert_resource(palette);
}

/// Pure presentation: it reads where the player is looking and draws a box
/// there. It never raycasts, and nothing downstream of it can tell whether
/// it ran.
fn update_block_highlight_sys(
    mut highlight: Query<(&mut Transform, &mut Visibility), With<BlockHighlight>>,
    look_target: Res<PlayerLookTarget>,
    voxel_world: VoxelWorld,
) {
    let Ok((mut transform, mut visibility)) = highlight.single_mut() else { return };

    let Some(LookTarget::Block { at, .. }) = look_target.target else {
        *visibility = Visibility::Hidden;
        return;
    };

    let Some(mut world_tf) = voxel_world.world_transform(at) else {
        *visibility = Visibility::Hidden;
        return;
    };

    // Nudge outward along the space's own axes so the wireframe doesn't
    // z-fight, then re-center on the block.
    let offset = Vec3::splat(0.5 * (1.0 - HIGHLIGHT_EPSILON));
    world_tf.translation += world_tf.rotation * offset;
    world_tf.scale *= HIGHLIGHT_EPSILON;

    *transform = world_tf;
    *visibility = Visibility::Visible;
}

/// What the overlay needs to draw itself this frame.
struct FaceGridState {
    /// World transform of the struck face: centred on it, rotated into its
    /// own frame, scaled to one block.
    frame: Transform,
    /// The cell the crosshair is over.
    cell: FaceCell,
    /// The face that cell addresses — what a click will act on.
    target_face: Direction,
    /// False when the player is holding a configuring tool and this block's
    /// orientation mode refuses `target_face`. The click will be swallowed,
    /// and the overlay should say so before it happens.
    legal: bool,
}

/// `None` means "draw nothing" — no block, not face-targeted, or its chunk
/// went away between the raycast and now.
fn resolve_face_grid(
    look_target: &PlayerLookTarget,
    block_registry: &BlockRegistry,
    voxel_world: &VoxelWorld,
) -> Option<FaceGridState> {
    let (at, voxel, face) = look_target.block()?;

    let definition = block_registry.get(BlockID(voxel.id()));
    if !definition.behaviors.has::<FaceTargeted>() { return None; }

    let world_tf = voxel_world.world_transform(at)?;

    let cell = look_target.cell_or_centre();
    let target_face = cell.resolve(face);

    // ── The face frame ───────────────────────────────────────────────────
    //
    // `world_transform` hands back the block's *corner*, the space's
    // rotation, and the space's scale. So the offset to the face centre is
    // measured from that corner, in space-local block units, and rotated
    // into world space on the way out. Uniform scale is assumed, same as the
    // cube highlight above.
    let offset = Vec3::splat(0.5) + face.as_vec3() * (0.5 + FACE_OVERLAY_LIFT);

    // `Mat3::from_cols(right, up, normal)` is a valid rotation because the
    // basis is orthonormal and right-handed — asserted in `face_grid`'s
    // tests, which is exactly why no handedness fixup appears here.
    let basis = face.face_basis();
    let face_rotation = Quat::from_mat3(&Mat3::from_cols(
        basis.right.as_vec3(),
        basis.up.as_vec3(),
        face.as_vec3(),
    ));

    let frame = Transform {
        translation: world_tf.translation + world_tf.rotation * (offset * world_tf.scale),
        rotation: world_tf.rotation * face_rotation,
        scale: world_tf.scale,
    };

    Some(FaceGridState { frame, cell, target_face, legal: true })
}

/// Draws the interaction grid, and marks the cell the crosshair is on with
/// the colour of the face that cell addresses.
///
/// Polls rather than observing `LookCellChanged`: writing two transforms is
/// cheaper than the bookkeeping needed to decide not to.
///
/// It reads `BlockRegistry` and the held item, which looks like a lot of
/// coupling for a wireframe — but it is the same inversion as everywhere
/// else in this codebase. Content declares `FaceTargeted`; presentation
/// notices and draws something. No block file names this system, and
/// deleting it leaves a game that still wrenches correctly, just blindly.
///
/// ## On the `Without` filters
///
/// Both queries take `&mut Transform`, so Bevy needs to prove they can never
/// see the same entity. Two `With` filters are not enough for that — an
/// entity could carry both markers. The mutual `Without` pair is what makes
/// the disjointness provable, and it is why this needs no `ParamSet`.
fn update_face_grid_sys(
    mut grid: Query<
        (&mut Transform, &mut Visibility),
        (With<FaceGridOverlay>, Without<FaceCellIndicator>),
    >,
    mut indicator: Query<
        (&mut Transform, &mut MeshMaterial3d<LineMaterial>),
        (With<FaceCellIndicator>, Without<FaceGridOverlay>),
    >,
    look_target: Res<PlayerLookTarget>,
    block_registry: Res<BlockRegistry>,
    palette: Res<HighlightMaterials>,
    voxel_world: VoxelWorld,
) {
    let Ok((mut grid_tf, mut grid_vis)) = grid.single_mut() else { return };

    // One early-out, one entity to hide. The indicator inherits.
    let Some(state) = resolve_face_grid(
        &look_target,
        &block_registry,
        &voxel_world,
    ) else {
        *grid_vis = Visibility::Hidden;
        return;
    };

    *grid_tf = state.frame;
    *grid_vis = Visibility::Visible;

    let Ok((mut cell_tf, mut material)) = indicator.single_mut() else { return };

    // Purely local: the parent already carries the face frame and the
    // space's scale, so the child only has to say "a third the size, over
    // there" in the face's own 2D coordinates.
    *cell_tf = Transform::from_translation(state.cell.centre_offset().extend(0.0))
        .with_scale(Vec3::splat(CELL_SIZE));

    let wanted = if state.legal {
        palette.for_face(state.target_face)
    } else {
        palette.rejected.clone()
    };
    if material.0 != wanted {
        material.0 = wanted;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BlockHighlightPlugin;

impl Plugin for BlockHighlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<LineMaterial>::default())
            .add_systems(PreStartup, spawn_block_highlight_sys)
            // Ordered after the raycast by `GameplaySet`'s configuration in
            // `app::schedule`, not by naming the raycaster.
            .add_systems(
                Update,
                (update_block_highlight_sys, update_face_grid_sys)
                    .in_set(GameplaySet::Present),
            );
    }
}