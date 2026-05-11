#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip
}
#import bevy_core_pipeline::tonemapping::tone_mapping

struct VertexInput {
    @builtin(instance_index) instance_index: u32,

    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(8) base_layer: u32,
    @location(9) overlay_layer: u32,
    @location(10) overlay_tint: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,

    @location(0) uv: vec2<f32>,

    @location(1) @interpolate(flat) base_layer:    u32,
    @location(2) @interpolate(flat) overlay_layer: u32,
    @location(3) overlay_tint: vec4<f32>,
};

@group(2) @binding(0)
var base_texture_array: texture_2d_array<f32>;

@group(2) @binding(1)
var base_sampler: sampler;

@group(2) @binding(2)
var overlay_texture_array: texture_2d_array<f32>;

@group(2) @binding(3)
var overlay_sampler: sampler;

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let model = mesh_functions::get_world_from_local(in.instance_index);

    let world_position = mesh_functions::mesh_position_local_to_world(
        model,
        vec4<f32>(in.position, 1.0),
    );

    out.clip_position = position_world_to_clip(world_position.xyz);
    out.uv            = in.uv;
    out.base_layer    = in.base_layer;
    out.overlay_layer = in.overlay_layer;
    out.overlay_tint  = in.overlay_tint;

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {

    let base_color = textureSample(
        base_texture_array,
        base_sampler,
        in.uv,
        i32(in.base_layer),
    );

    if (in.overlay_layer == 0u) {
        return base_color;
    }

    let overlay_color = textureSample(
        overlay_texture_array,
        overlay_sampler,
        in.uv,
        i32(in.overlay_layer - 1u),
    );

    let tinted_overlay =
        overlay_color * in.overlay_tint;

    let final_color = mix(
        base_color,
        tinted_overlay,
        overlay_color.a,
    );

    return final_color;
}