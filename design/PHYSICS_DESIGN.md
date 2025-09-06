# Thalassocracy — Submarine Physics Design

This document proposes a practical physics model for piloting submarines in the prototype. It balances “feels right” controls with server authority, determinism, and room to evolve into richer hydrodynamics later.

## Goals

- Server authoritative simulation with simple, tunable parameters.
- Inputs: thrust (surge), rudder (yaw), ballast differential (pitch). Future: lateral thrusters, trim.
- Flow-aware: motion responds to the effective water flow, without simulating the fluid itself.
- Numerically stable at fixed tick (e.g., 30 Hz), deterministic across platforms.
- Easy to extend toward more realistic hydrodynamics when needed.

## Scope (Baseline vs. Future)

- Baseline (now): 6DOF state with simplified forces/torques and damping. Yaw/pitch are active; roll can be damped/locked initially.
- Future: richer coefficients, added mass, cross-coupling, wake effects, multi-point flow sampling, contacts/collisions.

---

## State and Inputs

Per submarine (authoritative on server):
- Position `p` (world)
- Velocity `v` (world)
- Orientation `q` (quaternion, body→world)
- Angular velocity `ω` (body; rad/s)

Per-tick inputs (client → server → sim):
- Thrust `u_t ∈ [-1, 1]` (forward/back along +body X)
- Rudder `δr ∈ [-1, 1]` (yaw control about +body Y)
- Ballast diff `δb ∈ [-1, 1]` (pitch control about +body Z or X — pick and stick with a convention)

Environment:
- Flow `u_w = u_water(p)` sampled from the level’s flow field at the sub’s position.
- Density `ρ` (per biome; constant for now).

### Sign Conventions (Guardrails)

- Body axes: +X forward (bow), +Y up, +Z right (starboard).
- World yaw/heading: angle about +Y. 0 rad = facing +X. Positive yaw turns left (towards −Z), matching `Quat::from_rotation_y(+yaw)`.
- Rudder input `δr`: +1 = right rudder (starboard deflection). Under forward relative flow this produces a right turn (heading decreases). When reversing (relative flow negative), the effective sign flips (intuitive backing behavior).
- UI reflects this as “Rudder (+ = right)”.

---

## Precomputed Per‑Hull Parameters (SubPhysicsSpec)

For each hull class we precompute/tune and serialize:
- Mass/Inertia: `m`, `Ixx`, `Iyy`, `Izz` (assume diagonal; off‑diagonals ≈0 initially)
- Geometry refs: length `L`, diameter `D`, reference areas `S_forward`, `S_side`, `S_top`
- Drag & Damping (body-frame):
  - Quadratic drag: `Cxd`, `Cyd`, `Czd` (applied to `|v_b| v_b`)
  - Linear damping: `Xu`, `Yv`, `Zw` (optional small linear terms)
- Angular damping: `Kr`, `Kq` (or `Nr`, `Mq` if you prefer hydrodynamic notation)
- Controls:
  - Thrust: `T_max`, response constant `τ_thr` (first-order lag for throttle feel)
  - Rudder yaw effectiveness: `N_δr`
  - Rudder sideforce effectiveness: `Y_δr` (lateral lift scale with `q · S_side`)
  - Ballast pitch effectiveness: `M_δb`
  - Control limits: `δr_max`, `δb_max`, slew rates
- Buoyancy/Weight: displaced volume `V`, residual weight `ΔW`, CoB–CoM offset vector (small static restoring moments)

These are constants per hull class (data-driven) and multiplied by dynamic pressure as appropriate.

---

## Per‑Tick Algorithm (Server)

1) Sample flow and compute relative velocity
- World→body rotation `R = R(q)`
- Relative velocity in body frame: `v_b = Rᵀ (v − u_w)`

2) Forces (body frame → world)
- Thrust: `F_t = T(u_t) · x̂_body`, where `T(u_t) = u_t · T_max` (optionally filtered by `τ_thr`)
- Drag (quadratic): `F_d = −diag(Cxd, Cyd, Czd) ⊙ |v_b| ⊙ v_b`
- Linear damping (optional): `F_lin = −diag(Xu, Yv, Zw) · v_b`
- Rudder sideforce (centripetal): `F_r = (Y_δr · δr) · q · S_side · ê_right`, with sign flipped when reversing (multiply by `sign(u)` and optional front‑mount gain). This is the primary lateral force that bends the velocity vector.
- Net body force: `F_b = F_t + F_d + F_lin`
- Transform to world: `F = R F_b + F_ext` (e.g., small buoyancy residuals)

3) Torques (body frame)
- Dynamic pressure `q = ½ ρ |v_b|²` (or scale by the relevant component(s) of `v_b`)
- Rudder torque (yaw): `τ_r = (N_δr · δr) · q · ŷ_body`
- Ballast torque (pitch): `τ_b = (M_δb · δb) · q · axis_body`
- Angular damping: `τ_d = −diag(Kp, Kq, Kr) · ω`
- Optional flow-induced small moments: `τ_flow = f(v_b)` (start at 0)
- Net torque: `τ = τ_r + τ_b + τ_d + τ_flow`

