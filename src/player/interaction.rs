use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::content::block::{BlockRegistry, InteractsOnSecondary};
use crate::content::item::{ItemRegistry, PlacesBlock};
use crate::player::input::{PrimaryFire, SecondaryFire};
use crate::sim::block_entity::{BlockEntityEvent, Interactable};
use crate::sim::player::{
    LookTarget, LookTargetChanged, PlayerHeldItems, PlayerLookTarget,
};
use crate::space::{BlockEvent, BlockPos, ChunkMap, VoxelWorld, VoxelWriteRequest};
use crate::voxel::{BlockID, BlockRotation, Direction, Voxel, BLOCK_SIZE};
use crate::GameplaySet;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – RAYCASTING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A DDA ray is used to determine which block the holder is looking at, by
/// stepping through the blocks of a cubic grid. Put it on whatever needs to
/// raycast into the world — typically the player's camera — and it follows
/// that entity's transform.
#[derive(Debug, Component)]
pub struct DDARay {
    pub max_distance: f32,
}

/// Step through the blocks intersected by a ray, returning each coordinate
/// with the face that was entered through.
fn digital_differential_analysis(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Vec<(IVec3, Direction)> {
    // Determine the step direction for each axis based on the ray's direction.
    let step = Vec3::new(
        direction.x.signum(),
        direction.y.signum(),
        direction.z.signum(),
    );

    // How big an increment in the ray's path corresponds to hitting the first
    // block boundary on each axis. The minimum of these is the first t at
    // which we cross a boundary.
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

    // The signed change in t when stepping through one block on each axis.
    let t_delta = Vec3::new(
        if direction.x != 0.0 { step.x / direction.x } else { f32::INFINITY },
        if direction.y != 0.0 { step.y / direction.y } else { f32::INFINITY },
        if direction.z != 0.0 { step.z / direction.z } else { f32::INFINITY },
    );

    let mut current = origin.floor();
    let mut result: Vec<(IVec3, Direction)> = Vec::new();

    loop {
        if t_max.min_element() > max_distance {
            break; // Exceeded max distance
        }

        // The minimum of t_max says which axis we step through next.
        let axis = t_max.min_position();
        let face: Direction;
        match axis {
            0 => {
                current.x += step.x;
                t_max.x += t_delta.x;
                face = if step.x > 0.0 { Direction::West } else { Direction::East };
            }
            1 => {
                current.y += step.y;
                t_max.y += t_delta.y;
                face = if step.y > 0.0 { Direction::Down } else { Direction::Up };
            }
            2 => {
                current.z += step.z;
                t_max.z += t_delta.z;
                face = if step.z > 0.0 { Direction::North } else { Direction::South };
            }
            _ => unreachable!(), // min_position can never exceed 2
        }
        result.push((current.floor().as_ivec3(), face));
    }

    result
}

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

        if best.is_none_or(|(d, _)| distance < d) {
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
// SECTION 2 – FIRE HANDLERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn handle_primary_fire_obs(
    _event: On<Start<PrimaryFire>>,
    mut commands: Commands,
    look_target: Res<PlayerLookTarget>,
) {
    let Some(LookTarget::Block { at, .. }) = look_target.target else { return };

    // No `BlockEvent::Break` here — the write observer owns that, so a failed
    // break no longer lies to the audio system.
    commands.trigger(VoxelWriteRequest { at, voxel: Voxel::AIR });
}

/// Priority chain: entity-backed interaction, then entity-less interaction,
/// then placement. Mirrors Minecraft's rule that using a block beats using
/// the item.
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
    // TODO: add a sneak check here later.
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

    // ── 3. Otherwise do whatever the held item wants to do ───────────────
    let Some(held) = held_item.right_hand else { return };
    let def = item_registry.get(held.id);

    // -- 3.1 The item places a block
    let Some(PlacesBlock { block_id }) = def.components.get::<PlacesBlock>().copied() else { return };

    // `at.neighbor(face)` stays inside the same space, so right-clicking the
    // hull of a ship builds onto the ship, not into the air behind it.
    commands.trigger(VoxelWriteRequest {
        at:    at.neighbor(face),
        voxel: Voxel::new(block_id.0, BlockRotation::IDENTITY, 0),
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct PlayerInteractionPlugin;

impl Plugin for PlayerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_look_target_sys.in_set(GameplaySet::Input))
            .add_observer(handle_primary_fire_obs)
            .add_observer(handle_secondary_fire_obs);
    }
}
