use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};
use bevy::mesh::{Mesh, PrimitiveTopology};
use bevy::asset::{RenderAssetUsages};

use crate::plugin::block_registry::BlockRegistry;
use crate::plugin::chunk::{StaticWorldAccess, StaticWorldAccessMut};
use crate::plugin::inventory::main::*;
use crate::plugin::inventory::player_inventory::*;
use crate::plugin::inventory::item_registry::*;
use crate::plugin::state::GameUpdateState;
use crate::plugin::voxel::{Voxel, Direction};
use crate::plugin::geometry::meshing::{BLOCK_SIZE};
use crate::plugin::dimension::DimensionId;
use crate::plugin::controller::main::{MouseEvent, MouseAction};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – Plugin and Component Definitions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BlockInteractionPlugin;

impl Plugin for BlockInteractionPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to block interaction here
        app
        .add_plugins(MaterialPlugin::<LineMaterial>::default())

        // Keeps track of which block or other entity is being looked at right now
        // Figured out from the combo of all raycasting systems (currently just DDA)
        .insert_resource(PlayerLookTarget{ target: None })
        .insert_resource(PlayerHeldItems{ right_hand: None, left_hand: None })

        .add_systems(PreStartup, spawn_block_highlight_sys)

        .add_systems(Update, (
                cast_static_dda_ray_sys,
                update_block_highlight_sys
            ).run_if(in_state(GameUpdateState::Running))
        )

        .add_observer(update_look_target_obs)
        .add_observer(update_held_items_obs)
        .add_observer(handle_mouse_interaction_obs)
        .add_observer(static_voxel_write_obs)
        ;
    }
}

/// Current situation:
/// A FreeCamera has a DDARay component attached to it. Every frame we send out a DDARayResult event.
/// This event is used to update the position and visibility of the BlockHighlight entity as well as
/// establish which block is gong to be affected by right clicks or left clicks.
/// 
/// In the future, we need to have multiple different types of raycasting:
///  - To check if the player is looking at a mob
///  - If not, if the player is looking at a moving grid
///  - If not, check the static world.
/// Then we need to send out an event with the result. This will inform the highlight, any tooltips, and so on.

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – Raycasting
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// DDARays are used for raycasting to determine which block the player is looking at.
/// It does so by applying the DDA algorithm to step through the blocks in a cubic grid.
#[derive(Debug, Component)]
pub struct DDARay {
    pub max_distance: f32,
}

