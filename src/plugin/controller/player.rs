use std::f32::consts::FRAC_PI_2;
use bevy::prelude::*;

use avian3d::prelude::*;
use bevy_enhanced_input::prelude::*;
use crate::plugin::{block::interaction::DDARay, state::GameState};

// ── Tunables ──────────────────────────────────────────────────────────────────

const PLAYER_HEIGHT:    f32 = 1.9;
const PLAYER_RADIUS:    f32 = 0.4;
const CYL_HEIGHT:       f32 = PLAYER_HEIGHT;
const EYE_HEIGHT:       f32 = 1.75;
const CAM_LOCAL_Y:      f32 = EYE_HEIGHT - PLAYER_HEIGHT * 0.5;

const MOVE_SPEED:       f32 = 4.3;
const JUMP_SPEED:       f32 = 8.0;

const DEFAULT_SENSITIVITY: f32 = 0.0022;
const DEFAULT_REACH:       f32 = 8.0;
const PITCH_LIMIT:      f32 = FRAC_PI_2 - 0.01;

const GRAVITY_ACCEL:    f32 = 15.0;

// ── Actions ───────────────────────────────────────────────────────────────────

#[derive(InputAction)]
#[action_output(Vec2)]
struct Move;

#[derive(InputAction)]
#[action_output(Vec2)]
struct Look;

#[derive(InputAction)]
#[action_output(bool)]
struct Jump;

/// Action corresponding to left click in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct PrimaryFire;

/// Action corresponding to right click in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct SecondaryFire;

/// Action corresponding to middle mouse button in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct AltFire;

/// Action corresponding to key I in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct OpenPlayerInventory;

/// Action corresponding to key I in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct ClosePlayerInventory;

/// Actions corresponding to numbers 1-9 in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct SelectItem;

/// Action corresponding to Escape in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct OpenPauseMenu;

/// Action corresponding to Escape in the standard layout.
#[derive(InputAction)]
#[action_output(bool)]
pub struct ClosePauseMenu;

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct Player;

/// Set of inputs available to the player when they're in the game.
#[derive(Component)]
pub struct GameInput;

/// Set of inputs available to the player when they're browsing the pause menu.
#[derive(Component)]
pub struct PauseMenuInput;

/// Set of inputs available to the player when they're browsing their inventory.
#[derive(Component)]
pub struct InventoryInput;

/// Helper component for the "select n-th hotbar slot" action, to retrieve which hotbar index is selected by the action
#[derive(Component)]
pub struct HotbarSelection {
    pub index: usize,
}

#[derive(Component)]
pub struct FPSCamera {
    pub sensitivity: f32,
}

#[derive(Component, Default)]
struct PlayerMovementData {
    wish_dir:               Vec2,
    jump_queued:            bool,
    state:                  PlayerMovementState,
    time_since_grounded:    f32,
    time_since_jump_q:      f32,
}

#[derive(Default, PartialEq)]
enum PlayerMovementState {
    #[default]
    Grounded,
    Airborne,
}

// ── Spawn ─────────────────────────────────────────────────────────────────────

fn spawn_player_controller(mut commands: Commands) {
    commands
        .spawn((
            Player,
            PlayerMovementData::default(),
            InheritedVisibility::default(),

            RigidBody::Kinematic,
            Collider::cylinder(PLAYER_RADIUS, CYL_HEIGHT),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            Friction::new(0.0),
            Transform::from_xyz(0.0, 20.0, 0.0),

            build_game_input_actions(),
            build_pause_menu_input_actions(),
            build_player_inventory_menu_input_actions(),
            ContextActivity::<GameInput>::ACTIVE,
            ContextActivity::<PauseMenuInput>::INACTIVE,
            ContextActivity::<InventoryInput>::INACTIVE,

            children![(
                FPSCamera { sensitivity: DEFAULT_SENSITIVITY },
                DDARay { max_distance: DEFAULT_REACH },
                Camera3d::default(),
                Transform::from_xyz(0.0, CAM_LOCAL_Y, 0.0),
                SpatialListener::default(),
            )],
        ))
        .observe(on_look_fire)
        .observe(on_move_fire)
        .observe(on_move_complete)
        .observe(on_jump_start)
        .observe(on_open_player_inventory)
        .observe(on_close_player_inventory)
        .observe(on_open_pause_menu)
        .observe(on_close_pause_menu)

    ;
}

