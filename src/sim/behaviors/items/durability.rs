use serde::Deserialize;

use crate::content::item::behaviors::ItemBehavior;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DURABILITY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// This item wears out, after `max` uses.
///
/// ```json
/// { "durability": { "max": 128 } }
/// ```
///
/// ## Only the cap lives here, and that is not an omission
///
/// The old `Durability { max, current }` conflated two different things.
/// `max` is a fact about the item *type* — every iron pickaxe has the same
/// one — and belongs on the definition, which is what a behaviour is. The
/// *current* value is per-stack instance state: two pickaxes in the same
/// inventory have different ones, so a single shared definition cannot hold
/// it without every pickaxe in the game wearing out together.
///
/// There is nowhere to put it yet — `ItemStack` is `{ id, count }` and
/// carries no instance data at all. When it grows some, `current` goes
/// there and this stays exactly as it is.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Durability {
    /// Uses before the item breaks.
    pub max: u32,
}

impl ItemBehavior for Durability {
    const NAME: &'static str = "durability";
}
