#import bevy_pbr::{
    mesh_view_bindings::{globals, view},
    forward_io::VertexOutput,
}

@group(2) @binding(0) var<uniform> material_color: vec4<f32>;
@group(2) @binding(1) var<uniform> material_params: vec4<f32>; // x=intensity, y=edge_soft, z=along_pow

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let v = clamp(in.uv.y, 0.0, 1.0);
    let along = pow(1.0 - v, max(material_params.z, 0.001));

    let theta = in.uv.x * 6.28318530718;
    let r = max(0.0, 1.0 - v);
    let local_pos = vec3<f32>(r * cos(theta), r * sin(theta), v);
    let dist_from_axis = length(local_pos.xy);
    let edge_soft = material_params.y;
    let edge = 1.0 - smoothstep(0.85 - edge_soft, 1.0, dist_from_axis);

    let base = material_color.rgb;
    let intensity = material_color.a * material_params.x * along * edge;
    let out_rgb = base * intensity;
    return vec4<f32>(out_rgb, 1.0);
}

