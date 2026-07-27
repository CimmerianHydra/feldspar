use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::platform::collections::hash_map::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy_asset_loader::prelude::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Stable name → layer-index lookup for the two texture arrays.
/// Layer 0 of base is `missing`, layer 0 of overlay is `none`.
#[derive(Resource, Default)]
pub struct TextureRegistry {
    pub base:    HashMap<String, u32>,
    pub overlay: HashMap<String, u32>,
}

impl TextureRegistry {
    pub const MISSING_LAYER:    u32 = 0;
    pub const NO_OVERLAY_LAYER: u32 = 0;

    /// Returns the layer or warns + returns MISSING_LAYER.
    pub fn base_layer(&self, name: &str) -> u32 {
        match self.base.get(name) {
            Some(&l) => l,
            None => {
                warn!("Unknown base texture '{}' — falling back to 'missing'", name);
                Self::MISSING_LAYER
            }
        }
    }

    pub fn overlay_layer(&self, name: &str) -> u32 {
        match self.overlay.get(name) {
            Some(&l) => l,
            None => {
                warn!("Unknown overlay texture '{}' — falling back to 'none'", name);
                Self::NO_OVERLAY_LAYER
            }
        }
    }

    pub fn new() -> Self {
        TextureRegistry { base: HashMap::new(), overlay: HashMap::new() }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – ASSET COLLECTIONS
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

#[derive(AssetCollection, Resource)]
pub struct UITextureAssets {
    #[asset(path = "textures/ui", collection(typed, mapped))]
    pub handles: HashMap<String, Handle<Image>>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – ASSEMBLY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The two assembled array textures.
///
/// Content stops here: turning these into a `VoxelMaterial` is
/// [`crate::render::material`]'s job, because a material is a rendering
/// concern and content must not depend on the renderer.
#[derive(Resource, Clone)]
pub struct TextureArrays {
    pub base:    Handle<Image>,
    pub overlay: Handle<Image>,
}

pub fn assemble_texture_arrays_sys(
    mut commands:    Commands,
    block_textures:  Res<BlockTextureAssets>,
    mut images:      ResMut<Assets<Image>>,
    mut registry:    ResMut<TextureRegistry>,
) {
    let (base_handle, base_map) = build_array(
        &block_textures.base, &mut images, "missing",
    );
    registry.base = base_map;

    info!(
        "Base texture array populated from image assets: {} layers.",
        registry.base.len()
    );

    let (overlay_handle, overlay_map) = build_array(
        &block_textures.overlay, &mut images, "none",
    );
    registry.overlay = overlay_map;

    info!(
        "Overlay texture array populated from image assets: {} layers.",
        registry.overlay.len()
    );

    commands.insert_resource(TextureArrays {
        base:    base_handle,
        overlay: overlay_handle,
    });
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

        data.extend_from_slice(img.data.as_ref().expect("image has no CPU data"));
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
