use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::plugin::controller::player::{GameInput, UiInput, UiBack, UiCloseAll};
use crate::plugin::state::GameUpdate;
use crate::plugin::ui::cursor::CursorLockRequest;
use crate::plugin::ui::inventory::EntityUISession;

pub const UI_SCREEN_BASE_ZINDEX: i32 = 100;
pub const UI_SCREEN_ZINDEX_STEP: i32 = 10;
pub const UI_SCREEN_DIM_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// COMPONENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marks a UI root as one entry of a player's screen stack. Anything without
/// this is persistent HUD (the hotbar) and is never touched by the stack.
#[derive(Component, Debug)]
pub struct UiScreen {
    pub owner: Entity,
}

/// The single source of truth for "what UI is this player looking at".
/// Interaction mode and cursor state are *derived* from this, never set by
/// feature code.
#[derive(Component, Default, Debug)]
pub struct UiStack {
    screens: Vec<Entity>,
}

impl UiStack {
    pub fn is_empty(&self) -> bool { self.screens.is_empty() }
    pub fn depth(&self)    -> usize { self.screens.len() }
    pub fn top(&self)      -> Option<Entity> { self.screens.last().copied() }
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.screens.iter().copied()
    }
}

/// Everything a caller may say about a screen. Deliberately tiny: if a knob
/// isn't here, the caller doesn't get to touch it.
#[derive(Clone, Debug)]
pub struct UiPushOptions {
    pub dim: bool,
    /// World entities this screen is a view of, if any. Feeds `EntityUISession`, so
    /// breaking a barrel closes the barrel's screen.
    pub sources: Vec<Entity>,
}

impl Default for UiPushOptions {
    fn default() -> Self { Self { dim: true, sources: Vec::new() } }
}

impl UiPushOptions {
    pub fn new() -> Self { Self::default() }
    pub fn dimmed(mut self, dim: bool) -> Self { self.dim = dim; self }
    pub fn viewing(mut self, entities: impl IntoIterator<Item = Entity>) -> Self {
        self.sources.extend(entities);
        self
    }
}

/// The one interstitial layer. `should_block_lower` makes the topmost screen
/// swallow every click its own content doesn't consume, so lower screens and
/// the 3D world are automatically inert.
fn build_screen_backdrop(spec: &UiPushOptions) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(if spec.dim { UI_SCREEN_DIM_COLOR } else { Color::NONE }),
        Pickable { should_block_lower: true, is_hoverable: false },
        ZIndex(UI_SCREEN_BASE_ZINDEX), // overwritten by the reconciler
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE PUBLIC API — the only thing feature code ever calls
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait UiScreenCommandsExt {
    /// Spawns `content` inside a fresh screen root and stacks it on `player`.
    fn push_ui_screen<B: Bundle>(&mut self, player: Entity, options: UiPushOptions, content: B) -> Entity;
    fn pop_ui_screen(&mut self, player: Entity);
    /// Closes this screen *and everything opened on top of it*.
    fn close_ui_screen(&mut self, screen: Entity);
    fn close_all_ui_screens(&mut self, player: Entity);
}

impl UiScreenCommandsExt for Commands<'_, '_> {
    fn push_ui_screen<B: Bundle>(&mut self, player: Entity, options: UiPushOptions, content: B) -> Entity {
        let screen = self.spawn((
            build_screen_backdrop(&options),
            UiScreen { owner: player },
            EntityUISession { source_entities: options.sources.clone() },
            Children::spawn(Spawn(content)),
        )).id();

        self.queue(move |world: &mut World| {
            let Ok(mut player_ref) = world.get_entity_mut(player) else {
                despawn_screen(world, screen);
                return;
            };
            if !player_ref.contains::<UiStack>() {
                player_ref.insert(UiStack::default());
            }
            if let Some(mut stack) = player_ref.get_mut::<UiStack>() {
                stack.screens.push(screen);
            }
        });

        screen
    }

    fn pop_ui_screen(&mut self, player: Entity) {
        self.queue(move |world: &mut World| {
            let top = {
                let Some(mut stack) = world.get_mut::<UiStack>(player) else { return };
                let Some(top) = stack.screens.pop() else { return };
                top
            };
            despawn_screen(world, top);
        });
    }

    fn close_ui_screen(&mut self, screen: Entity) {
        self.queue(move |world: &mut World| {
            // Not stacked (persistent HUD, or already dead) — nothing to unwind.
            let Some(owner) = world.get::<UiScreen>(screen).map(|s| s.owner) else {
                despawn_screen(world, screen);
                return;
            };
            let doomed = {
                let Some(mut stack) = world.get_mut::<UiStack>(owner) else { return };
                let Some(idx) = stack.screens.iter().position(|&e| e == screen) else { return };
                // Screens above this one were opened *from* it, so they go too.
                stack.screens.split_off(idx)
            };
            for entity in doomed { despawn_screen(world, entity); }
        });
    }

    fn close_all_ui_screens(&mut self, player: Entity) {
        self.queue(move |world: &mut World| {
            let doomed = {
                let Some(mut stack) = world.get_mut::<UiStack>(player) else { return };
                std::mem::take(&mut stack.screens)
            };
            for entity in doomed { despawn_screen(world, entity); }
        });
    }
}

fn despawn_screen(world: &mut World, screen: Entity) {
    if let Ok(entity) = world.get_entity_mut(screen) {
        entity.despawn();
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE RECONCILER — every derived bit of state lives here and nowhere else
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn reconcile_ui_stack_sys(
    mut commands: Commands,
    stacks: Query<(Entity, &UiStack), Changed<UiStack>>,
    mut z_indices: Query<&mut ZIndex, With<UiScreen>>,
) {
    for (player, stack) in stacks.iter() {
        for (depth, screen) in stack.iter().enumerate() {
            if let Ok(mut z) = z_indices.get_mut(screen) {
                z.0 = UI_SCREEN_BASE_ZINDEX + (depth as i32) * UI_SCREEN_ZINDEX_STEP;
            }
        }

        // All these are idempotent actions, so enforcing them twice in a row is fine.
        if stack.is_empty() {
            commands.entity(player).insert((
                ContextActivity::<GameInput>::ACTIVE,
                ContextActivity::<UiInput>::INACTIVE,
            ));
            commands.trigger(CursorLockRequest::Lock);
            commands.set_state(GameUpdate::Enabled);
        } else {
            commands.entity(player).insert((
                ContextActivity::<GameInput>::INACTIVE,
                ContextActivity::<UiInput>::ACTIVE,
            ));
            commands.trigger(CursorLockRequest::Unlock);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// UNIVERSAL CONTROLS — these two replace every per-UI close action
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn ui_back_obs(event: On<Start<UiBack>>, mut commands: Commands) {
    commands.pop_ui_screen(event.context);
}

pub fn ui_close_all_obs(event: On<Start<UiCloseAll>>, mut commands: Commands) {
    commands.close_all_ui_screens(event.context);
}