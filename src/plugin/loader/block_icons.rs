//! Isometric block icons, rendered from baked models at load time.
//!
//! Each block gets a small offscreen render of its model, viewed in true
//! isometric, used as its item icon. Because the icon goes through the
//! *same* `VoxelMaterial` as the world, it is pixel-identical to the block
//! in-game — overlays and tints included — and it works for any model the
//! arena can produce, including future Blockbench imports.
//!
//! ## How the timing works (the part worth understanding)
//!
//! A render-target `Image`'s **handle is valid the moment the asset is
//! created**; only its pixels arrive later, once the baking camera has
//! rendered a frame. So we hand every item a valid handle immediately
//! (during loading), and the picture resolves a frame or two afterwards.
//! No CPU readback, no waiting on the bake before items can be built.
//!
//! ## How the bake works
//!
//! Each frame the holder's mesh is swapped to the next block and the camera
//! is pointed at that block's target, rendering exactly one icon per frame.
//! A single visible model at a time means the camera can't see the others,
//! so there's no per-model render layer bookkeeping. It's a one-shot loading
//! cost, so sequential is fine.

use std::collections::HashMap;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};

// ── Version-sensitive imports ────────────────────────────────────────────
// These paths moved around in the Bevy module reshuffle. If one fails to
// resolve, it has almost certainly just relocated — check the prelude or
// the `bevy::camera` / `bevy::render::camera` modules for your version.
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::camera::ScalingMode;
use bevy::camera::RenderTarget;

use crate::plugin::geometry::model::ModelArena;
use crate::plugin::geometry::meshing::{
    ATTRIBUTE_OVERLAY_LAYER, ATTRIBUTE_OVERLAY_TINT, ATTRIBUTE_TEXTURE_LAYER,
};
use crate::plugin::geometry::prelude::ConnectionMask;
use crate::plugin::loader::block_registry::{BlockID, BlockRegistry};
use crate::plugin::loader::texture_assets::VoxelMaterialHandle;
use crate::plugin::geometry::voxel::{BlockShape, Direction, Voxel};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONFIG
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Target icon resolution. 128 downsamples cleanly to the 64px UI slot and,
/// with nearest sampling, keeps pixel-art textures crisp. Bump to 256 for
/// retina-density UIs.
const ICON_RESOLUTION: u32 = 128;

/// Orthographic frustum height, in world units, and fixed for every icon.
///
/// This is the "always frame the full block" decision made concrete: the
/// camera is sized to the unit cube (its isometric silhouette spans a bit
/// over 1.6 units tall) plus a margin, and it never adapts to the model.
/// So a slab renders half-height in the lower portion of its icon — the
/// size cue you asked for — rather than being zoomed to fill.
const ICON_FRUSTUM_HEIGHT: f32 = 1.85;

/// Dedicated render layer for the bake scene. The main camera lives on
/// layer 0 and never sees these entities; this camera sees only them.
const ICON_LAYER: usize = 31;

// PBR light magnitudes are physically based and need visual calibration —
// treat these as starting points and tune against your textures.
const ICON_KEY_LUX: f32 = 6_000.0;
const ICON_AMBIENT: f32 = 1_200.0;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PUBLIC RESOURCES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Rendered icon per block. Populated during loading; handles are valid
/// immediately, pixels arrive within a couple of frames.
#[derive(Resource, Default)]
pub struct BlockIcons(pub HashMap<BlockID, Handle<Image>>);

impl BlockIcons {
    pub fn get(&self, id: BlockID) -> Option<Handle<Image>> {
        self.0.get(&id).cloned()
    }
}

/// Flips to `true` once every icon has been rendered and the bake scene
/// torn down. Gate a loading→in-game transition on this if you want zero
/// icon pop-in; otherwise icons simply resolve a frame or two after the
/// HUD appears, which is harmless since the handles are already bound.
#[derive(Resource, Default)]
pub struct BlockIconsReady(pub bool);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BlockIconPlugin;

