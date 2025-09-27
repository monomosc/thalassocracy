struct ViewUniform {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_position: vec4<f32>,
    screen_size: vec4<f32>,
    params: vec4<f32>,
};

struct ConeUniform {
    model: mat4x4<f32>,
    apex: vec4<f32>,                 // world apex
    direction_range: vec4<f32>,      // xyz: axis, w: range
    color_intensity: vec4<f32>,      // rgb color, a intensity
    angles: vec4<f32>                // x: cos_inner, y: cos_outer
};

struct GpuFog {
    /// Fog color
    base_color: vec4<f32>,
    /// The color used for the fog where the view direction aligns with directional lights
    directional_light_color: vec4<f32>,
    /// Allocated differently depending on fog mode.
    /// See `mesh_view_types.wgsl` for a detailed explanation
    be: vec3<f32>,
    /// The exponent applied to the directional light alignment calculation
    directional_light_exponent: f32,
    /// Allocated differently depending on fog mode.
    /// See `mesh_view_types.wgsl` for a detailed explanation
    bi: vec3<f32>,
    /// Unsigned int representation of the active fog falloff mode
    mode: u32,
}


// Derived cone parameters used throughout the shader.
struct ConeParams {
    apex: vec3<f32>,   // world-space apex of the cone
    axis: vec3<f32>,   // unit-length axis pointing down the cone
    range: f32,        // finite length of the cone (distance from apex to cap)

    cos_inner: f32,    // cosine of the inner half-angle (full intensity inside)
    cos_outer: f32,    // cosine of the outer cutoff half-angle
    tan_outer: f32,    // tangent of the outer half-angle (for radial falloff)
};

@group(0) @binding(0) var shadow_atlas: texture_depth_2d_array;
@group(0) @binding(1) var shadow_sampler: sampler_comparison;

@group(1) @binding(0) var<uniform> view_uniform: ViewUniform;
@group(1) @binding(1) var view_depth: texture_depth_2d;

@group(2) @binding(0) var<uniform> cone_uniform: ConeUniform;

@group(3) @binding(0) var<uniform> fog: GpuFog;

struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) ndc_xy: vec2<f32>,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};

// Convert an NDC coordinate back to world space using the precomputed inverse VP.
fn world_from_ndc(ndc: vec3<f32>) -> vec3<f32> {
    let clip = vec4<f32>(ndc, 1.0);
    let world = view_uniform.inv_view_proj * clip;
    return world.xyz / world.w;
}

// reduces light based on fog
fn fog_transmittance_from_light(d: f32, fog: GpuFog) -> f32 {
    // Returns T_light in [0..1], where 1 = no attenuation, 0 = fully attenuated.
    switch fog.mode {
        case 0u: { // linear: bi.x=start, be.x=end
            let start = fog.bi.x;
            let end   = fog.be.x;
            let denom = max(end - start, 1e-6);
            let t = clamp((d - start) / denom, 0.0, 1.0);  // fog amount
            return 1.0 - t; // transmittance = 1 - fog amount
        }
        case 1u: { // exponential: be.x = density
            let density = fog.be.x;
            return exp(-density * d);
        }
        case 2u: { // exponential^2: be.x = density
            let density = fog.be.x;
            let x = density * d;
            return exp(-(x * x));
        }
        default: { return 1.0; }
    }
}

// Fetch the minimum depth value around the current pixel (2x2 footprint).
// This gives us a conservative clamp when comparing against main-scene geometry.
fn sample_depth_min(frag_coord: vec2<f32>, screen_size: vec2<f32>) -> f32 {
    let size_i = vec2<i32>(screen_size);
    let base = frag_coord - vec2<f32>(0.5, 0.5);
    let base_i = vec2<i32>(floor(base));
    let offsets = array<vec2<i32>, 4>(
        vec2<i32>(0, 0),
        vec2<i32>(1, 0),
        vec2<i32>(0, 1),
        vec2<i32>(1, 1),
    );
    var min_depth = 1.0;
    for (var i: u32 = 0u; i < 4u; i = i + 1u) {
        let coord = clamp(base_i + offsets[i], vec2<i32>(0, 0), size_i - vec2<i32>(1, 1));
        min_depth = min(min_depth, textureLoad(view_depth, coord, 0));
    }
    return min_depth;
}

