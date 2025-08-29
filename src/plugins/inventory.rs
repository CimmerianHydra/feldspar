use bevy::{prelude::*, ui::{FocusPolicy, RelativeCursorPosition}};
use std::collections::HashMap;
use super::item::Item;

/// --------- INVENTORY LOGIC ---------
/// 
/// Totally decoupled from UI, just handled via events.

// ---------- Core data ----------

pub type ItemId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub id: ItemId,
    pub count: u16,
    pub max_stack: u16,
}

impl ItemStack {
    pub fn new(id: ItemId, count: u16, max_stack: u16) -> Self {
        Self { id, count, max_stack }
    }
    pub fn space_left(&self) -> u16 {
        self.max_stack.saturating_sub(self.count)
    }
    pub fn is_full(&self) -> bool { self.count >= self.max_stack }
    pub fn is_empty(&self) -> bool { self.count == 0 }
}

// Each *inventory* is its own entity.
#[derive(Component, Debug)]
pub struct Inventory {
    pub capacity: usize,                    // total number of logical slots (0..capacity-1)
    pub slots: HashMap<usize, ItemStack>,   // sparse storage: only occupied slots stored
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Self { capacity, slots: HashMap::new() }
    }
    pub fn get(&self, i: usize) -> Option<&ItemStack> { self.slots.get(&i) }
    pub fn get_mut(&mut self, i: usize) -> Option<&mut ItemStack> { self.slots.get_mut(&i) }
    pub fn set(&mut self, i: usize, stack: Option<ItemStack>) {
        if let Some(s) = stack { self.slots.insert(i, s); } else { self.slots.remove(&i); }
    }
    pub fn in_bounds(&self, i: usize) -> bool { i < self.capacity }
}

// Tag the “owner” who this inventory belongs to (player, chest, machine, ...).
#[derive(Component, Debug)]
#[relationship(relationship_target = EntityInventoryVector)]
pub struct InventoryOwnedBy(pub Entity);

#[derive(Component, Debug)]
#[relationship_target(relationship = InventoryOwnedBy, linked_spawn)]
pub struct EntityInventoryVector(Vec<Entity>);

// Disambiguate multiple inventories per owner (“Main”, “Input”, “Output”, etc.)
#[derive(Component, Debug, Clone)]
pub struct InventoryStorage;

#[derive(Component, Debug, Clone)]
pub struct InventoryInput;

#[derive(Component, Debug, Clone)]
pub struct InventoryOutput;

#[derive(Component, Debug, Clone)]
pub struct InventoryEquip; // Objects in this inventory will also play out effects

// ---------- Events ----------

// Request/command coming from UI or gameplay code.
#[derive(BufferedEvent, Debug)]
pub struct InventoryRequest {
    pub id: u64,          // arbitrary correlation id; can be 0 if unused
    pub action: InventoryAction,
}

#[derive(Debug)]
pub enum InventoryAction {
    // Put exactly this stack into slot (or None to clear).
    Set {
        inv: Entity,
        slot: usize,
        stack: Option<ItemStack>,
    },
    // Move items between slots, possibly across inventories.
    Move {
        from_inv: Entity,
        from_slot: usize,
        to_inv: Entity,
        to_slot: usize,
        amount: u16,       // how many to move (<= source.count)
        allow_swap: bool,  // if dst has a different item, swap instead of failing
    },
}

// Result + “what changed” notification.
#[derive(BufferedEvent, Debug)]
pub struct InventoryResult {
    pub id: u64,
    pub ok: bool,
    pub details: Option<String>,
}

// Per-inventory batched changes (so UI can refresh efficiently).
#[derive(BufferedEvent, Debug)]
pub struct InventoryChanged {
    pub inv: Entity,
    pub changes: Vec<SlotChange>,
}

#[derive(Debug)]
pub struct SlotChange {
    pub slot: usize,
    pub new_stack: Option<ItemStack>,
}

