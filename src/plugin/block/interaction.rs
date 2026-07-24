use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};
use bevy::mesh::{Mesh, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use bevy_enhanced_input::prelude::*;

use crate::plugin::block::behavior::InteractsOnSecondary;
use crate::plugin::block::entities::{BlockEntityEvent, Interactable};
use crate::plugin::controller::player::{AltFire, PrimaryFire, SecondaryFire};
use crate::plugin::geometry::meshing::BLOCK_SIZE;
use crate::plugin::inventory::player::*;
use crate::plugin::item::components::*;
use crate::plugin::loader::block_registry::*;
use crate::plugin::loader::item_registry::*;
use crate::plugin::space::prelude::*;
use crate::plugin::state::GameState;
use crate::plugin::voxel::{Direction, Voxel};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BlockInteractionPlugin;

impl Plugin for BlockInteractionPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins(MaterialPlugin::<LineMaterial>::default())

        .insert_resource(PlayerLookTarget { target: None })
        .insert_resource(PlayerHeldItems::default())

        .add_systems(PreStartup, spawn_block_highlight_sys)

        .add_systems(Update, (
                update_look_target_sys,
                update_block_highlight_sys,
            ).chain().run_if(in_state(GameState::InGame))
        )

        .add_observer(handle_primary_fire_obs)
        .add_observer(handle_secondary_fire_obs)
        .add_observer(voxel_write_obs)
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – Raycasting
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// DDARays are used for raycasting to determine which block the player is looking at.
/// It does so by applying the DDA algorithm to step through the blocks in a cubic grid.
/// This struct goes on whatever object needs to raycast into the world, and follows its
/// transform. For example, it would be a component of the player's camera.
#[derive(Debug, Component)]
pub struct DDARay {
    pub max_distance: f32,
}

fn digital_differential_analysis(origin: Vec3, direction: Vec3, max_distance: f32) -> Vec<(IVec3, Direction)> {
    // This function will perform the DDA algorithm to step through the blocks in the world and return the coordinates of the blocks that are intersected by the ray, along with the face of the block that is hit.
    // Step through the blocks in the chunk using DDA until we hit a block or exceed max_distance.
    let step = Vec3::new(
        direction.x.signum(),
        direction.y.signum(),
        direction.z.signum(),
    ); // Determine the step direction for each axis based on the ray's direction.

    // Determine how big of an increment in the ray's path corresponds to hitting the first boundary of a block in each axis.
    // This depends on the direction and origin. The minimum of these will be the first t at which we hit a block boundary.

    // Temporary function to calculate t_max for each axis.
    fn t_max_component(origin: f32, dir: f32) -> f32 {
        if dir == 0.0 { return f32::INFINITY; }
        let frac = origin.rem_euclid(1.0);
        if dir > 0.0 { (1.0 - frac) / dir } else { t_max_component(-origin, -dir) }
    }

    let mut t_max = Vec3::new(
        t_max_component(origin.x, direction.x),
        t_max_component(origin.y, direction.y),
        t_max_component(origin.z, direction.z),
    );

    // Determine the value, with sign, of the change in t when stepping through one block in each axis.
    let t_delta = Vec3::new(
        if direction.x != 0.0 { step.x / direction.x } else { f32::INFINITY },
        if direction.y != 0.0 { step.y / direction.y } else { f32::INFINITY },
        if direction.z != 0.0 { step.z / direction.z } else { f32::INFINITY },
    );
    
    // Current block coordinates of the algorithm, and result buffer.
    let mut current = origin.floor();
    let mut result: Vec<(IVec3, Direction)> = Vec::new();

    // Loop until we exceed max_distance
    loop {
        if t_max.min_element() > max_distance {
            break; // Exceeded max distance
        }

        // Take the minimum of t_max to determine which axis we step through next.
        let axis = t_max.min_position();
        let face : Direction; // Will need to determine based on the ray's direction and the block it hits.
        match axis {
            0 => {
                // Step through x axis
                current.x += step.x;
                t_max.x += t_delta.x;
                face = if step.x > 0.0 {Direction::West} else {Direction::East};
            },
            1 => {
                // Step through y axis
                current.y += step.y;
                t_max.y += t_delta.y;
                face = if step.y > 0.0 {Direction::Down} else {Direction::Up};  
            },
            2 => {
                // Step through z axis
                current.z += step.z;
                t_max.z += t_delta.z;
                face = if step.z > 0.0 {Direction::North} else {Direction::South};
            },
            _ => unreachable!(), // Result from min_element can't ever be greater than 2.
        }
        let block_coord = current.floor().as_ivec3(); // Get the block coordinates of the ray's origin.
        result.push((block_coord, face));
    };

    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – Block Highlighting
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const HIGHLIGHT_EPSILON: f32 = 1.01; // Keeping highlight block slightly larger than the block size to avoid z-fighting.
const LINE_SHADER_PATH: &str = "shaders\\line_material.wgsl"; // Line shader


#[derive(Component)]
pub struct BlockHighlight;

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

/// A list of lines with a start and end position
#[derive(Debug, Clone)]
struct LineList {
    lines: Vec<(Vec3, Vec3)>,
}

impl From<LineList> for Mesh {
    fn from(line: LineList) -> Self {
        let vertices: Vec<_> = line.lines.into_iter().flat_map(|(a, b)| [a, b]).collect();

        Mesh::new(
            // This tells wgpu that the positions are list of lines
            // where every pair is a start and end point
            PrimitiveTopology::LineList,
            RenderAssetUsages::RENDER_WORLD,
        )
        // Add the vertices positions as an attribute
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
        // Bot face(CCW)
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, s_l)),
        (Vec3::new(0.0, 0.0, s_l), Vec3::new(s_l, 0.0, s_l)),
        (Vec3::new(s_l, 0.0, s_l), Vec3::new(s_l, 0.0, 0.0)),
        (Vec3::new(s_l, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)),
    ];

    LineList {
        lines: array.into()
    }
}