// Build a `ConeParams` struct from the raw uniform, guaranteeing sensible values.
fn cone_params_from_uniform(u: ConeUniform) -> ConeParams {
    let apex = u.apex.xyz;
    let axis = normalize(u.direction_range.xyz);
    let range = max(u.direction_range.w, 0.001);

    var cos_inner = clamp(u.angles.x, 0.0, 1.0);
    var cos_outer = clamp(u.angles.y, 0.0, cos_inner);

    let eps = 1e-8;
    var tan_outer: f32;
    if (cos_outer > 1.0 - eps) {
        tan_outer = 0.0;
    } else {
        let sin_outer = sqrt(max(1.0 - cos_outer * cos_outer, 0.0));
        tan_outer = sin_outer / max(cos_outer, eps); // ≥ 0
    }

    return ConeParams(apex, axis, range, cos_inner, cos_outer, tan_outer);
}

// True if `point` lies within the finite cone volume (including the cap).
fn is_point_inside_cone(apex: vec3<f32>, axis: vec3<f32>, cos_outer: f32, range: f32, point: vec3<f32>) -> bool {
    let rel = point - apex;
    let dist = length(rel);
    let eps = 1e-4;

    if dist <= eps { return true; }

    let axial = dot(rel, axis);
    if axial < -eps { return false; }
    if axial > range + eps { return false; }

    let dp = axial / dist;
    return dp + eps >= cos_outer;
}

// Sanity-check that a candidate `t` actually hits the cone surface within range.
fn validate_cone_hit(
    t: f32,
    origin: vec3<f32>,
    dir: vec3<f32>,
    apex: vec3<f32>,
    axis: vec3<f32>,
    cos_outer: f32,
    range: f32,
) -> bool {
    if t <= 1e-4 { return false; }
    let p = origin + dir * t;
    return is_point_inside_cone(apex, axis, cos_outer, range, p);
}

// Intersect the camera ray with the finite cone.
// Returns `vec2<t_enter, t_exit>` describing the interval inside the cone.
// If no hit is found, the result is `(large, small)` so callers can detect failure.
fn intersect_cone(
    apex: vec3<f32>,
    axis: vec3<f32>,
    cos_outer: f32,
    range: f32,
    origin: vec3<f32>,
    dir: vec3<f32>,
    inside: bool,
) -> vec2<f32> {
    let rel = origin - apex;
    let cos2 = cos_outer * cos_outer;
    let dd = dot(dir, dir);
    let dl = dot(dir, axis);
    let xl = dot(rel, axis);
    let dx = dot(dir, rel);
    let xx = dot(rel, rel);

    let a = dl * dl - cos2 * dd;
    let b = 2.0 * (xl * dl - cos2 * dx);
    let c = xl * xl - cos2 * xx;

    var t_enter = 1e32;
    var t_exit  = -1e32;
    var found = false;

    if abs(a) > 1e-6 {
        let discr = b * b - 4.0 * a * c;
        if discr >= -1e-6 {
            let safe_discr = max(discr, 0.0);
            let sqrt_d = sqrt(safe_discr);
            let inv = 0.5 / a;
            let t0 = (-b - sqrt_d) * inv;
            let t1 = (-b + sqrt_d) * inv;
            if validate_cone_hit(t0, origin, dir, apex, axis, cos_outer, range) {
                t_enter = min(t_enter, t0);
                t_exit  = max(t_exit,  t0);
                found = true;
            }
            if validate_cone_hit(t1, origin, dir, apex, axis, cos_outer, range) {
                t_enter = min(t_enter, t1);
                t_exit  = max(t_exit,  t1);
                found = true;
            }
        }
    } else if abs(b) > 1e-6 {
        let t_lin = -c / (2.0 * b);
        if validate_cone_hit(t_lin, origin, dir, apex, axis, cos_outer, range) {
            t_enter = min(t_enter, t_lin);
            t_exit  = max(t_exit,  t_lin);
            found = true;
        }
    }

    let denom = dl;
    if abs(denom) > 1e-6 {
        let t_cap = (range - xl) / denom;
        if validate_cone_hit(t_cap, origin, dir, apex, axis, cos_outer, range) {
            t_enter = min(t_enter, t_cap);
            t_exit  = max(t_exit,  t_cap);
            found = true;
        }
    }

    if inside { t_enter = 0.0; }

    if !found || t_exit <= t_enter + 1e-4 {
        return vec2<f32>(1e32, -1e32);
    }
    return vec2<f32>(max(t_enter, 0.0), t_exit);
}

