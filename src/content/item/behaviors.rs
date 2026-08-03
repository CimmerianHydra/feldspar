//! What an item *can do*. The exact twin of `content::block::behaviors`,
//! and deliberately so — same bag shape, same one-trait contract, same
//! registry, same JSON spelling.
//!
//! Two differences, both forced by the domain rather than chosen:
//!
//! - **Items resolve against the block registry.** `places_block` names a
//!   block, and a name has to become a `BlockID` at load. That is what
//!   [`ItemLoadContext`] carries, and it is why item definitions are loaded
//!   *after* every block has been registered.
//! - **No lifecycle hooks yet.** A block behaviour decorates an entity at
//!   placement; an item behaviour is currently read by whoever cares —
//!   `player::interaction` for `orients_blocks` and `places_block`,
//!   `render::highlight` for the legality overlay. When items grow an
//!   `on_use`, it goes on [`ItemBehaviorData`] next to `as_any`, and every
//!   existing behaviour picks up the no-op default for free.

use bevy::prelude::*;
use serde::de::DeserializeOwned;
use std::any::Any;
use std::sync::Arc;

use crate::content::behavior::{
    BehaviorFamily, BehaviorOf, BehaviorRegistry, RegisterBehaviorExtension,
};
use crate::content::block::registry::BlockRegistry;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE LOAD CONTEXT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What an item behaviour may consult while resolving its config.
///
/// `Copy`, because `resolve` hands it to every factory in the loop. Grow it
/// by adding a field, not by threading a new argument through the registry.
#[derive(Clone, Copy)]
pub struct ItemLoadContext<'a> {
    /// Already fully populated — item loading runs after block registration.
    pub blocks: &'a BlockRegistry,
    /// The item being resolved, so a behaviour's own warning can name it.
    pub item_name: &'a str,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE AUTHOR-FACING TRAIT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One thing an item can do. **The only trait an item behaviour implements.**
pub trait ItemBehavior: DeserializeOwned + Default + Send + Sync + Sized + 'static {
    /// The string an item JSON uses to request this. snake_case.
    const NAME: &'static str;

    /// Whether listing this twice on one item is meaningful.
    const REPEATABLE: bool = false;

    /// Systems, resources and sub-plugins this behaviour needs. Called once
    /// at registration, so deleting the file takes its wiring with it.
    fn build(_app: &mut App) {}

    /// Contribute to the bag.
    ///
    /// The default pushes `self` and ignores the context, which is right for
    /// every behaviour whose JSON config *is* its runtime data. Override
    /// when resolution has real work to do — `places_block` turns a block
    /// name into a `BlockID` here, and drops itself if the name is wrong.
    fn apply(self, _ctx: ItemLoadContext<'_>, into: &mut ItemBehaviors) {
        into.push(self);
    }
}

/// Object-safe face of [`ItemBehavior`]. Never implemented by hand.
///
/// Currently only a downcast. This is the seam where `on_use` lands.
pub trait ItemBehaviorData: Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
}

impl<I: ItemBehavior> ItemBehaviorData for I {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE BAG
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Everything one item type can do, in declaration order.
#[derive(Clone, Default)]
pub struct ItemBehaviors(Vec<Arc<dyn ItemBehaviorData>>);

impl ItemBehaviors {
    pub fn push<I: ItemBehavior>(&mut self, behavior: I) {
        self.0.push(Arc::new(behavior));
    }

    /// Builder form, for the generated items that never see JSON — the
    /// block→item pass and the substance cross product.
    pub fn with<I: ItemBehavior>(mut self, behavior: I) -> Self {
        self.push(behavior);
        self
    }

    pub fn get<I: ItemBehavior>(&self) -> Option<&I> {
        self.0.iter().find_map(|b| b.as_any().downcast_ref::<I>())
    }

    pub fn has<I: ItemBehavior>(&self) -> bool {
        self.get::<I>().is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn ItemBehaviorData>> {
        self.0.iter()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – THE FAMILY WIRING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marker tying [`ItemBehaviors`] into the generic machinery.
pub struct ItemFamily;

impl BehaviorFamily for ItemFamily {
    type Bag = ItemBehaviors;
    type Context<'a> = ItemLoadContext<'a>;
    const LABEL: &'static str = "item";
}

impl<I: ItemBehavior> BehaviorOf<ItemFamily> for I {
    const NAME: &'static str = <I as ItemBehavior>::NAME;
    const REPEATABLE: bool = <I as ItemBehavior>::REPEATABLE;

    fn apply(self, ctx: ItemLoadContext<'_>, into: &mut ItemBehaviors) {
        ItemBehavior::apply(self, ctx, into)
    }

    fn build(app: &mut App) {
        <I as ItemBehavior>::build(app)
    }
}

pub type ItemBehaviorRegistry = BehaviorRegistry<ItemFamily>;

/// Ergonomic registration from a plugin's `build()`.
pub trait RegisterItemBehaviorExtension {
    fn register_item_behavior<I: ItemBehavior>(&mut self) -> &mut Self;
}

impl RegisterItemBehaviorExtension for App {
    fn register_item_behavior<I: ItemBehavior>(&mut self) -> &mut Self {
        self.register_behavior::<ItemFamily, I>()
    }
}