impl Plugin for BlockIconPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BlockIcons>()
            .init_resource::<BlockIconsReady>()
            // The bake driver only exists while there's baking to do, so
            // this run condition confines it to the loading window with no
            // state dependency.
            .add_systems(
                Update,
                drive_block_icon_baking_sys.run_if(resource_exists::<IconBaker>),
            );

        // NOTE: `setup_block_icon_baking_sys` is deliberately NOT added
        // here — it must be ordered against systems this plugin can't see.
        // Add it to your loading schedule with:
        //
        //     setup_block_icon_baking_sys
        //         .after(assemble_texture_arrays_sys)   // needs the material
        //         .after(bake_block_geometry)           // needs the arena
        //
        // and make `populate_item_registry_from_blocks_sys` run
        // `.after(setup_block_icon_baking_sys)` so the icon handles exist
        // when items are built. See the accompanying notes.
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INTERNAL BAKE STATE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Drives the sequential bake. Present only while baking is in progress;
/// removed at teardown, which also disables the driver via its run
/// condition.
#[derive(Resource)]
struct IconBaker {
    /// One (target image, model mesh) pair per block, in dispatch order.
    queue: Vec<(Handle<Image>, Handle<Mesh>)>,
    next: usize,
    camera: Entity,
    holder: Entity,
    light: Entity,
}

#[derive(Component)]
struct IconCamera;