const MIN_MARCH_STEPS: u32 = 4u;      // minimum number of samples per ray
const MAX_MARCH_STEPS: u32 = 64u;     // maximum number of samples per ray
const TARGET_STEP_LENGTH: f32 = 0.5;  // desired spacing (in metres) between samples
const EPS: f32 = 1e-6;

// Diagnostic information returned alongside the accumulated colour.
struct MarchResult {
    color: vec3<f32>,             // accumulated scattering
    hit_ratio: f32,               // fraction of march steps that contributed
    weight_ratio: f32,            // average angular/radial weight per step
    clamped_length_ratio: f32,    // (depth-clamped length) / (cone range)
    raw_length_ratio: f32,        // (analytic length) / (cone range)
}

// Integrate the volumetric lighting along the camera ray inside the cone.
fn march_cone(
    camera_pos: vec3<f32>,
    ray_dir: vec3<f32>,
    camera_depth: f32,
    scatter_strength: f32,
) -> MarchResult {
    let cone = cone_params_from_uniform(cone_uniform);

    let inside = is_point_inside_cone(cone.apex, cone.axis, cone.cos_outer, cone.range, camera_pos);
    let interval = intersect_cone(cone.apex, cone.axis, cone.cos_outer, cone.range, camera_pos, ray_dir, inside);
    var t_start = interval.x;
    var t_end   = interval.y;
    if t_end <= t_start + EPS {
        return MarchResult(vec3<f32>(0.0), 0.0, 0.0, 0.0, 0.0);
    }

    let raw_length = max(t_end - t_start, 0.0);
    let raw_length_ratio = clamp(raw_length / cone.range, 0.0, 1.0);
    if raw_length_ratio <= EPS {
        return MarchResult(vec3<f32>(0.0), 0.0, 0.0, 0.0, 0.0);
    }

    t_end = min(t_end, camera_depth);
    if t_end <= t_start + EPS {
        return MarchResult(vec3<f32>(0.0), 0.0, 0.0, 0.0, raw_length_ratio);
    }

    let desired_steps = clamp(
        u32(ceil(raw_length / TARGET_STEP_LENGTH)),
        MIN_MARCH_STEPS,
        MAX_MARCH_STEPS,
    );

    let clamped_length = clamp(min(camera_depth - t_start, raw_length), 0.0, raw_length);
    if clamped_length <= EPS {
        return MarchResult(vec3<f32>(0.0), 0.0, 0.0, 0.0, raw_length_ratio);
    }

    let dt = clamped_length / f32(desired_steps);
    if dt <= EPS {
        return MarchResult(vec3<f32>(0.0), 0.0, 0.0, 0.0, raw_length_ratio);
    }

    let clamped_length_ratio = clamp(clamped_length / cone.range, 0.0, 1.0);

    var accum = vec3<f32>(0.0);
    var transmittance = 1.0;
    let base_color = cone_uniform.color_intensity.xyz;
    let light_intensity = cone_uniform.color_intensity.w;
    let sigma_a = 0.25;
    var weight_sum = 0.0;
    let samples_f = f32(desired_steps);

    for (var step: u32 = 0u; step < desired_steps; step = step + 1u) {
        let sample_t = t_start + (f32(step) + 0.5) * dt;
        if sample_t > t_start + clamped_length { break; }

        let sample_pos = camera_pos + ray_dir * sample_t;
        let rel  = sample_pos - cone.apex;
        let dist = length(rel);
        if dist == 0.0 || dist > cone.range { continue; }

        let dir_to_point = rel / dist;
        let dp = dot(dir_to_point, cone.axis);
        let angular_margin = 0.08;
        let soft_outer = cone.cos_outer - angular_margin;
        let denom = max(cone.cos_inner - soft_outer, 1e-5);
        let angular_weight = clamp((dp - soft_outer) / denom, 0.0, 1.0);

        // edge softening
        let axial = dot(rel, cone.axis);
        var radial_weight: f32 = 1.0;
        if cone.tan_outer > 0.0 {
            let radius_limit = axial * cone.tan_outer;
            if radius_limit > 1e-5 {
                let radial_len = length(rel - cone.axis * axial);
                let ratio = clamp(radial_len / (radius_limit + 1e-5), 0.0, 1.0);
                let one_minus = 1.0 - ratio;
                radial_weight = one_minus * one_minus;
            }
        }
        
        let d_light = length(rel);

        //distance from lightsource by fog
        let t_light = fog_transmittance_from_light(d_light, fog);

        let distance_falloff = 1.0 / (1.0 + axial * axial * 0.12);
        let weight = angular_weight * radial_weight;
        weight_sum += weight;

        let scatter = base_color
            * (light_intensity * distance_falloff * weight * scatter_strength * raw_length_ratio * t_light);
        accum += scatter * transmittance * dt;

        let extinction = sigma_a * dt;
        transmittance *= exp(-extinction);
        if transmittance <= 1e-3 { break; }
    }

    var hit_ratio = 0.0;
    var weight_ratio = 0.0;
    if weight_sum > 0.0 {
        hit_ratio = clamp(weight_sum / samples_f, 0.0, 1.0);
        weight_ratio = clamp(weight_sum / samples_f, 0.0, 1.0);
    }

    return MarchResult(accum, hit_ratio, weight_ratio, clamped_length_ratio, raw_length_ratio);
}

