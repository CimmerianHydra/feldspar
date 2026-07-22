use bevy::audio::*;
use bevy::prelude::*;

use crate::plugin::block::interaction::BlockEvent;
use crate::plugin::loader::block_registry::BlockRegistry;
use crate::plugin::space::prelude::VoxelWorld;

/// Half-width of the random pitch interval, in playback-speed units.
pub const AUDIO_PITCH_HALFRANGE: f32 = 0.16;

/// Per-block sound bundle. `None` means "this block makes no sound
/// for this action" — air uses the default and is silent everywhere.
#[derive(Clone, Default)]
pub struct SoundProfile {
    pub on_place: Option<Handle<AudioSource>>,
    pub on_break: Option<Handle<AudioSource>>,
}


pub struct BlockAudioPlugin;

impl Plugin for BlockAudioPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(GlobalVolume::new(Volume::Linear(10.0)))

            .add_observer(play_block_sound_obs)
        ;
    }
}



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
        BlockEvent::Interact { .. } => return,
    };

    let Some(handle) = sound else { return; };
    let Some(pos) = voxel_world.world_position(at) else { return; };

    let pitch = 1.0 + (rand::random::<f32>() * 2.0 - 1.0) * AUDIO_PITCH_HALFRANGE;

    commands.spawn((
        AudioPlayer(handle.clone()),
        PlaybackSettings::DESPAWN.with_speed(pitch).with_spatial(true),
        Transform::from_translation(pos),
    ));
}