// ---------- Plugin ----------

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<InventoryRequest>()
           .add_event::<InventoryResult>()
           .add_event::<InventoryChanged>()
           .add_systems(Update, apply_inventory_requests);
    }
}

// ---------- Systems (backend apply) ----------

fn apply_inventory_requests(
    mut ev_in: EventReader<InventoryRequest>,
    mut ev_out_result: EventWriter<InventoryResult>,
    mut ev_out_changed: EventWriter<InventoryChanged>,
    mut q_inv: Query<&mut Inventory>,
) {
    for req in ev_in.read() {
        let mut ok = true;
        let mut details = None;
        let mut changed_per_inv: HashMap<Entity, Vec<SlotChange>> = HashMap::new();

        match &req.action {
            InventoryAction::Set {
                inv,
                slot,
                stack
            } => {
                let result = (
                    || -> Result<(), String> {
                        let mut inv_ref = q_inv.get_mut(*inv).map_err(|_| "Inventory entity not found")?;
                        if !inv_ref.in_bounds(*slot) { return Err("Slot out of bounds".into()); }
                        inv_ref.set(*slot, *stack);
                        // record change
                        changed_per_inv.entry(*inv)
                            .or_default()
                            .push(SlotChange { slot: *slot, new_stack: *stack });
                        Ok(())
                    }
                )();
                if let Err(e) = result { ok = false; details = Some(e); }
            }

            InventoryAction::Move {
                from_inv,
                from_slot,
                to_inv,
                to_slot,
                amount,
                allow_swap 
            } => {
                let result = (
                    || -> Result<(), String> {
                        let [mut src, mut dst] = q_inv.get_many_mut([*from_inv, *to_inv])
                        .map_err(|_| "Source or destination inventory not found")?;
                        
                        if !src.in_bounds(*from_slot) || !dst.in_bounds(*to_slot) {
                            return Err("Slot out of bounds".into());
                        }

                        // Borrow-dance: extract src stack, work in locals, then write back.
                        let mut src_stack = match src.get(*from_slot).cloned() {
                            Some(s) => s,
                            None => return Err("Source slot empty".into()),
                        };
                        let move_n = (*amount).min(src_stack.count);
                        if move_n == 0 { return Err("Move amount is zero".into()) };

                        match dst.get(*to_slot).cloned() {
                            // Destination empty means simple move
                            None => {
                                let moved = ItemStack::new(src_stack.id, move_n, src_stack.max_stack);
                                dst.set(*to_slot, Some(moved));
                                src_stack.count -= move_n;
                                if src_stack.count == 0 { src.set(*from_slot, None); }
                                else { src.set(*from_slot, Some(src_stack)); }

                                changed_per_inv.entry(*from_inv).or_default()
                                    .push(SlotChange { slot: *from_slot, new_stack: src.get(*from_slot).cloned() });
                                changed_per_inv.entry(*to_inv).or_default()
                                    .push(SlotChange { slot: *to_slot, new_stack: dst.get(*to_slot).cloned() });
                            }

                            Some(mut dst_stack) => {
                                if dst_stack.id == src_stack.id {
                                    // Merge stacks (respect max_stack)
                                    let can_take = dst_stack.space_left();
                                    let take = move_n.min(can_take);
                                    if take == 0 {
                                        return Err("Destination stack full".into());
                                    }
                                    dst_stack.count += take;
                                    dst.set(*to_slot, Some(dst_stack));

                                    src_stack.count -= take;
                                    if src_stack.count == 0 { src.set(*from_slot, None); }
                                    else { src.set(*from_slot, Some(src_stack)); }

                                    changed_per_inv.entry(*from_inv).or_default()
                                        .push(SlotChange { slot: *from_slot, new_stack: src.get(*from_slot).cloned() });
                                    changed_per_inv.entry(*to_inv).or_default()
                                        .push(SlotChange { slot: *to_slot, new_stack: dst.get(*to_slot).cloned() });
                                } else if *allow_swap && move_n == src_stack.count {
                                    // Swap full stacks (only if moving the full source stack)
                                    dst.set(*to_slot, Some(src_stack));
                                    src.set(*from_slot, Some(dst_stack));

                                    changed_per_inv.entry(*from_inv).or_default()
                                        .push(SlotChange { slot: *from_slot, new_stack: src.get(*from_slot).cloned() });
                                    changed_per_inv.entry(*to_inv).or_default()
                                        .push(SlotChange { slot: *to_slot, new_stack: dst.get(*to_slot).cloned() });
                                } else {
                                    return Err("Destination occupied by different item (swap not allowed)".into());
                                }
                            }
                        }

                        Ok(())
                    }
                )();
                if let Err(e) = result { ok = false; details = Some(e); }
            }
        }

        // Emit per-inventory change batches
        for (inv, changes) in changed_per_inv {
            if !changes.is_empty() {
                ev_out_changed.write(InventoryChanged { inv, changes });
            }
        }
        // Emit request result (OK / error)
        ev_out_result.write(InventoryResult { id: req.id, ok, details });
    }
}


