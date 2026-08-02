use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

use crate::content::block::asset::BehaviorEntry;
use crate::space::BlockPos;
use crate::voxel::{BlockID, BlockRotation, Direction};

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

/// Declares that this block's interactions are addressed by **target face**
/// rather than by the face the ray happened to strike.
///
/// Named for the interaction, not for the overlay it produces. The 3×3 grid
/// the renderer draws is what this *looks like*; what it *means* is that
/// `BlockEntityEvent::target_face` may differ from `face`. Presentation
/// observing this is the same inversion as `ui::inventory` observing
/// `Inventory` — content declares a capability, the renderer notices.
///
/// Independent of `Orientable` in both directions:
/// - a pipe is face-targeted and *not* orientable (corners toggle a
///   connection on the far side, they do not spin the pipe);
/// - a four-facing furnace is orientable and *not* face-targeted, because a
///   grid on a block with four legal orientations is noise.
#[derive(Clone, Copy, Default, Debug, Deserialize)]
pub struct FaceTargeted;

/// Which orientations this block is allowed to take.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrientationMode {
    /// All six. Machines with a single distinguished output face.
    #[default]
    Full,
    /// North/South/East/West. Anything with a fixed top — furnaces,
    /// workbenches, most player-facing UI machines.
    Horizontal,
    /// The three axes. A log pointed "down" is a log pointed "up"; this
    /// folds the pair rather than rejecting it, so the click still lands.
    Axis,
}

/// Declares that this block responds to a configuring tool by reorienting.
///
/// Orientation lives in the voxel's rotation bits, not on the block entity.
/// That is deliberate and it is why a stair or a slope can be wrenched with
/// no entity at all, and why orientation survives chunk unload, save/load
/// and a grid splitting in half without anyone writing persistence code.
/// A block entity that caches a facing refreshes it from `BlockEvent::Rotate`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Orientable {
    /// Which of the block's *local* faces the player is aiming. `Up` is the
    /// natural default because `BlockRotation` enumerates by where local +Y
    /// lands, so `reference: Up` makes the maths a straight lookup.
    pub reference: Direction,
    pub mode: OrientationMode,
}


impl Default for Orientable {
    fn default() -> Self {
        Self { reference: Direction::Up, mode: OrientationMode::Full }
    }
}

impl Orientable {
    /// Fold a requested target into a legal one, or reject it.
    ///
    /// `None` means "this block cannot point there" — the highlight greys
    /// the cell out and the click is swallowed rather than silently doing
    /// something else.
    pub fn normalize(&self, target: Direction) -> Option<Direction> {
        match self.mode {
            OrientationMode::Full => Some(target),

            OrientationMode::Horizontal => match target {
                Direction::Up | Direction::Down => None,
                horizontal => Some(horizontal),
            },

            // Fold each axis onto its positive representative.
            OrientationMode::Axis => Some(match target {
                Direction::Down  => Direction::Up,
                Direction::West  => Direction::East,
                Direction::North => Direction::South,
                positive => positive,
            }),
        }
    }

    /// The rotation this block should take to aim at `target`, or `None` if
    /// the target is illegal for its mode.
    ///
    /// Returns the *current* rotation unchanged when the block already
    /// points there, which the caller uses to skip a pointless voxel write.
    pub fn rotation_for(
        &self,
        current: BlockRotation,
        target: Direction,
    ) -> Option<BlockRotation> {
        let target = self.normalize(target)?;
        Some(current.point_toward(self.reference, target))
    }
}


/// The twin of `ItemComponents`. A product of optionals, not a sum type: a
/// block participates in each capability independently.
///
/// Stays `Clone` (hence `Arc`, not `Box`) because `BlockDefinition` is
/// cloned during registration and will eventually be serialized.
#[derive(Clone, Default)]
pub struct BlockComponents {
    spawns_entities:        Option<SpawnsBlockEntities>,
    face_targeted:          Option<FaceTargeted>,
    orientable:             Option<Orientable>,
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

