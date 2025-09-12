use bevy::prelude::*;
use std::time::Instant;

use crate::hud_controls::ThrustInput;

/// Rolling network client stats updated by net.rs systems.
#[derive(Resource, Debug)]
pub struct NetClientStats {
    pub last_state_instant: Option<Instant>,
    pub inter_arrival_ewma_ms: f32,
    pub last_acked_tick: Option<u64>,
    pub last_server_tick: Option<u64>,
    /// Magnitude of last forced snap (pos error in meters at snap time).
    pub last_snap_magnitude_m: f32,
}

impl Default for NetClientStats {
    fn default() -> Self {
        Self {
            last_state_instant: None,
            inter_arrival_ewma_ms: 0.0,
            last_acked_tick: None,
            last_server_tick: None,
            last_snap_magnitude_m: 0.0,
        }
    }
}

/// Raw reconciliation errors extracted from ServerCorrection each frame.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ReconcileErrors {
    pub pos_err_m: f32,
    pub orientation_error_deg: f32,
    pub vel_err_mps: f32,
}

/// Aggregated, smoothed indicator of client/server divergence (0..1).
#[derive(Resource, Debug)]
pub struct DesyncMetrics {
    pub adj_factor_ema: f32, // 0 (in sync) .. 1 (very out of sync)
    pub snapshot_age_ms: f32,
    pub arrival_jitter_ms: f32,
    pub unacked_inputs: u32,
    pub last_snap_magnitude_m: f32,
    // For debug display
    pub last_pos_err_m: f32,
    pub last_yaw_err_deg: f32,
    pub last_vel_err_mps: f32,
}

impl Default for DesyncMetrics {
    fn default() -> Self {
        Self {
            adj_factor_ema: 0.0,
            snapshot_age_ms: 0.0,
            arrival_jitter_ms: 0.0,
            unacked_inputs: 0,
            last_snap_magnitude_m: 0.0,
            last_pos_err_m: 0.0,
            last_yaw_err_deg: 0.0,
            last_vel_err_mps: 0.0,
        }
    }
}

pub struct DesyncMetricsPlugin;

impl Plugin for DesyncMetricsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetClientStats>()
            .init_resource::<ReconcileErrors>()
            .init_resource::<DesyncMetrics>()
            // Compute reconciliation errors once per frame
            .add_systems(Update, sample_reconcile_errors)
            // Aggregate into a single indicator
            .add_systems(Update, aggregate_desync_metric);
    }
}

/// Read current ServerCorrection (if any) and produce raw errors.
fn sample_reconcile_errors(
    q_sub: Query<(&Transform, &crate::scene::submarine::Velocity, Option<&crate::scene::submarine::ServerCorrection>), With<crate::scene::submarine::Submarine>>,
    mut errs: ResMut<ReconcileErrors>,
) {
    let Ok((t, v, corr)) = q_sub.single() else { return; };
    if let Some(c) = corr {
        let pos_err = t.translation.distance(c.target_pos);
        // Use yaw-only difference for orientation error to avoid pitch/roll inflating error during turns.
        let yaw = |q: Quat| {
            let f = q * Vec3::X; // mesh forward
            f.z.atan2(f.x) // +left, -right
        };
        let mut dyaw = yaw(t.rotation) - yaw(c.target_rot);
        // wrap to [-pi, pi]
        if dyaw > std::f32::consts::PI { dyaw -= std::f32::consts::TAU; }
        if dyaw < -std::f32::consts::PI { dyaw += std::f32::consts::TAU; }
        let orientation_err = dyaw.abs().to_degrees();
        let vel_err = ((**v) - c.target_vel).length();
        errs.pos_err_m = pos_err;
        errs.orientation_error_deg = orientation_err;
        errs.vel_err_mps = vel_err;
    } else {
        // No outstanding correction -> assume aligned
        errs.pos_err_m = 0.0;
        errs.orientation_error_deg = 0.0;
        errs.vel_err_mps = 0.0;
    }
}

/// Aggregate multiple signals into a single 0..1 adjustment factor.
fn aggregate_desync_metric(
    time: Res<Time>,
    stats: Res<NetClientStats>,
    errs: Res<ReconcileErrors>,
    inputs: Option<Res<ThrustInput>>,
    mut out: ResMut<DesyncMetrics>,
) {
    let now = Instant::now();
    // Snapshot age
    let snap_age_ms = stats
        .last_state_instant
        .map(|t| now.saturating_duration_since(t).as_secs_f32() * 1000.0)
        .unwrap_or(0.0);
    // Input backlog (if we ever received an ack)
    let backlog = if let (Some(inp), Some(ack)) = (inputs.as_ref(), stats.last_acked_tick) {
        let sent = inp.tick;
        sent.saturating_sub(ack) as u32
    } else {
        0
    };

    // Keep last values for UI
    out.snapshot_age_ms = snap_age_ms;
    out.arrival_jitter_ms = stats.inter_arrival_ewma_ms;
    out.unacked_inputs = backlog;
    out.last_snap_magnitude_m = stats.last_snap_magnitude_m;
    out.last_pos_err_m = errs.pos_err_m;
    out.last_yaw_err_deg = errs.orientation_error_deg;
    out.last_vel_err_mps = errs.vel_err_mps;

    // Normalization tolerances (tunable):
    let pos_tol = 0.75_f32; // meters
    let yaw_tol = 3.0_f32; // degrees
    let vel_tol = 0.5_f32; // m/s
    let stale_tol = 75.0_f32; // ms
    let backlog_tol = 8.0_f32; // input ticks
    let jitter_tol = 30.0_f32; // ms
    let snap_tol = 3.0_f32; // meters (large snap ~ full contribution)

    // Weights: must sum loosely to ~1 but not required
    let w_pos = 0.35;
    let w_yaw = 0.20;
    let w_vel = 0.15;
    let w_stale = 0.15;
    let w_backlog = 0.10;
    let w_jitter = 0.03;
    let w_snap = 0.02;

    let clamp01 = |x: f32| x.clamp(0.0, 1.0);
    // If the player is actively steering, relax error contributions to avoid spiky Adj during tight maneuvers.
    let steer = inputs.as_ref().map(|i| i.yaw.abs()).unwrap_or(0.0);
    let relax = 1.0 - 0.5 * steer; // up to 50% relaxation at |yaw|=1

    let term_pos = w_pos * relax * clamp01(errs.pos_err_m / pos_tol);
    let term_yaw = w_yaw * relax * clamp01(errs.orientation_error_deg / yaw_tol);
    let term_vel = w_vel * relax * clamp01(errs.vel_err_mps / vel_tol);
    let term_stale = w_stale * clamp01(snap_age_ms / stale_tol);
    let term_backlog = w_backlog * clamp01(backlog as f32 / backlog_tol);
    let term_jitter = w_jitter * clamp01(stats.inter_arrival_ewma_ms / jitter_tol);
    let term_snap = w_snap * clamp01(stats.last_snap_magnitude_m / snap_tol);

    let raw = term_pos + term_yaw + term_vel + term_stale + term_backlog + term_jitter + term_snap;
    // Map to 0..1 with soft saturation
    let adj = 1.0 - (-raw).exp();

    // Time-based EMA smoothing with ~0.7s time constant
    let tau = 0.7_f32;
    let dt = time.delta_secs().max(1e-3);
    let alpha = 1.0 - (-dt / tau).exp();
    out.adj_factor_ema = out.adj_factor_ema + alpha * (adj - out.adj_factor_ema);
}