/// --------- INVENTORY UI ---------
/// 
/// Handles the components and methods to display a given Inventory with a UI.
/// The idea is to have a system that generates a UI based on the provided data
/// as well as some provided config.
/// The UI sends signals that modify the logical state of the inventory. (todo)

const GRID_COLS: usize = 10;
const GRID_ROWS: usize = 3;
const SLOT_SIZE: f32 = 80.0;
const GAP: f32 = 8.0;

const COLOR_UI_BG: Color = Color::srgb_u8(32, 36, 44);
const COLOR_UI_SLOT: Color = Color::srgb_u8(46, 52, 64);
const COLOR_UI_OUTLINE: Color = Color::srgb_u8(90, 98, 120);

pub struct UiInventoryPlugin;

impl Plugin for UiInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiDragState>()
            .add_systems(PreStartup, setup)
            .add_systems(Startup, demo)
            .add_systems(
                Update,
                (
                    pick_up_item,
                    track_hovered_slot,
                    dragged_item_follow_cursor,
                    drop_item_on_click_release,
                ),
            );
    }
}

fn demo(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    q_root: Query<Entity, With<UiBackground>>,
    q_overlay: Query<Entity, With<UiDragOverlay>>,
) {

    // camera
    let cam = commands.spawn(Camera2d).id();

    let root = q_root.single().unwrap();
    let demo_inventory = build_inventory(&mut commands, cam, 30);
    let demo_panel = build_inventory_ui_grid(&mut commands, GRID_ROWS as u16, GRID_COLS as u16, Some(root));

    let ui_slot_entities = vec![];

    // Demo items in first few slots
    let font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");
    for (i, &slot) in ui_slot_entities.iter().take(3).enumerate() {
        spawn_item_in_slot(
            &mut commands,
            slot,
            &font,
            &format!("Item {}", i + 1),
            Color::srgb_u8(88, 130, 236),
        );
    }

    // Always call this at the end so it renders on top
    let ol = q_overlay.single().unwrap();
}

/// --------- Systems (Node-based updates) ---------
/// 
/// Might just be the cleanest thing I've seen for this UI system. Extremely reusable.


use bevy::window::PrimaryWindow;

#[derive(Component)]
pub struct UiItemSlot;

// Singleton that handles the item dragging overlay.
#[derive(Component)]
pub struct UiDragOverlay;

// Singleton that represents the root of all UI nodes.
#[derive(Component)]
pub struct UiBackground;

#[derive(Resource, Default)]
pub struct UiDragState {
    pub item: Option<Entity>,
    pub origin_slot: Option<Entity>,
    pub hovered_slot: Option<Entity>,
    pub grab_offset: Vec2,
    pub origin_free_drop_px: Option<Vec2>,
}

#[derive(Component)]
#[require(RelativeCursorPosition)]
pub struct UiFreeDrop;

