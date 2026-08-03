//! What a block *can do*, as an open list rather than a closed struct.
//!
//! This file contains no behaviours. It contains the bag they land in, the
//! one trait they implement, and the registry that maps a JSON name onto
//! one. Every actual behaviour lives in `sim::behaviors::blocks`, one file
//! each, and nothing here ever learns their names.
//!
//! ## Why a `Vec`, not a struct of optionals
//!
//! The bag used to be `BlockComponents`: one `Option<T>` field per
//! capability, one line in a macro, one `apply`. Adding a behaviour meant
//! three edits in `content` for something that conceptually lived entirely
//! in `sim` — and because the struct had to *name* every capability type,
//! no capability could live above this layer.
//!
//! A `Vec<Arc<dyn BlockBehaviorData>>` is empty for the overwhelming
//! majority of blocks, allocates nothing when empty, preserves the JSON
//! array's order for free, and clones as a handful of refcount bumps. A
//! block carries one to three behaviours, so a linear scan comparing
//! `TypeId` beats hashing — and `get::<T>()` is off every hot path anyway
//! (once per click, once per placement, once per frame for the look target).

use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;
use serde::de::DeserializeOwned;
use std::any::Any;
use std::sync::Arc;

use crate::content::behavior::{
    BehaviorFamily, BehaviorOf, BehaviorRegistry, RegisterBehaviorExtension,
};
use crate::space::BlockPos;
use crate::voxel::BlockID;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE SPAWN CONTEXT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Everything a behaviour is allowed to know about the block it's decorating.
///
/// Borrows `Commands` rather than owning it, so several behaviours run
/// back-to-back against one root entity inside a single observer.
///
/// Note what isn't here: no dimension, no world position, no notion of
/// static-versus-moving. A `BlockPos` is all a machine needs, and it means
/// the identical behaviour works on terrain and on a ship.
///
/// ## The root is lazy, and that is the point
///
/// `root` starts `None` and is reserved on first use. A behaviour that
/// never touches the context — `orientable`, `face_targeted` — leaves it
/// `None`, and the caller then knows not to promote the voxel to an entity
/// at all. Every stair in the world would otherwise get a useless entity.
///
/// The alternative was a `SPAWNS_ENTITY` const next to `on_place`, which is
/// a footgun: implement the hook, forget the const, and the behaviour
/// silently never runs. Here, touching the context *is* the declaration.
pub struct BlockSpawnContext<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    root: Option<Entity>,
    /// Space-local position of the block that was placed.
    pub at: BlockPos,
    /// The ID of the block that was placed.
    pub block_id: BlockID,
}

impl<'a, 'w, 's> BlockSpawnContext<'a, 'w, 's> {
    pub fn new(commands: &'a mut Commands<'w, 's>, at: BlockPos, block_id: BlockID) -> Self {
        Self { commands, root: None, at, block_id }
    }

    /// The block-entity every behaviour in the list decorates, reserving it
    /// if this is the first behaviour to ask.
    ///
    /// `spawn_empty` reserves the id immediately even though the spawn
    /// itself is deferred, so every behaviour in this pass sees the same
    /// entity and the caller can attach the `BlockEntityTag` afterwards.
    pub fn root(&mut self) -> Entity {
        match self.root {
            Some(entity) => entity,
            None => {
                let entity = self.commands.spawn_empty().id();
                self.root = Some(entity);
                entity
            }
        }
    }

    /// The root, if any behaviour asked for one. Read by the caller after
    /// the whole list has run.
    pub fn spawned(&self) -> Option<Entity> {
        self.root
    }

    /// Attach components to the root block-entity.
    pub fn insert(&mut self, bundle: impl Bundle) -> &mut Self {
        let root = self.root();
        self.commands.entity(root).insert(bundle);
        self
    }

    /// Spawn a sub-entity parented to the root. Despawning the root takes
    /// these with it, and they inherit the grid's motion through it.
    pub fn spawn_child(&mut self, bundle: impl Bundle) -> Entity {
        let root = self.root();
        let child = self.commands.spawn(bundle).id();
        self.commands.entity(root).add_child(child);
        child
    }

