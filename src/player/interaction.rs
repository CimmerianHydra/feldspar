use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::content::block::{BlockRegistry, FaceTargeted, Orientable};
use crate::content::item::{ItemRegistry, OrientsBlocks, PlacesBlock};
use crate::player::input::{PrimaryFire, SecondaryFire};
use crate::sim::block_entity::{BlockEntityEvent, Interactable};
use crate::sim::player::{
    LookCellChanged, LookTarget, LookTargetChanged, PlayerHeldItems, PlayerLookTarget,
};
use crate::space::{BlockEvent, BlockPos, ChunkMap, VoxelWorld, VoxelWriteRequest};
use crate::voxel::{BlockID, BlockRotation, Direction, FaceCell, Voxel, BLOCK_SIZE};
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

/// One voxel hit, in the space's own frame.
#[derive(Clone, Copy, Debug)]
pub struct VoxelHit {
    pub coord: IVec3,
    /// The face the ray entered through.
    pub face: Direction,
    /// Ray parameter at entry. `origin + direction * distance` is the exact
    /// point on the face — which is the whole reason this type exists.
    pub distance: f32,
}

/// Walk the voxels a ray passes through and return the first one `solid`
/// accepts.
///
/// Returns on the first hit rather than materialising every voxel out to
/// `max_distance`, which used to allocate a `Vec` per space per frame.
///
/// The voxel the ray *starts* inside is never tested: it has no entry face,
/// so there would be nothing meaningful to report. That matches the old
/// behaviour — a player with their head inside a block sees through it.
pub fn raycast_voxels(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    mut solid: impl FnMut(IVec3) -> bool,
) -> Option<VoxelHit> {
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

    loop {
        // The minimum of t_max is both the axis we step through next and the
        // parameter at which we enter the next voxel. Read it *before* the
        // increment — that value is the entry point, and it is the only new
        // information this rewrite needed.
        let axis = t_max.min_position();
        let entry = t_max[axis];

        if entry > max_distance {
            return None;
        }

        let face = match axis {
            0 => {
                current.x += step.x;
                t_max.x += t_delta.x;
                if step.x > 0.0 { Direction::West } else { Direction::East }
            }
            1 => {
                current.y += step.y;
                t_max.y += t_delta.y;
                if step.y > 0.0 { Direction::Down } else { Direction::Up }
            }
            2 => {
                current.z += step.z;
                t_max.z += t_delta.z;
                if step.z > 0.0 { Direction::North } else { Direction::South }
            }
            _ => unreachable!(), // min_position can never exceed 2
        };

        let coord = current.floor().as_ivec3();
        if solid(coord) {
            return Some(VoxelHit { coord, face, distance: entry });
        }
    }
}

/// Casts into every voxel space and keeps the nearest hit.
///
/// The trick that makes moving grids nearly free: instead of writing a
/// second raycaster, transform the *ray* into each space's local frame and
/// run the identical DDA. A ship rotated 40 degrees is just a ray pointing
/// somewhere else as far as the algorithm is concerned.
///
/// Now also resolves the sub-face cell. That happens *here*, once, with one
/// writer — not in the two places that consume it — so the highlight can
/// never draw a cell the dispatcher would resolve differently.
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

    // (world distance, coarse target, space-local hit point)
    let mut best: Option<(f32, LookTarget, Vec3)> = None;

    for (space, space_tf) in spaces.iter() {
        // World ray -> space-local ray. Uniform scale is assumed; with
        // non-uniform scale the direction would need rescaling per axis.
        let inverse = space_tf.affine().inverse();
        let local_origin = inverse.transform_point3(origin) / BLOCK_SIZE;
        let local_dir = (inverse.matrix3 * forward).normalize_or_zero();
        if local_dir == Vec3::ZERO { continue; }

        let Some(hit) = raycast_voxels(local_origin, local_dir, ray.max_distance, |coord| {
            !voxel_world.get_voxel(BlockPos::new(space, coord)).is_air()
        }) else { continue };

        let at = BlockPos::new(space, hit.coord);
        let voxel = voxel_world.get_voxel(at);

        // The exact point on the face, in the space's own frame.
        let local_point = local_origin + local_dir * hit.distance;

        // Compare in world space so hits in different frames are
        // commensurable. Using the real hit point rather than the block
        // centre also fixes the old tie-break, where a distant block struck
        // near its front face could lose to a nearer one struck near its back.
        let world_point = space_tf.transform_point(local_point * BLOCK_SIZE);
        let distance = world_point.distance(origin);

        if best.is_none_or(|(d, _, _)| distance < d) {
            best = Some((
                distance,
                LookTarget::Block { at, voxel, face: hit.face },
                local_point,
            ));
        }
    }

    // Mob raycasting slots in here: cast once in world space, and if the
    // hit is nearer than `best`, replace it with LookTarget::Mob.

    let new_target = best.map(|(_, target, _)| target);
    let new_hit = best.map(|(_, _, point)| point);

    // The cell is derived from the hit point and the face, and cached
    // alongside both.
    let new_cell = match (new_target, new_hit) {
        (Some(LookTarget::Block { at, face, .. }), Some(point)) => {
            Some(FaceCell::from_local_hit(point - at.pos.as_vec3(), face))
        }
        _ => None,
    };

    // Coarse and fine are a strict hierarchy: a coarse change is always a
    // fine change too. Testing them independently rather than nesting is
    // what makes "moved from one block's centre cell to another's" fire.
    let coarse_changed = new_target != look_target.target;
    let fine_changed = coarse_changed || new_cell != look_target.cell;

    look_target.target = new_target;
    look_target.hit = new_hit;
    look_target.cell = new_cell;

    if coarse_changed {
        commands.trigger(LookTargetChanged(new_target));
    }
    if fine_changed {
        commands.trigger(LookCellChanged { target: new_target, cell: new_cell });
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
    let Some((at, _, _)) = look_target.block() else { return };

    // Breaking stays coarse: the 3×3 grid addresses *interactions*, and a
    // corner click that broke the block behind the one you were aiming at
    // would be indefensible.
    commands.trigger(VoxelWriteRequest { at, voxel: Voxel::AIR });
}

