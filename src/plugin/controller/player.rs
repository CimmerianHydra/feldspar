use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

use crate::plugin::state::*;

use avian3d::prelude::*;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy_enhanced_input::prelude::*;
use crate::plugin::block_interaction::DDARay;

// ── Tunables ──────────────────────────────────────────────────────────────────

const PLAYER_HEIGHT:    f32 = 1.9;
const PLAYER_RADIUS:    f32 = 0.40;
const CAPSULE_LENGTH:   f32 = PLAYER_HEIGHT;
const EYE_HEIGHT:       f32 = 1.75;
const CAM_LOCAL_Y:      f32 = EYE_HEIGHT - PLAYER_HEIGHT * 0.5;

const MOVE_SPEED:       f32 = 4.3;
const JUMP_SPEED:       f32 = 8.0;
const GROUND_SKIN:      f32 = 0.05;

const DEFAULT_SENSITIVITY: f32 = 0.0022;
const DEFAULT_REACH:       f32 = 8.0;
const PITCH_LIMIT:      f32 = FRAC_PI_2 - 0.01;

const GRAVITY_ACCEL:    f32 = 15.0;

// ── Actions ───────────────────────────────────────────────────────────────────

#[derive(InputAction)]
#[action_output(Vec2)]
struct Move;

#[derive(InputAction)]
#[action_output(bool)]
struct Jump;

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct Player;

#[derive(Component)]
struct PlayerInput;

#[derive(Component)]
pub struct FPSCamera {
    pub sensitivity: f32,
}

#[derive(Component, Default)]
struct PlayerMovement {
    wish_dir:    Vec2,
    jump_queued: bool,
    grounded:    bool,
}

// ── Spawn ─────────────────────────────────────────────────────────────────────

fn spawn_player(mut commands: Commands) {
    commands
        .spawn((
            Player,
            PlayerMovement::default(),
            InheritedVisibility::default(),

            RigidBody::Kinematic,
            Collider::cylinder(PLAYER_RADIUS, CAPSULE_LENGTH),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            Friction::new(0.0),
            Transform::from_xyz(0.0, 20.0, 0.0),

            PlayerInput,
            actions!(PlayerInput[
                (
                    Action::<Move>::new(),
                    DeadZone::default(),
                    Bindings::spawn(Cardinal::wasd_keys()),
                ),
                (
                    Action::<Jump>::new(),
                    bindings![KeyCode::Space],
                ),
            ]),

            children![(
                FPSCamera { sensitivity: DEFAULT_SENSITIVITY },
                DDARay { max_distance: DEFAULT_REACH },
                Camera3d::default(),
                Transform::from_xyz(0.0, CAM_LOCAL_Y, 0.0),
            )],
        ))
        .observe(on_move_fire)
        .observe(on_move_complete)
        .observe(on_jump_start);
}

// ── Look ──────────────────────────────────────────────────────────────────────
//
// Mirrors the convention of camera_mouse_sys, but splits the rotation:
//   - Body owns yaw (rotation around +Y).
//   - Camera child owns pitch (rotation around +X), clamped.
//
// Because each transform holds only one axis of rotation, we don't need the
// full YXZ Euler round-trip — `from_rotation_y` / `from_rotation_x` are enough.

fn player_look_sys(
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut player_q: Query<(&mut Transform, &Children), With<Player>>,
    mut camera_q: Query<(&mut Transform, &FPSCamera), Without<Player>>,
) {
    let Ok((mut body_tf, children)) = player_q.single_mut() else { return };
    for &child in children {
        if let Ok((mut cam_tf, camera_data)) = camera_q.get_mut(child) {

            if mouse_motion.delta == Vec2::ZERO { return; }

            let delta_x = mouse_motion.delta.x * camera_data.sensitivity;
            let delta_y = mouse_motion.delta.y * camera_data.sensitivity;

            // Body yaw: read current yaw, subtract delta_x, rebuild.
            let (yaw, _, _) = body_tf.rotation.to_euler(EulerRot::YXZ);
            body_tf.rotation = Quat::from_rotation_y(yaw - delta_x);

            // Camera pitch: same idea on whichever child is the FpsCamera.
    
            let (_, pitch, _) = cam_tf.rotation.to_euler(EulerRot::YXZ);
            let new_pitch = (pitch - delta_y).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            cam_tf.rotation = Quat::from_rotation_x(new_pitch);
        }
    }
}

