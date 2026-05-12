// Extended-material shader. The mesh carries three custom per-vertex
// attributes:
//   * texture_layer  (u32)      → which layer of `array_texture` to sample
//   * overlay_layer  (u32)      → which layer of `array_overlay` to sample
//   * overlay_tint   (vec4<f32>) → colour the overlay sample is multiplied by
//
// Fragment stage: sample base + overlay, tint overlay, "over"-composite onto
// base, hand to the standard PBR pipeline so lighting / tone-mapping behave
// exactly like a regular StandardMaterial.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_functions::{get_world_from_local, mesh_normal_local_to_world},
    forward_io::{VertexOutput, FragmentOutput},
    view_transformations::position_world_to_clip,
}

// Extension bindings live in the material bind group at slots 100+
// (slots 0–99 belong to the StandardMaterial base).
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var array_texture: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var array_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var array_overlay: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var array_overlay_sampler: sampler;

// Vertex inputs — the `@location` numbers must match the ones we hand to the
// pipeline in `VoxelMaterialExtension::specialize` on the Rust side.
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(8) texture_layer: u32,
    @location(9) overlay_layer: u32,
    @location(10) overlay_tint: vec4<f32>,
};

// Varyings into the fragment stage. The `u32` layer attributes are
// `flat`-interpolated because integers cannot be interpolated linearly; the
// tint is smooth-interpolated, which is harmless when adjacent vertices
// share a tint and useful if they don't.
struct VoxelVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) instance_index: u32,
    @location(4) @interpolate(flat) texture_layer: u32,
    @location(5) @interpolate(flat) overlay_layer: u32,
    @location(6) overlay_tint: vec4<f32>,
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
    out.overlay_layer = in.overlay_layer;
    out.overlay_tint = in.overlay_tint;

    return out;
}

@fragment
fn fragment(
    in: VoxelVertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Build a standard `VertexOutput` from our custom one so we can call into
    // the StandardMaterial helpers.
    var std_in: VertexOutput;
    std_in.position = in.position;
    std_in.world_position = in.world_position;
    std_in.world_normal = in.world_normal;
    std_in.uv = in.uv;
    std_in.instance_index = in.instance_index;

    // Resolve StandardMaterial properties (metallic, roughness, emissive, …).
    var pbr_input = pbr_input_from_standard_material(std_in, is_front);

    // Sample base, sample overlay, tint, "over"-composite. The base is
    // treated as opaque underneath, so the final alpha follows the base.
    // Overlay layer 0 is reserved as "no overlay": we force its alpha to 0
    // here so the convention holds even if a future overlay texture has
    // non-transparent data at layer 0.
    let base = textureSample(array_texture, array_sampler, in.uv, in.texture_layer);
    let overlay_raw = textureSample(array_overlay, array_overlay_sampler, in.uv, in.overlay_layer);
    let overlay = overlay_raw * in.overlay_tint;
    let overlay_alpha = select(overlay.a, 0.0, in.overlay_layer == 0u);
    let composited_rgb = mix(base.rgb, overlay.rgb, overlay_alpha);
    pbr_input.material.base_color = vec4<f32>(composited_rgb, base.a);

    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
