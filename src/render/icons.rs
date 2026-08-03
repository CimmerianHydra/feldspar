//! Isometric block icons, generated on demand from baked models.
//!
//! ## Requires synchronous pipeline compilation
//!
//! This depends on the app being built with
//!
//! ```ignore
//! DefaultPlugins.set(RenderPlugin { synchronous_pipeline_compilation: true, ..default() })
//! ```
//!
//! Without it, wgpu compiles the icon pipeline asynchronously and **skips
//! the draw every frame until it's ready** — the target clears but no
//! geometry lands. With it, the first render blocks until the pipeline is
//! compiled, then every icon after reuses the cached pipeline and renders
//! in a single frame.
//!
//! ## Shape
//!
//! `BlockIcons` is a registry. `ensure()` returns a valid handle
//! immediately and queues the render. One **persistent** studio — camera,
//! model holder, light on an isolated render layer — drains the queue, one
//! icon per frame, repointing the camera at each target in turn.
//!
//! ## Why block→item registration lives here
//!
//! A block item's `ItemDisplay` *is* a rendered icon, so the system that
//! turns every block into a placeable item needs the model arena and the
//! icon studio. Rather than have `content` reach up into `render` for them,
//! the registration pass lives here and writes down into the item registry.

use std::collections::VecDeque;
use std::time::Duration;

use iyes_progress::Progress;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::mesh::Mesh;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

// Camera types live in `bevy_camera` in this Bevy version. If a path fails
// to resolve it has just moved — check `bevy::camera` and its submodules.
// `Msaa` is re-exported by the prelude; if not, it's `bevy::camera::Msaa`.
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, OrthographicProjection, Projection, RenderTarget, ScalingMode};

use crate::content::block::BlockRegistry;
use crate::content::item::{ItemBehaviors, ItemDefinition, ItemDisplay, ItemKind, ItemRegistry, MAX_STACK};
use crate::sim::behaviors::items::places_block::PlacesBlock;
use crate::render::material::VoxelMaterials;
use crate::render::mesh::mesher::build_single_model_mesh;
use crate::render::mesh::model::ModelArena;
use crate::voxel::{BlockID, BlockShape, ConnectionMask, Direction, ModelID, Voxel};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONFIG
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const ICON_RESOLUTION: u32 = 128;

/// Orthographic frustum height, fixed for every icon so relative block
/// sizes read true. Raise slightly if blocks feel tight against the edges.
const ICON_FRUSTUM: f32 = 1.85;

/// Isolated render layer. The main camera sees layer 0 and never these
/// entities; the icon camera sees only them.
const ICON_LAYER: usize = 31;

/// Placeholder for a not-yet-rendered icon — opaque, so a pending icon is a
/// visible grey square (a diagnostic). A successful render clears to
/// transparent and overwrites it.
const PLACEHOLDER: [u8; 4] = [70, 70, 70, 255];

// PBR light magnitudes need visual calibration. Direction is set in
// `spawn_icon_studio_sys`.
const ICON_KEY_LUX: f32 = 4_000.0;
const ICON_AMBIENT: f32 = 250.0;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What determines an icon: the block (texture slots) and its resolved
/// model (geometry). Voxels differing only in non-geometric state share one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct IconKey {
    block: BlockID,
    model: ModelID,
}

struct PendingIcon {
    target: Handle<Image>,
    mesh: Handle<Mesh>,
}

/// Cache of generated icons plus the queue of renders still owed. Persists
/// across sessions; entries are only ever added.
#[derive(Resource, Default)]
pub struct BlockIcons {
    cache: bevy::platform::collections::HashMap<IconKey, Handle<Image>>,
    pending: VecDeque<PendingIcon>,
}

impl BlockIcons {
    /// The icon handle for a voxel, generating it if absent. Returns
    /// immediately with a valid handle; if new, the render is queued.
    pub fn ensure(
        &mut self,
        voxel: Voxel,
        registry: &BlockRegistry,
        arena: &ModelArena,
        images: &mut Assets<Image>,
        meshes: &mut Assets<Mesh>,
    ) -> Handle<Image> {
        let block = BlockID(voxel.id());
        let model = registry.get(block).models.resolve(voxel);
        let key = IconKey { block, model };

        if let Some(handle) = self.cache.get(&key) {
            return handle.clone();
        }

        let target = images.add(new_icon_target());
        let mesh = meshes.add(build_single_model_mesh(voxel, registry, arena));

        self.cache.insert(key, target.clone());
        self.pending.push_back(PendingIcon {
            target: target.clone(),
            mesh,
        });
        target
    }