    /// Attach an observer scoped to this block-entity alone. This is how a
    /// behavior ships its own interaction handler without any central
    /// dispatch table ever learning that this block type exists.
    pub fn observe<E: EntityEvent, B: Bundle, M>(
        &mut self,
        observer: impl IntoObserverSystem<E, B, M>,
    ) -> &mut Self {
        let root = self.root();
        self.commands.entity(root).observe(observer);
        self
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE AUTHOR-FACING TRAIT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One thing a block can do. **The only trait a behaviour file implements.**
///
/// Every method has a default, so a behaviour costs exactly what it uses:
///
/// - `orientable` implements `NAME` and nothing else. It never spawns an
///   entity and never registers a system, and the defaults say so.
/// - `barrel` implements `NAME` and `on_place`.
/// - `chute` implements `NAME`, `on_place` and `build`.
///
/// There is no second family, no spawner trait and no macro. The thing that
/// used to require `spawner_behavior!` — "these two families are disjoint" —
/// stopped being true the moment the hooks got defaults.
pub trait BlockBehavior: DeserializeOwned + Default + Send + Sync + Sized + 'static {
    /// The string a block JSON uses to request this. snake_case.
    const NAME: &'static str;

    /// Whether listing this twice on one block is meaningful. Lookup finds
    /// the first, so this is nearly always a copy-paste — hence `false`.
    const REPEATABLE: bool = false;

    /// The block was placed, or its chunk was hydrated from disk. Touch
    /// `ctx` and the voxel is promoted to an ECS entity; leave it alone and
    /// it stays a bare voxel.
    fn on_place(&self, _ctx: &mut BlockSpawnContext) {}

    /// Teardown that isn't just "despawn" — spilling inventory onto the
    /// floor, unlinking from a power network. The entity is despawned
    /// recursively afterwards regardless.
    fn on_break(&self, _commands: &mut Commands, _root: Entity) {}

    /// Systems, resources and sub-plugins this behaviour needs. Called once
    /// at registration, which is what makes a behaviour file self-contained:
    /// delete the file and its registration line, and its systems go too.
    fn build(_app: &mut App) {}

    /// Contribute to the bag. The default pushes `self`; override to
    /// contribute *implied* behaviours alongside it — see `Orientable`,
    /// which implies `FaceTargeted`.
    fn apply(self, into: &mut BlockBehaviors) {
        into.push(self);
    }
}

/// Object-safe face of [`BlockBehavior`], so the bag can hold a
/// heterogeneous list. Never implemented by hand — the blanket impl below
/// covers every `BlockBehavior`, forwarding the hooks and supplying the
/// downcast.
pub trait BlockBehaviorData: Send + Sync + 'static {
    fn on_place(&self, ctx: &mut BlockSpawnContext);
    fn on_break(&self, commands: &mut Commands, root: Entity);
    fn as_any(&self) -> &dyn Any;
}

impl<B: BlockBehavior> BlockBehaviorData for B {
    fn on_place(&self, ctx: &mut BlockSpawnContext) {
        BlockBehavior::on_place(self, ctx)
    }