fn build_game_input_actions() -> impl Bundle
{
    (GameInput,
    actions!(GameInput[
            (
                Action::<Look>::new(),
                Bindings::spawn(Spawn((
                    Binding::mouse_motion(),
                    Negate::all(),
                ))),
            ),
            (Action::<PrimaryFire>::new(), bindings![MouseButton::Left]),
            (Action::<SecondaryFire>::new(), bindings![MouseButton::Right]),
            (Action::<Move>::new(), DeadZone::default(), Bindings::spawn(Cardinal::wasd_keys())),
            (Action::<Jump>::new(), bindings![KeyCode::Space]),
            (
                Action::<OpenPauseMenu>::new(),
                ActionSettings {
                    require_reset: true,
                    ..Default::default()
                },
                bindings![KeyCode::Escape],
            ),
            (
                Action::<OpenPlayerInventory>::new(),
                // We set `require_reset` to `true` because `CloseInventory` action uses the same input,
                // and we want it to be triggerable only after the button is released.
                ActionSettings {
                    require_reset: true,
                    ..Default::default()
                },
                bindings![KeyCode::KeyI],
            ),
            (Action::<SelectItem>::new(), HotbarSelection { index: 0 }, bindings![KeyCode::Digit1]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 1 }, bindings![KeyCode::Digit2]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 2 }, bindings![KeyCode::Digit3]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 3 }, bindings![KeyCode::Digit4]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 4 }, bindings![KeyCode::Digit5]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 5 }, bindings![KeyCode::Digit6]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 6 }, bindings![KeyCode::Digit7]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 7 }, bindings![KeyCode::Digit8]),
            (Action::<SelectItem>::new(), HotbarSelection { index: 8 }, bindings![KeyCode::Digit9]),
        ]),
    )
}

fn build_pause_menu_input_actions() -> impl Bundle
{
    (PauseMenuInput,
    actions!(PauseMenuInput[
            (
                Action::<ClosePauseMenu>::new(),
                ActionSettings {
                    require_reset: true,
                    ..Default::default()
                },
                bindings![KeyCode::Escape],
            ),
        ]),
    )
}

fn build_player_inventory_menu_input_actions() -> impl Bundle
{
    (InventoryInput,
    actions!(InventoryInput[
            (
                Action::<ClosePlayerInventory>::new(),
                ActionSettings {
                    require_reset: true,
                    ..Default::default()
                },
                bindings![KeyCode::KeyI, KeyCode::Escape],
            ),
        ]),
    )
}




// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INPUT OBSERVERS - TO BE SPAWNED INTO THE PLAYER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn on_move_fire(fire: On<Fire<Move>>, mut players: Query<&mut PlayerMovementData>) {
    if let Ok(mut mv) = players.get_mut(fire.context) {
        mv.wish_dir = fire.value;
    }
}

fn on_move_complete(done: On<Complete<Move>>, mut players: Query<&mut PlayerMovementData>) {
    if let Ok(mut mv) = players.get_mut(done.context) {
        mv.wish_dir = Vec2::ZERO;
    }
}

fn on_jump_start(start: On<Start<Jump>>, mut players: Query<&mut PlayerMovementData>) {
    if let Ok(mut mv) = players.get_mut(start.context) {
        mv.jump_queued = true;
        mv.time_since_jump_q = 0.0;
    }
}

fn on_open_player_inventory(
    start: On<Start<OpenPlayerInventory>>,
    mut commands: Commands,
) {
    commands.entity(start.context).insert((ContextActivity::<GameInput>::INACTIVE, ContextActivity::<InventoryInput>::ACTIVE));
}

fn on_close_player_inventory(
    start: On<Start<ClosePlayerInventory>>,
    mut commands: Commands,
) {
    commands.entity(start.context).insert((ContextActivity::<GameInput>::ACTIVE, ContextActivity::<InventoryInput>::INACTIVE));
}

fn on_open_pause_menu(
    start: On<Start<OpenPauseMenu>>,
    mut commands: Commands,
) {
    commands.entity(start.context).insert((ContextActivity::<GameInput>::INACTIVE, ContextActivity::<PauseMenuInput>::ACTIVE));
}

fn on_close_pause_menu(
    start: On<Start<ClosePauseMenu>>,
    mut commands: Commands,
) {
    commands.entity(start.context).insert((ContextActivity::<GameInput>::ACTIVE, ContextActivity::<PauseMenuInput>::INACTIVE));
}

// ── Look ──────────────────────────────────────────────────────────────────────
//
// Mirrors the convention of camera_mouse_sys, but splits the rotation:
//   - Body owns yaw (rotation around +Y).
//   - Camera child owns pitch (rotation around +X), clamped.
//
// Because each transform holds only one axis of rotation, we don't need the
// full YXZ Euler round-trip — `from_rotation_y` / `from_rotation_x` are enough.