    /// How many renders are still owed. The loader holds the splash screen
    /// until this reaches zero, so the hotbar's first frame shows real
    /// icons rather than the grey placeholder.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// STUDIO
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marker that the studio exists. Its presence stops the spawn system from
/// firing again and would gate a despawn if you add one.
#[derive(Resource)]
struct IconStudio;

#[derive(Component)]
struct IconCamera;

#[derive(Component)]
struct IconModelHolder;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VARIANT SELECTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Representative voxel per block: identity rotation and default state for
/// standard shapes; a North↔South pipe segment so pipes aren't a lone core
/// cube. The single place to add engine-native variant icons later.
pub fn icon_voxel(shape: &BlockShape, id: BlockID) -> Voxel {
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
// BLOCK → ITEM
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Give every registered block a placeable item whose icon is a render of
/// that block.
///
/// TODO: add a way to inject additional item components from the block
/// definition.
pub fn register_block_items_sys(
    block_registry: Res<BlockRegistry>,
    mut item_registry: ResMut<ItemRegistry>,
    mut block_icons: ResMut<BlockIcons>,
    arena: Res<ModelArena>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // id 0 is air — no block, no icon, no item.
    for id in 1..block_registry.size() {
        let block_id = BlockID(id as u16);
        let block = block_registry.get(block_id);

        let voxel = icon_voxel(&block.shape, block_id);
        let icon = block_icons.ensure(voxel, &block_registry, &arena, &mut images, &mut meshes);

        item_registry.register(ItemDefinition {
            name: block.name.clone(),
            display_name: block.display_name.clone(),
            max_stack: MAX_STACK,
            kind: ItemKind::Block { block_id },
            display: ItemDisplay::Image { image: icon },
            behaviors: ItemBehaviors::default()
                .with(PlacesBlock::resolved(block.name.clone(), block_id)),
        });
    }

    info_once!(
        "ItemRegistry populated from blocks: {} entries.",
        item_registry.len()
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ASSET CONSTRUCTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A render target pre-filled with the placeholder colour, usable as both a
/// render attachment and a sampled texture.
fn new_icon_target() -> Image {
    let size = Extent3d {
        width: ICON_RESOLUTION,
        height: ICON_RESOLUTION,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &PLACEHOLDER,
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
// STUDIO SPAWN + WARM-UP
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Spawn the studio the first frame its dependencies exist, and seed the
/// warm-up. Runs every frame but does nothing until the arena is present,
/// then fires exactly once.
///
/// The gate is `ModelArena`, which only appears after the voxel material has
/// been built (both happen in the load chain in `app::loading`, material
/// first). `VoxelMaterialHandle` is `init_resource`'d empty at startup, so
/// its mere presence proves nothing — the arena is the honest signal.
///
/// The warm-up: the holder starts with a real block mesh and the camera
/// points at a scratch target, so the very next rendered frame — behind the
/// loading screen — forces the (synchronous) pipeline compile. After that
/// the processor's real renders each take one frame.
fn spawn_icon_studio_sys(
    mut commands: Commands,
    material: Res<VoxelMaterials>,
    arena: Option<Res<ModelArena>>,
    registry: Option<Res<BlockRegistry>>,
    studio: Option<Res<IconStudio>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
) {
    if studio.is_some() {
        return; // already spawned
    }
    let (Some(arena), Some(registry)) = (arena, registry) else {
        return; // dependencies not ready yet
    };

    let layer = RenderLayers::layer(ICON_LAYER);
    let center = Vec3::splat(0.5);
    let dir = Vec3::ONE.normalize();

    // Warm-up mesh: any real block. Its render compiles the icon pipeline.
    let warmup_mesh = if registry.size() > 1 {
        meshes.add(build_single_model_mesh(Voxel::full(1), &registry, &arena))
    } else {
        Handle::default()
    };

    commands.spawn((
        IconModelHolder,
        Mesh3d(warmup_mesh),
        MeshMaterial3d(material.mask.clone()),
        Transform::default(),
        Visibility::Visible,
        layer.clone(),
    ));

    // Key light from upper-front-left, off the view axis so the three
    // visible faces read distinctly under true isometric.
    commands.spawn((
        DirectionalLight {
            illuminance: ICON_KEY_LUX,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(Vec3::new(-1.0, 2.5, 1.5)).looking_at(center, Vec3::Y),
        layer.clone(),
    ));

    // The camera starts aimed at a scratch target purely to warm the
    // pipeline; the processor repoints it at real icon targets from the next
    // frame on. `Msaa::Off` so it writes the single-sample image directly.
    let scratch = images.add(new_icon_target());
    commands.spawn((
        IconCamera,
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        Msaa::Off,
        RenderTarget::Image(scratch.into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: ICON_FRUSTUM,
                height: ICON_FRUSTUM,
            },
            ..OrthographicProjection::default_3d()
        }),
        AmbientLight {
            brightness: ICON_AMBIENT,
            ..default()
        },
        Transform::from_translation(center + dir * 10.0).looking_at(center, Vec3::Y),
        layer,
    ));

    commands.insert_resource(IconStudio);
    info!("Icon studio spawned; warming pipeline behind the loading screen.");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PROCESSOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One icon per frame: repoint the persistent camera at the next queued
/// target and swap in its mesh. The previous frame's target has finished
/// rendering, so its pixels are already in place — nothing to track. When
/// the queue empties, deactivate the camera so it stops consuming a pass;
/// it reactivates the moment a new request (startup or runtime) arrives.
///
/// Self-gates on the studio: until the camera and holder exist, the queries
/// come up empty and it does nothing.
fn process_icon_requests_sys(
    mut icons: ResMut<BlockIcons>,
    mut cameras: Query<(&mut Camera, &mut RenderTarget), With<IconCamera>>,
    mut holders: Query<&mut Mesh3d, With<IconModelHolder>>,
) {
    let Ok((mut camera, mut target)) = cameras.single_mut() else { return };
    let Ok(mut holder) = holders.single_mut() else { return };

    match icons.pending.pop_front() {
        Some(job) => {
            *target = RenderTarget::Image(job.target.into());
            holder.0 = job.mesh;
            if !camera.is_active {
                camera.is_active = true;
            }
        }
        None => {
            if camera.is_active {
                camera.is_active = false;
            }
        }
    }
}

/// Hold the load until the icon queue drains. One icon per rendered frame,
/// so this is a step measured in frames, not in work — which is precisely
/// why it has to report partial progress rather than block.
///
/// **Requires `synchronous_pipeline_compilation`** (see the module docs).
/// Without it the icon pipeline is never ready, no target is ever written,
/// and this would wait forever — hence the bail-out.
pub fn await_block_icons_sys(
    icons: Res<BlockIcons>,
    time: Res<Time>,
    mut queued: Local<Option<u32>>,
    mut waited: Local<Duration>,
) -> Progress {
    // First call: whatever is in the queue right now is the denominator.
    let total = *queued.get_or_insert(icons.pending() as u32);
    let done = total.saturating_sub(icons.pending() as u32);

    *waited += time.delta();
    if *waited > ICON_DRAIN_TIMEOUT && done < total {
        warn_once!(
            "Icon queue still has {} entries after {:?} — continuing without them. \
             Is `synchronous_pipeline_compilation` enabled?",
            icons.pending(),
            ICON_DRAIN_TIMEOUT,
        );
        return Progress { done: total, total };
    }

    Progress { done, total }
}

/// Generous: a few hundred icons at one per frame is a second or two.
const ICON_DRAIN_TIMEOUT: Duration = Duration::from_secs(20);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BlockIconPlugin;

impl Plugin for BlockIconPlugin {
    fn build(&self, app: &mut App) {
        // No state gating: both systems run from app start and self-gate on
        // their dependencies, so generation begins the moment the material
        // and arena appear during loading and continues into the game.
        app.init_resource::<BlockIcons>()
            .add_systems(Update, (spawn_icon_studio_sys, process_icon_requests_sys).chain());
    }
}
