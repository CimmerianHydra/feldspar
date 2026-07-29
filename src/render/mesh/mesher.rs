use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, MeshVertexAttribute, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::VertexFormat;

use crate::content::block::{BlockRegistry};
use crate::content::block::definition::{RenderClass, TextureSlot, RENDER_CLASS_COUNT};
use crate::render::material::VoxelMaterials;
use crate::render::mesh::element::BakedQuad;
use crate::render::mesh::model::ModelArena;
use crate::space::{ChunkSlot, VoxelChunk};
use crate::voxel::{BlockID, Direction, CHUNK_SIZE};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – VERTEX FORMAT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const ATTRIBUTE_TEXTURE_LAYER: MeshVertexAttribute =
    MeshVertexAttribute::new("Texture_Layer", 1000, VertexFormat::Uint32);

pub const ATTRIBUTE_OVERLAY_LAYER: MeshVertexAttribute =
    MeshVertexAttribute::new("Overlay_Layer", 1001, VertexFormat::Uint32);

pub const ATTRIBUTE_OVERLAY_TINT: MeshVertexAttribute =
    MeshVertexAttribute::new("Overlay_Tint", 1002, VertexFormat::Float32x4);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – SYSTEMS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marks a chunk's per-class mesh child.
#[derive(Component)]
pub struct ChunkSurface(pub RenderClass);

/// One mesh under construction per render class.
///
/// The chunk builder pushes quads in and does not care how many meshes come
/// out the other end, which keeps the bucketing out of the culling loop.
#[derive(Default)]
pub struct SurfaceMeshes {
    builders: [MeshBuilder; RENDER_CLASS_COUNT],
}

impl SurfaceMeshes {
    #[inline]
    pub fn push(&mut self, quad: &BakedQuad, slot: &TextureSlot, origin: Vec3) {
        self.builders[slot.class.index()].push(quad, slot, origin);
    }

    /// One mesh per non-empty class, indexed by `RenderClass::index()`.
    pub fn finish(self) -> [Option<Mesh>; RENDER_CLASS_COUNT] {
        self.builders.map(|b| (!b.is_empty()).then(|| b.finish()))
    }
}

#[derive(Default)]
struct MeshBuilder {
    positions:      Vec<[f32; 3]>,
    normals:        Vec<[f32; 3]>,
    uvs:            Vec<[f32; 2]>,
    texture_layers: Vec<u32>,
    overlay_layers: Vec<u32>,
    overlay_tints:  Vec<[f32; 4]>,
    indices:        Vec<u32>,
}

impl MeshBuilder {
    fn push(&mut self, quad: &BakedQuad, slot: &TextureSlot, origin: Vec3) {
        let base = self.positions.len() as u32;

        for (i, &vert) in quad.verts.iter().enumerate() {
            self.positions.push((vert + origin).to_array());
            self.normals.push(quad.normal.to_array());
            self.uvs.push(quad.uvs[i].to_array());
            self.texture_layers.push(slot.base_layer);
            self.overlay_layers.push(slot.overlay_layer);
            self.overlay_tints.push(slot.tint);
        }

        // Degenerate quads (slope side triangles, verts[3] == verts[2]) still
        // emit both; the second collapses to zero area and the GPU discards
        // it, which is cheaper than branching here.
        self.indices.extend_from_slice(&[
            base, base + 1, base + 2,
            base, base + 2, base + 3,
        ]);
    }

    fn is_empty(&self) -> bool { self.indices.is_empty() }

