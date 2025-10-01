# âœ¨ Visual Fidelity â€” Feature-Complete Targets

**Scope:** baseline visuals for routine piloting (â€œnormal drivingâ€) should convey believable underwater lighting, motion cues, and UI feedback without debug tooling. This checklist captures work remaining to hit that bar.

---

## Lighting & Atmosphere

### Volumetric Cone Lighting (in progress)
- **Current:** custom cone phase with prototype WGSL, debug UI, and a single runtime path (legacy removed).
- **Target:** production-ready cone renderer with shadowed scattering, runtime quality tiers, and art-tunable parameters (color, intensity, extinction).
- **Key gaps:** finalize render graph hookup (single source of truth for cone phase), implement camera-inside handling & cull variants, finish light occlusion sampling, authoring pipeline for spotlight â†’ cone settings, QA on performance budgets (step count vs. resolution).

### Depth-Fog & Color Grading Harmonization (recommended)
- **Why:** cones, environment fog, and hull lights must share a consistent extinction palette so distance reads correctly.
- **Needs:** depth-aware fog profile per biome, LUT-based color grading tied to depth/pressure, hooks for local overrides (stations, vents).

---

## Water Motion & Particulates

### Localized Flowfield Around Sub (not started)
- **Goal:** sample a higher-resolution CFD proxy near the player to drive wake deformation, bubble trails, and camera sway.
- **Work:** blend global flow grid with analytic thruster/ballast contributions, expose query API for VFX/HUD, budget compute so it runs every frame on client and server prediction step.

### Flow-Integrated Compute Particle System (not started)
- **Goal:** GPU particle sim that advects particulate density (motes, bubbles, silt) using the combined flow field.
- **Work:** compute dispatch for particle advection & lifetime, indirect draw/instance buffer for rendering (streak billboards / point sprites), level-of-detail rules (disable beyond AOI), authoring controls for density by biome/event.

### Contact & Wake Effects (recommended)
- **Why:** ground effect, wall wash, and thruster impingement sell motion cues during docking and tunnel flight.
- **Needs:** decals or screen-space projectors for silt kick-up, hull-projected light falloff tied to velocity, audio hooks for synchronized rumble.

---

## Interface & Feedback

### In-Game UI Framework (not started)
- **Current:** debug Egui overlays for HUD/instrumentation.
- **Target:** Bevy UI (or custom retained-mode) implementation with production layouts, animation states, and input focus handling.
- **Work:** define UI skin tokens (colors, typography), migrate HUD widgets (flow, ballast, cargo, damage), integrate with gamepad/keyboard, add diegetic overlays (e.g., glass curvature, warning strobes).

### Camera & Post Stack Polish (recommended)
- **Why:** unified exposure, bloom, and chromatic aberration reinforce speed and depth; currently tuned for debug visibility.
- **Needs:** auto-exposure tuned for caves, adjustable motion blur, vignette for peripheral focus, per-biome settings.

---

## Cross-Cutting Tasks

- **Authoring Tooling:** inspector panels and presets for lighting/particles/UI to speed iteration.
- **Performance Budgets:** establish target frame budgets on mid-range GPU (cone pass, particles, UI) with profiling gates.
- **Asset Pipeline:** placeholder â†’ production asset handoff (meshes, textures) with LODs compatible with the above systems.

---

## Open Questions

1. Do we snapshot/promote the compute particle system to server authority for shared sight lines, or accept client-side embellishment?
2. How should volumetric cones interact with future fog volumes (vents, bloom events)?
3. Should the in-game UI become diegetic glass cockpit, or remain HUD overlay with minimal chrome?