    fn on_break(&self, commands: &mut Commands, root: Entity) {
        BlockBehavior::on_break(self, commands, root)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE BAG
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Everything one block type can do, in declaration order.
///
/// Empty for a plain cube, and an empty `Vec` allocates nothing. `Clone` is
/// a few refcount bumps, which matters because `BlockDefinition` is cloned
/// during registration.
#[derive(Clone, Default)]
pub struct BlockBehaviors(Vec<Arc<dyn BlockBehaviorData>>);

impl BlockBehaviors {
    pub fn push<B: BlockBehavior>(&mut self, behavior: B) {
        self.0.push(Arc::new(behavior));
    }

    /// The first behaviour of this type, or `None`.
    ///
    /// A linear scan over one to three entries. If a genuinely hot caller
    /// ever appears, resolve the value once onto `BlockDefinition` at load
    /// rather than reaching for a `HashMap` here.
    pub fn get<B: BlockBehavior>(&self) -> Option<&B> {
        self.0.iter().find_map(|b| b.as_any().downcast_ref::<B>())
    }

    pub fn has<B: BlockBehavior>(&self) -> bool {
        self.get::<B>().is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn BlockBehaviorData>> {
        self.0.iter()
    }

    /// Run every behaviour's placement hook, in declaration order, against
    /// one shared context.
    pub fn on_place(&self, ctx: &mut BlockSpawnContext) {
        for behavior in &self.0 {
            behavior.on_place(ctx);
        }
    }

    /// Run every behaviour's teardown hook. The root is despawned
    /// recursively by the caller afterwards.
    pub fn on_break(&self, commands: &mut Commands, root: Entity) {
        for behavior in &self.0 {
            behavior.on_break(commands, root);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – THE FAMILY WIRING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marker tying [`BlockBehaviors`] into the generic machinery.
pub struct BlockFamily;

impl BehaviorFamily for BlockFamily {
    type Bag = BlockBehaviors;
    /// Blocks resolve from their own config alone. Items are the ones that
    /// need to look a block name up, which is why item loading comes after
    /// block registration and not the other way round.
    type Context<'a> = ();
    const LABEL: &'static str = "block";
}

/// The bridge. Blanket over `BlockBehavior`, instantiated at
/// `BehaviorOf<BlockFamily>` — the item side does the same at
/// `BehaviorOf<ItemFamily>`, and the two never overlap because the trait's
/// parameter differs.
impl<B: BlockBehavior> BehaviorOf<BlockFamily> for B {
    const NAME: &'static str = <B as BlockBehavior>::NAME;
    const REPEATABLE: bool = <B as BlockBehavior>::REPEATABLE;

    fn apply(self, _ctx: (), into: &mut BlockBehaviors) {
        BlockBehavior::apply(self, into)
    }

    fn build(app: &mut App) {
        <B as BlockBehavior>::build(app)
    }
}

pub type BlockBehaviorRegistry = BehaviorRegistry<BlockFamily>;

/// Ergonomic registration from a plugin's `build()`.
pub trait RegisterBlockBehaviorExtension {
    fn register_block_behavior<B: BlockBehavior>(&mut self) -> &mut Self;
}

impl RegisterBlockBehaviorExtension for App {
    fn register_block_behavior<B: BlockBehavior>(&mut self) -> &mut Self {
        self.register_behavior::<BlockFamily, B>()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::behavior::BehaviorEntry;
    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Default, Deserialize, Debug, PartialEq)]
    #[serde(default, deny_unknown_fields)]
    struct Tuning {
        value: u32,
    }

    impl BlockBehavior for Tuning {
        const NAME: &'static str = "tuning";
    }

    #[derive(Default, Deserialize)]
    struct Marker;

    impl BlockBehavior for Marker {
        const NAME: &'static str = "marker";
    }

    /// A behaviour that contributes another alongside itself — the shape
    /// `Orientable` uses to imply `FaceTargeted`.
    #[derive(Default, Deserialize)]
    struct Implier;

    impl BlockBehavior for Implier {
        const NAME: &'static str = "implier";
        fn apply(self, into: &mut BlockBehaviors) {
            into.push(Marker);
            into.push(self);
        }
    }

    fn entry(name: &str, data: Option<Value>) -> BehaviorEntry {
        BehaviorEntry { name: name.to_string(), data }
    }

    #[test]
    fn empty_bag_finds_nothing() {
        let bag = BlockBehaviors::default();
        assert!(bag.is_empty());
        assert!(!bag.has::<Tuning>());
    }

    /// A bare name and an explicit null must be the same thing, or every
    /// pre-existing block file changes meaning.
    #[test]
    fn bare_name_uses_default_config() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<Tuning>();

        let bare = registry.resolve(&[entry("tuning", None)], (), "test_block");
        let null = registry.resolve(&[entry("tuning", Some(Value::Null))], (), "test_block");

        assert_eq!(bare.get::<Tuning>(), Some(&Tuning { value: 0 }));
        assert_eq!(null.get::<Tuning>(), Some(&Tuning { value: 0 }));
    }

    /// Partial config must fill the rest from `Default`, not from zero.
    #[test]
    fn configured_entry_overrides_default() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<Tuning>();

        let value: Value = serde_json::from_str(r#"{"value": 7}"#).unwrap();
        let bag = registry.resolve(&[entry("tuning", Some(value))], (), "test_block");

        assert_eq!(bag.get::<Tuning>(), Some(&Tuning { value: 7 }));
    }

    /// A typo in a config key must warn and skip, never half-apply.
    #[test]
    fn malformed_config_is_skipped() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<Tuning>();

        let value: Value = serde_json::from_str(r#"{"valeu": 7}"#).unwrap();
        let bag = registry.resolve(&[entry("tuning", Some(value))], (), "test_block");

        assert!(bag.is_empty());
    }

    /// An unregistered name warns and skips rather than failing the load.
    #[test]
    fn unknown_behavior_is_skipped() {
        let registry = BlockBehaviorRegistry::default();
        let bag = registry.resolve(&[entry("nonexistent", None)], (), "test_block");
        assert!(bag.is_empty());
    }

    /// The implication lives in `apply`, so it must survive resolution.
    #[test]
    fn apply_can_contribute_implied_behaviors() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<Implier>();

        let bag = registry.resolve(&[entry("implier", None)], (), "test_block");

        assert!(bag.has::<Implier>());
        assert!(bag.has::<Marker>());
    }

    /// …but not the other way around.
    #[test]
    fn implication_is_one_directional() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<Marker>();

        let bag = registry.resolve(&[entry("marker", None)], (), "test_block");

        assert!(bag.has::<Marker>());
        assert!(!bag.has::<Implier>());
    }

    /// Declaration order is what the hooks run in.
    #[test]
    fn order_is_preserved() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<Tuning>();
        registry.register::<Marker>();

        let bag = registry.resolve(
            &[entry("marker", None), entry("tuning", None)],
            (),
            "test_block",
        );

        assert_eq!(bag.len(), 2);
        assert!(bag.iter().next().unwrap().as_any().is::<Marker>());
    }
}