    fn finish(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(ATTRIBUTE_TEXTURE_LAYER, self.texture_layers);
        mesh.insert_attribute(ATTRIBUTE_OVERLAY_LAYER, self.overlay_layers);
        mesh.insert_attribute(ATTRIBUTE_OVERLAY_TINT, self.overlay_tints);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}


/// Rebuild the render mesh of every chunk whose voxels changed.
///
/// `Changed<VoxelChunk>` rather than a `NeedsRemeshing` marker: Bevy already
/// tracks this, `VoxelWorldMut::set_voxel` only writes through `DerefMut`
/// when the value actually differs, and — the part that matters — `space`
/// no longer has to know that a renderer exists in order to tell it
/// something happened.
pub fn update_dirty_mesh_sys(
    mut commands: Commands,
    mut meshes:   ResMut<Assets<Mesh>>,
    materials:    Res<VoxelMaterials>,
    registry:     Res<BlockRegistry>,
    arena:        Res<ModelArena>,
    chunks:       Query<(Entity, &VoxelChunk, &ChunkSlot, Option<&Children>), Changed<VoxelChunk>>,
    existing:     Query<&ChunkSurface>,
) {
    for (chunk_entity, chunk, slot, children) in &chunks {
        // This is here literally only to write it in the chunk entity's name.
        let cx = slot.coord.x;
        let cy = slot.coord.y;
        let cz = slot.coord.z;

        let mut built = build_chunk_surfaces(chunk, &registry, &arena);

        // Reuse the child that already owns each class rather than churning
        // entities on every remesh — a chunk being rebuilt is the common case,
        // a chunk changing which classes it contains is rare.
        let mut owner: [Option<Entity>; RENDER_CLASS_COUNT] = [None; RENDER_CLASS_COUNT];
        for &child in children.into_iter().flatten() {
            if let Ok(surface) = existing.get(child) {
                owner[surface.0.index()] = Some(child);
            }
        }

        for class in RenderClass::ALL {
            match (built[class.index()].take(), owner[class.index()]) {
                (Some(mesh), Some(entity)) => {
                    commands.entity(entity).insert(Mesh3d(meshes.add(mesh)));
                }
                (Some(mesh), None) => {
                    let idx = class.index();
                    commands.entity(chunk_entity).with_child((
                        Name::new(format!("ChunkSurface [{idx}] ({cx}, {cy}, {cz})")),
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(materials.get(class)),
                        ChunkSurface(class),
                        Visibility::Visible,
                    ));
                }
                // This class emptied out — a wall of glass got broken.
                (None, Some(entity)) => commands.entity(entity).despawn(),
                (None, None) => {}
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – MESHING CORE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Local-space neighbour of `pos` across `face`, or `None` at a chunk edge.
/// Cross-chunk visibility is still the standing TODO.
fn neighbor_pos(pos: UVec3, face: Direction) -> Option<UVec3> {
    let np = IVec3::new(pos.x as i32, pos.y as i32, pos.z as i32) + face.as_ivec3();

    if np.x < 0 || np.y < 0 || np.z < 0
        || np.x >= CHUNK_SIZE as i32
        || np.y >= CHUNK_SIZE as i32
        || np.z >= CHUNK_SIZE as i32
    {
        return None;
    }
    Some(np.as_uvec3())
}

/// Whether a quad should be emitted, given what sits across its cull face.
///
/// Occlusion is not a hand-written `covers_face` match: it is a property the
/// model baker derived by rasterising each boundary face, read here as
/// `model.occludes(dir)`. That is what makes this correct for slabs, slopes,
/// pipes and imported meshes alike, with no shape-specific code in the
/// mesher.
///
/// A quad with an interior cull tag is always drawn. A quad on a boundary
/// is dropped only when the neighbour's *resolved model* fully covers the
/// facing side.
fn is_visible(
    quad: &BakedQuad,
    chunk: &VoxelChunk,
    pos: UVec3,
    registry: &BlockRegistry,
    arena: &ModelArena,
) -> bool {
    let Some(dir) = quad.cull.dir() else {
        return true; // interior face
    };

    match neighbor_pos(pos, dir) {
        None => true, // chunk boundary — render for now (see TODO above)
        Some(npos) => {
            let neighbor = chunk.get_local(npos);
            if neighbor.is_air() {
                return true;
            }
            let n_def = registry.get(BlockID(neighbor.id()));
            let n_model = arena.model(n_def.models.resolve(neighbor));
            !n_model.occludes(dir.opposite())
        }
    }
}

/// Build the render mesh for one chunk.
///
/// The hot loop is: resolve a `ModelID` from the voxel, borrow that model's
/// baked quads, and for each visible quad look its texture up by slot index.
/// There is no per-voxel allocation and no per-face texture `match` — that
/// was precomputed into `def.texture_slots` at load.
fn build_chunk_surfaces(chunk: &VoxelChunk, registry: &BlockRegistry, arena: &ModelArena) -> [Option<Mesh>; 2] {
    let mut surfaces = SurfaceMeshes::default();

    for (pos, voxel) in chunk.iter_non_air() {
        let def = registry.get(BlockID(voxel.id()));
        let model_id = def.models.resolve(voxel);
        let origin = pos.as_vec3();



        for quad in arena.quads(model_id) {
            if !is_visible(quad, chunk, pos, registry, arena) {
                continue;
            }

            // One array index replaces the two texture `match`es the old
            // mesher ran per face. `slot` was validated against the model's
            // slot count at bake time, so this can't be out of range.
            let slot: &TextureSlot = &def.texture_slots[quad.slot as usize];

            // A bit more of an elegant way of writing the large, repetitive attributes
            // See the details of [MeshBuilder::push] if you want to know more
            surfaces.push(quad, slot, origin);
        }
    }

    surfaces.finish()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – ICON MESHES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single model's mesh, in the chunk mesher's vertex format so it renders
/// through the voxel material unchanged.
///
/// No neighbour culling — an icon has no neighbours; the GPU backface-culls
/// interior faces. Verts stay in `[0,1]³`; the icon camera looks at the cube
/// centre.
pub fn build_single_model_mesh(
    voxel: crate::voxel::Voxel,
    registry: &BlockRegistry,
    arena: &ModelArena,
) -> Mesh {
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

        let slot: &TextureSlot = &def.texture_slots[quad.slot as usize];
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
