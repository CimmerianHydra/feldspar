#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> horizon_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> top_color: vec4<f32>;


@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {

    let dir = normalize(in.world_position);

    let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
    let t2 = pow(t, 1.5);

    let color = mix(
        horizon_color.rgb,
        top_color.rgb,
        t2
    );

    return vec4(color, 1.0);
}