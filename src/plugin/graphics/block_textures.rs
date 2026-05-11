use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::*;

use crate::plugin::graphics::block_material::VoxelBaseMaterial;

pub fn create_texture_array(
    paths: &[&str],
    images: &mut Assets<Image>,
) -> Handle<Image> {

    assert!(
        !paths.is_empty(),
        "Texture array requires at least one texture"
    );

    // ========================================================
    // LOAD ALL IMAGES
    // ========================================================

    let mut loaded_images = Vec::new();

    for path in paths {
        let dyn_img = image::open(path)
            .unwrap_or_else(|e| {
                panic!("Failed to load image '{}': {}", path, e)
            });

        let rgba = dyn_img.to_rgba8();

        loaded_images.push(rgba);
    }

    // ========================================================
    // VALIDATE DIMENSIONS
    // ========================================================

    let width = loaded_images[0].width();
    let height = loaded_images[0].height();

    for img in &loaded_images {
        assert_eq!(
            img.width(),
            width,
            "All textures in texture array must have same width."
        );

        assert_eq!(
            img.height(),
            height,
            "All textures in texture array must have same height."
        );
    }

    // ========================================================
    // CONCATENATE PIXEL DATA
    // ========================================================

    let mut combined_data = Vec::new();

    for img in &loaded_images {
        combined_data.extend_from_slice(img.as_raw());
    }

    // ========================================================
    // CREATE ARRAY TEXTURE
    // ========================================================

    let layer_count = loaded_images.len() as u32;

    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: layer_count,
        },
        TextureDimension::D2,
        combined_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );

    // ========================================================
    // IMPORTANT GPU SETTINGS
    // ========================================================

    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST;

    // Pixel-art friendly
    image.sampler = bevy::image::ImageSampler::nearest();

    images.add(image)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BLOCK TEXTURES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const NO_OVERLAY: u32 = 0;

#[derive(Clone, Debug)]
pub enum FaceTextures {
    /// First index is into base texture array, second into overlay array.
    Default(u32, u32),
    /// First index is into base texture array, second into overlay array.
    /// Color is used to tint the overlay texture.
    Tinted(u32, u32, Color),
}

pub enum BlockAppearance {

    /// All six faces use the same layer pair. Default choice.
    Uniform(FaceTextures),
    /// Top/bottom differ from sides.  Interior faces (slab inner wall, stair
    /// riser) are treated as sides since they face no chunk boundary.
    TopBottomSides {
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
}

impl Default for BlockAppearance {
    fn default() -> Self {
        BlockAppearance::Uniform(FaceTextures::Default(1, NO_OVERLAY))
    }
}