// ── Input observers ───────────────────────────────────────────────────────────

fn on_move_fire(fire: On<Fire<Move>>, mut players: Query<&mut PlayerMovement>) {
    if let Ok(mut mv) = players.get_mut(fire.context) {
        mv.wish_dir = fire.value;
    }
}

fn on_move_complete(done: On<Complete<Move>>, mut players: Query<&mut PlayerMovement>) {
    if let Ok(mut mv) = players.get_mut(done.context) {
        mv.wish_dir = Vec2::ZERO;
    }
}

fn on_jump_start(start: On<Start<Jump>>, mut players: Query<&mut PlayerMovement>) {
    if let Ok(mut mv) = players.get_mut(start.context) {
        mv.jump_queued = true;
    }
}

// ── Physics step ──────────────────────────────────────────────────────────────

const GROUND_ANGLE_LIMIT: f32 = std::f32::consts::FRAC_PI_4 + 3.0; // max walkable slope: slightly more steep than a 45° slope

fn step(
    move_and_slide: MoveAndSlide,
    time: Res<Time>,
    mut players: Query<
        (
            Entity,
            &Collider,
            &mut Transform,
            &mut LinearVelocity,
            &mut PlayerMovement,
        ),
        With<Player>,
    >,
) {
    let dt = time.delta();

    for (entity, collider, mut tf, mut vel, mut mv) in &mut players {
        // 1. Camera-relative horizontal wish velocity.
        let wish_local = Vec3::new(mv.wish_dir.x, 0.0, -mv.wish_dir.y);
        let planar     = (tf.rotation * wish_local).normalize_or_zero() * MOVE_SPEED;

        // 2. Compose the velocity move-and-slide will use this tick.
        //    Horizontal comes from input; vertical is whatever physics has
        //    accumulated (gravity below, jump impulse).
        vel.x = planar.x;
        vel.z = planar.z;

        // 3. Gravity (skip when grounded so we don't drill into the floor;
        //    move-and-slide will zero residual downward velocity anyway).
        if !mv.grounded {
            vel.y -= GRAVITY_ACCEL * dt.as_secs_f32();
        }

        // 4. Jump: consume the queued flag while we're still grounded.
        if mv.jump_queued && mv.grounded {
            vel.y = JUMP_SPEED;
            mv.grounded = false; // we just left the ground
        }
        mv.jump_queued = false;

        // 5. Move-and-slide. Updates ground state from the hits we observe.
        let mut hit_ground   = false;
        let mut hit_ceiling  = false;

        let MoveAndSlideOutput { position, projected_velocity } =
            move_and_slide.move_and_slide(
                collider,
                tf.translation,
                tf.rotation,
                vel.0,
                dt,
                &MoveAndSlideConfig {
                    skin_width: GROUND_SKIN,
                    ..default()
                },
                &SpatialQueryFilter::from_excluded_entities([entity]),
                |hit| {
                    // Surface classification: floor if its normal points
                    // up steeply enough; ceiling if it points down.
                    let dot_up = hit.normal.dot(Vec3::Y);
                    let angle  = dot_up.acos();
                    if dot_up > 0.0 && angle <= GROUND_ANGLE_LIMIT {
                        hit_ground = true;
                    }
                    if dot_up < 0.0 {
                        hit_ceiling = true;
                    }
                    MoveAndSlideHitResponse::Accept
                },
            );

        // 6. Write back the new pose and ground state.
        tf.translation = position;
        mv.grounded    = hit_ground;

        // 7. Kill vertical velocity on floor/ceiling contact so it doesn't
        //    accumulate. Horizontal velocity follows the slide projection.
        vel.0 = projected_velocity;
        if hit_ground && vel.y < 0.0 { vel.y = 0.0; }
        if hit_ceiling && vel.y > 0.0 { vel.y = 0.0; }
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct PlayerControllerPlugin;

impl Plugin for PlayerControllerPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins(EnhancedInputPlugin)
        .add_input_context::<PlayerInput>()

        .add_systems(Update, spawn_player.run_if(run_once))
        .add_systems(Update, player_look_sys.run_if(in_state(GameUpdateState::Running)))
        .add_systems(FixedUpdate, step.run_if(in_state(GameUpdateState::Running)));
    }
}