    /// The `&mut` twin of [`BlockComponents::with`]. Replaces whatever was
    /// there — which is what a behaviour contributing a component wants,
    /// since two `orientable` entries on one block is a mistake, not a
    /// composition.
    pub fn insert<T: BlockComponent>(&mut self, value: T) {
        T::set(self, value);
    }

    /// Append a spawner rather than replacing the list.
    ///
    /// The one genuinely *additive* contribution. `SpawnsBlockEntities` is a
    /// list precisely so multi-entity and multiblock machines compose, so a
    /// behaviour that spawns must push, never overwrite — otherwise
    /// `["barrel", "chute"]` would silently be just the chute.
    pub fn add_spawner(&mut self, spawner: Arc<dyn BlockEntitySpawner>) {
        self.spawns_entities
            .get_or_insert_with(SpawnsBlockEntities::default)
            .0
            .push(spawner);
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
block_component!(FaceTargeted, face_targeted);
block_component!(Orientable, orientable);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – BEHAVIORS  (the JSON bridge)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One named, configurable contribution to a block's component bag.
///
/// ## Why this replaced "name → spawner"
///
/// The registry used to map a name onto a *spawner singleton with fixed
/// config*. That had two consequences worth naming, because both were quiet
/// and both were costly:
///
/// - **Configuration lived at the registration site.** `ChuteSpawner` carries
///   `batch` and `batches_per_second` and its own docs call that "a real
///   choice for an upgrade tree" — but the choice was made once, in Rust, in
///   `BehaviorsPlugin`. Wooden, iron and steel chutes needed three Rust
///   types to express three numbers.
///
/// - **Only spawners could ride the registry.** A behaviour that contributes
///   *data* rather than an entity — `InteractsOnSecondary`, `Orientable`,
///   `FaceTargeted` — had no route in, so each became a top-level field on
///   `BlockDefinitionAsset` plus a branch in `populate_block_registry_sys`.
///   That is precisely the central match this registry exists to avoid,
///   rebuilt one `if` at a time.
///
/// A behaviour is now a deserializable config struct that knows how to apply
/// itself to a bag. Spawning is one thing it can do, not the only thing.
///
/// ## The contract
///
/// - `NAME` is what block JSON writes. Lowercase snake_case by convention.
/// - `Default` is the config a bare `"name"` entry gets, so the common case
///   stays a bare string.
/// - `apply` runs once per entry, in array order.
pub trait BlockBehavior: DeserializeOwned + Default + Send + Sync + 'static {
    /// The string a block JSON uses to request this.
    const NAME: &'static str;

    /// Whether listing this behaviour twice on one block is meaningful.
    ///
    /// `true` for spawners: `["chute", "chute"]` is odd but legal, and a
    /// multiblock genuinely may want two of something. `false` for data
    /// components, where the second entry silently overwrites the first and
    /// is always a mistake — so the loader warns.
    const REPEATABLE: bool = false;

    /// Contribute to the bag. Consumes `self` so a spawner can move
    /// straight into its `Arc` without a clone.
    fn apply(self, into: &mut BlockComponents);
}

/// Object-safe face of [`BlockBehavior`], so the registry can hold a
/// heterogeneous map.
///
/// Deserialization happens *inside* the erased call, which is the whole
/// trick: the registry never learns the concrete type, but the concrete type
/// still gets a typed, validated config struct rather than a `Value` to
/// rummage through.
trait BehaviorFactory: Send + Sync + 'static {
    fn apply(
        &self,
        data: Option<&Value>,
        into: &mut BlockComponents,
    ) -> Result<(), serde_json::Error>;

    fn repeatable(&self) -> bool;
}

/// The adapter. Zero-sized; the map stores `Arc<dyn BehaviorFactory>` and
/// every entry is a vtable pointer and nothing else.
struct TypedFactory<B: BlockBehavior>(PhantomData<fn() -> B>);

impl<B: BlockBehavior> BehaviorFactory for TypedFactory<B> {
    fn apply(
        &self,
        data: Option<&Value>,
        into: &mut BlockComponents,
    ) -> Result<(), serde_json::Error> {
        let behavior = match data {
            // A bare `"chute"` and an explicit `{"chute": null}` mean the
            // same thing. Handling null here rather than relying on the
            // type's own `Deserialize` means a unit-struct behaviour doesn't
            // have to be null-deserializable to be usable bare.
            None | Some(Value::Null) => B::default(),
            Some(value) => B::deserialize(value)?,
        };

        behavior.apply(into);
        Ok(())
    }

