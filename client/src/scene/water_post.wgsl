// Re-export or copy of assets/water_post.wgsl so load_internal_asset! can inline it.
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(2) @binding(0) var<uniform> params: vec4<f32>; // x=strength, y=debugFlag, z,w unused

fn luminance(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)); }

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let x = fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
    let y = fract(sin(dot(p, vec2<f32>(269.5, 183.3))) * 43758.5453);
    return vec2<f32>(x, y);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a.x, b.x, u.x), mix(c.x, d.x, u.x), u.y);
}

@fragment
fn fragment(@location(0) uv_in: vec2<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(src_tex, 0));
    let texel = 1.0 / dims;
    var depth_val: f32 = textureLoad(depth_tex, vec2<u32>(uv_in * dims), 0);
    // Reverse-Z: emphasize mid/far distances for visibility
    let dist_raw = clamp(1.0 - depth_val, 0.0, 1.0);
    let dist_n = smoothstep(0.05, 0.7, dist_raw);
    let base = textureSample(src_tex, src_samp, uv_in);
    let t = 0.0;
    let n = noise(uv_in * 24.0 + vec2<f32>(t * 0.15, -t * 0.12)) * 2.0 - 1.0;
    let n2 = noise(uv_in * 12.0 + vec2<f32>(-t * 0.1, t * 0.17)) * 2.0 - 1.0;
    let refr_strength = 1.6 * dist_n * params.x;
    let offset = vec2<f32>(n, n2) * texel * 6.0 * refr_strength;
    let refr = textureSample(src_tex, src_samp, clamp(uv_in + offset, vec2<f32>(0.0), vec2<f32>(1.0)));
    let lum = luminance(base.rgb);
    var diff = vec3<f32>(0.0);
    let radius = texel * (1.0 + 10.0 * dist_n * params.x);
    for (var i = -2; i <= 2; i = i + 1) {
        let w = 1.0 - abs(f32(i)) / 3.0;
        diff += textureSample(src_tex, src_samp, uv_in + vec2<f32>(radius.x * f32(i), 0.0)).rgb * w;
        diff += textureSample(src_tex, src_samp, uv_in + vec2<f32>(0.0, radius.y * f32(i))).rgb * w;
    }
    diff /= (2.0 * 5.0);
    let water = vec3<f32>(0.06, 0.3, 0.38);
    let absorption = clamp(dist_n * 1.8 * params.x, 0.0, 1.0);
    var col = mix(base.rgb, refr.rgb, 0.65 * dist_n * params.x);
    col = mix(col, diff, 0.35 * dist_n * (0.3 + lum) * params.x);
    col = mix(col, water, 0.45 * absorption);
    if (params.y > 0.5) {
        // Debug: apply a faint blue overlay proportional to strength to visualize the pass
        col = mix(col, water, 0.15 * params.x);
    }
    return vec4<f32>(col, 1.0);
}
