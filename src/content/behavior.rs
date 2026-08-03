//! The behaviour machinery, once, for every family that has one.
//!
//! Blocks and items both want the same thing: a JSON array of names with
//! optional config, resolved at load time into a bag of typed data, with a
//! registry that no central match statement ever has to know about.
//!
//! Exactly one thing differs between the two — what the bag is, and what
//! resolution needs to consult. Both are captured by [`BehaviorFamily`], so
//! everything below is written once and instantiated twice.
//!
//! ## What lives here and what doesn't
//!
//! This file knows about *names*, *config blobs* and *warnings*. It does not
//! know what a behaviour can do. Lifecycle hooks (`on_place`, `on_break`,
//! and whatever items grow later) belong to the family's own trait, next to
//! the bag they act on, because they are genuinely different per family.

use bevy::prelude::*;
use serde::de;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE JSON ENTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One element of a `"behaviors"` array, in any definition file.
///
/// Two spellings, because most behaviours want their defaults:
/// ```json
/// "behaviors": [
///   "barrel",
///   { "chute": { "batch": 4, "batches_per_second": 8 } },
///   { "orientable": { "mode": "horizontal" } }
/// ]
/// ```
///
/// ## Why an array of single-key objects, and not a map
///
/// A map (`"behaviors": { "chute": {...} }`) reads better and forbids
/// duplicate keys for free. Two things beat that here:
///
/// - **Order survives.** The bag is a list and runs in order against one
///   shared root. `serde_json`'s `Map` is a `BTreeMap` by default, so a map
///   would silently alphabetise behaviours — a bug that would surface as a
///   multiblock assembling wrong, months later, with nothing in the diff to
///   blame.
/// - **Nothing migrates.** `"behaviors": ["barrel"]` is still valid, so every
///   file written before any of this keeps working verbatim.
#[derive(Debug, Clone)]
pub struct BehaviorEntry {
    pub name: String,
    /// The config that followed the name, if any. Interpreted by the
    /// behaviour's own factory, never by this module.
    pub data: Option<Value>,
}

