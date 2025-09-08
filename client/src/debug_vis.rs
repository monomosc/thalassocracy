use bevy::prelude::*;
use bevy::pbr::wireframe::WireframeConfig;
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use bevy_inspector_egui::InspectorOptions;
use levels::{sample_flow_at, Vec3f, builtins::greybox_level};
use crate::scene::SimSet;
use crate::scene::submarine::{Submarine, Velocity, SubTelemetry};

#[derive(Resource, Debug, Clone, Reflect, InspectorOptions)]
#[reflect(Resource)]
pub struct DebugVis {
    pub labels: bool,
    pub wireframe_global: bool,
    pub flow_arrows: bool,
    pub overlay: bool,
    pub speed_arrow: bool,
    pub telemetry: bool,
    pub desync_indicator: bool,
}

impl Default for DebugVis {
    fn default() -> Self {
        Self { labels: true, wireframe_global: false, flow_arrows: false, overlay: true, speed_arrow: true, telemetry: false, desync_indicator: true }
    }
}

#[derive(Component)]
pub struct LabelNode;

pub struct DebugVisPlugin;

impl Plugin for DebugVisPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugVis>()
            .register_type::<DebugVis>()
            .add_plugins(ResourceInspectorPlugin::<DebugVis>::default())
            .add_systems(Startup, spawn_debug_overlay)
            .add_systems(Update, (apply_wireframe_flag, apply_label_visibility, apply_overlay_visibility, update_debug_overlay))
            .add_systems(Update, draw_speed_arrow.after(SimSet));
    }
}

fn apply_wireframe_flag(vis: Res<DebugVis>, mut cfg: ResMut<WireframeConfig>) {
    if vis.is_changed() {
        cfg.global = vis.wireframe_global;
    }
}

fn apply_label_visibility(vis: Res<DebugVis>, mut q: Query<&mut Visibility, With<LabelNode>>) {
    if !vis.is_changed() { return; }
    let visible = vis.labels;
    for mut v in &mut q {
        *v = if visible { Visibility::Visible } else { Visibility::Hidden };
    }
}

#[derive(Component)]
struct DebugOverlayNode;

fn spawn_debug_overlay(mut commands: Commands, assets: Res<AssetServer>) {
    // Create a top-right anchored text node; content will be filled by updater
    let font: Handle<Font> = assets.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top: Val::Px(10.0),
            ..Default::default()
        },
        Text::new(String::new()),
        TextFont { font, font_size: 16.0, ..Default::default() },
        TextColor(Color::WHITE),
        DebugOverlayNode,
        Name::new("Debug Overlay"),
    ));
}

fn apply_overlay_visibility(vis: Res<DebugVis>, mut q: Query<&mut Visibility, With<DebugOverlayNode>>) {
    if !vis.is_changed() { return; }
    let visible = vis.overlay;
    for mut v in &mut q {
        *v = if visible { Visibility::Visible } else { Visibility::Hidden };
    }
}

#[derive(Resource, Default)]
struct OverlayLastYaw { yaw: f32, time: f32, rate: f32 }

