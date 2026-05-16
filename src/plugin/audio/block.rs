use bevy::audio::*;

const AUDIO_PITCH_VARIANCE: f32 = 1.0;

pub struct SoundProfile {
    on_place:   AudioSource,
    on_break:   AudioSource,
    on_walk:    AudioSource,
}