// Click an item to start dragging it
fn pick_up_item(
    mut commands: Commands,
    mut drag: ResMut<UiDragState>,
    q_overlay: Query<Entity, With<UiDragOverlay>>,
    // We only care about items whose Interaction changed this frame
    mut q_items: Query<
        (Entity, &mut Node, &mut ZIndex, &mut FocusPolicy, Option<&ChildOf>),
        (With<Item>, Changed<Interaction>),
    >,
    // Used to check if we picked up from a FreeDrop area to store where it was
    q_freedrop: Query<(&ComputedNode, &RelativeCursorPosition), With<UiFreeDrop>>,
    // Used to read the concrete Interaction value for each item
    q_interaction: Query<&Interaction>,
    // To read the cursor position in window space
    q_windows: Query<&Window, With<PrimaryWindow>>,
) {
    // If we're already dragging something, ignore new presses
    if drag.item.is_some() { return };

    // Get the primary window; without it we can't compute cursor-based positioning
    let Ok(window) = q_windows.single() else { return };

    // Iterate only over items that had Interaction changes this frame
    for (item,
        mut node,
        mut z,
        mut focus,
        parent) in &mut q_items {
        // Read the new Interaction state (Pressed/Hovered/None)
        let Ok(interaction) = q_interaction.get(item) else { continue };

        // We only react to a *press* on an item to begin dragging
        if !matches!(interaction, Interaction::Pressed) { continue };
        
        // Update the focus policy of Item so it doesn't block Slot buttons
        *focus = FocusPolicy::Pass;
            
        // Remember where the item came from (its parent slot)
        drag.origin_slot = Some(parent.unwrap().0);

        // Mark this item as the one being dragged, wrap it in Option
        drag.item = Some(item);

        // Cursor in window coordinates (top-left origin)
        let Some(cursor) = window.cursor_position() else { continue };
        
        // Store the "grab" offset — where inside the item the cursor is.
        // For this demo we assume center; if you want pixel-perfect behavior,
        // compute (cursor - item_top_left) at pickup time.
        drag.grab_offset = Vec2::new(SLOT_SIZE * 0.5, SLOT_SIZE * 0.5);

        // Additionally, if the item came from a FreeDrop area, store it in DragState
        if let Ok((comp, rel)) = q_freedrop.get(parent.unwrap().0) {

            // We want the item's top-left, not the cursor position; subtract grab offset
            let item_top_left_in_slot_px = cursor_px_in_node(comp, rel) - drag.grab_offset;

            drag.origin_free_drop_px = Some(item_top_left_in_slot_px);
        } else {
            drag.origin_free_drop_px = None;
        }

        // Reparent the item under the Overlay so it draws above everything
        if let Ok(overlay) = q_overlay.single() {
            commands.entity(item).set_parent_in_place(overlay);
        }

        // Switch from flow layout to absolute positioning so we can place it freely
        node.position_type = PositionType::Absolute;

        // Seed its position so it appears centered under the cursor on pickup
        node.left = Val::Px(cursor.x - drag.grab_offset.x);
        node.top  = Val::Px(cursor.y - drag.grab_offset.y);

        // Bring it to the very front while dragging
        *z = ZIndex(999);
    }
}

// While dragging, keep the item positioned under the mouse cursor
fn dragged_item_follow_cursor(
    drag: Res<UiDragState>,
    // To mutate the item's Node (left/top values)
    mut q_node: Query<&mut Node>,
    // To fetch current cursor position
    q_windows: Query<&Window, With<PrimaryWindow>>,
) {
    // Only run if there *is* an active dragged item
    let Some(item) = drag.item else { return };

    // Access its Node to tweak absolute position
    let Ok(mut node) = q_node.get_mut(item) else { return };

    // Fetch cursor position; if not present (e.g., cursor left window), do nothing
    let Ok(window) = q_windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };

    // Maintain the initial grab offset so the item doesn't "jump" as you move
    let pos = cursor - drag.grab_offset;

    // Place the item in absolute UI coordinates
    node.left = Val::Px(pos.x);
    node.top  = Val::Px(pos.y);
}