@vertex
fn vertex(@location(0) position: vec3<f32>) -> VertexOutput {
    let local = vec4<f32>(position, 1.0);
    let world = cone_uniform.model * local;
    let clip  = view_uniform.view_proj * world;

    var out: VertexOutput;
    out.clip_position = clip;
    out.world_position = world.xyz / world.w;
    out.ndc_xy = clip.xy / clip.w;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let screen_size = view_uniform.screen_size.xy;
    if screen_size.x < 1.0 || screen_size.y < 1.0 {
        return FragmentOutput(vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }

    let scatter_factor = max(view_uniform.params.x, 0.0);
    if scatter_factor <= 0.0 {
        return FragmentOutput(vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }
    let scatter_strength = scatter_factor * 1e-5;
    let debug_mode = u32(view_uniform.params.y + 0.5);

    let camera_pos = view_uniform.camera_position.xyz;
    var ray_dir = in.world_position - camera_pos;
    let ray_length = length(ray_dir);
    if ray_length <= EPS {
        return FragmentOutput(vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }
    ray_dir = ray_dir / ray_length;

    let ndc_xy = in.clip_position.xy / in.clip_position.w;
    let max_coord = screen_size - vec2<f32>(1.0, 1.0);
    let uv = ndc_xy * 0.5 + vec2<f32>(0.5, 0.5);
    let frag_coord = clamp(uv * screen_size, vec2<f32>(0.0, 0.0), max_coord);
    let pixel = vec2<i32>(frag_coord + vec2<f32>(0.5, 0.5));
    let depth_sample = sample_depth_min(frag_coord, screen_size);

    var camera_depth = cone_uniform.direction_range.w;
    var depth_vis = clamp(depth_sample, 0.0, 1.0);
    // Depth clamp disabled for debugging
    /*
    if depth_sample > 1e-6 {
        let clip_z = depth_sample * 2.0 - 1.0;
        let depth_world = world_from_ndc(vec3<f32>(ndc_xy, clip_z));
        let geom_depth = dot(depth_world - camera_pos, ray_dir);
        if geom_depth > 0.0 {
            camera_depth = max(geom_depth - 0.002, 0.0);
        }
    }
    */

    let result = march_cone(camera_pos, ray_dir, camera_depth, scatter_strength);

    var output_color = result.color;
    if debug_mode == 1u {
        let v = pow(result.hit_ratio, 0.2);
        output_color = vec3<f32>(v, 0.2 * (1.0 - v), 1.0 - v);
    } else if debug_mode == 2u {
        let v = pow(result.weight_ratio, 0.2);
        output_color = vec3<f32>(0.1, v, 1.0 - v);
    } else if debug_mode == 3u {
        let v = clamp(result.clamped_length_ratio * 10.0, 0.0, 1.0);
        output_color = vec3<f32>(v, v * 0.5, 1.0 - v);
    } else if debug_mode == 4u {
        let v = clamp(result.raw_length_ratio * 10.0, 0.0, 1.0);
        output_color = vec3<f32>(v, 1.0 - v, v * 0.5);
    } else if debug_mode == 5u {
        let v = clamp(depth_vis, 0.0, 1.0);
        output_color = vec3<f32>(1.0 - v, v, 0.5 * (1.0 - v));
    }

    return FragmentOutput(vec4<f32>(output_color, 1.0));
}
