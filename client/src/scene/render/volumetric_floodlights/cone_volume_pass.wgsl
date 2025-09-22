struct ViewUniform {
    inv_view_proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
    camera_position: vec4<f32>;
    screen_size: vec4<f32>;
};

struct ConeUniform {
    model: mat4x4<f32>;
    apex: vec4<f32>;
    direction_range: vec4<f32>;
    color_intensity: vec4<f32>;
    angles: vec4<f32>;
};

@group(0) @binding(0) var shadow_atlas: texture_depth_2d_array;
@group(0) @binding(1) var shadow_sampler: sampler_comparison;

@group(1) @binding(0) var<uniform> view_uniform: ViewUniform;
@group(1) @binding(1) var view_depth: texture_depth_2d;

@group(2) @binding(0) var<uniform> cone_uniform: ConeUniform;

struct VertexInput {
    @location(0) position: vec3<f32>;
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>;
    @location(0) world_position: vec3<f32>;
};

struct FragmentOutput {
    @location(0) color: vec4<f32>;
};

fn ndc_from_position(position: vec2<f32>, inv_screen_size: vec2<f32>) -> vec2<f32> {
    let uv = position * inv_screen_size;
    return vec2<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0);
}

fn world_from_ndc(ndc: vec3<f32>) -> vec3<f32> {
    let clip = vec4<f32>(ndc, 1.0);
    let world = view_uniform.inv_view_proj * clip;
    return world.xyz / world.w;
}

fn is_point_inside_cone(apex: vec3<f32>, axis: vec3<f32>, tan_outer: f32, range: f32, point: vec3<f32>) -> bool {
    let rel = point - apex;
    let axial = dot(rel, axis);
    if axial < 0.0 || axial > range {
        return false;
    }
    let radius = axial * tan_outer;
    let radial = rel - axis * axial;
    return dot(radial, radial) <= radius * radius + 1e-4;
}

fn validate_cone_hit(
    t: f32,
    origin: vec3<f32>,
    dir: vec3<f32>,
    apex: vec3<f32>,
    axis: vec3<f32>,
    tan_outer: f32,
    range: f32,
) -> bool {
    if t <= 1e-4 {
        return false;
    }
    let sample_pos = origin + dir * t;
    let rel = sample_pos - apex;
    let axial = dot(rel, axis);
    if axial < 0.0 || axial > range {
        return false;
    }
    let radius = axial * tan_outer;
    let radial = rel - axis * axial;
    return dot(radial, radial) <= radius * radius + 1e-4;
}

fn intersect_cone(
    apex: vec3<f32>,
    axis: vec3<f32>,
    cos_outer: f32,
    tan_outer: f32,
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
    var t_exit = -1e32;
    var found = false;

    if abs(a) > 1e-6 {
        let discr = b * b - 4.0 * a * c;
        if discr >= 0.0 {
            let sqrt_d = sqrt(discr);
            let inv = 0.5 / a;
            let t0 = (-b - sqrt_d) * inv;
            let t1 = (-b + sqrt_d) * inv;
            if validate_cone_hit(t0, origin, dir, apex, axis, tan_outer, range) {
                t_enter = min(t_enter, t0);
                t_exit = max(t_exit, t0);
                found = true;
            }
            if validate_cone_hit(t1, origin, dir, apex, axis, tan_outer, range) {
                t_enter = min(t_enter, t1);
                t_exit = max(t_exit, t1);
                found = true;
            }
        }
    } else if abs(b) > 1e-6 {
        let t_lin = -c / (2.0 * b);
        if validate_cone_hit(t_lin, origin, dir, apex, axis, tan_outer, range) {
            t_enter = min(t_enter, t_lin);
            t_exit = max(t_exit, t_lin);
            found = true;
        }
    }

    let denom = dl;
    if abs(denom) > 1e-6 {
        let t_cap = (range - xl) / denom;
        if validate_cone_hit(t_cap, origin, dir, apex, axis, tan_outer, range) {
            t_enter = min(t_enter, t_cap);
            t_exit = max(t_exit, t_cap);
            found = true;
        }
    }

    if inside {
        t_enter = 0.0;
    }

    if !found || t_exit <= t_enter + 1e-4 {
        return vec2<f32>(1e32, -1e32);
    }

    return vec2<f32>(max(t_enter, 0.0), t_exit);
}

const MARCH_STEPS: u32 = 16u;
const EPS: f32 = 1e-4;

