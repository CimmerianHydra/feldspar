use serde::Deserialize;

use crate::content::item::behaviors::ItemBehavior;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FUEL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// This item burns, and is worth `value` units of heat doing so.
///
/// ```json
/// { "fuel": { "value": 1600 } }
/// ```
///
/// Nothing consumes this yet — no furnace exists. It is here because it is
/// the cheapest possible demonstration of the point: a capability with no
/// consumer costs one file and one registration line, and the day a furnace
/// arrives it queries `behaviors.get::<Fuel>()` and every fuel in the game
/// already works.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Fuel {
    /// Heat units. Scale is arbitrary until a furnace fixes it; coal at
    /// 1600 and a plank at 300 is the Minecraft convention and a fine
    /// starting point.
    pub value: u32,
}

impl ItemBehavior for Fuel {
    const NAME: &'static str = "fuel";
}