impl<'de> Deserialize<'de> for BehaviorEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// Untagged buffers the input and tries in order, which is what lets
        /// a bare string and a configured object share one array.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Bare(String),
            Configured(Map<String, Value>),
        }

        match Raw::deserialize(deserializer)? {
            Raw::Bare(name) => Ok(BehaviorEntry { name, data: None }),

            Raw::Configured(map) => {
                let mut entries = map.into_iter();

                let Some((name, data)) = entries.next() else {
                    return Err(de::Error::custom(
                        "a behavior object needs exactly one key — \
                         the behavior's name — with its config as the value",
                    ));
                };

                // Two keys is ambiguous about order, and order is
                // load-bearing. Reject rather than guess.
                if let Some((second, _)) = entries.next() {
                    return Err(de::Error::custom(format!(
                        "a behavior object needs exactly one key, but this one \
                         has both '{name}' and '{second}'. Write them as two \
                         separate array entries."
                    )));
                }

                Ok(BehaviorEntry { name, data: Some(data) })
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE FAMILY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The three facts that distinguish one behaviour family from another.
///
/// A zero-sized marker type implements this — `BlockFamily`, `ItemFamily` —
/// and every generic below is parameterised on it. That is what lets the
/// two blanket impls of [`BehaviorOf`] coexist: they instantiate the trait
/// at different `F`, so coherence never has to prove that a block behaviour
/// isn't also an item behaviour.
pub trait BehaviorFamily: Send + Sync + 'static {
    /// What resolution produces.
    type Bag: Default + Send + Sync + 'static;

    /// What resolution needs beyond the JSON. `()` for blocks; items need
    /// the `BlockRegistry` to turn a block name into a `BlockID`.
    ///
    /// `Copy` because `resolve` hands it to every factory in the loop.
    type Context<'a>: Copy;

    /// The noun used in warnings: "block", "item".
    const LABEL: &'static str;
}

/// One named, configurable contribution to a bag of family `F`.
///
/// Never implemented by hand. Each family provides its own author-facing
/// trait (`BlockBehavior`, `ItemBehavior`) and a blanket impl into this one,
/// so a behaviour file implements exactly one trait.
///
/// ## The contract
///
/// - `NAME` is what JSON writes. Lowercase snake_case by convention.
/// - `Default` is the config a bare `"name"` entry gets, so the common case
///   stays a bare string.
/// - `apply` runs once per entry, in array order.
pub trait BehaviorOf<F: BehaviorFamily>:
    DeserializeOwned + Default + Send + Sync + Sized + 'static
{
    const NAME: &'static str;

    /// Whether listing this behaviour twice on one definition is
    /// meaningful. Almost always `false`: the second entry shadows the
    /// first on lookup, and is nearly always a copy-paste.
    const REPEATABLE: bool;

    /// Contribute to the bag. Consumes `self` so a behaviour can move
    /// straight into its `Arc` without a clone.
    fn apply(self, ctx: F::Context<'_>, into: &mut F::Bag);

    /// Systems, resources and observers this behaviour needs. Called once,
    /// at registration, so a behaviour ships its own wiring and deleting
    /// the file takes the wiring with it.
    fn build(app: &mut App);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – ERASURE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Object-safe face of [`BehaviorOf`], so the registry can hold a
/// heterogeneous map.
///
/// Deserialization happens *inside* the erased call, which is the whole
/// trick: the registry never learns the concrete type, but the concrete type
/// still gets a typed, validated config struct rather than a `Value` to
/// rummage through.
trait ErasedFactory<F: BehaviorFamily>: Send + Sync + 'static {
    fn apply(
        &self,
        data: Option<&Value>,
        ctx: F::Context<'_>,
        into: &mut F::Bag,
    ) -> Result<(), serde_json::Error>;

    fn repeatable(&self) -> bool;
}

/// The adapter. Zero-sized; the map stores `Arc<dyn ErasedFactory<F>>` and
/// every entry is a vtable pointer and nothing else.
struct TypedFactory<F, B>(PhantomData<fn() -> (F, B)>);

impl<F: BehaviorFamily, B: BehaviorOf<F>> ErasedFactory<F> for TypedFactory<F, B> {
    fn apply(
        &self,
        data: Option<&Value>,
        ctx: F::Context<'_>,
        into: &mut F::Bag,
    ) -> Result<(), serde_json::Error> {
        let behavior = match data {
            // A bare `"chute"` and an explicit `{"chute": null}` mean the
            // same thing. Handling null here rather than relying on the
            // type's own `Deserialize` means a unit-struct behaviour doesn't
            // have to be null-deserializable to be usable bare.
            None | Some(Value::Null) => B::default(),
            Some(value) => B::deserialize(value)?,
        };

        behavior.apply(ctx, into);
        Ok(())
    }

    fn repeatable(&self) -> bool {
        B::REPEATABLE
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – THE REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Maps the entries in a `"behaviors"` array to bag contributions.
///
/// Every feature plugin registers its own entries at `build()` time, so
/// adding a machine never edits a central match. The loader resolves the
/// array into a finished bag once, at registry population.
#[derive(Resource)]
pub struct BehaviorRegistry<F: BehaviorFamily> {
    factories: HashMap<String, Arc<dyn ErasedFactory<F>>>,
}

// Hand-written: deriving would demand `F: Default`, which a marker type has
// no reason to be.
impl<F: BehaviorFamily> Default for BehaviorRegistry<F> {
    fn default() -> Self {
        Self { factories: HashMap::new() }
    }
}

impl<F: BehaviorFamily> BehaviorRegistry<F> {
    pub fn register<B: BehaviorOf<F>>(&mut self) {
        let previous = self
            .factories
            .insert(B::NAME.to_string(), Arc::new(TypedFactory::<F, B>(PhantomData)));

        if previous.is_some() {
            warn!(
                "{} behavior '{}' registered twice. Selecting the later one.",
                F::LABEL,
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

    /// Resolve a whole `"behaviors"` array into a finished bag.
    ///
    /// Every failure is a warning and a skip, never a panic and never a
    /// silent success: a typo should be visible in the log at load, not as a
    /// barrel that mysteriously refuses to hold anything three hours later.
    ///
    /// Order is preserved, which matters because the bag is a list and
    /// lifecycle hooks run in the order they were declared.
    pub fn resolve(
        &self,
        entries: &[BehaviorEntry],
        ctx: F::Context<'_>,
        owner: &str,
    ) -> F::Bag {
        let mut bag = F::Bag::default();
        let mut seen: HashSet<&str> = HashSet::new();

        for entry in entries {
            let Some(factory) = self.factories.get(entry.name.as_str()) else {
                warn!(
                    "{} '{owner}' requests unknown behavior '{}' — skipped. \
                     Registered behaviors: {}",
                    F::LABEL,
                    entry.name,
                    self.names().join(", "),
                );
                continue;
            };

            let first_time = seen.insert(entry.name.as_str());
            if !first_time && !factory.repeatable() {
                warn!(
                    "{} '{owner}' lists behavior '{}' more than once. Lookup \
                     finds the first, so the later entries are dead weight — \
                     this is almost certainly a copy-paste.",
                    F::LABEL,
                    entry.name,
                );
            }

            if let Err(error) = factory.apply(entry.data.as_ref(), ctx, &mut bag) {
                warn!(
                    "{} '{owner}': behavior '{}' has malformed config — \
                     skipped. {error}",
                    F::LABEL,
                    entry.name,
                );
            }
        }

        bag
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – REGISTRATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The generic entry point. Families wrap it in a one-line extension with a
/// friendlier turbofish — see `RegisterBlockBehaviorExtension`.
///
/// The name is gone from the call site: it lives on the type as `NAME`, so
/// the string JSON writes and the string the plugin registers cannot drift
/// apart. `B::build` runs here too, so a behaviour's systems arrive with it.
pub trait RegisterBehaviorExtension {
    fn register_behavior<F: BehaviorFamily, B: BehaviorOf<F>>(&mut self) -> &mut Self;
}

impl RegisterBehaviorExtension for App {
    fn register_behavior<F: BehaviorFamily, B: BehaviorOf<F>>(&mut self) -> &mut Self {
        self.init_resource::<BehaviorRegistry<F>>();

        // Scoped so the resource borrow ends before `B::build` takes `&mut App`.
        {
            let mut registry = self.world_mut().resource_mut::<BehaviorRegistry<F>>();
            registry.register::<B>();
        }

        B::build(self);
        self
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    /// A bare name and an explicit null must be the same thing, or every
    /// pre-existing definition file changes meaning.
    #[test]
    fn bare_name_and_null_both_parse() {
        let bare: BehaviorEntry = serde_json::from_str(r#""barrel""#).unwrap();
        assert_eq!(bare.name, "barrel");
        assert!(bare.data.is_none());

        let null: BehaviorEntry = serde_json::from_str(r#"{"barrel": null}"#).unwrap();
        assert_eq!(null.name, "barrel");
        assert_eq!(null.data, Some(Value::Null));
    }

    #[test]
    fn configured_entry_keeps_its_blob() {
        let entry: BehaviorEntry =
            serde_json::from_str(r#"{"chute": {"batch": 4}}"#).unwrap();

        assert_eq!(entry.name, "chute");
        assert!(entry.data.is_some());
    }

    /// Two keys is ambiguous about order, and order is load-bearing.
    #[test]
    fn two_keys_in_one_object_is_rejected() {
        let result: Result<BehaviorEntry, _> =
            serde_json::from_str(r#"{"chute": null, "barrel": null}"#);
        assert!(result.is_err());
    }
}