    fn repeatable(&self) -> bool { B::REPEATABLE }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – THE REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Maps the entries in a block's `"behaviors"` array to component
/// contributions.
///
/// Every feature plugin registers its own entries at `build()` time, so
/// adding a machine never edits a central match. The block loader resolves
/// the array into a finished `BlockComponents` once, at registry population.
#[derive(Resource, Default)]
pub struct BlockBehaviorRegistry {
    factories: HashMap<String, Arc<dyn BehaviorFactory>>,
}

impl BlockBehaviorRegistry {
    pub fn register<B: BlockBehavior>(&mut self) {
        let previous = self
            .factories
            .insert(B::NAME.to_string(), Arc::new(TypedFactory::<B>(PhantomData)));

        if previous.is_some() {
            warn!(
                "block behavior '{}' registered twice. Selecting the later one.",
                B::NAME
            );
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.factories.contains_key(name)
    }

    /// Every registered name, sorted. Used to make an unknown-behavior
    /// warning actionable instead of merely truthful.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.factories.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Resolve a block's whole `"behaviors"` array into its component bag.
    ///
    /// Every failure is a warning and a skip, never a panic and never a
    /// silent success: a typo should be visible in the log at load, not as a
    /// barrel that mysteriously refuses to hold anything three hours later.
    ///
    /// Order is preserved, which matters because `SpawnsBlockEntities` runs
    /// its list in order against one shared root.
    pub fn resolve(&self, entries: &[BehaviorEntry], block_name: &str) -> BlockComponents {
        let mut components = BlockComponents::default();
        let mut seen: HashSet<&str> = HashSet::new();

        for entry in entries {
            let Some(factory) = self.factories.get(entry.name.as_str()) else {
                warn!(
                    "block '{block_name}' requests unknown behavior '{}' — skipped. \
                     Registered behaviors: {}",
                    entry.name,
                    self.names().join(", "),
                );
                continue;
            };

            let first_time = seen.insert(entry.name.as_str());
            if !first_time && !factory.repeatable() {
                warn!(
                    "block '{block_name}' lists behavior '{}' more than once. \
                     It contributes a single component, so only the last entry \
                     survives — this is almost certainly a copy-paste.",
                    entry.name,
                );
            }

            if let Err(error) = factory.apply(entry.data.as_ref(), &mut components) {
                warn!(
                    "block '{block_name}': behavior '{}' has malformed config — \
                     skipped. {error}",
                    entry.name,
                );
            }
        }

        components
    }
}

/// Ergonomic registration from a plugin's `build()`.
///
/// The name is gone from the call site: it lives on the type as
/// `BlockBehavior::NAME`, so the string a block JSON writes and the string
/// the plugin registers cannot drift apart.
pub trait RegisterBlockBehaviorExtension {
    fn register_block_behavior<B: BlockBehavior>(&mut self) -> &mut Self;
}

impl RegisterBlockBehaviorExtension for App {
    fn register_block_behavior<B: BlockBehavior>(&mut self) -> &mut Self {
        self.init_resource::<BlockBehaviorRegistry>();
        self.world_mut()
            .resource_mut::<BlockBehaviorRegistry>()
            .register::<B>();
        self
    }
}

/// Stamps the `BlockBehavior` impl for a spawner-flavoured behaviour, which
/// is most of them.
///
/// A blanket `impl<S: BlockEntitySpawner + …> BlockBehavior for S` would be
/// nicer, but it would collide with the hand-written impls for data-only
/// behaviours: coherence can't prove `Orientable: !BlockEntitySpawner`, even
/// within one crate. A macro is the honest way to say "these two families
/// are disjoint".
///
/// Usage, next to the spawner it describes:
/// ```ignore
/// spawner_behavior!(ChuteSpawner, "chute");
/// ```
#[macro_export]
macro_rules! spawner_behavior {
    ($ty:ty, $name:literal) => {
        impl $crate::content::block::components::BlockBehavior for $ty {
            const NAME: &'static str = $name;
            // Spawners compose: a multi-entity block may legitimately list
            // two, and the bag appends rather than replaces.
            const REPEATABLE: bool = true;

            fn apply(
                self,
                into: &mut $crate::content::block::components::BlockComponents,
            ) {
                into.add_spawner(::std::sync::Arc::new(self));
            }
        }
    };
}




impl BlockBehavior for FaceTargeted {
    const NAME: &'static str = "face_targeted";
    fn apply(self, into: &mut BlockComponents) {
        into.insert(self);
    }
}




impl BlockBehavior for Orientable {
    const NAME: &'static str = "orientable";