pub fn spawn_block_highlight_sys(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LineMaterial>>,
) {
    let shape = build_cuboid_of_lines(BLOCK_SIZE * HIGHLIGHT_EPSILON);

    // Spawn a block highlight entity with the BlockHighlight component and a transparent material.
    commands.spawn((
        Mesh3d(meshes.add(shape)),
        MeshMaterial3d(materials.add(LineMaterial {
            color: LinearRgba::WHITE,
        })),
        Transform::default(),
        Visibility::Hidden, // Start hidden until we have a block to highlight.
        BlockHighlight,
        bevy::light::NotShadowCaster, // Prevents casting shadows
    ));
}


pub fn update_block_highlight_sys(
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
// SECTION 3 – LOOK TARGET  (was sections 3 + 5)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One variant for voxels, whatever space they're in. The old
/// `StaticVoxel` / `MovingGridVoxel` split is gone — `BlockPos` carries the
/// distinction, and no consumer downstream has to branch on it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LookTarget {
    Block { at: BlockPos, voxel: Voxel, face: Direction },
    Mob   { entity: Entity },
}

#[derive(Resource, Default)]
pub struct PlayerLookTarget {
    pub target: Option<LookTarget>,
}

/// Fired when the look target changes, for tooltips and anything else that
/// shouldn't re-raycast.
#[derive(Event, Clone, Copy, Debug)]
pub struct LookTargetChanged(pub Option<LookTarget>);

/// Casts into every voxel space and keeps the nearest hit.
///
/// The trick that makes moving grids nearly free: instead of writing a
/// second raycaster, transform the *ray* into each space's local frame and
/// run the identical DDA. A ship rotated 40 degrees is just a ray pointing
/// somewhere else as far as the algorithm is concerned.
pub fn update_look_target_sys(
    mut commands: Commands,
    mut look_target: ResMut<PlayerLookTarget>,
    rays: Query<(&DDARay, &GlobalTransform)>,
    spaces: Query<(Entity, &GlobalTransform), With<ChunkMap>>,
    voxel_world: VoxelWorld,
) {
    let Ok((ray, ray_tf)) = rays.single() else { return };

    let origin = ray_tf.translation();
    let forward = ray_tf.forward().as_vec3();

    let mut best: Option<(f32, LookTarget)> = None;

    for (space, space_tf) in spaces.iter() {
        // World ray -> space-local ray. Uniform scale is assumed; with
        // non-uniform scale the direction would need rescaling per axis.
        let inverse = space_tf.affine().inverse();
        let local_origin = inverse.transform_point3(origin) / BLOCK_SIZE;
        let local_dir = (inverse.matrix3 * forward).normalize_or_zero();
        if local_dir == Vec3::ZERO { continue; }

        let hits = digital_differential_analysis(local_origin, local_dir, ray.max_distance);

        let Some((pos, face)) = hits.into_iter().find(|(coord, _)| {
            !voxel_world.get_voxel(BlockPos::new(space, *coord)).is_air()
        }) else { continue };

        let at = BlockPos::new(space, pos);
        let voxel = voxel_world.get_voxel(at);

        // Compare in world space so hits in different frames are commensurable.
        let Some(world_pos) = voxel_world.world_position(at) else { continue };
        let distance = world_pos.distance(origin);

        if best.map_or(true, |(d, _)| distance < d) {
            best = Some((distance, LookTarget::Block { at, voxel, face }));
        }
    }

    // Mob raycasting slots in here: cast once in world space, and if the
    // hit is nearer than `best`, replace it with LookTarget::Mob.

    let new_target = best.map(|(_, t)| t);
    if new_target != look_target.target {
        look_target.target = new_target;
        commands.trigger(LookTargetChanged(new_target));
    }
}



// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – VOXEL WRITES  (the authoritative path)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Renamed from `StaticVoxelWriteRequest` — it was never really static.
#[derive(Event, Clone, Copy, Debug)]
pub struct VoxelWriteRequest {
    pub at:    BlockPos,
    pub voxel: Voxel,
}

/// The one place in the codebase that decides a block was placed or broken.
///
/// Every writer — input, worldgen, machines, explosions, save-loading —
/// funnels through here, in any space, so block-entities can never fall out
/// of sync with the voxel grid.
fn voxel_write_obs(
    event: On<VoxelWriteRequest>,
    mut commands: Commands,
    mut voxel_world: VoxelWorldMut,
) {
    let at  = event.at;
    let new = event.voxel;

    // `None` means the chunk isn't loaded: nothing written, so emit nothing.
    let Some(old) = voxel_world.set_voxel(at, new) else { return };
    if old == new { return; }

    // Break first, then place. A replace-in-place therefore tears down the
    // old block-entity before building the new one, which is the only order
    // that leaves the chunk index consistent.
    if !old.is_air() {
        commands.trigger(BlockEvent::Break { block_id: BlockID(old.id()), at });
    }
    if !new.is_air() {
        commands.trigger(BlockEvent::Place { block_id: BlockID(new.id()), at });
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – BLOCK EVENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Carries a `BlockPos`, not an `IVec3`. Consumers that need world-space
/// geometry (audio, particles) call `VoxelWorld::world_position`.
#[derive(Event, Clone, Copy, Debug)]
pub enum BlockEvent {
    Place    { block_id: BlockID, at: BlockPos },
    Break    { block_id: BlockID, at: BlockPos },
    /// Only for blocks with no entity — see `InteractsOnUse`.
    Interact { block_id: BlockID, at: BlockPos, player: Entity },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 7 – FIRE HANDLERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn handle_primary_fire_obs(
    _event: On<Start<PrimaryFire>>,
    mut commands: Commands,
    look_target: Res<PlayerLookTarget>,
) {
    let Some(LookTarget::Block { at, .. }) = look_target.target else { return };

    // No BlockEvent::Break here any more — the write observer owns that, so
    // a failed break no longer lies to the audio system.
    commands.trigger(VoxelWriteRequest { at, voxel: Voxel::AIR });
}

/// Priority chain: entity-backed interaction, then entity-less
/// interaction, then placement. Mirrors Minecraft's rule that using a block
/// beats using the item.
fn handle_secondary_fire_obs(
    event: On<Start<SecondaryFire>>,
    mut commands: Commands,
    look_target:    Res<PlayerLookTarget>,
    held_item:      Res<PlayerHeldItems>,
    block_registry: Res<BlockRegistry>,
    item_registry:  Res<ItemRegistry>,
    voxel_world:    VoxelWorld,
    interactables:  Query<(), With<Interactable>>,
) {
    let Some(LookTarget::Block { at, voxel, face }) = look_target.target else { return };

    let player   = event.context;
    let block_id = BlockID(voxel.id());

    // ── 1. Entity-backed interaction wins ────────────────────────────────
    // Add a sneak check here later: `if !sneaking { ... }`.
    if let Some(block_entity) = voxel_world.block_entity_at(at) {
        if interactables.contains(block_entity) {
            commands.trigger(BlockEntityEvent { entity: block_entity, player, at, face });
            return;
        }
    }

    // ── 2. Entity-less interaction: levers, doors, buttons ───────────────
    if block_registry.get(block_id).components.has::<InteractsOnSecondary>() {
        commands.trigger(BlockEvent::Interact { block_id, at, player });
        return;
    }

    // ── 3. Otherwise place whatever the held item places ─────────────────
    let Some(held) = held_item.right_hand else { return };
    let def = item_registry.get(held.id);

    let Some(PlacesBlock { block_id }) = def.components.get::<PlacesBlock>().copied() else { return };

    let shape = block_registry.get(block_id).shape.clone();

    // `at.neighbor(face)` stays inside the same space, so right-clicking the
    // hull of a ship builds onto the ship, not into the air behind it.
    commands.trigger(VoxelWriteRequest {
        at:    at.neighbor(face),
        voxel: Voxel::new(block_id.0, shape, face),
    });
}
