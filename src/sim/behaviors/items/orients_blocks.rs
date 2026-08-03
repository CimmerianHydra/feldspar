use serde::Deserialize;

use crate::content::item::behaviors::ItemBehavior;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ORIENTS BLOCKS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// This item reorients blocks that declare `orientable`.
///
/// A capability rather than an item, for the reason every one of these is:
/// the dispatcher in `player::interaction` names *what a tool can do*, never
/// *which tool it is*, so wrench tiers, a creative-mode configurator and a
/// modded screwdriver all cost one JSON line each and no Rust at all.
///
/// The entire wrench is this file plus four lines of JSON:
/// ```json
/// { "name": "wrench", "display_name": "Wrench", "max_stack": 1,
///   "display": "icons/items/wrench.png",
///   "behaviors": ["orients_blocks"] }
/// ```
///
/// If a single tool later needs to do several configuring jobs — GregTech's
/// wrench both rotates machines and wrenches pipe connections — add a second
/// behaviour rather than growing this one. The dispatch site in
/// `player::interaction` is written to make that additive: it matches on the
/// item's capabilities, not on a chain of `if item == …`.
#[derive(Clone, Copy, Default, Debug, Deserialize)]
pub struct OrientsBlocks;

impl ItemBehavior for OrientsBlocks {
    const NAME: &'static str = "orients_blocks";
}
