//! Block sounds.
//!
//! Sits alongside [`crate::render`] in the presentation layer: it observes
//! `BlockEvent` and plays whatever the block definition nominated. Nothing
//! below it knows this module exists, which is why the simulation runs
//! identically with the plugin removed.

use bevy::audio::*;
use bevy::prelude::*;

use crate::content::block::BlockRegistry;
use crate::space::{BlockEvent, VoxelWorld};

/// Half-width of the random pitch interval, in playback-speed units.
pub const AUDIO_PITCH_HALFRANGE: f32 = 0.16;

fn play_block_sound_obs(
    event: On<BlockEvent>,
    mut commands: Commands,
    block_registry: Res<BlockRegistry>,
    voxel_world: VoxelWorld,
) {
    let (at, sound) = match *event {
        BlockEvent::Place { block_id, at, .. } =>
            (at, block_registry.get(block_id).sound_profile.on_place.as_ref()),
        BlockEvent::Break { block_id, at, .. } =>
            (at, block_registry.get(block_id).sound_profile.on_break.as_ref()),
        _ => return,
    };

    let Some(handle) = sound else { return };
    let Some(pos) = voxel_world.world_position(at) else { return };

    let pitch = 1.0 + (rand::random::<f32>() * 2.0 - 1.0) * AUDIO_PITCH_HALFRANGE;

    commands.spawn((
        AudioPlayer(handle.clone()),
        PlaybackSettings::DESPAWN.with_speed(pitch).with_spatial(true),
        Transform::from_translation(pos),
    ));
}

pub struct BlockAudioPlugin;

impl Plugin for BlockAudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalVolume::new(Volume::Linear(10.0)))
            .add_observer(play_block_sound_obs);
    }
}
