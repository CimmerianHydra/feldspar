use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Mesh, PrimitiveTopology};
use bevy::{prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef};

use crate::sim::player::{LookTarget, PlayerLookTarget};
use crate::space::VoxelWorld;
use crate::voxel::BLOCK_SIZE;
use crate::GameplaySet;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – LINE MATERIAL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Keeping the highlight slightly larger than the block avoids z-fighting.
pub const HIGHLIGHT_EPSILON: f32 = 1.01;

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
        // Sides (CCW)
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE HIGHLIGHT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct BlockHighlight;

fn spawn_block_highlight_sys(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LineMaterial>>,
) {
    let shape = build_cuboid_of_lines(BLOCK_SIZE * HIGHLIGHT_EPSILON);

    commands.spawn((
        Mesh3d(meshes.add(shape)),
        MeshMaterial3d(materials.add(LineMaterial {
            color: LinearRgba::WHITE,
        })),
        Transform::default(),
        Visibility::Hidden, // Start hidden until we have a block to highlight.
        BlockHighlight,
        bevy::light::NotShadowCaster,
    ));
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
            .add_systems(Update, update_block_highlight_sys.in_set(GameplaySet::Present));
    }
}
