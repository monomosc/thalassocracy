# SubPhysicsSpec Tuning Guidance

This document explains how to tune `SubPhysicsSpec` and interpret the parameters in the shared `levels` crate. It covers units, conventions, recommended workflows, and common symptoms with fixes.

## Units and Conventions

- Units: SI (meters, seconds, kilograms, Newtons, radians)
- Body axes:
  - +X: forward (heading)
  - +Y: up (perpendicular to hull deck)
  - +Z: right (starboard)
- Yaw convention: positive yaw turns the nose left (towards −Z). Right rudder input is positive, mapped to a negative yaw torque for forward motion.
- Orientation: `Quatf` (w,x,y,z). Forward vector is `orientation.rotate_vec3([1,0,0])`.

## Parameter Reference

- Mass & Inertia
  - `m` [kg]: Dry mass at 50% ballast fill baseline.
  - `ixx`, `iyy`, `izz` [kg·m²]: Moments of inertia around body axes. Start with cylinder approximations then iterate.

- Geometry & Areas
  - `length`, `diameter` [m]: Characteristic hull size. Used for cross-sectional areas and control arm lengths.
  - `s_forward`, `s_side`, `s_top` [m²]: Reference areas for quadratic drag (frontal, side, top). Coarse geometry suffices.

- Hydrodynamic Drag (quadratic + linear)
  - `cxd`, `cyd`, `czd` [-]: Quadratic coefficients for surge (+X), sway (+Z), heave (+Y in body frame). Larger → stronger speed-squared resistance.
  - `xu`, `yv`, `zw` [N·s/m]: Linear damping (helpful near zero speed; prevents jitter). Keep small relative to quadratic terms.

- Angular Damping (yaw and pitch)
  - `kr` [N·m·s/rad]: Linear yaw rate damping.
  - `kr2` [N·m·(s/rad)²]: Quadratic yaw rate damping (strong at large yaw rates).
  - `nr_v` [-]: Dynamic yaw damping multiplier scaled by surge dynamic pressure (stabilizes yaw at speed).
  - `kq` [N·m·s/rad]: Linear pitch rate damping (about body-right). Increase to quell pitch oscillations.

- Propulsion
  - `t_max` [N]: Maximum thrust along +X at input = ±1.
  - `tau_thr` [s]: Throttle response time constant. Currently used on the client side; server uses the instantaneous value.

- Control Surfaces & Couplings
  - `n_delta_r` [-]: Rudder yaw torque effectiveness. Scales with dynamic pressure and lever arm.
  - `y_delta_r` [-]: Lateral (sideforce) effectiveness from rudder (centripetal force proxy).
  - `n_beta` [-]: Weathervane torque coefficient (aligns heading to incoming flow; reduces sideslip).
  - `n_ws` [-]: Sideslip coupling torque coefficient; turns the nose into lateral flow.
  - `delta_r_max` [-]: Rudder deflection cap (input space).
  - `m_delta_b` [-]: Ballast-related scalar (reserved for later control modeling).

- Buoyancy & Ballast
  - `volume_m3` [m³]: Hull displaced volume at neutral buoyancy (50% ballast baseline). Used for buoyancy force magnitude baseline.
  - `ballast_tanks: Vec<BallastTankSpec>`: Tank layout and capacity.
    - `pos_body` [m]: Tank position relative to COM in body frame. +X forward tank should produce nose-down when heavier.
    - `capacity_kg` [kg]: Maximum ballast mass per tank (water mass).
  - `cb_offset_body` [m]: Center-of-buoyancy offset from COM in body coordinates. +Y moves COB above COM, creating a restoring pitch/roll torque.

## Recommended Tuning Workflow

1. Geometry & Mass
   - Set `length`, `diameter`, compute areas. Choose `m` realistically. Start with cylinder inertia approximations.

2. Neutral Buoyancy Baseline
   - Set `volume_m3` ≈ hull displaced volume at neutral. Keep ballast fills at 0.5 by default.
   - Choose `cb_offset_body.y = 0.08–0.18 m` for a small sub; this gives gentle self-righting in pitch/roll.

