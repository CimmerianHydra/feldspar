use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::player::input::{Jump, Move};
use crate::sim::player::Player;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TUNABLES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const PLAYER_HEIGHT: f32 = 1.9;
pub const PLAYER_RADIUS: f32 = 0.4;
pub const CYL_HEIGHT:    f32 = PLAYER_HEIGHT;
pub const EYE_HEIGHT:    f32 = 1.75;
pub const CAM_LOCAL_Y:   f32 = EYE_HEIGHT - PLAYER_HEIGHT * 0.5;

const MOVE_SPEED:    f32 = 4.3;
const JUMP_SPEED:    f32 = 8.0;
const GRAVITY_ACCEL: f32 = 15.0;

/// Max walkable slope: slightly steeper than 45°.
const GROUND_DOTPROD_LIMIT:  f32 = 0.51;
/// How far below the feet we probe.
const GROUND_PROBE_DISTANCE: f32 = 0.10;
/// Shrink the probe shape relative to the body.
const GROUND_PROBE_SHRINK:   f32 = 0.10;

const COYOTE_TIME: f32 = 0.1;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – STATE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component, Default)]
pub struct PlayerMovementData {
    wish_dir:            Vec2,
    jump_queued:         bool,
    state:               PlayerMovementState,
    time_since_grounded: f32,
    time_since_jump_q:   f32,
}

#[derive(Default, PartialEq)]
enum PlayerMovementState {
    #[default]
    Grounded,
    Airborne,
}

/// The physical part of a player body.
pub fn build_player_body() -> impl Bundle {
    (
        PlayerMovementData::default(),
        RigidBody::Kinematic,
        Collider::cylinder(PLAYER_RADIUS, CYL_HEIGHT),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
        Friction::new(0.0),
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – INTENT
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE PHYSICS STEP
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn step(
    spatial: SpatialQuery,
    move_and_slide: MoveAndSlide,
    time: Res<Time<Physics>>,
    mut players: Query<
        (Entity, &Collider, &mut Transform, &mut LinearVelocity, &mut PlayerMovementData),
        With<Player>,
    >,
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
        }

        // Handle jumping.
        let can_jump = mv.time_since_grounded < COYOTE_TIME;
        if can_jump && mv.jump_queued {
            vel.y = JUMP_SPEED;
            mv.state = PlayerMovementState::Airborne;
        }
        mv.jump_queued = false;

        // Move-and-slide. We don't rely on its callback for ground state.
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
    spatial:  &SpatialQuery,
    entity:   Entity,
    position: Vec3,
    rotation: Quat,
) -> PlayerMovementState {
    // Shrink the probe so it doesn't catch on walls we're sliding against.
    let probe = Collider::cylinder(PLAYER_RADIUS - GROUND_PROBE_SHRINK, CYL_HEIGHT);

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
        Some(h) if h.normal1.dot(Vec3::Y) > GROUND_DOTPROD_LIMIT => PlayerMovementState::Grounded,
        _ => PlayerMovementState::Airborne,
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct PlayerMovementPlugin;

impl Plugin for PlayerMovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, step)
            .add_observer(on_move_fire)
            .add_observer(on_move_complete)
            .add_observer(on_jump_start);
    }
}
