# Submarine Input Pipeline

This note documents the three-layer control path that now feeds the shared
submarine dynamics (levels::submarine_physics). The goal is to keep the raw UI
intent separate from the actuated control surfaces so we can add features like
lag, rate limits, or force feedback without touching the physics integrator.

## Layers

- SubInputs – transient UI or network intent (-1..1 values for thrust, yaw,
  and ballast pumps). It is produced by client input gathering or received from
  the server.
- SubInputState – persistent actuator state consumed by physics. Today it is
  a plain copy of the requested values, but it is the place to model timing
  effects, servo dynamics, or damage.
- SubState – the physical state advanced by step_submarine.

SubStepDebug now records both the commanded SubInputState and, when
available, the raw SubInputs for diagnostics.

## Client Flow

1. Player input is stored each frame in ThrustInput.
2. update_sub_input_state copies that resource into
   SubInputStateComp(SubInputState) on the local submarine entity. This is the
   hook for client-side smoothing or UI feedback.
3. simulate_submarine only reads SubInputStateComp and hands it to
   step_submarine_dbg, ensuring the integrator never sees the raw inputs.

## Server Flow

- ControlInputComp tracks the latest validated input from the client.
- server_physics_tick updates SubInputStateComp immediately before calling
  step_submarine. Future server-side filtering or latency compensation will
  live in that update path while the tick continues to consume only
  SubInputState.

## Future Extensions

- Apply per-channel slew rates or actuator lag inside SubInputState.
- Model failure modes by clamping or biasing SubInputState independently of
  the pilot intent.
- Add telemetry or replay tooling by logging both layers and comparing them
  during analysis.
