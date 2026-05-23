use bevy::prelude::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BLOCK TEXTURES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug)]
pub enum FaceTextures {
    /// First index is into base texture array.
    Simple(u32),
    /// Has both texture and (non-tinted) overlay.
    Bilayer(u32, u32),
    /// First index is into base texture array, second into overlay array.
    /// Color is used to tint the overlay texture.
    Tinted(u32, u32, Color),
}

pub enum BlockAppearance {

    /// All six faces use the same texture. Default choice.
    Uniform(FaceTextures),
    /// Top/bottom differ from sides.
    TopBotSide {
        up:    FaceTextures,
        down:  FaceTextures,
        side:  FaceTextures,
    },
    PerFace {
        up:    FaceTextures,
        down:  FaceTextures,
        north: FaceTextures,
        south: FaceTextures,
        east:  FaceTextures,
        west:  FaceTextures,
    },
    /// All six faces use the same texture, but an "internal" texture is defined
    /// for all those faces that don't sit on the boundary of the voxel.
    UniformWithInternal {
        ext:    FaceTextures,
        int:    FaceTextures,
    }
}

impl Default for BlockAppearance {
    fn default() -> Self {
        BlockAppearance::Uniform(FaceTextures::Simple(1))
    }
}