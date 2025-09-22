# Volumetric Floodlight Cones in Bevy (0.16.x)
**Camera & Light Occlusion with a Custom Render Phase**  
*(concept-first; light on code, sufficient to implement)*

---

## Overview

We render underwater “light shafts” by drawing **proxy cone volumes** in a **custom render phase** that runs **after opaques/transparents and before bloom**. Each cone’s fragment shader **raymarches inside the cone** along the camera ray, accumulating single scattering. Two occlusion tests make it believable:

- **Camera occlusion** — clamp the march to the **camera depth buffer**, so beams don’t pass through walls in front of the camera.
- **Light occlusion** — test each sample against the **spotlight’s shadow map** (atlas slice) so geometry carves **shadow shapes inside the fog**.

We support multiple cones (up to ~16) by keeping per-cone uniforms small, running at **downsampled resolution**, and using **few (jittered) steps** per ray.

---

## Render Order (Concept)

```
Main 3D View
 ├─ Opaque3d            (writes: HDR Color, Depth)
 ├─ AlphaMask3d
 ├─ Transparent3d
 ├─ ConeVolume3d  ← our phase (reads: Depth, Shadow Atlas; writes: HDR additively)
 ├─ Bloom
 ├─ Tonemapping
 └─ Dither/UI...
```

**Dependencies:**  
- Camera depth texture (from Opaque3d or a prepass).  
- Spotlight shadow map atlas (depth array created by the light shadow pass).  
- View uniforms (camera matrices & world-space camera position).

---

## ECS Data Model

**Game World Components**
- `ConeVolume` — tag; drawn only by our custom phase.
- `LinkedLight(Entity)` — the spotlight this cone belongs to.
- `Handle<Mesh>` — cone mesh (32–64 slices are enough).
- `Transform/GlobalTransform` — cone placement (align with spotlight).
- *(Optional)* Gameplay-facing settings (color tint, intensity, range).

**Render World (extracted)**
- `ConeLightParams` (per draw):  
  - `light_view_proj : Mat4` (spotlight VP for shadow lookup)  
  - `atlas_layer    : u32`   (which slice in the shadow atlas)  
  - `bias           : f32`   (shadow bias)  
  - `range          : f32`  
  - `intensity      : f32`  
  - `color          : Vec3`  
  - `cos_inner/outer: f32`  
  - `sigma_e        : f32`   (extinction)  
  - *(Optionally `light_pos`, `light_dir` if not reconstructible from transforms)*

- Per-view bindings: camera matrices, camera world position, **camera depth texture**.  
- Global bindings: **shadow atlas** (`texture_depth_2d_array`) + **comparison sampler**.

---

## Custom Render Phase & Node

Create:
- A **render phase type** `ConeVolumePhase`.
- A **view node** `ConeVolume3dNode` inserted **after `Transparent3d`** and **before `Bloom`**.
- A **queue system** that:
  1. Iterates cones, follows `LinkedLight` to fetch spotlight render data (VP + atlas layer).
  2. Performs a **camera-inside-cone** test.
  3. Chooses a **pipeline variant** with **`Cull::Back`** (outside) or **`Cull::None`** (inside).
  4. Uploads `ConeLightParams` and enqueues a draw item in `ConeVolumePhase`.

> Keep cones out of the built-in Opaque/Transparent phases. Only the custom phase draws them.

---

## Pipeline State (Cone Material)

- **Color target**: the main view’s **HDR** color target (or a downsampled intermediate).
- **Blend**: **additive** (`src=ONE, dst=ONE`).
- **Depth test**: **LessEqual** (read-only).
- **Depth write**: **disabled**.
- **Cull mode**:  
  - **`Back`** when camera **outside** the cone (normal case).  
  - **`None`** when camera **inside** the cone (to rasterize interior walls).
- **Topology**: triangle list.

*(Two pipeline specializations for the cull mode keeps cost low.)*

---

## Camera-Inside-Cone Test (queue-time)

Given spotlight apex `A`, axis `d` (normalized), outer half-angle `θ` (`cos_outer`), range `R`, camera position `C`:

```
v = C - A
inside = ( length(v) <= R ) && ( dot(normalize(v), d) >= cos(θ) )
```

Use `inside` to pick the cull-mode pipeline.

---

## Shader Bindings (WGSL concept)

```
@group(0) @binding(0) var shadow_atlas   : texture_depth_2d_array;
@group(0) @binding(1) var shadow_sampler : sampler_comparison;

@group(1) @binding(0) var<uniform> u_cone : ConeLightParams;

@group(2) @binding(0) var<uniform> u_view : CameraParams;        // inv_view, inv_proj, inv_view_proj, camera_pos
@group(2) @binding(1) var depth_tex       : texture_depth_2d;     // camera depth
```

> Group(0) = global per-frame resources; Group(1) = per-light/per-draw; Group(2) = per-view.

---

## Fragment Shader Core (pseudo)

1) **Reconstruct camera ray** from `frag_coord` using `u_view.inv_view_proj`.  
2) **Ray vs cone intersection** → `[t_enter, t_exit]`.  
3) **Clamp bounds**:
   - `t_enter = max(t_enter, 0.0)`  *(camera-inside case)*  
   - `t_exit  = min(t_exit, camera_depth_t, u_cone.range)`  
   - if `t_exit <= t_enter` → return 0.
4) **Raymarch** `N` steps between `t_enter..t_exit`:
   - `x = cam + t * ray_dir`
   - **Shadow test**: project `x` with `u_cone.light_view_proj`; sample **shadow atlas** at `atlas_layer` with comparison sampler; skip contribution if occluded.
   - **Spot attenuation**: `spot = smoothstep(cos_outer, cos_inner, dot(normalize(x - light_pos), light_dir))`
   - **Distance falloff**: `atten = 1 / (1 + k * dist^2)`
   - **Accumulate**: `L += Tr * (spot * atten * intensity) * color * dt`
   - **Extinction**: `Tr *= exp(-sigma_e * dt)`
5) **Return** `vec4(L, 1)` with **additive blending**.

**Camera occlusion** = clamp to camera depth at step 3.  
**Light occlusion**  = shadow-map comparison in step 4.

---

## Multi-Light Wiring (N cones)

- **One** global shadow atlas & sampler bound once.  
- **Per-cone** uniforms carry:
  - `light_view_proj`
  - `atlas_layer`
  - cone & medium params
- The shader is identical for all cones; only the per-draw uniform changes.

---

## Performance Guidance

- **Downsampled pass**: render cone volumes to **½ or ¼ res** buffer → depth-aware blur → upsample.  
- **Steps**: **6–12** jittered, optionally reprojection-dithered.  
- **Shadow lookups**: not every step; stride 2 or cluster samples.  
- **Frustum culling**: skip cones outside view.  
- **Mesh tessellation**: 32–64 slices are enough; don’t overdo.

---