// Track which slot is currently hovered while dragging,
// so we know the potential drop target
fn track_hovered_slot(
    // Take the DragState. Needs to be mutable since item and drop target will change
    mut drag: ResMut<UiDragState>,
    // Only consider slots that changed Interaction this frame
    q_changed_slots: Query<(Entity, &Interaction), (With<UiItemSlot>, Changed<Interaction>)>,
) {
    // Only meaningful if an item is being dragged
    if drag.item.is_none() { return };
    for (slot, interaction) in &q_changed_slots {
        match *interaction {
            // When a slot becomes Hovered, record it as the current drop target
            Interaction::Hovered => {
                drag.hovered_slot = Some(slot)
                },

            // On None/Pressed we might be leaving hover; if this was our tracked slot, clear it
            Interaction::None | Interaction::Pressed => {
                if drag.hovered_slot == Some(slot) {
                    drag.hovered_slot = None;
                }
            }
        }
    }
}

// When the left mouse button is released, drop the item
// into the hovered slot (if any), otherwise back to its origin
fn drop_item_on_click_release(
    mut commands: Commands,
    // To obtain current dragging state
    mut drag: ResMut<UiDragState>,
    // To detect left button release precisely
    buttons: Res<ButtonInput<MouseButton>>,
    // To query item node properties and reset layout properties back to grid layout
    mut q_node: Query<(&mut Node, &mut FocusPolicy, &mut ZIndex)>,
    // To check if the original node was a FreeDrop
    q_freedrop: Query<(&ComputedNode, &RelativeCursorPosition), With<UiFreeDrop>>
) {
    // Bail if we're not currently dragging
    if drag.item.is_none() { return };

    // Only act exactly on the frame the left mouse is released
    if !buttons.just_released(MouseButton::Left) { return };

    // Extract and clear the active item handle
    let item = drag.item.take().expect("There was no item to take from UiDragState.");

    // Choose the drop parent:
    // - preferred: the slot currently hovered
    // - fallback: the original slot we picked the item from
    // I love Rust's built-ins
    let target_parent = drag.hovered_slot.or(drag.origin_slot);

    if let Some(slot) = target_parent {
        // Reparent the item into its new (or original) slot
        commands.entity(item).set_parent_in_place(slot);
    }

    if let Ok((
            mut node,
            mut focus,
            mut z_idx,
        )) = q_node.get_mut(item) {

        if let Some(slot) = target_parent {
            // Branch A: If the UiItemSlot is also a free drop area
            if let Ok((comp, rel)) = q_freedrop.get(slot) {

                // We want the item's top-left, not the cursor position: subtract grab offset
                // Fallback: if we don't have a new target slot, and our origin is FreeDrop, use stored coords
                let item_top_left_in_slot_px = if !drag.hovered_slot.is_none() {
                    cursor_px_in_node(comp, rel) - drag.grab_offset
                } else { 
                    drag.origin_free_drop_px.unwrap_or_default()
                };
                
                node.position_type = PositionType::Absolute;
                node.left = Val::Px(item_top_left_in_slot_px.x);
                node.top  = Val::Px(item_top_left_in_slot_px.y);
            }

            // Branch B: If the UiItemSlot is not a freedrop
            else {
                node.position_type = PositionType::Relative;
                node.left = Val::Auto;
                node.top  = Val::Auto;
            }
        }
        
        *focus = FocusPolicy::Block; // We want it to block any Buttons underneath again
        *z_idx = ZIndex(0);
    }

    // Fully reset drag state for the next interaction
    drag.origin_slot = None;
    drag.hovered_slot = None;
    drag.grab_offset = Vec2::ZERO;

    // TODO: if successful, send out update event

}

