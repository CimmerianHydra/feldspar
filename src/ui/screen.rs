use bevy::prelude::*;

use crate::ui::cursor::CursorLockRequest;
use crate::GameUpdate;

pub const UI_SCREEN_BASE_ZINDEX: i32 = 100;
pub const UI_SCREEN_ZINDEX_STEP: i32 = 10;
pub const UI_SCREEN_DIM_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – COMPONENTS
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

/// Keeps track of the world entities a UI is a view of.
///
/// Useful to query the UI objects involved with entities — inventory UI,
/// machine UI — and to despawn a root when one of its subjects goes away.
/// (Multiplayer will want this expressed per-viewer rather than globally.)
#[derive(Component)]
pub struct EntityUISession {
    pub source_entities: Vec<Entity>,
}

/// Close whatever UI is a view of `source_entity`.
///
/// A plain `Event`, not an `EntityEvent`: the commonest reason to send one is
/// that `source_entity` has just been destroyed, and targeting an event at a
/// dead entity is asking for trouble.
#[derive(Event, Clone, Copy, Debug)]
pub struct EntityUISessionEndRequest {
    pub source_entity: Entity,
}

/// Marks a screen as owning the keyboard. While it's on top of the stack every
/// input context goes inactive, so keystrokes reach a text buffer instead of
/// firing actions. Without this, typing "e" into the console opens the inventory.
#[derive(Component)]
pub struct TextCapture;

/// The reconciler's mirror of `TextCapture`, on the *player*. Lets keyboard
/// ingestion be one query filter instead of a stack walk.
#[derive(Component)]
pub struct TextFocus;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – REQUESTS AND NOTIFICATIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// "Pop this player's topmost screen." Raised by whoever owns the binding —
/// `player::input` — so that `ui` never names an input action.
#[derive(Event, Clone, Copy, Debug)]
pub struct UiBackRequest {
    pub player: Entity,
}

/// "Close every screen this player has open."
#[derive(Event, Clone, Copy, Debug)]
pub struct UiCloseAllRequest {
    pub player: Entity,
}

/// What the player's attention is on, derived from the stack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UiFocus {
    /// No screens: the game has the mouse and the keyboard.
    World,
    /// At least one screen, none of which wants raw text.
    Screens,
    /// The topmost screen is capturing text.
    Text,
}

/// Announced whenever the derived focus changes.
///
/// This is the inversion that lets `ui` stay below `player`: the reconciler
/// decides *what state the player is in*, and `player::input` decides what
/// that means for input contexts. Presentation never touches a binding.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct UiFocusChanged {
    #[event_target]
    pub player: Entity,
    pub focus:  UiFocus,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE BACKDROP
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Everything a caller may say about a screen. Deliberately tiny: if a knob
/// isn't here, the caller doesn't get to touch it.
#[derive(Clone, Debug)]
pub struct UIPushOptions {
    pub dim: bool,
    /// World entities this screen is a view of, if any. Feeds
    /// `EntityUISession`, so breaking a barrel closes the barrel's screen.
    pub sources: Vec<Entity>,
    /// Does this UI capture text and swallow every keystroke while it's on top?
    pub captures_text: bool,
}

impl Default for UIPushOptions {
    fn default() -> Self { Self {
        dim: true,
        sources: Vec::new(),
        captures_text: false,
    } }
}

impl UIPushOptions {
    pub fn new() -> Self { Self::default() }
    pub fn dimmed(mut self, dim: bool) -> Self { self.dim = dim; self }
    pub fn viewing(mut self, entities: impl IntoIterator<Item = Entity>) -> Self {
        self.sources.extend(entities);
        self
    }
    pub fn capturing_text(mut self) -> Self { self.captures_text = true; self }
}

