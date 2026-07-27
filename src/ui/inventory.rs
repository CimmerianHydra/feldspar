use bevy::{
    ecs::{lifecycle::HookContext, lifecycle::Remove, world::DeferredWorld},
    prelude::*,
};

use crate::content::item::{ItemRegistry, ItemStack};
use crate::sim::block_entity::BlockEntityEvent;
use crate::sim::inventory::{Inventory, InventoryChangedEvent, InventoryClickedEvent};
use crate::sim::player::PlayerInventoryAccess;
use crate::ui::crafting::build_inventory_crafting_ui;
use crate::ui::screen::{EntityUISessionEndRequest, UIPushOptions, UiScreenCommandsExt};
use crate::ui::theme::*;
use crate::ui::widget::{build_base_ui_item_slot, build_ui_item_display};

pub const MAX_PLAYER_INVENTORY_UI_COLS: usize = 9;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – SLOTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A UI node that views one slot of one inventory.
#[derive(Component)]
#[component(on_add = inventory_slot_on_add)]
pub struct InventorySlot {
    pub source_entity: Entity, // The inventory entity associated with this slot
    pub slot_index: usize,
}

fn inventory_slot_on_add(mut world: DeferredWorld, ctx: HookContext) {
    // Pull the data out of the slot we just received.
    let (source_entity, slot_index) = {
        let slot = world
            .entity(ctx.entity)
            .get::<InventorySlot>()
            .expect("on_add hook always runs after the component is in place");
        (slot.source_entity, slot.slot_index)
    };

    // Re-use the existing sync pipeline. The observer will look up the
    // current inventory contents and render this exact slot.
    world.commands().trigger(InventoryUISyncRequest {
        entity: source_entity,
        index:  slot_index,
    });
}

/// Builder function that returns a bundle of all relevant components for an
/// inventory item slot.
fn build_inventory_ui_item_slot(source_entity: Entity, slot_index: usize) -> impl Bundle {
    (
        build_base_ui_item_slot(),
        InventorySlot { source_entity, slot_index },
        Pickable { should_block_lower: true, is_hoverable: true },
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – PANELS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Event)]
pub struct InventoryUISyncRequest {
    pub entity: Entity, // The inventory entity we would like to view
    pub index: usize,
}