    /// **This is where the implication belongs.**
    ///
    /// An orientable block is addressed by target face, so it gets the grid.
    /// Resolving that here rather than at every read site means no consumer
    /// has to remember an `||`, and the next implication — whatever it turns
    /// out to be — touches this function and nothing else.
    fn apply(self, into: &mut BlockComponents) {
        into.insert(FaceTargeted);
        into.insert(self);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct Tuning { value: u32 }

    impl BlockBehavior for Tuning {
        const NAME: &'static str = "tuning";
        fn apply(self, _into: &mut BlockComponents) {}
    }

    fn entry(name: &str, data: Option<Value>) -> BehaviorEntry {
        BehaviorEntry { name: name.to_string(), data }
    }

    /// A bare name and an explicit null must be the same thing, or every
    /// pre-existing block file changes meaning.
    #[test]
    fn bare_name_uses_default_config() {
        let factory = TypedFactory::<Tuning>(PhantomData);
        let mut bag = BlockComponents::default();

        assert!(factory.apply(None, &mut bag).is_ok());
        assert!(factory.apply(Some(&Value::Null), &mut bag).is_ok());
    }

    /// Partial config must fill the rest from `Default`, not from zero.
    #[test]
    fn partial_config_keeps_other_defaults() {
        let json: BehaviorEntry =
            serde_json::from_str(r#"{"orientable": {"mode": "horizontal"}}"#).unwrap();

        let value = json.data.unwrap();
        let orientable = Orientable::deserialize(&value).unwrap();

        assert_eq!(orientable.mode, OrientationMode::Horizontal);
        assert_eq!(orientable.reference, Direction::Up);
    }

    /// A typo in a config key must fail loudly rather than be ignored.
    #[test]
    fn unknown_config_key_is_rejected() {
        let value: Value = serde_json::from_str(r#"{"referance": "North"}"#).unwrap();
        assert!(Orientable::deserialize(&value).is_err());
    }

    /// The implication lives in `apply`, so it must survive resolution.
    #[test]
    fn orientable_implies_face_targeted() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<Orientable>();

        let bag = registry.resolve(&[entry("orientable", None)], "test_block");

        assert!(bag.has::<Orientable>());
        assert!(bag.has::<FaceTargeted>());
    }

    /// …but not the other way around.
    #[test]
    fn face_targeted_does_not_imply_orientable() {
        let mut registry = BlockBehaviorRegistry::default();
        registry.register::<FaceTargeted>();

        let bag = registry.resolve(&[entry("face_targeted", None)], "test_block");

        assert!(bag.has::<FaceTargeted>());
        assert!(!bag.has::<Orientable>());
    }

    #[test]
    fn behavior_entry_parses_both_spellings() {
        let bare: BehaviorEntry = serde_json::from_str(r#""barrel""#).unwrap();
        assert_eq!(bare.name, "barrel");
        assert!(bare.data.is_none());

        let configured: BehaviorEntry =
            serde_json::from_str(r#"{"chute": {"batch": 4}}"#).unwrap();
        assert_eq!(configured.name, "chute");
        assert!(configured.data.is_some());
    }

    #[test]
    fn behavior_entry_rejects_two_keys() {
        let result: Result<BehaviorEntry, _> =
            serde_json::from_str(r#"{"chute": {}, "barrel": {}}"#);
        assert!(result.is_err());
    }
}