4) Integrate (semi‑implicit Euler; stable and cheap)
- Linear: `a = F / m`; then `v += a dt`; `p += v dt`
- Angular: `ω̇ = I⁻¹ ( τ − ω × (I ω) )`; then `ω += ω̇ dt`
- Orientation: `Δq = exp_quat(½ ω dt)`; `q = normalize(q ⊗ Δq)`

5) Limits & Constraints
- Clamp inputs; cap angular rates if needed; blend linear vs quadratic damping around low speeds.

Note: Quaternion is the right orientation representation for 3D without gimbal lock; normalize periodically.

---

## Networking

- Server authoritative tick; client sends input ticks (thrust, rudder, ballast) and predicts locally.
- Server periodically broadcasts `StateDelta` (position/velocity/orientation later), with tick index.
- Client reconciles: corrects state with smoothing (lerp/exponential) or short rewind.
- Determinism: fixed dt, no random sources in physics; any stochastic flow variance must be deterministic in time/space.

---

## Wake Effect Placeholder (Future)

- Compute a wake scalar/field behind the sub: `w_strength = k_wake |v_b|`, direction `−x̂_body`, decays with distance/time.
- Wake modulates base flow field for trailing craft (AOI-limited to reduce cost).
- Eventually couple to drag/flow sampling on followers.

---

## Decomposition vs. Combined Form

Decomposed (forces/torques → integrate):
- Pros: simple, readable, testable; add features term-by-term; easy clamping and tuning; ECS-friendly.
- Cons: ignores some coupling (added mass, full 6×6 hydrodynamics) unless added manually.

Combined (matrix/Lie group):
- Pros: compact representation, captures couplings with mass/inertia/added-mass matrices; amenable to implicit integrators.
- Cons: heavier math/implementation; harder to tune for “feel”; may be overkill for prototype.

Recommendation: keep them decomposed for now. If future needs arise, introduce coupling terms progressively (e.g., added mass diagonal, then off-diagonals).

---

## Expansion Points

- Hydrodynamics:
  - Added mass matrices (diagonal first) and cross-coupling terms.
  - Angle-of-attack/side-slip small-angle models for flow-induced moments (`N_β`, `M_α`).
  - Multi-point flow sampling along hull to approximate gradients.
- Controls & Actuators:
  - Thrust response lag, ballast fill/empty time constants, control rate limits.
  - Additional control surfaces (stern planes), lateral thrusters.
  - Autopilot layers (hold depth/heading) on top of manual inputs.
- Integration & Stability:
  - RK2/Heun or RK4 for smoother orientation at low tick rates; keep semi-implicit Euler as default.
  - Constraint stabilization for locked roll or depth holds.
- Content/Gameplay:
  - Pressure/structural limits coupled to depth; damage over time beyond thresholds.
  - Contacts/collisions with walls (broadphase AABB + impulse response).
- Networking/Perf:
  - Snapshot interpolation for remote subs; AOI culling; compact deltas.
  - Deterministic “variance” sources tied to world time; seed by position.

---

## Data Sketch (Rust)

```rust
// Shared (levels crate)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubPhysicsSpec {
    pub m: f32,
    pub ixx: f32, pub iyy: f32, pub izz: f32,
    pub cxd: f32, pub cyd: f32, pub czd: f32,
    pub xu: f32, pub yv: f32, pub zw: f32,
    pub kr: f32, pub kq: f32,
    pub t_max: f32, pub tau_thr: f32,
    pub n_delta_r: f32, pub m_delta_b: f32, pub y_delta_r: f32,
    pub delta_r_max: f32, pub delta_b_max: f32,
    // geometry refs, buoyancy residuals…
}
```

The server holds `SubPhysicsSpec` per hull, runs the per‑tick step using inputs + flow, and broadcasts authoritative state. The client uses the same spec for prediction/reconciliation.

---

## Tuning Notes

- Use SI units throughout; keep consistent scale across content.
- Start with modest `T_max` and high damping; reduce damping until motion feels responsive.
- Blend linear and quadratic drag: `F = −(k_lin + k_quad |v|) v` to avoid sticky behavior at low speeds.
- Clamp inputs; limit angular rates to keep camera and remote interpolation stable.
- Tune `Y_δr` for desired turning radius: `F_lat ≈ m v² / R`. Example: `m=1200 kg`, `v=4 m/s`, `R=20 m` → `F_lat≈960 N`; with seawater `q≈8.2 kPa` and `S_side≈3 m²`, choose `Y_δr≈0.04`.

---

## Summary

- Represent orientation with quaternions; integrate semi‑implicitly.
- Decompose forces/torques for clarity and tuning speed.
- Precompute hull parameters once; multiply by dynamic pressure for control authority.
- Keep determinism and fixed tick; layer networking correction on top.
- Provide clear hooks to evolve toward richer hydrodynamics and gameplay features.