3. Drag
   - Start with `cxd ≈ 0.3–0.6`, `cyd ≈ 2–4`, `czd ≈ 1–2`.
   - Add small linear terms: `xu ≈ 20–60`, `yv ≈ 40–100`, `zw ≈ 30–80` to tame zero-speed jitter.

4. Angular Damping
   - Yaw: `kr ≈ 300–600`, `kr2 ≈ 80–150`, `nr_v ≈ 0.01–0.05`.
   - Pitch: `kq ≈ 150–350`. Increase if you overshoot pitch; decrease if too sluggish.

5. Control Effectiveness
   - `n_delta_r ≈ 0.004–0.01`. Increase until rudder authority feels right at typical speeds.
   - `y_delta_r ≈ 0.02–0.06` to achieve reasonable turning radii.
   - `n_beta ≈ 0.05–0.2`: more pulls the nose into relative flow stronger (reduces sideslip quickly).
   - Add guards for reversed flow (already in the model with `sign_u` and front-mount gain).

6. Ballast Behavior
   - Place tanks along +X/−X (fore/aft) so filling forward pitches nose down (as tested).
   - `capacity_kg`: start with 60–100 kg per tank for a 1–3 m sub scale; reduce if pitch response is too strong.

7. Iterate
   - Use the debug telemetry to observe: surge `u`, sideslip `w`, dynamic pressure `q_dyn`, pitch rate, yaw rate.
   - Adjust in small steps; prefer changing one parameter family at a time.

## Common Symptoms and Fixes

- Sub flips through 360° when pitching
  - Cause: torque independent of orientation or insufficient damping.
  - Fix: ensure COB offset is non-zero; verify world-frame torque (`r × F`) is used; increase `kq`; reduce tank lever arm/capacity.

- Rudder has too little authority
  - Increase `n_delta_r` and/or `y_delta_r`. Ensure `s_side`, `length` are reasonable.

- Excessive sideslip; nose won’t align with flow
  - Increase `n_beta` (weathervane) and/or `n_ws` (sideslip coupling). Verify body-axis drag `cyd` is high enough.

- Shaky at low speed or when flow reverses
  - Increase linear damping (`xu`, `yv`, `zw`) modestly; check dynamic terms do not kick in at very low `u`.

- Yaw oscillates at speed (snaking)
  - Increase `kr`, `kr2`, or `nr_v`. Consider reducing `n_delta_r`.

- Pitch oscillates while ascending/descending
  - Increase `kq`. Increase `cb_offset_body.y`. Lower ballast capacities.

## Debugging Aids

- Enable DebugVis telemetry to view: `u`, `v`, `w`, `q_dyn`, `tau_*`, `yaw_err`, `yaw_rate`, `heading_yaw`.
- Use the desync indicator to ensure you’re not fighting server reconciliation while evaluating handling.
- Write focused unit tests (see `levels/tests/`) for sign conventions and ballast responses.

## Safety Clamps and Guards

- Angular rate clamps protect against runaway: yaw `r_max`, pitch `q_max`. Adjust if dynamics saturate prematurely.
- Dynamic pressure and sign handling: ensure `sign_u` branches are correct for reversed flow.
- Always clamp ballast fill to `[0,1]`.

## Tuning Order Cheatsheet

1. Geometry, mass, inertia
2. Neutral buoyancy (`volume_m3`), `cb_offset_body`
3. Drag (`c*`, then `x*`)
4. Angular damping (`kr*`, `kq`)
5. Control authority (`n_delta_r`, `y_delta_r`, `n_beta`, `n_ws`)
6. Ballast capacities and positions
7. Iterate with telemetry

## References in Code

- Spec definition: `levels/src/sub_specs.rs`
- Physics integration: `levels/src/submarine_physics.rs`
- Pitch tests: `levels/tests/pitch_ballast_effect.rs`

