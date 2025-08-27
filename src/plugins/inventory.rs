use bevy::{prelude::*, ui::{FocusPolicy, RelativeCursorPosition}};
use std::collections::HashMap;
use super::item::Item;

/// --------- INVENTORY LOGIC ---------
/// 
/// Totally decoupled from UI, just handled via events.



/// An inventory must have item slots (as an array of entities). Each slot stores an item.
#[derive(Component)]
//#[require(ItemSlots)]
pub struct Inventory; // Inventory marker, requires the bundle to have an entity vector.

#[derive(Component, Default)]
pub struct ItemSlots {
    slot_map : HashMap<usize, Entity>,
    slot_max : usize,
}


/// --------- INVENTORY UI ---------
/// 
/// Handles the components and methods to display a given Inventory with a UI.
/// The idea is to have a system that generates a UI based on the provided data
/// as well as some provided config.
/// The UI sends signals that modify the logical state of the inventory. (todo)

const GRID_COLS: usize = 4;
const GRID_ROWS: usize = 2;
const SLOT_SIZE: f32 = 80.0;
const GAP: f32 = 8.0;

const COLOR_UI_BG: Color = Color::srgb_u8(32, 36, 44);
const COLOR_UI_SLOT: Color = Color::srgb_u8(46, 52, 64);
const COLOR_UI_OUTLINE: Color = Color::srgb_u8(90, 98, 120);

pub struct UiInventoryPlugin;

impl Plugin for UiInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiDragState>()
            .add_systems(Startup, setup)
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

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {

    let freedrop : bool = true;

    // camera
    commands.spawn(Camera2d);

    let font = asset_server.load("fonts/FiraSans-Bold.ttf");

    // Root (full screen)
    let root = commands
        .spawn((
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

    // Inventory panel (CSS Grid)
    let panel = commands
        .spawn((
            Node {
                display: Display::Grid,

                // This part needs to be done better...
                grid_template_columns: vec![GridTrack::px(SLOT_SIZE),
                                            GridTrack::px(SLOT_SIZE),
                                            GridTrack::px(SLOT_SIZE),
                                            GridTrack::px(SLOT_SIZE)],
                grid_template_rows: vec![GridTrack::px(SLOT_SIZE), GridTrack::px(SLOT_SIZE)],

                // This part is probably fine
                column_gap: Val::Px(GAP),
                row_gap: Val::Px(GAP),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(COLOR_UI_BG),
            BorderColor::all(Color::srgb_u8(70, 80, 96)),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .set_parent_in_place(root).id();
    



    // Slots
    let mut ui_slot_entities = Vec::new();
    for _ in 0..GRID_ROWS * GRID_COLS {
        let slot = commands
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

    
    if freedrop {
        // // Free drop panel (CSS)
        // let free_drop_panel = commands
        // .spawn((
        //     Node {
        //         display: Display::Flex,
        //         padding: UiRect::all(Val::Px(16.0)),
        //         ..default()
        //     },
        //     BackgroundColor(COLOR_UI_BG),
        //     BorderColor::all(Color::srgb_u8(70, 80, 96)),
        //     BorderRadius::all(Val::Px(8.0)),
        // ))
        // .set_parent_in_place(root).id();

        // Free drop area
        let free_drop_slot = commands
        .spawn((
            UiItemSlot,
            UiFreeDrop,
            RelativeCursorPosition::default(),
            Button, // gives Interaction
            Node {
                width: Val::Px(3.0*GAP + 4.0*SLOT_SIZE),
                height: Val::Px(2.0*GAP + 3.0*SLOT_SIZE),
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
    ui_slot_entities.push(free_drop_slot);
    }


    // Demo items in first few slots
    for (i, &slot) in ui_slot_entities.iter().take(3).enumerate() {
        spawn_item_in_slot(
            &mut commands,
            slot,
            &font,
            &format!("Item {}", i + 1),
            Color::srgb_u8(88, 130, 236),
        );
    }

    // Ensure overlay renders atop (already child of root)
    let _ = overlay;
}

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

/// --------- Systems (Node-based updates) ---------
/// 
/// Might just be the cleanest thing I've seen for this UI system. Extremely reusable.


use bevy::window::PrimaryWindow;

#[derive(Component)]
pub struct UiItemSlot;

#[derive(Component)]
pub struct UiDragOverlay;

#[derive(Resource, Default)]
pub struct UiDragState {
    pub item: Option<Entity>,
    pub origin_slot: Option<Entity>,
    pub hovered_slot: Option<Entity>,
    pub grab_offset: Vec2,
    pub last_free_drop_px: Option<Vec2>,
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

        // Reparent the item under the Overlay so it draws above everything
        if let Ok(overlay) = q_overlay.single() {
            commands.entity(item).set_parent_in_place(overlay);
        }

        // Switch from flow layout to absolute positioning so we can place it freely
        node.position_type = PositionType::Absolute;

        // Seed its position so it appears centered under the cursor on pickup
        node.left = Val::Px(cursor.x - SLOT_SIZE * 0.5);
        node.top  = Val::Px(cursor.y - SLOT_SIZE * 0.5);

        // Bring it to the very front while dragging
        *z = ZIndex(999);

        // Store the "grab" offset — where inside the item the cursor is.
        // For this demo we assume center; if you want pixel-perfect behavior,
        // compute (cursor - item_top_left) at pickup time.
        drag.grab_offset = Vec2::new(SLOT_SIZE * 0.5, SLOT_SIZE * 0.5);

        // Additionally, if the item came from a FreeDrop area, store it in DragState
        if let Ok((comp, rel)) = q_freedrop.get(parent.unwrap().0) {

            // Returns normalized coordinates ranging from (-0.5, 0.5) in both directions
            let normalized = if let Some(n) = rel.normalized { n } else { Vec2::ZERO };

            // Convert normalized slot coords (0..1) to pixels inside the slot
            let slot_size = comp.size; // (width, height) in pixels after layout
            let cursor_in_slot_px = (normalized + Vec2::splat(0.5)) * slot_size;

            // We want the item's top-left, not the cursor position; subtract grab offset
            let item_top_left_in_slot_px = cursor_in_slot_px - drag.grab_offset;

            drag.last_free_drop_px = Some(item_top_left_in_slot_px);
        } else {
            drag.last_free_drop_px = None;
        }
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
    // To query node properties and reset layout properties back to grid layout
    mut q_node: Query<(&mut Node, &mut FocusPolicy, &mut ZIndex)>,
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
    // I love Rust's built-ins.
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

                // Returns normalized coordinates ranging from (-0.5, 0.5) in both directions
                let normalized = if let Some(n) = rel.normalized { n } else { Vec2::ZERO };

                // Convert normalized slot coords (0..1) to pixels inside the slot
                let slot_size = comp.size; // (width, height) in pixels after layout
                let cursor_in_slot_px = (normalized + Vec2::splat(0.5)) * slot_size;

                // We want the item's top-left, not the cursor position: subtract grab offset
                let item_top_left_in_slot_px = if !drag.hovered_slot.is_none() {
                    cursor_in_slot_px - drag.grab_offset
                } else { 
                    drag.last_free_drop_px.unwrap_or_default()
                };
                // Fallback: if we don't have a new target slot, and our origin is FreeDrop, use stored coords

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

    // TODO: if successful, update logic inventory

}