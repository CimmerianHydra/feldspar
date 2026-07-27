use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

use crate::space::BlockPos;
use crate::voxel::BlockID;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE SPAWNER TRAIT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Everything a spawner is allowed to know about the block it's decorating.
///
/// Borrows `Commands` rather than owning it, so several spawners run
/// back-to-back against one root entity inside a single observer.
///
/// Note what isn't here: no dimension, no world position, no notion of
/// static-versus-moving. A `BlockPos` is all a machine needs, and it means
/// the identical spawner works on terrain and on a ship.
pub struct BlockSpawnContext<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    /// The block-entity every spawner in the list decorates. Already carries
    /// a `BlockEntityTag`, so it's already indexed and parented.
    pub root:       Entity,
    /// Space-local position of the block that was placed.
    pub at:         BlockPos,
    /// The ID of the block that was placed.
    pub block_id:   BlockID,
}

impl<'a, 'w, 's> BlockSpawnContext<'a, 'w, 's> {
    /// Attach components to the root block-entity.
    pub fn insert(&mut self, bundle: impl Bundle) -> &mut Self {
        self.commands.entity(self.root).insert(bundle);
        self
    }

    /// Spawn a sub-entity parented to the root. Despawning the root takes
    /// these with it, and they inherit the grid's motion through it.
    pub fn spawn_child(&mut self, bundle: impl Bundle) -> Entity {
        let child = self.commands.spawn(bundle).id();
        self.commands.entity(self.root).add_child(child);
        child
    }

    /// Attach an observer scoped to this block-entity alone. This is how a
    /// behavior ships its own interaction handler without any central
    /// dispatch table ever learning that this block type exists.
    pub fn observe<E: EntityEvent, B: Bundle, M>(
        &mut self,
        observer: impl IntoObserverSystem<E, B, M>,
    ) -> &mut Self {
        self.commands.entity(self.root).observe(observer);
        self
    }
}

/// One reusable unit of "what a placed block turns into".
///
/// Object-safe and stateless per call: the instance lives in the registry
/// behind an `Arc` and holds only its *configuration*. Per-block state goes
/// on the entity it spawns.
///
/// The trait lives in `content` rather than in `sim` because
/// `BlockDefinition` holds a list of them. Putting it a layer up would make
/// content depend on simulation, which is exactly the cycle this layout
/// exists to prevent. What it can *do* is still simulation — it only ever
/// touches `Commands`.
pub trait BlockEntitySpawner: Send + Sync + 'static {
    fn spawn(&self, ctx: &mut BlockSpawnContext);

    /// Teardown that isn't just "despawn" — spilling inventory onto the
    /// floor, unlinking from a power network. The entity is despawned
    /// recursively afterwards regardless.
    fn despawn(&self, _commands: &mut Commands, _root: Entity) {}
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE BLOCK COMPONENT BAG
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Declares that placing this block promotes it to an ECS entity, built by
/// running every spawner in order against one shared root.
///
/// A list, not a single spawner — that's what makes multi-entity blocks and
/// multiblock machines composable rather than bespoke.
#[derive(Clone, Default)]
pub struct SpawnsBlockEntities(pub Vec<Arc<dyn BlockEntitySpawner>>);

impl SpawnsBlockEntities {
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn BlockEntitySpawner>> { self.0.iter() }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
}

/// Declares that right-clicking this block does something even though it
/// has no entity — levers, doors, buttons. Handled by a global observer on
/// `BlockEvent::Interact` that matches on `BlockID`.
///
/// Entity-backed blocks don't need this; they use the `Interactable` ECS
/// marker instead, which is strictly more precise.
#[derive(Clone, Copy, Default, Debug)]
pub struct InteractsOnSecondary;

/// The twin of `ItemComponents`. A product of optionals, not a sum type: a
/// block participates in each capability independently.
///
/// Stays `Clone` (hence `Arc`, not `Box`) because `BlockDefinition` is
/// cloned during registration and will eventually be serialized.
#[derive(Clone, Default)]
pub struct BlockComponents {
    spawns_entities:        Option<SpawnsBlockEntities>,
    interacts_on_secondary: Option<InteractsOnSecondary>,
}

/// Uniform typed accessor. Identical call signature to a future
/// `TypeId -> Box<dyn Any>` version, so that upgrade is a drop-in.
pub trait BlockComponent: Sized {
    fn get(bag: &BlockComponents) -> Option<&Self>;
    fn set(bag: &mut BlockComponents, value: Self);
}

impl BlockComponents {
    pub fn get<T: BlockComponent>(&self) -> Option<&T> { T::get(self) }
    pub fn has<T: BlockComponent>(&self) -> bool { self.get::<T>().is_some() }

    /// Builder form: `BlockComponents::default().with(SpawnsBlockEntities(..))`
    pub fn with<T: BlockComponent>(mut self, value: T) -> Self {
        T::set(&mut self, value);
        self
    }
}

/// Stamps out the two-line impl. Adding a capability is one field above and
/// one line here.
macro_rules! block_component {
    ($ty:ty, $field:ident) => {
        impl BlockComponent for $ty {
            fn get(bag: &BlockComponents) -> Option<&Self> { bag.$field.as_ref() }
            fn set(bag: &mut BlockComponents, v: Self) { bag.$field = Some(v); }
        }
    };
}

block_component!(SpawnsBlockEntities, spawns_entities);
block_component!(InteractsOnSecondary, interacts_on_secondary);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – NAME -> SPAWNER REGISTRY  (the JSON bridge)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Maps the strings in a block's `"behaviors"` array to spawner instances.
///
/// Every feature plugin registers its own entries at `build()` time, so
/// adding a machine never edits a central match. The block loader resolves
/// names into `Arc`s once, at registry population.
#[derive(Resource, Default)]
pub struct BlockBehaviorRegistry {
    spawners: HashMap<String, Arc<dyn BlockEntitySpawner>>,
}

impl BlockBehaviorRegistry {
    pub fn register(&mut self, name: impl Into<String>, spawner: Arc<dyn BlockEntitySpawner>) {
        let name = name.into();
        if self.spawners.insert(name.clone(), spawner).is_some() {
            warn!("block behavior '{name}' registered twice. Selecting the later one.");
        }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn BlockEntitySpawner>> {
        self.spawners.get(name).cloned()
    }

    /// Resolve a JSON name list. Unknown names warn loudly and are skipped:
    /// a typo should be visible at load, not as a barrel that silently
    /// refuses to hold anything.
    pub fn resolve(&self, names: &[String], block_name: &str) -> Option<SpawnsBlockEntities> {
        if names.is_empty() { return None; }

        let mut out = Vec::with_capacity(names.len());
        for name in names {
            match self.get(name) {
                Some(spawner) => out.push(spawner),
                None => warn!("block '{block_name}' requests unknown behavior '{name}'"),
            }
        }

        if out.is_empty() { None } else { Some(SpawnsBlockEntities(out)) }
    }
}

/// Ergonomic registration from a plugin's `build()`.
pub trait RegisterBlockBehaviorExtension {
    fn register_block_behavior(
        &mut self,
        name: impl Into<String>,
        spawner: impl BlockEntitySpawner,
    ) -> &mut Self;
}

impl RegisterBlockBehaviorExtension for App {
    fn register_block_behavior(
        &mut self,
        name: impl Into<String>,
        spawner: impl BlockEntitySpawner,
    ) -> &mut Self {
        self.init_resource::<BlockBehaviorRegistry>();
        self.world_mut()
            .resource_mut::<BlockBehaviorRegistry>()
            .register(name, Arc::new(spawner));
        self
    }
}
