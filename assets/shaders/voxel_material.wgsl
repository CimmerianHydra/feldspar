// Extended-material shader. Adds one new per-vertex attribute
// (`texture_layer`) which is used in the fragment stage to pick a layer of a
// 2D-array texture. Everything else routes through the usual `StandardMaterial`
// PBR machinery so lighting, tone-mapping, etc. behave exactly as they do for
// a regular StandardMaterial.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_functions::{get_world_from_local, mesh_normal_local_to_world},
    forward_io::{VertexOutput, FragmentOutput},
    view_transformations::position_world_to_clip,
}

// Extension bindings. These live in the material bind group, starting at
// slot 100 (slots 0–99 are owned by the StandardMaterial base).
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var array_texture: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var array_sampler: sampler;

// Vertex inputs. The shader_location numbers must match the ones we hand to
// the pipeline in `VoxelMaterialExtension::specialize` on the Rust side.
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(8) texture_layer: u32,
};

// Varyings passed to the fragment stage. We forward everything the standard
// PBR fragment shader will need, plus our custom `texture_layer`.
struct VoxelVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) instance_index: u32,
    // `flat` is required: u32 cannot be linearly interpolated, and for a
    // voxel mesh all vertices of a face share a single texture layer anyway.
    @location(4) @interpolate(flat) texture_layer: u32,
};

@vertex
fn vertex(in: Vertex) -> VoxelVertexOutput {
    var out: VoxelVertexOutput;

    let world_from_local = get_world_from_local(in.instance_index);
    out.world_position = world_from_local * vec4<f32>(in.position, 1.0);
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_normal_local_to_world(in.normal, in.instance_index);
    out.uv = in.uv;
    out.instance_index = in.instance_index;
    out.texture_layer = in.texture_layer;

    return out;
}

@fragment
fn fragment(
    in: VoxelVertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Build a standard `VertexOutput` from our custom one, so we can hand it
    // to the StandardMaterial helpers. We only need to populate the fields
    // the helpers actually read; gated fields (tangents, vertex colors, etc.)
    // are inactive because their `#ifdef`s are not set for this mesh.
    var std_in: VertexOutput;
    std_in.position = in.position;
    std_in.world_position = in.world_position;
    std_in.world_normal = in.world_normal;
    std_in.uv = in.uv;
    std_in.instance_index = in.instance_index;

    // Let StandardMaterial gather everything (metallic, roughness, emissive,
    // …) from its own bindings.
    var pbr_input = pbr_input_from_standard_material(std_in, is_front);

    // Override the base color with a sample from our array texture, picking
    // the layer specified by the vertex attribute.
    let sampled = textureSample(array_texture, array_sampler, in.uv, in.texture_layer);
    pbr_input.material.base_color = sampled;

    // Standard alpha-discard, lighting, and post-lighting (tone mapping, fog,
    // …) — identical to what StandardMaterial does for an ordinary mesh.
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