fn march_cone(
    camera_pos: vec3<f32>,
    ray_dir: vec3<f32>,
    camera_depth: f32,
) -> vec3<f32> {
    let apex = cone_uniform.apex.xyz;
    let axis = normalize(cone_uniform.direction_range.xyz);
    let range = max(cone_uniform.direction_range.w, 0.001);
    let cos_inner = cone_uniform.angles.x;
    let cos_outer = cone_uniform.angles.y;
    let cos_outer_clamped = clamp(cos_outer, -0.9999, 0.9999);
    let cos_inner_clamped = clamp(cos_inner, cos_outer_clamped, 0.9999);
    let sin_outer_sq = max(1.0 - cos_outer_clamped * cos_outer_clamped, 1e-4);
    let tan_outer = sqrt(sin_outer_sq) / max(cos_outer_clamped, 1e-4);

    let inside = is_point_inside_cone(apex, axis, tan_outer, range, camera_pos);
    let interval = intersect_cone(apex, axis, cos_outer_clamped, tan_outer, range, camera_pos, ray_dir, inside);
    var t_start = interval.x;
    var t_end = interval.y;
    if t_end <= t_start + EPS {
        return vec3<f32>(0.0);
    }

    t_end = min(t_end, camera_depth);
    if t_end <= t_start + EPS {
        return vec3<f32>(0.0);
    }

    let steps = MARCH_STEPS;
    let dt = (t_end - t_start) / f32(steps);
    if dt <= EPS {
        return vec3<f32>(0.0);
    }

    var accum = vec3<f32>(0.0);
    var transmittance = 1.0;
    let base_color = cone_uniform.color_intensity.xyz;
    let light_intensity = cone_uniform.color_intensity.w;
    let sigma_a = 0.35;

    for (var step: u32 = 0u; step < steps; step = step + 1u) {
        let sample_t = t_start + (f32(step) + 0.5) * dt;
        if sample_t > t_end {
            break;
        }
        if sample_t >= camera_depth - 0.001 {
            break;
        }

        let sample_pos = camera_pos + ray_dir * sample_t;
        let rel = sample_pos - apex;
        let axial = dot(rel, axis);
        if axial < 0.0 || axial > range {
            continue;
        }
        let radius_limit = axial * tan_outer;
        let radial_vec = rel - axis * axial;
        let radial_sq = dot(radial_vec, radial_vec);
        if radial_sq > radius_limit * radius_limit + 1e-4 {
            continue;
        }

        let radius_ratio = if radius_limit > 1e-4 {
            sqrt(radial_sq) / radius_limit
        } else {
            0.0
        };
        let edge = 1.0 - smoothstep(0.8, 1.0, clamp(radius_ratio, 0.0, 1.0));
        let dir_to_point = normalize(rel);
        let spot = smoothstep(cos_outer_clamped, cos_inner_clamped, dot(dir_to_point, axis));
        let distance_falloff = 1.0 / (1.0 + axial * axial * 0.02);

        let scatter = base_color * (light_intensity * distance_falloff * spot * edge * 4.0);
        accum += scatter * transmittance * dt;

        let extinction = sigma_a * dt;
        transmittance *= exp(-extinction);
        if transmittance <= 1e-3 {
            break;
        }
    }

    return accum;
}

@vertex
fn vertex(@location(0) position: vec3<f32>) -> VertexOutput {
    let local = vec4<f32>(position, 1.0);
    let world = cone_uniform.model * local;
    let clip = view_uniform.view_proj * world;
    return VertexOutput(clip, world.xyz / world.w);
}

@fragment
fn fragment(
    @builtin(position) position: vec4<f32>,
    in: VertexOutput,
) -> FragmentOutput {
    let _ = textureDimensions(shadow_atlas);
    let _ = shadow_sampler;

    let screen_size = view_uniform.screen_size.xy;
    let inv_screen_size = view_uniform.screen_size.zw;
    if screen_size.x < 1.0 || screen_size.y < 1.0 {
        return FragmentOutput(vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }

    let ndc_xy = ndc_from_position(position.xy, inv_screen_size);
    let camera_pos = view_uniform.camera_position.xyz;
    var ray_dir = in.world_position - camera_pos;
    let ray_length = length(ray_dir);
    if ray_length <= EPS {
        return FragmentOutput(vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }
    ray_dir = ray_dir / ray_length;

    let max_coord = screen_size - vec2<f32>(1.0, 1.0);
    let clamped = clamp(position.xy, vec2<f32>(0.0, 0.0), max_coord);
    let pixel = vec2<i32>(clamped);
    let depth_sample = textureLoad(view_depth, pixel, 0);

    var camera_depth = 1e9;
    if depth_sample < 0.999999 {
        let clip_z = depth_sample * 2.0 - 1.0;
        let depth_world = world_from_ndc(vec3<f32>(ndc_xy, clip_z));
        let geom_depth = dot(depth_world - camera_pos, ray_dir);
        if geom_depth > 0.0 {
            camera_depth = max(geom_depth - 0.002, 0.0);
        }
    }

    let accum = march_cone(camera_pos, ray_dir, camera_depth);
    return FragmentOutput(vec4<f32>(accum, 1.0));
}