pub fn build_inventory_ui(
    source_entity: Entity,
    capacity: usize,
    max_cols: usize,
) -> impl Bundle {
    let cols = capacity.min(max_cols) as u16;

    (
        Node {
            display: Display::Grid,
            align_content: AlignContent::FlexStart,
            grid_template_columns: RepeatedGridTrack::auto(cols),
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        Pickable::IGNORE,
        // Once spawned, this automatically spawns as many children as
        // needed, building the correct item slots.
        Children::spawn(SpawnIter(
            (0..capacity).map(move |i| build_inventory_ui_item_slot(source_entity, i)),
        )),
    )
}

/// The standard player screen layout: caller's panel on top, player's main
/// inventory beneath it, hotbar at the bottom.
///
/// Returns `None` if the player has no `PlayerInventories` yet, or if one of
/// its handles points at a despawned entity — callers just `else { return }`.
///
/// Pass `()` as `top_panel` for a bare inventory screen.
pub fn build_player_ui_with_top_panel(
    player: Entity,
    player_inv_access: &PlayerInventoryAccess<'_, '_>,
    top_panel: impl Bundle,
) -> Option<impl Bundle> {
    // Both borrows end here — only the capacities and entity ids escape,
    // so the returned bundle owns everything it needs.
    let set = player_inv_access.get_from_player(player)?;

    let (main_entity, main_capacity) = {
        let inv = player_inv_access.main(player)?;
        (set.main, inv.capacity())
    };

    let (hotbar_entity, hotbar_capacity) = {
        let inv = player_inv_access.hotbar(player)?;
        (set.hotbar, inv.capacity())
    };

    Some((
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        Pickable::IGNORE,
        children![
            top_panel,
            build_inventory_ui(main_entity,   main_capacity,   MAX_PLAYER_INVENTORY_UI_COLS),
            build_inventory_ui(hotbar_entity, hotbar_capacity, MAX_PLAYER_INVENTORY_UI_COLS),
        ],
    ))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – OPENING SCREENS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// "Show this player their own inventory." Raised by `player::input`, which
/// owns the keybinding.
#[derive(Event, Clone, Copy, Debug)]
pub struct OpenPlayerInventoryRequest {
    pub player: Entity,
}

/// For now the player's "open inventory" action opens the inventory together
/// with the spatial crafting interface.
fn open_player_inventory_obs(
    event: On<OpenPlayerInventoryRequest>,
    mut commands: Commands,
    player_inv_access: PlayerInventoryAccess,
) {
    let player = event.player;

    let Some(set)     = player_inv_access.get_from_player(player).copied() else { return };
    let Some(spatial) = player_inv_access.crafting_grid(player) else { return };

    let involved_entities = vec![set.main, set.hotbar, set.crafting_grid];

    let top_panel = build_inventory_crafting_ui(set.crafting_machine, set.crafting_grid, spatial);

    let Some(panel) = build_player_ui_with_top_panel(player, &player_inv_access, top_panel) else {
        error!("There was a problem opening the player's inventory.");
        return;
    };

    commands.push_ui_screen(
        player,
        UIPushOptions { dim: true, sources: involved_entities, ..default() },
        panel,
    );
}

/// **The inversion.**
///
/// Any block-entity that turns out to have an `Inventory` gets an inventory
/// screen when it is used. The barrel does not call this, does not know it
/// exists, and does not import a single UI symbol — and the next twenty
/// machines that hold items get their screen for free.
fn open_block_inventory_obs(
    event: On<BlockEntityEvent>,
    mut commands: Commands,
    inventories: Query<&Inventory>,
    players: PlayerInventoryAccess,
) {
    let (block_entity, player) = (event.entity, event.player);

    let Ok(inventory) = inventories.get(block_entity) else { return };
    let capacity = inventory.capacity();

    let Some(sources) = players.ui_sources(player) else { return };
    let Some(ui) = build_player_ui_with_top_panel(
        player,
        &players,
        build_inventory_ui(block_entity, capacity, MAX_PLAYER_INVENTORY_UI_COLS),
    ) else { return };

    commands.push_ui_screen(
        player,
        UIPushOptions::new().viewing(sources).viewing([block_entity]),
        ui,
    );
}

/// The other half of the inversion: when an inventory goes away — the barrel
/// was broken, the chunk unloaded — close whatever was looking at it.
///
/// `Remove` fires on despawn too, so this covers every way an inventory can
/// disappear without any behaviour having to remember to say so.
fn close_screens_for_removed_inventory_obs(
    removed: On<Remove, Inventory>,
    mut commands: Commands,
) {
    commands.trigger(EntityUISessionEndRequest { source_entity: removed.entity });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – CLICKS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Pointer geometry is the UI's business; what a click *means* is the data
/// side's. This resolves the former and hands over the latter.
fn inventory_slot_click_obs(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    available_slots: Query<&InventorySlot>,
) {
    let Ok(slot_data) = available_slots.get(click.entity) else { return };
    commands.trigger(InventoryClickedEvent {
        entity:     slot_data.source_entity,
        slot_index: slot_data.slot_index,
        button:     click.button,
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – SYNC
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Redraw a single slot: wipe any existing item-display child, then respawn
/// one if the slot is non-empty. Single source of truth for "what does a slot
/// look like in the UI".
fn render_slot_contents(
    commands:       &mut Commands,
    slot_ui_entity: Entity,
    stack:          Option<ItemStack>,
    item_registry:  &ItemRegistry,
) {
    commands.entity(slot_ui_entity).despawn_children();

    if let Some(stack) = stack {
        let display_entity = commands
            .spawn(build_ui_item_display(
                &item_registry.get(stack.id).display,
                stack.count,
            ))
            .id();
        commands.entity(slot_ui_entity).add_child(display_entity);
    }
}

/// On-demand sync: when the event is received, redraw only the slot that was
/// affected. Slots whose `source_entity` doesn't match the event target are
/// skipped, so this is cheap even with many UIs (or none) in existence.
pub fn inventory_sync_obs(
    event: On<InventoryUISyncRequest>,
    mut commands: Commands,
    slots_q: Query<(Entity, &InventorySlot)>,
    inventory_q: Query<&Inventory>,
    item_registry: Res<ItemRegistry>,
) {
    // The inventory the event is about must still exist and be readable.
    let Ok(inventory) = inventory_q.get(event.entity) else { return };

    // The slot index must be in range. Bounds violation here means somebody
    // emitted a bogus event — bail rather than panic.
    let Some(&stack) = inventory.slots().get(event.index) else { return };

    // Redraw every slot UI that points at this (inventory, index).
    // Normally that's one entity; the loop also handles the edge case where
    // multiple UIs view the same inventory.
    for (slot_ui_entity, slot_data) in slots_q.iter() {
        if slot_data.source_entity != event.entity { continue; }
        if slot_data.slot_index    != event.index  { continue; }
        render_slot_contents(&mut commands, slot_ui_entity, stack, &item_registry);
    }
}

/// A change on the data side becomes a redraw request on the view side.
fn inventory_changed_to_ui_sync_obs(event: On<InventoryChangedEvent>, mut commands: Commands) {
    commands.trigger(InventoryUISyncRequest {
        entity: event.entity,
        index:  event.index,
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct UiInventoryPlugin;

impl Plugin for UiInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(open_player_inventory_obs)
            .add_observer(open_block_inventory_obs)
            .add_observer(close_screens_for_removed_inventory_obs)
            .add_observer(inventory_slot_click_obs)
            .add_observer(inventory_sync_obs)
            .add_observer(inventory_changed_to_ui_sync_obs);
    }
}