#[allow(clippy::too_many_arguments)]
fn update_debug_overlay(
    time: Res<Time>,
    mut last: Local<OverlayLastYaw>,
    mut q_text: Query<&mut Text, With<DebugOverlayNode>>,
    q_sub: Query<(&Transform, &Velocity), With<Submarine>>,
    controls: Option<Res<crate::hud_controls::ThrustInput>>,
    vis: Res<DebugVis>,
    telemetry: Option<Res<SubTelemetry>>,
    pause: Option<Res<crate::sim_pause::SimPause>>,
    desync: Option<Res<crate::desync_metrics::DesyncMetrics>>,
) {
    let Ok(mut text) = q_text.single_mut() else { return; };
    let Ok((transform, vel)) = q_sub.single() else {
        text.0 = "No submarine".to_string();
        return;
    };

    // Position and speed
    let p = transform.translation;
    let v = **vel;
    let speed = v.length();

    // Yaw and yaw rate estimate from transform
    use bevy::prelude::EulerRot;
    let (_rx, yaw, _rz) = transform.rotation.to_euler(EulerRot::YXZ);
    let t = time.elapsed_secs();
    let dt = (t - last.time).max(1e-3);
    let mut dyaw = yaw - last.yaw;
    // unwrap to nearest
    if dyaw > std::f32::consts::PI { dyaw -= std::f32::consts::TAU; }
    if dyaw < -std::f32::consts::PI { dyaw += std::f32::consts::TAU; }
    last.rate = dyaw / dt;
    last.yaw = yaw;
    last.time = t;

    // Inputs
    let (thrust, rudder) = if let Some(c) = controls { (c.value, c.yaw) } else { (0.0, 0.0) };

    // Flow sample
    let level = greybox_level();
    let (flow, _var) = sample_flow_at(&level, Vec3f { x: p.x, y: p.y, z: p.z }, t);
    let flow_mag = (flow.x * flow.x + flow.y * flow.y + flow.z * flow.z).sqrt();
    let rel = Vec3::new(v.x - flow.x, v.y - flow.y, v.z - flow.z);
    let rel_speed = rel.length();

    let paused = pause.map(|p| p.0).unwrap_or(false);
    // Optional sync indicator line appended to overlay
    let sync_line = if vis.desync_indicator {
        if let Some(d) = desync {
            format!(
                "\nSYNC Adj {:>4.2}  age {:>3.0}ms  unack {:>2}  pos {:>4.2}m  ang {:>4.1}°",
                d.adj_factor_ema,
                d.snapshot_age_ms,
                d.unacked_inputs,
                d.last_pos_err_m,
                d.last_yaw_err_deg,
            )
        } else {
            "\nSYNC n/a".to_string()
        }
    } else { String::new() };

    if vis.telemetry {
        if let Some(t) = telemetry {
            let d = &t.0;
            text.0 = format!(
                "{}POS  {:7.2} {:7.2} {:7.2}\nSPD  {:5.2} m/s  REL {:5.2}\nYAW  {:6.1} deg  dYAW {:6.1} deg/s\nIN   T:{:>5.2}  R:{:>5.2}\nWATER {:5.2} ({:5.2},{:5.2},{:5.2})\n-- TELEMETRY{} --\nREL u:{:>5.2} v:{:>5.2} w:{:>5.2}\nQ   {:>6.1}  sign_u:{:>+3.0}  fm:{:>4.1}\nTAU ctl:{:>7.1} d_lin:{:>7.1} d_q:{:>7.1} d_v:{:>7.1}\nTAU ws:{:>7.1} beta:{:>7.1}  TOT:{:>7.1}\nERR {:>6.2} deg  ACC {:>6.3} r/s²{}",
                if paused { "PAUSED \n" } else { "" },
                if paused { " (last frame)" } else { "" },
                p.x, p.y, p.z,
                speed, rel_speed,
                yaw.to_degrees(), last.rate.to_degrees(),
                thrust, rudder,
                flow_mag, flow.x, flow.y, flow.z,
                d.u, d.v, d.w,
                d.q_dyn, d.sign_u, d.front_mount_gain,
                d.tau_control, d.tau_damp_lin, d.tau_damp_quad, d.tau_damp_dyn,
                d.tau_ws, d.tau_beta, d.tau_total,
                d.yaw_err.to_degrees(), d.yaw_acc,
                sync_line,
            );
        } else {
            text.0 = format!(
                "{}POS  {:7.2} {:7.2} {:7.2}\nSPD  {:5.2} m/s  REL {:5.2}\nYAW  {:6.1} deg  dYAW {:6.1} deg/s\nIN   T:{:>5.2}  R:{:>5.2}\nWATER {:5.2} ({:5.2},{:5.2},{:5.2})\n(telemetry unavailable){}",
                if paused { "PAUSED \n" } else { "" },
                p.x, p.y, p.z,
                speed, rel_speed,
                yaw.to_degrees(), last.rate.to_degrees(),
                thrust, rudder,
                flow_mag, flow.x, flow.y, flow.z,
                sync_line,
            );
        }
    } else {
        text.0 = format!(
            "{}POS  {:7.2} {:7.2} {:7.2}\nSPD  {:5.2} m/s  REL {:5.2}\nYAW  {:6.1} deg  dYAW {:6.1} deg/s\nIN   T:{:>5.2}  R:{:>5.2}\nWATER {:5.2} ({:5.2},{:5.2},{:5.2}){}",
            if paused { "PAUSED \n" } else { "" },
            p.x, p.y, p.z,
            speed, rel_speed,
            yaw.to_degrees(), last.rate.to_degrees(),
            thrust, rudder,
            flow_mag, flow.x, flow.y, flow.z,
            sync_line,
        );
    }
}

fn draw_speed_arrow(
    vis: Option<Res<DebugVis>>,
    mut gizmos: Gizmos,
    q_sub: Query<(&Transform, &Velocity), With<Submarine>>,
    time: Res<Time>,
) {
    let Some(vis) = vis else { return; };
    if !vis.speed_arrow { return; }
    let Ok((transform, vel)) = q_sub.single() else { return; };
    let p = transform.translation + Vec3::Y * 1.5;
    let v_world = **vel;
    let speed_w = v_world.length();
    if speed_w >= 1e-3 {
        let dir = v_world / speed_w;
        let end = p + dir * speed_w;
        gizmos.arrow(p, end, Color::srgb(0.2, 1.0, 0.2)); // green: world velocity
    }

    // Also draw water-relative velocity arrow (cyan) for clarity
    let level = greybox_level();
    let (flow, _var) = sample_flow_at(&level, Vec3f { x: p.x, y: p.y, z: p.z }, time.elapsed_secs());
    let v_rel = v_world - Vec3::new(flow.x, flow.y, flow.z);
    let speed_rel = v_rel.length();
    if speed_rel >= 1e-3 {
        let dir = v_rel / speed_rel;
        let end = p + dir * speed_rel;
        gizmos.arrow(p, end, Color::srgb(0.2, 1.0, 1.0)); // cyan: water-relative velocity
    }
}