fn on_look_fire(
    look: On<Fire<Look>>,
    mut bodies: Query<(&mut Transform, &Children), With<Player>>,
    mut cameras: Query<(&mut Transform, &FPSCamera), Without<Player>>,
) {
    let Ok((mut body_tf, children)) = bodies.get_mut(look.context) else { return };

    // Body yaw.
    let (yaw, _, _) = body_tf.rotation.to_euler(EulerRot::YXZ);
    // Sensitivity is pulled from whichever child is the FPS camera.
    // We need it before we can apply yaw, so peek at the first matching child.
    let sensitivity = children
        .iter()
        .find_map(|c| cameras.get(c).ok().map(|(_, cam)| cam.sensitivity))
        .unwrap_or(DEFAULT_SENSITIVITY);

    body_tf.rotation = Quat::from_rotation_y(yaw + look.value.x * sensitivity);

    // Camera pitch.
    for &child in children {
        if let Ok((mut cam_tf, _)) = cameras.get_mut(child) {
            let (_, pitch, _) = cam_tf.rotation.to_euler(EulerRot::YXZ);
            let new_pitch = (pitch + look.value.y * sensitivity)
                .clamp(-PITCH_LIMIT, PITCH_LIMIT);
            cam_tf.rotation = Quat::from_rotation_x(new_pitch);
        }
    }
}


// ── Physics step ──────────────────────────────────────────────────────────────

const GROUND_DOTPROD_LIMIT:  f32 = 0.51;  // max walkable slope: slightly more steep than a 45° slope (dot product with vertcal almost 0.5)
const GROUND_PROBE_DISTANCE: f32 = 0.10;  // how far below the feet we probe
const GROUND_PROBE_SHRINK:   f32 = 0.10;  // shrink the probe shape vs body

const COYOTE_TIME:  f32 = 0.1;

fn step(
    spatial: SpatialQuery,
    move_and_slide: MoveAndSlide,
    time: Res<Time<Physics>>,
    mut players: Query<(Entity, &Collider, &mut Transform, &mut LinearVelocity, &mut PlayerMovementData), With<Player>>,
) {
    let dt = time.delta();

    for (entity, collider, mut tf, mut vel, mut mv) in &mut players {
        // Input -> planar wish velocity.
        let wish_local = Vec3::new(mv.wish_dir.x, 0.0, -mv.wish_dir.y);
        let planar     = (tf.rotation * wish_local).normalize_or_zero() * MOVE_SPEED;
        vel.x = planar.x;
        vel.z = planar.z;

        if mv.state == PlayerMovementState::Airborne {
            // Apply gravity, only when airborne.
            vel.y -= GRAVITY_ACCEL * dt.as_secs_f32();
        };

        // Handle jumping
        let can_jump = mv.time_since_grounded < COYOTE_TIME;
        if can_jump && mv.jump_queued {
            vel.y = JUMP_SPEED;
            mv.state = PlayerMovementState::Airborne;
        }
        mv.jump_queued = false;

        // Move-and-slide. We no longer rely on its callback for ground state.
        let MoveAndSlideOutput { position, projected_velocity } =
            move_and_slide.move_and_slide(
                collider,
                tf.translation,
                tf.rotation,
                vel.0,
                dt,
                &MoveAndSlideConfig::default(),
                &SpatialQueryFilter::from_excluded_entities([entity]),
                |_| MoveAndSlideHitResponse::Accept,
            );
        tf.translation = position;
        vel.0 = projected_velocity;

        // Ground probe. Authoritative source of "am I on the ground?"
        // Shapecast a slightly-smaller copy of the body downward and see
        // if it hits something with a near-vertical normal.
        let new_state = probe_ground(&spatial, entity, tf.translation, tf.rotation);
        
        if new_state == PlayerMovementState::Grounded {
            mv.time_since_grounded = 0.0;
        } else {
            mv.time_since_grounded += dt.as_secs_f32();
        }
        mv.state = new_state;

        // Clamp tiny downward velocity when grounded so it doesn't
        // accumulate while we're glued to the floor.
        if mv.state == PlayerMovementState::Grounded && vel.y < 0.0 {
            vel.y = 0.0;
        }
    }
}

fn probe_ground(
    spatial:     &SpatialQuery,
    entity:      Entity,
    position:    Vec3,
    rotation:    Quat,
) -> PlayerMovementState {

    // Shrink the probe so it doesn't catch on walls we're sliding against.
    // For a cylinder, you'd shrink the radius; for a capsule, same idea.
    let probe = Collider::cylinder(PLAYER_RADIUS - GROUND_PROBE_SHRINK, CYL_HEIGHT) ;

    let filter = SpatialQueryFilter::default().with_excluded_entities([entity]);
    let hit = spatial.cast_shape(
        &probe,
        position,
        rotation,
        Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(GROUND_PROBE_DISTANCE),
        &filter,
    );

    match hit {
        Some(h) => {
            if h.normal1.dot(Vec3::Y) > GROUND_DOTPROD_LIMIT {PlayerMovementState::Grounded}
            else {PlayerMovementState::Airborne}
        },  // ~45° max slope
        None    => PlayerMovementState::Airborne,
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct PlayerControllerPlugin;

impl Plugin for PlayerControllerPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins(EnhancedInputPlugin)
        .add_input_context::<GameInput>()
        .add_input_context::<PauseMenuInput>()
        .add_input_context::<InventoryInput>()

        .add_systems(OnEnter(GameState::InGame), spawn_player_controller)
        .add_systems(FixedUpdate, step)

        ;
    }
}