/// The one interstitial layer. `should_block_lower` makes the topmost screen
/// swallow every click its own content doesn't consume, so lower screens and
/// the 3D world are automatically inert.
fn build_screen_backdrop(spec: &UIPushOptions) -> impl Bundle {
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
// SECTION 4 – THE PUBLIC API — the only thing feature code ever calls
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait UiScreenCommandsExt {
    /// Spawns `content` inside a fresh screen root and stacks it on `player`.
    fn push_ui_screen<B: Bundle>(&mut self, player: Entity, options: UIPushOptions, content: B) -> Entity;
    fn pop_ui_screen(&mut self, player: Entity);
    /// Closes this screen *and everything opened on top of it*.
    fn close_ui_screen(&mut self, screen: Entity);
    fn close_all_ui_screens(&mut self, player: Entity);
}

impl UiScreenCommandsExt for Commands<'_, '_> {
    fn push_ui_screen<B: Bundle>(&mut self, player: Entity, options: UIPushOptions, content: B) -> Entity {
        let screen = self.spawn((
            build_screen_backdrop(&options),
            UiScreen { owner: player },
            EntityUISession { source_entities: options.sources.clone() },
            Children::spawn(Spawn(content)),
        )).id();

        if options.captures_text {
            self.entity(screen).insert(TextCapture);
        };

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
// SECTION 5 – THE RECONCILER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Every derived bit of screen state lives here and nowhere else.
pub fn reconcile_ui_stack_sys(
    mut commands:  Commands,
    stacks:        Query<(Entity, &UiStack), Changed<UiStack>>,
    mut z_indices: Query<&mut ZIndex, With<UiScreen>>,
    text_capture:  Query<(), With<TextCapture>>,
) {
    for (player, stack) in stacks.iter() {
        for (depth, screen) in stack.iter().enumerate() {
            if let Ok(mut z) = z_indices.get_mut(screen) {
                z.0 = UI_SCREEN_BASE_ZINDEX + (depth as i32) * UI_SCREEN_ZINDEX_STEP;
            }
        }

        let capturing_text = stack.top().is_some_and(|top| text_capture.contains(top));

        let focus = if stack.is_empty() {
            UiFocus::World
        } else if capturing_text {
            UiFocus::Text
        } else {
            UiFocus::Screens
        };

        match focus {
            UiFocus::World => {
                commands.entity(player).remove::<TextFocus>();
                commands.trigger(CursorLockRequest::Lock);
                commands.set_state(GameUpdate::Enabled);
            }
            UiFocus::Text => {
                commands.entity(player).insert(TextFocus);
                commands.trigger(CursorLockRequest::Unlock);
            }
            UiFocus::Screens => {
                commands.entity(player).remove::<TextFocus>();
                commands.trigger(CursorLockRequest::Unlock);
            }
        }

        commands.trigger(UiFocusChanged { player, focus });
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – UNIVERSAL CONTROLS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn ui_back_obs(event: On<UiBackRequest>, mut commands: Commands) {
    commands.pop_ui_screen(event.player);
}

fn ui_close_all_obs(event: On<UiCloseAllRequest>, mut commands: Commands) {
    commands.close_all_ui_screens(event.player);
}

fn entity_ui_session_end_obs(
    close_requests: On<EntityUISessionEndRequest>,
    mut commands: Commands,
    sessions: Query<(Entity, &EntityUISession, Has<UiScreen>)>,
) {
    let source = close_requests.source_entity;

    for (ui_entity, session, is_screen) in sessions.iter() {
        if !session.source_entities.contains(&source) { continue; }
        // Never despawn a screen behind the stack's back, or the derived
        // cursor/context state drifts.
        if is_screen { commands.close_ui_screen(ui_entity); }
        else         { commands.entity(ui_entity).despawn(); }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct UiScreenPlugin;

impl Plugin for UiScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, reconcile_ui_stack_sys)
            .add_observer(ui_back_obs)
            .add_observer(ui_close_all_obs)
            .add_observer(entity_ui_session_end_obs);
    }
}