#[derive(Event)]
pub struct DDAResult {
    pub hits: Vec<(IVec3, Direction)>
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

fn cast_static_dda_ray_sys(
    mut commands: Commands,
    query: Query<(Entity, &DDARay, &GlobalTransform)>,
) {
    if let Ok((entity, ray, g_transform)) = query.single() {
        let origin = g_transform.translation();
        let direction = g_transform.forward();
        let hits = digital_differential_analysis(origin, direction.as_vec3(), ray.max_distance);
        commands.trigger(DDAResult {
            hits: hits
        });
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – Block Highlighting
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
    mut highlight_query: Query<(&mut Transform, &mut Visibility), With<BlockHighlight>>,
    player_look_target: Res<PlayerLookTarget>,
) {
    if let Ok((mut transform, mut visibility)) = highlight_query.single_mut() {
        match &player_look_target.target {
            Some(LookTarget::StaticVoxel { pos, .. }) => {
                transform.translation = pos.as_vec3() - Vec3::splat(0.5 * (HIGHLIGHT_EPSILON - 1.0)) ;
                *visibility = Visibility::Visible;
            },
            _ => {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – Remove/Place Block TEMPORARY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn handle_mouse_interaction_obs(
    mouse_event: On<MouseEvent>,
    mut commands: Commands,
    look_target: Res<PlayerLookTarget>,
    held_item: Res<PlayerHeldItems>,
    block_registry: Res<BlockRegistry>,
    item_registry: Res<ItemRegistry>,
) {
    if mouse_event.action == MouseAction::Primary {
        match look_target.target {
            Some(LookTarget::StaticVoxel { chunk_entity, voxel, pos, face }) => {
                let event = StaticVoxelWriteRequest {
                    block_coord: pos,
                    dimension: DimensionId::OVERWORLD,
                    voxel: Voxel::AIR,
                };
                commands.trigger(event)
            },
            _ => { return }
        }
    } else if mouse_event.action == MouseAction::Secondary {
            match look_target.target {
            Some(LookTarget::StaticVoxel { chunk_entity, voxel, pos, face }) => {
                let neighbor_pos = pos + face.as_ivec3();

                if let Some(held_item_right) = held_item.right_hand {
                    let kind = item_registry.get(held_item_right.id).kind.clone();
                    match kind {
                        ItemKind::Block { block_id } => {
                            let block_data = block_registry.get(block_id);
                            let shape = block_data.shape.clone();

                            let event = StaticVoxelWriteRequest {
                                block_coord: neighbor_pos,
                                dimension: DimensionId::OVERWORLD,
                                voxel: Voxel::new(block_id.0, shape, face),
                            };

                            commands.trigger(event)
                        }
                        _ => { return }
                    }
                }


            },
            _ => { return }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – Player Look Target
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// This section deals with creating and mantaining the "look target" of a player
/// as a Resource. Systems can draw upon this Resource to display tooltips and
/// update the block highlight.

#[derive(Resource, Default)]
pub struct PlayerLookTarget{
    target: Option<LookTarget>
}

#[derive(Clone, PartialEq, Eq)]
pub enum LookTarget {
    StaticVoxel {
        chunk_entity:   Entity,
        voxel:          Voxel,
        pos:            IVec3,
        face:           Direction,
    },
    MovingGridVoxel {
        grid_entity:    Entity,
        voxel:          Voxel,
        local_pos:      IVec3,
        face:           Direction,
    },
    Mob {
        entity:         Entity,
    },
}

#[derive(Event)]
pub struct LookTargetChanged {
    pub old: Option<LookTarget>,
    pub new:  Option<LookTarget>,
}

fn update_look_target_obs(
    dda_event: On<DDAResult>,
    mut target:  ResMut<PlayerLookTarget>,
    static_world_access: StaticWorldAccess,
) {
    let hit_blocks = dda_event.hits.clone();
    
    if let Some((block_coord, face)) = hit_blocks.into_iter().find(|(coord, _)| {
        // Check if the block at this coordinate is not air.
        // This closure short-circuits at the first non-air block in the voxel data.
        static_world_access.get_voxel(*coord, DimensionId::OVERWORLD).is_air() != true
    }) {
        if let Some(entity) = static_world_access.get_chunk_entity(block_coord, DimensionId::OVERWORLD) {
            let voxel = static_world_access.get_voxel(block_coord, DimensionId::OVERWORLD);
            target.target = Some(LookTarget::StaticVoxel {
                chunk_entity: entity,
                voxel,
                pos: block_coord,
                face: face
            });
        } else { target.target = None }
    } else {
        target.target = None
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – Voxel Writing Events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Event)]
pub struct StaticVoxelWriteRequest {
    block_coord: IVec3,
    dimension: DimensionId,
    voxel: Voxel,
}

fn static_voxel_write_obs(
    event: On<StaticVoxelWriteRequest>,
    mut static_world_access: StaticWorldAccessMut,
) {
    static_world_access.set_voxel(event.block_coord, event.dimension, event.voxel);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – Player Held Item
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Resource, Default)]
pub struct PlayerHeldItems{
    right_hand: Option<ItemStack>,
    left_hand: Option<ItemStack>,
}

fn update_held_items_obs(
    event: On<PlayerHotbarSelectedChange>,
    mut held_items: ResMut<PlayerHeldItems>,
    inv_query: Query<&Inventory, With<PlayerInventory>>,
) {
    if let Ok(inventory) = inv_query.single() {
        let next_slot = event.new_index;
        held_items.right_hand = inventory.slots()[next_slot];
    }
}