/// Priority chain: reorientation, entity-backed interaction, entity-less
/// interaction, then placement. Mirrors Minecraft's rule that using a block
/// beats using the item, with the configuring tool sitting above all of it.
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
    let Some((at, voxel, face)) = look_target.block() else { return };

    let player   = event.context;
    let block_id = BlockID(voxel.id());
    let def      = block_registry.get(block_id);

    // Which face this interaction addresses. Computed once, at the top,
    // and handed to every rung below — so nothing downstream has to know
    // that face targeting exists, and a handler that ignores `target_face`
    // behaves exactly as it did before.
    let target_face = if def.components.has::<FaceTargeted>() {
        look_target.cell_or_centre().resolve(face)
    } else {
        face
    };

    // ── 0. A configuring tool consumes the interaction ───────────────────
    //
    // Above the entity rung on purpose: without this, wrenching an
    // orientable barrel would open its screen and the barrel could never be
    // turned. The tool swallows the click whether or not the block responds,
    // which is predictable — a wrench never places a block by accident.
    //
    // This is the seam to widen if a tool grows more configuring jobs:
    // match on the item's capability set here, not on new rungs below.
    if let Some(held) = held_item.right_hand {
        if item_registry.get(held.id).components.has::<OrientsBlocks>() {
            if let Some(orientable) = def.components.get::<Orientable>() {
                if let Some(rotation) = orientable.rotation_for(voxel.rotation(), target_face) {
                    // Already pointing there — skip the write rather than
                    // emit a no-op that dirties a chunk and wakes the mesher.
                    if rotation != voxel.rotation() {
                        commands.trigger(VoxelWriteRequest {
                            at,
                            voxel: voxel.with_rotation(rotation),
                        });
                    }
                }
                // `None` means the mode rejects that face — the highlight
                // already greyed the cell out, so eat the click silently.
            }
            return;
        }
    }

    // ── 1. Entity-backed interaction wins ────────────────────────────────
    // TODO: add a sneak check here later.
    if let Some(block_entity) = voxel_world.block_entity_at(at) {
        if interactables.contains(block_entity) {
            commands.trigger(BlockEntityEvent {
                entity: block_entity,
                player,
                at,
                face,
                target_face,
            });
            return;
        }
    }

    // ── 3. Otherwise do whatever the held item wants to do ───────────────
    let Some(held) = held_item.right_hand else { return };
    let item_def = item_registry.get(held.id);

    // ── 3.1 The item places a block
    let Some(PlacesBlock { block_id }) =
        item_def.components.get::<PlacesBlock>().copied() else { return };

    // Placement uses the *geometric* face, never the target face. Clicking
    // the top-left corner of a wall should build against that wall, not
    // inside it.
    //
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    /// The hit point must land on the face plane, or the cell derivation is
    /// reading a point inside the block and every cell comes out centre.
    #[test]
    fn hit_point_lies_on_the_entry_face() {
        let origin = Vec3::new(0.5, 0.5, -4.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);

        let hit = raycast_voxels(origin, dir, 16.0, |c| c == IVec3::ZERO).unwrap();
        assert_eq!(hit.face, Direction::North);

        let point = origin + dir * hit.distance;
        assert!((point.z - 0.0).abs() < 1e-5, "entered at z = {}", point.z);
    }

    /// An off-axis ray must produce the cell you'd expect from where it
    /// visually lands, not from the block centre.
    #[test]
    fn off_centre_hit_produces_off_centre_cell() {
        // Aimed high and to the right of a block's north face.
        let origin = Vec3::new(0.8, 0.9, -4.0);
        let dir = Vec3::Z;

        let hit = raycast_voxels(origin, dir, 16.0, |c| c == IVec3::ZERO).unwrap();
        let point = origin + dir * hit.distance;
        let cell = FaceCell::from_local_hit(point, hit.face);

        // North's basis has `right = West`, so a high +X hit is on the
        // grid's *left*, top row.
        assert_eq!(cell.row, 0);
        assert_eq!(cell.col, 0);
    }

    #[test]
    fn ray_that_hits_nothing_reports_nothing() {
        let hit = raycast_voxels(Vec3::ZERO, Vec3::Y, 8.0, |_| false);
        assert!(hit.is_none());
    }

    /// The starting voxel is skipped — a player inside a block sees out.
    #[test]
    fn origin_voxel_is_not_reported() {
        let hit = raycast_voxels(Vec3::splat(0.5), Vec3::Z, 8.0, |c| c == IVec3::ZERO);
        assert!(hit.is_none());
    }
}