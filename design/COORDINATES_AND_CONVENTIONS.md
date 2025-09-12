# Coordinates, Bases, and Sign Conventions

This document captures the shared assumptions across client, server, and the `levels` crate. It is the single source of truth for frames, axes, and sign conventions used by physics, input, HUD, and rendering.

## Frames and Bases

- World axes (right-handed):
  - +X: right (east)
  - +Y: up
  - +Z: forward (down-tunnel / gameplay forward)

- Submarine body axes (right-handed):
  - +Z: forward (heading)
  - +Y: up
  - +X: right (starboard)

- Orientation (`Quatf`): body→world rotation. The body forward vector in world space is `orientation * Vec3f::new(0.0, 0.0, 1.0)`.

- Heading/yaw extraction (radians):
  - `let f = orientation * Vec3f::new(0.0, 0.0, 1.0);`
  - `let yaw = (-f.x).atan2(f.z); // +yaw = left turn`

## Physics State and Integration

- Linear state in world frame:
  - `position: Vec3f` (m)
  - `velocity: Vec3f` (m/s)

- Angular state in body frame:
  - `ang_mom: Vec3f` = body angular momentum L (kg·m²·rad/s)
  - Angular velocity: `omega = I^{-1} * L` per-axis using spec `ixx, iyy, izz` (rad/s)

- Orientation update (body-axis deltas, post-multiply):
  - Yaw about +Y (up): `delta_yaw = axis_angle(+Y, omega.y * dt)`
  - Pitch about +X (right): `delta_pitch = axis_angle(+X, omega.x * dt)`
  - Roll about +Z (forward): `delta_roll = axis_angle(+Z, omega.z * dt)`
  - `orientation = (orientation * delta_pitch * delta_yaw * delta_roll).normalize()`

- Euler’s equations in body frame for angular momentum:
  - `Ldot = tau - omega × L`
  - `L += Ldot * dt`

- Yaw control and damping:
  - Right rudder input is positive (`inputs.yaw > 0`).
  - Positive input produces a right turn (positive yaw) under forward flow.
  - Control torque uses dynamic pressure and flips with flow direction (sign of surge).
  - Damping terms: linear (`kr`), quadratic (`kr2`), and speed-coupled (`nr_v * q`).

- Pitch/roll torques:
  - From ballast gravity and COB buoyancy: torque is computed as `(r × F) · axis_world` and projected onto +X (pitch) and +Z (roll) body axes.
  - Linear damping: pitch (`kq`) and small roll (`kp`).

- Lateral (centripetal) behavior while turning:
  - A centripetal acceleration approximation aligns velocity with curvature: `a_c ≈ -u * r * right_world`.
  - Anisotropic drag (quadratic + linear) is applied per body axis and recomposed to world.

## Flow Fields and Builtins

- Uniform flow vectors are expressed in world axes (+Z forward).
- Built-in levels define tunnel/torus flows along +Z to match gameplay forward.

## Inputs and Signs

- Thrust: `inputs.thrust ∈ [-1, 1]` applies along body +Z.
- Rudder: `inputs.yaw ∈ [-1, 1]`, positive = right rudder = right turn (+yaw).
- Ballast pumps: `pump_fwd`, `pump_aft` in [-1, 1] change tank fill; positive pumps water in.

## HUD / Instruments

- Flow instrument and “sideslip” use body axes with +Z forward:
  - Water-relative velocity in body frame via `rel_body = conj(orientation) * (v - flow)`.
  - Surge `u = rel_body.z` (forward), Sway `w` maps to body-right (+X), Heave `v = rel_body.y` (up).
  - Indicator projection centers when incoming flow ≈ +Z (from ahead), with x = right (n.x/|n.z|), y = up (n.y/|n.z|).

## Camera

- Follow camera tracks body forward using +Z: `dir = rotation * Vec3::Z`.
- “Behind sub” is along −Z in world when aligning with the sub’s orientation.

## Naming (body axes → motion components)

- Surge (u): along +Z (forward)
- Sway (w): along +X (right)
- Heave (v): along +Y (up)
