use bevy::prelude::*;

use crate::command::{
    ArgSpec, CommandContext, CommandError, CommandLevel, CommandResult, CommandSpec, Feedback,
};
use crate::content::item::ItemRegistry;
use crate::sim::inventory::stack::insert_and_notify;
use crate::sim::inventory::storage::Inventory;
use crate::sim::player::PlayerInventories;

const SUGGESTION_LIMIT: usize = 5;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – /give
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn give_command_spec() -> CommandSpec {
    CommandSpec::new("give")
        .alias("g")
        .help("Put items straight into the calling player's inventory.")
        .arg(ArgSpec::item("item"))
        .arg(ArgSpec::int("amount").default_int(1))
        .requires_player()
        .level(CommandLevel::Cheat)
}

/// An ordinary system, with ordinary params. The only thing marking it as a
/// command handler is its input and output — which is exactly the point, and
/// exactly why `sim` can own a command while the console screen lives six
/// layers above it.
pub fn give_cmd(
    In(context):        In<CommandContext>,
    mut commands:       Commands,
    item_registry:      Res<ItemRegistry>,
    player_inventories: Query<&PlayerInventories>,
    mut inventories:    Query<&mut Inventory>,
) -> CommandResult {
    let player = context.player()?;

    // The schema guarantees both of these exist: `item` is required, `amount`
    // has a default. So the infallible accessors are the right ones, and a
    // failure here would be a schema bug, not user error.
    let requested_name   = context.text("item")?.to_ascii_lowercase();
    let requested_amount = context.int("amount")?;

    // ── Validate before touching the world ───────────────────────────────
    if requested_amount < 1 {
        return Err(CommandError::Failed("'amount' has to be at least 1".into()));
    }
    if requested_amount > u16::MAX as i64 {
        return Err(CommandError::Failed(format!("'amount' has to be at most {}", u16::MAX)));
    }
    let requested_amount = requested_amount as u16;

    // ── Resolve the item ─────────────────────────────────────────────────
    // Case-folding is a policy decision, and it belongs here rather than in the
    // binder: the console doesn't know that item keys happen to be lowercase.
    let Some(item_id) = item_registry.by_name(requested_name.clone()) else {
        let suggestions = suggest_item_names(&item_registry, &requested_name);
        return Err(CommandError::Failed(if suggestions.is_empty() {
            format!("No item named '{requested_name}'.")
        } else {
            format!(
                "No item named '{requested_name}'. Did you mean: {}?",
                suggestions.join(", ")
            )
        }));
    };
    let display_name = item_registry.get(item_id).display_name.clone();

    // ── Resolve the destinations ─────────────────────────────────────────
    let Ok(&inventory_set) = player_inventories.get(player) else {
        return Err(CommandError::Failed("The calling player entity has no inventories".into()));
    };

    // Hotbar first, then inventory.
    let mut given = 0u16;
    let mut left  = requested_amount;

    for target in [inventory_set.hotbar, inventory_set.main] {
        if left == 0 { break; }

        let Ok(mut inventory) = inventories.get_mut(target) else { continue };
        let result = insert_and_notify(
            &mut commands,
            target,
            &mut inventory,
            item_id,
            left,
            &item_registry,
        );

        // Deliberately derived from `transferred` rather than read off
        // `remainder`.
        given += result.transferred;
        left   = left.saturating_sub(result.transferred);
    }

    // ── Report ───────────────────────────────────────────────────────────
    if given == 0 {
        return Err(CommandError::Failed(format!("No inventory space available for [{display_name}].")));
    }
    if left > 0 {
        return Ok(Feedback::warn(format!(
            "Added [{display_name}]x{given} to player inventory. Inventory full: {left} could not fit."
        )));
    }
    Ok(Feedback::success(format!("Added [{display_name}]x{given} to player inventory.")))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – NAME SUGGESTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Prefix matches first, then substring. Walks the whole registry, which is
/// fine because it only ever runs on the failure path.
fn suggest_item_names(registry: &ItemRegistry, needle: &str) -> Vec<String> {
    let mut prefixed:  Vec<String> = Vec::new();
    let mut contained: Vec<String> = Vec::new();

    for id in registry.ids() {
        let name = &registry.get(id).name;
        if name.starts_with(needle) {
            prefixed.push(name.clone());
        } else if name.contains(needle) {
            contained.push(name.clone());
        }
    }

    prefixed.sort();
    contained.sort();
    prefixed.extend(contained);
    prefixed.truncate(SUGGESTION_LIMIT);
    prefixed
}
