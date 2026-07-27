use avian3d::prelude::*;
use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use std::collections::HashMap;

use crate::space::chunk::ChunkMap;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – WHAT A SPACE IS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// A *voxel space* is an entity that holds a `ChunkMap`, defines a coordinate
// frame, and owns a rigid body.
//
// Dimensions and moving grids are the same kind of thing, differing only in
// which `RigidBody` variant they carry. Chunks are never bodies — they
// contribute collision shape to whatever space they belong to. One rule, no
// exceptions, and no entity has two authorities writing its transform.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component, Reflect)]
pub struct DimensionID(pub u8);

impl DimensionID {
    pub const OVERWORLD:    Self = Self(0);
    pub const UNDERWORLD:   Self = Self(1);
    pub const LUA:          Self = Self(2);
    pub const MARS:         Self = Self(3);
}

/// Marks a space as a dimension — the kind that doesn't move.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[component(on_add = dimension_on_add, on_remove = dimension_on_remove)]
pub struct Dimension(pub DimensionID);

/// Marks a space as a movable voxel construct.
#[derive(Component, Reflect, Default, Debug)]
pub struct MovingGrid;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – SPAWNING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Spawn a dimension space. The `Dimension` hook registers it in
/// `DimensionRegistry` automatically.
///
/// It's a static body: terrain doesn't move, and its chunks attach their
/// colliders to it through the hierarchy exactly the way a ship's do.
pub fn spawn_dimension(world: &mut World, id: DimensionID, name: &str) -> Entity {
    world
        .spawn((
            Dimension(id),
            ChunkMap::default(),
            RigidBody::Static,
            // Identity frame. Present so the same transform math works for
            // dimensions and grids without a special case.
            Transform::default(),
            Visibility::default(),
            Name::new(format!("Dimension<{name}>")),
        ))
        .id()
}

/// Bundle for a movable voxel construct - ship, vehicle, station.
///
/// Mass, inertia, and center of mass are all derived from the colliders its
/// chunks contribute, so don't set them by hand; set `ColliderDensity` on
/// the chunks instead (see `CHUNK_COLLIDER_DENSITY` in `crate::physics`).
pub fn build_moving_grid(transform: Transform, name: &str) -> impl Bundle {
    (
        MovingGrid,
        ChunkMap::default(),
        RigidBody::Dynamic,
        transform,
        Visibility::default(),
        Name::new(format!("Grid<{name}>")),
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE DIMENSION REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Stable-ID lookup for dimensions. Save files reference `DimensionID`,
/// which survives across runs; `Entity` does not.
#[derive(Resource, Default)]
pub struct DimensionRegistry {
    by_id: HashMap<DimensionID, Entity>,
}

impl DimensionRegistry {
    #[inline]
    pub fn get(&self, id: DimensionID) -> Option<Entity> { self.by_id.get(&id).copied() }

    /// Panics only if `SpacePlugin` wasn't added, which is a setup error
    /// rather than a runtime condition.
    #[inline]
    pub fn overworld(&self) -> Entity {
        self.get(DimensionID::OVERWORLD)
            .expect("SpacePlugin must be added before anything touches the overworld")
    }
}

fn dimension_on_add(mut world: DeferredWorld, ctx: HookContext) {
    let Some(&Dimension(id)) = world.entity(ctx.entity).get::<Dimension>() else { return };
    let entity = ctx.entity;
    if let Some(mut reg) = world.get_resource_mut::<DimensionRegistry>() {
        if reg.by_id.insert(id, entity).is_some() {
            warn!("dimension {id:?} registered twice — later entity wins");
        }
    }
}

fn dimension_on_remove(mut world: DeferredWorld, ctx: HookContext) {
    let Some(&Dimension(id)) = world.entity(ctx.entity).get::<Dimension>() else { return };
    if let Some(mut reg) = world.get_resource_mut::<DimensionRegistry>() {
        if reg.by_id.get(&id) == Some(&ctx.entity) {
            reg.by_id.remove(&id);
        }
    }
}
