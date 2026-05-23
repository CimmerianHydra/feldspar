use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::image::ImageSampler;
use bevy::platform::collections::hash_map::HashMap;
use bevy_asset_loader::prelude::*;

use crate::plugin::graphics::block_material::{VoxelMaterial, VoxelMaterialExtension};

use crate::plugin::loader::texture_registry::*;


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ASSET COLLECTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// All PNGs under each folder are auto-loaded and indexed by asset path.
/// Drop a new file in, restart, it's available by stem name — no code change.
#[derive(AssetCollection, Resource)]
pub struct BlockTextureAssets {
    #[asset(path = "textures/blocks/base", collection(typed, mapped))]
    pub base: HashMap<String, Handle<Image>>,

    #[asset(path = "textures/blocks/overlay", collection(typed, mapped))]
    pub overlay: HashMap<String, Handle<Image>>,
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VOXEL MATERIAL HANDLE  (so worldgen can grab it without rebuilding)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Resource)]
pub struct VoxelMaterialHandle(pub Handle<VoxelMaterial>);

impl Default for VoxelMaterialHandle {
    fn default() -> Self {
        VoxelMaterialHandle(Handle::default())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ASSEMBLY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn assemble_texture_arrays_sys(
    mut commands:    Commands,
    block_textures:  Res<BlockTextureAssets>,
    mut images:      ResMut<Assets<Image>>,
    mut vox_mat:     ResMut<Assets<VoxelMaterial>>,
    mut registry:    ResMut<TextureRegistry>,
) {
    let (base_handle, base_map) = build_array(
        &block_textures.base, &mut images, "missing",
    );
    registry.base = base_map;

    let (overlay_handle, overlay_map) = build_array(
        &block_textures.overlay, &mut images, "none",
    );
    registry.overlay = overlay_map;

    bevy::log::info!(
        "Texture arrays built — base: {} layers, overlay: {} layers",
        registry.base.len(), registry.overlay.len()
    );

    // Build the material once and stash the handle for everyone else.
    let mat = vox_mat.add(VoxelMaterial {
        base: StandardMaterial {
            base_color:           Color::WHITE,
            metallic:             0.0,
            perceptual_roughness: 0.8,
            ..default()
        },
        extension: VoxelMaterialExtension {
            array_texture: base_handle,
            array_overlay: overlay_handle,
        },
    });
    commands.insert_resource(VoxelMaterialHandle(mat));
}

/// Builds one array texture from a mapped collection of single-layer images.
/// `reserved` MUST exist in the input — it gets layer 0.
/// Returns (handle to the array image, name→layer map).
fn build_array(
    input:    &HashMap<String, Handle<Image>>,
    images:   &mut Assets<Image>,
    reserved: &str,
) -> (Handle<Image>, HashMap<String, u32>) {
    // Pull file-stem out of an asset path: "textures/blocks/base/dirt.png" → "dirt"
    fn stem(path: &str) -> &str {
        let name = path.rsplit('/').next().unwrap_or(path);
        name.split('.').next().unwrap_or(name)
    }

    // Stable order: reserved first, then alphabetical by stem.
    let mut entries: Vec<(&str, &Handle<Image>)> = input.iter()
        .map(|(p, h)| (stem(p), h))
        .collect();
    entries.sort_by_key(|(s, _)| *s);
    if let Some(i) = entries.iter().position(|(s, _)| *s == reserved) {
        entries.swap(0, i);
    } else {
        panic!("Reserved texture '{}' missing from collection", reserved);
    }

    // Pull pixel data; validate dimensions/format on the way.
    let first = images.get(entries[0].1).expect("reserved texture not loaded");
    let (w, h) = (first.width(), first.height());
    let fmt    = first.texture_descriptor.format;
    let bytes_per_layer = (w * h * 4) as usize;

    let mut data = Vec::with_capacity(bytes_per_layer * entries.len());
    let mut name_map = HashMap::new();

    for (layer, (name, handle)) in entries.iter().enumerate() {
        let img = images.get(*handle)
            .unwrap_or_else(|| panic!("Texture '{}' didn't load", name));
        assert_eq!(img.width(), w,  "texture '{}' has wrong width",  name);
        assert_eq!(img.height(), h, "texture '{}' has wrong height", name);
        assert_eq!(img.texture_descriptor.format, fmt, "texture '{}' wrong format", name);

        data.extend_from_slice(&img.data.as_ref().expect("image has no CPU data"));
        name_map.insert(name.to_string(), layer as u32);
    }

    let mut array_img = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: entries.len() as u32 },
        TextureDimension::D2,
        data,
        fmt,
        RenderAssetUsages::default(),
    );
    array_img.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    array_img.sampler = ImageSampler::nearest();

    (images.add(array_img), name_map)
}