#[derive(Component)]
struct IconModelHolder;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VARIANT SELECTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The representative voxel an icon renders for a given block.
///
/// Standard shapes use identity rotation and default state — the canonical
/// look. Pipes get North↔South connections so the icon shows a straight
/// segment through the block rather than a lone, cube-like core.
///
/// This is the single place to add engine-native variant icons later: a
/// rotated log, a specific machine orientation, and so on.
fn icon_voxel(shape: &BlockShape, id: BlockID) -> Voxel {
    match shape {
        BlockShape::Pipe => {
            let mask = ConnectionMask::NONE
                .with(Direction::North, true)
                .with(Direction::South, true);
            Voxel::full(id.0).with_state(mask.0 as u16)
        }
        _ => Voxel::full(id.0),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ICON MESH
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Build the mesh for one block's icon.
///
/// This is `build_chunk_mesh` for a single voxel with the neighbour
/// culling removed — an icon has no neighbours, so every quad is emitted
/// and the GPU backface-culls the interior ones. Verts stay in the model's
/// native `[0,1]³`; the camera looks at the cube's centre, so no offset is
/// applied.
fn build_icon_mesh(voxel: Voxel, registry: &BlockRegistry, arena: &ModelArena) -> Mesh {
    let def = registry.get(BlockID(voxel.id()));
    let model_id = def.models.resolve(voxel);

    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut indices = Vec::<u32>::new();
    let mut texture_layers = Vec::<u32>::new();
    let mut overlay_layers = Vec::<u32>::new();
    let mut overlay_tints = Vec::<[f32; 4]>::new();
    let mut index_offset = 0u32;

    for quad in arena.quads(model_id) {
        let slot = &def.texture_slots[quad.slot as usize];
        for (i, &vert) in quad.verts.iter().enumerate() {
            positions.push(vert.to_array());
            normals.push(quad.normal.to_array());
            uvs.push(quad.uvs[i].to_array());
            texture_layers.push(slot.base_layer);
            overlay_layers.push(slot.overlay_layer);
            overlay_tints.push(slot.tint);
        }
        indices.extend_from_slice(&[
            index_offset,     index_offset + 1, index_offset + 2,
            index_offset,     index_offset + 2, index_offset + 3,
        ]);
        index_offset += 4;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(ATTRIBUTE_TEXTURE_LAYER, texture_layers);
    mesh.insert_attribute(ATTRIBUTE_OVERLAY_LAYER, overlay_layers);
    mesh.insert_attribute(ATTRIBUTE_OVERLAY_TINT, overlay_tints);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// A fresh offscreen render target. Transparent-cleared, nearest-sampled,
/// usable both as a render attachment (the camera writes it) and a texture
/// (the UI samples it).
fn new_icon_target() -> Image {
    let size = Extent3d {
        width: ICON_RESOLUTION,
        height: ICON_RESOLUTION,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::RENDER_ATTACHMENT;
    image.sampler = ImageSampler::nearest();
    image
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SETUP  (add to your loading schedule — see plugin note)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Allocate a target and mesh per block, publish the handles, and spawn the
/// isometric bake scene. Must run after the material and arena exist and
/// before item registration reads `BlockIcons`.
pub fn setup_block_icon_baking_sys(
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    arena: Res<ModelArena>,
    material: Res<VoxelMaterialHandle>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ready: ResMut<BlockIconsReady>,
) {
    let mut icons = BlockIcons::default();
    let mut queue = Vec::new();

    // Skip id 0 (air).
    for id in 1..registry.size() {
        let block_id = BlockID(id as u16);
        let def = registry.get(block_id);

        let voxel = icon_voxel(&def.shape, block_id);
        let mesh = meshes.add(build_icon_mesh(voxel, &registry, &arena));
        let target = images.add(new_icon_target());

        icons.0.insert(block_id, target.clone());
        queue.push((target, mesh));
    }

    // Nothing to render (only air registered). Publish empties and report
    // done so a gated transition doesn't stall.
    if queue.is_empty() {
        commands.insert_resource(icons);
        ready.0 = true;
        info!("Block icons: nothing to bake.");
        return;
    }

    // ---- the isometric bake scene ---------------------------------------
    //
    // Everything below lives on ICON_LAYER, invisible to the main camera.
    // This block is the version-sensitive part; the surrounding logic is
    // stable.

    let layer = RenderLayers::layer(ICON_LAYER);
    let center = Vec3::splat(0.5);

    // Model holder: an empty mesh to start; the driver swaps in each block
    // before the camera is activated, so this empty is never rendered.
    let holder = commands
        .spawn((
            IconModelHolder,
            Mesh3d(Handle::default()),
            MeshMaterial3d(material.0.clone()),
            Transform::default(),
            Visibility::Visible,
            layer.clone(),
        ))
        .id();

    // Key light, aimed from upper-front-left. Under true isometric the
    // three visible faces are symmetric, so the light must be asymmetric
    // for top / left / right to read as distinct shades.
    let light = commands
        .spawn((
            DirectionalLight {
                illuminance: ICON_KEY_LUX,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(Vec3::new(-1.0, 2.5, 1.5)).looking_at(center, Vec3::Y),
            layer.clone(),
        ))
        .id();

    // True isometric: view along (1,1,1), looking at the cube centre, with
    // world-up so the cube's vertical edges stay vertical in the image.
    // Orthographic and fixed-size — see ICON_FRUSTUM_HEIGHT.
    let dir = Vec3::ONE.normalize();
    let camera = commands
        .spawn((
            IconCamera,
            Camera3d::default(),
            Camera {
                // Off until the driver points it at the first target, so no
                // stray render lands on the main window.
                is_active: false,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(queue[0].0.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: ICON_FRUSTUM_HEIGHT,
                    height: ICON_FRUSTUM_HEIGHT,
                },
                ..OrthographicProjection::default_3d()
            }),
            // Per-view ambient fill. If your Bevy version doesn't accept
            // AmbientLight as a camera component, delete this line — the
            // global ambient will apply instead.
            AmbientLight {
                brightness: ICON_AMBIENT,
                ..default()
            },
            Transform::from_translation(center + dir * 10.0).looking_at(center, Vec3::Y),
            layer.clone(),
        ))
        .id();

    let count = queue.len();
    commands.insert_resource(icons);
    commands.insert_resource(IconBaker {
        queue,
        next: 0,
        camera,
        holder,
        light,
    });

    info!("Block icons: baking {count} icons.");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DRIVER  (added by the plugin)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One icon per frame: point the camera at the next target, swap the model
/// in. The previous frame's target is guaranteed rendered by now, so when
/// the queue drains we tear the scene down and report ready.
fn drive_block_icon_baking_sys(
    mut commands: Commands,
    mut baker: ResMut<IconBaker>,
    mut cameras: Query<(&mut Camera, &mut RenderTarget), With<IconCamera>>,
    mut holders: Query<&mut Mesh3d, With<IconModelHolder>>,
    mut ready: ResMut<BlockIconsReady>,
) {
    if baker.next >= baker.queue.len() {
        commands.entity(baker.camera).despawn();
        commands.entity(baker.holder).despawn();
        commands.entity(baker.light).despawn();
        commands.remove_resource::<IconBaker>();
        ready.0 = true;
        info!("Block icons: bake complete.");
        return;
    }

    let (target, mesh) = baker.queue[baker.next].clone();

    if let Ok((mut camera, mut render_target)) = cameras.single_mut() {
        *render_target = RenderTarget::Image(target.into());
        camera.is_active = true;
    }
    if let Ok(mut holder_mesh) = holders.single_mut() {
        holder_mesh.0 = mesh;
    }

    baker.next += 1;
}