fn cursor_px_in_node(comp: &ComputedNode, rel: &RelativeCursorPosition) -> Vec2 {
    // Returns normalized coordinates ranging from (-0.5, 0.5) in both directions
    let normalized = if let Some(n) = rel.normalized { n } else { Vec2::ZERO };

    // Convert normalized slot coords (0..1) to pixels inside the slot
    let slot_size = comp.size; // (width, height) in pixels after layout
    let cursor_in_slot_px = (normalized + Vec2::splat(0.5)) * slot_size;

    return cursor_in_slot_px
}



/// --------- INVENTORY SPAWNING ---------
/// 
/// Functions for the creation and management of logical and UI inventories.

pub fn setup(
    mut commands : Commands,
) {
    // Root (full screen)
    let root = commands
        .spawn((
            UiBackground,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(20, 20, 24)),
        )).id();

    // Overlay (absolute, topmost)
    let overlay = commands
        .spawn((
            UiDragOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ZIndex(1000),
        ))
        .set_parent_in_place(root).id();

}

pub fn build_inventory(
    commands : &mut Commands,
    owner : Entity,
    capacity : usize,
) -> Entity {
    commands.spawn((
        Inventory::new(capacity),
        InventoryOwnedBy(owner),
    )).id()
}

pub fn build_ui_panel(
    commands : &mut Commands,
    root : Option<Entity>,
) -> Entity {
    let panel = commands
            .spawn((
                Node{
                    padding: UiRect::all(Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(COLOR_UI_BG),
                BorderColor::all(COLOR_UI_OUTLINE),
                BorderRadius::all(Val::Px(8.0)),
            )).id();
    
    if let Some(r) = root {
        commands.entity(panel).set_parent_in_place(r);
    }

    panel
}

pub fn build_inventory_ui_grid(
    commands : &mut Commands,
    rows : u16,
    cols : u16,
    root : Option<Entity>,
) -> Entity {
    // Inventory panel (CSS Grid)
    let panel = commands
        .spawn((
            Node {
                display: Display::Grid,

                // Is it RepeatedGridTrack or GridTrack that I should use? Dunno
                grid_template_columns: RepeatedGridTrack::px(cols, SLOT_SIZE),
                grid_template_rows: RepeatedGridTrack::px(rows, SLOT_SIZE),

                // This part is probably fine
                column_gap: Val::Px(GAP),
                row_gap: Val::Px(GAP),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(COLOR_UI_BG),
            BorderColor::all(COLOR_UI_OUTLINE),
            BorderRadius::all(Val::Px(8.0)),
        )).id();
    
    if let Some(r) = root {
        commands.entity(panel).set_parent_in_place(r);
    }
    
    // Slots
    let mut ui_slot_entities = Vec::new();
    for _ in 0..rows*cols {
        let slot: Entity = commands
            .spawn((
                UiItemSlot,
                Button, // gives Interaction
                Node {
                    width: Val::Px(SLOT_SIZE),
                    height: Val::Px(SLOT_SIZE),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(COLOR_UI_SLOT),
                Outline {
                    width : Val::Px(1.0),
                    color : COLOR_UI_OUTLINE,
                    ..default()
                },
                BorderRadius::all(Val::Px(6.0)),
            ))
            .set_parent_in_place(panel).id();
        ui_slot_entities.push(slot);
    }

    panel
    // TODO: bind UI slots to logical inventory slots for event writing.
}

/// --------- INVENTORY SPAWNING DEMO ---------

// Spawns a demo item in UI slot, UI only.
fn spawn_item_in_slot(
    commands: &mut Commands,
    slot: Entity,
    font: &Handle<Font>,
    label: &str,
    color: Color,
) {
    let item = commands
        .spawn((
            Item,
            Button, // clickable
            Node {
                width: Val::Px(SLOT_SIZE - 10.0),
                height: Val::Px(SLOT_SIZE - 10.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(color),
            BorderRadius::all(Val::Px(4.0)),
            ZIndex(0),
            FocusPolicy::Block,
        ))
        .set_parent_in_place(slot)
        .id();
    
    commands.entity(item).with_children(|c| {
        c.spawn((
            Text::new(label),
            TextFont {
                font: font.clone(),
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    });
}