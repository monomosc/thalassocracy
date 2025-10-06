// Screen-space water post-process: "water wobble"
#import bevy_render::view::View




@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

@group(1) @binding(0) var<uniform> view_uniform: View;
@group(1) @binding(1) var depth_tex: texture_depth_2d;

struct GlobalsUniform {
    time: f32,
    delta_time: f32,
    frame_count: u32,
}
@group(3) @binding(0) var<uniform> globals: GlobalsUniform;

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>
};

struct WobbleParams {
    base_amp: f32,   // base amplitude in UV units (e.g. 0.001–0.005)
    freq: f32,       // frequency of noise
    speed: f32,      // scroll speed
    near: f32,
    far: f32,
};


fn linearize_depth(depth: f32) -> f32 {
    // Avoid divide-by-zero for background pixels
    return view_uniform.clip_from_view[3][2] / max(depth, 1e-6);
}


// Bevy supplies a fullscreen vertex shader; we only implement fragment.
fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let x = fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
    let y = fract(sin(dot(p, vec2<f32>(269.5, 183.3))) * 43758.5453);
    return vec2<f32>(x, y);
}
fn hash3(p: vec2<f32>) -> vec2<f32> {
    // credit: Dave Hoskins / IQ style
    let p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    let p4 = p3 + vec3<f32>(dot(p3, p3.yzx + 33.33));
    return vec2<f32>(fract((p4.x + p4.y) * p4.z));
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash3(i);
    let b = hash3(i + vec2<f32>(1.0, 0.0));
    let c = hash3(i + vec2<f32>(0.0, 1.0));
    let d = hash3(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    let mixed = mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
    return mixed.x;
}

fn wobble_params() -> WobbleParams {
    return WobbleParams(
        0.01,  // base_amp  (~0.001–0.005)
        6.0,    // freq      (cycles across screen)
        0.8,   // speed     (Hz-ish)
        0.1, //near
        100.0,   // far plane (adjust to match your camera)
    );
}
// --------------------------------------
// Core wobble function
// Takes original uv, linear depth, and time
// Returns perturbed uv
// --------------------------------------
fn apply_wobble(uv: vec2<f32>, linear_depth: f32, time: f32) -> vec2<f32> {

    let WOBBLE = wobble_params();

    let dims = vec2<f32>(textureDimensions(depth_tex));
    let texel = 1.0 / dims;
    let d_left = textureLoad(depth_tex, vec2<i32>((uv - vec2(texel.x, 0.0)) * dims), 0);
    let d_right = textureLoad(depth_tex, vec2<i32>((uv + vec2(texel.x, 0.0)) * dims), 0);
    let d_up = textureLoad(depth_tex, vec2<i32>((uv - vec2(0.0, texel.y)) * dims), 0);
    let d_down = textureLoad(depth_tex, vec2<i32>((uv + vec2(0.0, texel.y)) * dims), 0);
    let grad = abs(d_left - d_right) + abs(d_up - d_down);
    let edge_factor = clamp(1.0 - grad * 80.0, 0.0, 1.0); // tune 
    let amp = WOBBLE.base_amp * (linear_depth / 100.0) * edge_factor;


    // animated noise field
    let n = noise(uv * WOBBLE.freq + vec2<f32>(time * WOBBLE.speed, 0.0));

    // offset vector (could vary orientation per-channel for chromatic shimmer)
    let offset = vec2<f32>(n, n * 0.7) * amp;

    return uv + offset;
}

@fragment
fn fragment(@location(0) uv_in: vec2<f32>) -> @location(0) vec4<f32> {
    let depth_dims_u = textureDimensions(depth_tex);
    let depth_dims = vec2<f32>(depth_dims_u);
    let depth_sample = textureLoad(depth_tex, vec2<i32>(uv_in * depth_dims), 0);
    let linear_depth = linearize_depth(depth_sample);

    // Wobble
    let uv_wobbled = apply_wobble(uv_in, linear_depth, globals.time);

    // Depth-aware guard to avoid sampling past foreground silhouettes
    let wobble_coords = clamp(
        uv_wobbled * depth_dims,
        vec2<f32>(0.0),
        max(depth_dims - vec2<f32>(1.0), vec2<f32>(0.0)),
    );
    let depth_wobbled = textureLoad(depth_tex, vec2<i32>(wobble_coords), 0);
    let depth_gap = max(depth_sample - depth_wobbled - 0.002, 0.0);
    let wobble_penalty = clamp(depth_gap * 200.0, 0.0, 1.0);
    let sample_uv = mix(uv_wobbled, uv_in, vec2<f32>(wobble_penalty));

    let color_dims_u = textureDimensions(src_tex, 0);
    let color_dims = vec2<f32>(color_dims_u);
    let texel = 1.0 / color_dims;

    let base = textureSample(src_tex, src_samp, sample_uv);
    let dist_n = clamp(1.0 - depth_sample, 0.0, 1.0);

    // Refraction offset via animated flow noise
    let n = noise(sample_uv * 24.0 + vec2<f32>(globals.time * 0.15, -globals.time * 0.12)) * 2.0 - 1.0;
    let n2 = noise(sample_uv * 12.0 + vec2<f32>(-globals.time * 0.1, globals.time * 0.17)) * 2.0 - 1.0;
    let refraction_strength = 0.75 * dist_n;
    let refraction_offset = vec2<f32>(n, n2) * texel * 4.0 * refraction_strength;
    let refraction_uv_raw = clamp(sample_uv + refraction_offset, vec2<f32>(0.0), vec2<f32>(1.0));

    // Guard refraction sample against crossing depth discontinuities
    let refraction_coords = clamp(
        refraction_uv_raw * depth_dims,
        vec2<f32>(0.0),
        max(depth_dims - vec2<f32>(1.0), vec2<f32>(0.0)),
    );
    let depth_refraction = textureLoad(depth_tex, vec2<i32>(refraction_coords), 0);
    let refraction_gap = max(depth_sample - depth_refraction - 0.002, 0.0);
    let refraction_penalty = clamp(refraction_gap * 200.0, 0.0, 1.0);
    let refraction_uv = mix(refraction_uv_raw, sample_uv, vec2<f32>(refraction_penalty));
    let refr = textureSample(src_tex, src_samp, refraction_uv);

    // Diffusion blur
    let lum = luminance(base.rgb);
    var diff = vec3<f32>(0.0);
    let radius = texel * (1.0 + 6.0 * dist_n);
    for (var i = -2; i <= 2; i = i + 1) {
        let w = 1.0 - abs(f32(i)) / 3.0;
        let offset_x = clamp(sample_uv + vec2<f32>(radius.x * f32(i), 0.0), vec2<f32>(0.0), vec2<f32>(1.0));
        let offset_y = clamp(sample_uv + vec2<f32>(0.0, radius.y * f32(i)), vec2<f32>(0.0), vec2<f32>(1.0));
        diff += textureSample(src_tex, src_samp, offset_x).rgb * w;
        diff += textureSample(src_tex, src_samp, offset_y).rgb * w;
    }
    diff /= (2.0 * 5.0);

    var col = mix(base.rgb, refr.rgb, vec3<f32>(0.35 * dist_n));
    col = mix(col, diff, vec3<f32>(0.15 * dist_n * (0.3 + lum)));

    return vec4<f32>(col, base.a);
}


