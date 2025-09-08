// Screen-space water post-process: absorption, diffusion, refraction.

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

// If present, we sample depth to modulate effect strength.
@group(1) @binding(0) var depth_tex: texture_depth_2d;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

// Bevy supplies a fullscreen vertex shader; we only implement fragment.

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

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

    // Sample depth if available
    var depth_val: f32 = 0.5; // default mid
    // depth texture optional path: guard using textureDimensions on src only
    // If pipeline didn't bind depth, WGSL validation would fail, so assume it's bound.
    depth_val = textureLoad(depth_tex, vec2<u32>(uv_in * dims), 0);

    // Reverse-Z in Bevy: far=0, near~1. Map to [0..1] distance proxy where 0=near,1=far
    let dist_n = clamp(1.0 - depth_val, 0.0, 1.0);

    // Base color
    let base = textureSample(src_tex, src_samp, uv_in);

    // Refraction offset via simple flow noise
    let t = 0.0; // could be animated via push constants in future
    let n = noise(uv_in * 24.0 + vec2<f32>(t * 0.15, -t * 0.12)) * 2.0 - 1.0;
    let n2 = noise(uv_in * 12.0 + vec2<f32>(-t * 0.1, t * 0.17)) * 2.0 - 1.0;
    let refr_strength = 0.75 * dist_n;
    let offset = vec2<f32>(n, n2) * texel * 4.0 * refr_strength;
    let refr = textureSample(src_tex, src_samp, clamp(uv_in + offset, vec2<f32>(0.0), vec2<f32>(1.0)));

    // Diffusion: small cross blur biased by distance and brightness
    let lum = luminance(base.rgb);
    var diff = vec3<f32>(0.0);
    let taps = 3.0 + dist_n * 5.0;
    let radius = texel * (1.0 + 6.0 * dist_n);
    for (var i = -2; i <= 2; i = i + 1) {
        let w = 1.0 - abs(f32(i)) / 3.0;
        diff += textureSample(src_tex, src_samp, uv_in + vec2<f32>(radius.x * f32(i), 0.0)).rgb * w;
        diff += textureSample(src_tex, src_samp, uv_in + vec2<f32>(0.0, radius.y * f32(i))).rgb * w;
    }
    diff /= (2.0 * 5.0);

    // Water tint and absorption
    let water = vec3<f32>(0.06, 0.3, 0.38);
    let absorption = clamp(dist_n * 1.4, 0.0, 1.0);

    var col = mix(base.rgb, refr.rgb, 0.35 * dist_n);
    col = mix(col, diff, 0.15 * dist_n * (0.3 + lum));
    col = mix(col, water, 0.25 * absorption);

    return vec4<f32>(col, 1.0);
}
