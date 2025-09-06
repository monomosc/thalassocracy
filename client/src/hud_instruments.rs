use bevy::prelude::*;
use levels::{builtins::greybox_level, sample_flow_at, Vec3f};

const INSTR_SIZE: f32 = 140.0; // px
const RING_THICKNESS: f32 = 2.0; // px
const DOT_SIZE: f32 = 12.0; // px
const MAX_REL_SPEED: f32 = 6.0; // m/s for color normalization
const SMOOTH_ALPHA: f32 = 0.2; // EMA for dot position/color

#[derive(Component)]
struct FlowInstrRoot;

#[derive(Component)]
struct FlowInstrRing;

#[derive(Component)]
struct FlowInstrDot;

#[derive(Resource, Default, Clone, Copy)]
struct FlowInstrState {
    pos: Vec2,
    /// Longitudinal water-relative speed along body +X (surge); >0 = coming from front, <0 = from back
    surge: f32,
}

pub struct HudInstrumentsPlugin;

impl Plugin for HudInstrumentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlowInstrState>()
            .add_systems(Startup, (spawn_flow_instr, spawn_ballast_hud))
            .add_systems(Update, (update_flow_state, draw_flow_instr, update_ballast_hud));
    }
}

fn spawn_flow_instr(mut commands: Commands) {
    // Bottom-center overlay container
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(INSTR_SIZE),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(Color::NONE),
            FlowInstrRoot,
            Name::new("Flow Instrument Root"),
        ))
        .with_children(|root| {
            // Ring
            root
                .spawn((
                    Node {
                        width: Val::Px(INSTR_SIZE),
                        height: Val::Px(INSTR_SIZE),
                        border: UiRect::all(Val::Px(RING_THICKNESS)),
                        ..Default::default()
                    },
                    BorderRadius::MAX,
                    BackgroundColor(Color::NONE),
                    BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                    FlowInstrRing,
                    Name::new("Flow Instrument Ring"),
                ))
                .with_children(|ring| {
                    // Dot (absolute within ring)
                    ring.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(DOT_SIZE),
                            height: Val::Px(DOT_SIZE),
                            ..Default::default()
                        },
                        BorderRadius::MAX,
                        BackgroundColor(Color::WHITE),
                        FlowInstrDot,
                        Name::new("Flow Instrument Dot"),
                    ));
                });
        });
}

fn update_flow_state(
    time: Res<Time>,
    mut state: ResMut<FlowInstrState>,
    q_sub: Query<(&Transform, &crate::scene::Velocity), With<crate::scene::Submarine>>,
) {
    let Ok((transform, vel)) = q_sub.single() else {
        // decay to center if no sub
        state.pos *= 1.0 - SMOOTH_ALPHA;
        state.surge *= 1.0 - SMOOTH_ALPHA;
        return;
    };

    let p = transform.translation;
    let v = **vel;
    // Sample flow at sub position
    let level = greybox_level();
    let (flow, _var) = sample_flow_at(&level, Vec3f { x: p.x, y: p.y, z: p.z }, time.elapsed_secs());
    let rel = Vec3::new(v.x - flow.x, v.y - flow.y, v.z - flow.z);
    // Longitudinal relative speed (body frame)
    
    // Body frame: rotate world -> local
    let rot_inv = transform.rotation.conjugate();
    let rel_body = rot_inv * rel;
    let u_rel = rel_body.x;
    // Incoming flow direction towards the nose
    let n = (-rel_body).normalize_or_zero();
    // Project to instrument plane using gnomonic-like mapping to keep center stable
    let denom = (n.x).abs().max(1e-3); // use |forward| to avoid sign flip when flow from rear; center when n ~ +X
    // We want center when n ~ +X (incoming at bow). If using denom = n.x, positive when forward.
    // Map: x = n.z/denom (right), y = n.y/denom (up)
    let mut dot = Vec2::new(n.z / denom, n.y / denom);
    if !dot.x.is_finite() || !dot.y.is_finite() { dot = Vec2::ZERO; }
    // Clamp to unit circle
    let mag = dot.length();
    if mag > 1.0 {
        dot /= mag;
    }

    // Smooth
    state.pos = state.pos.lerp(dot, SMOOTH_ALPHA);
    state.surge = state.surge + (u_rel - state.surge) * SMOOTH_ALPHA;
}

fn draw_flow_instr(
    state: Res<FlowInstrState>,
    mut q_root: Query<&GlobalTransform, With<FlowInstrRing>>,
    mut q_dot: Query<(&mut Node, &mut BackgroundColor), With<FlowInstrDot>>,
) {
    let Ok(_ring_xform) = q_root.single_mut() else { return; };
    let Ok((mut dot_node, mut dot_color)) = q_dot.single_mut() else { return; };

    // Compute pixel position inside the ring node
    // Get ring top-left and size in local coordinates is INSTR_SIZE
    // Node absolute left/top relative to ring's top-left
    let r = INSTR_SIZE * 0.5 - DOT_SIZE * 0.5 - RING_THICKNESS; // inner radius minus dot radius and border
    let center = Vec2::splat(INSTR_SIZE * 0.5 - DOT_SIZE * 0.5);
    let pos_px = center + state.pos * r;
    dot_node.left = Val::Px(pos_px.x);
    dot_node.top = Val::Px(INSTR_SIZE - DOT_SIZE - pos_px.y); // UI Y downwards

    // Color by relative speed
    // Color encoding: direction + magnitude of longitudinal flow
    // Backflow (from stern): deep magenta at high |u|, red near zero
    // Frontflow (from bow): red near zero → yellow → green at high |u|
    let mag = state.surge.abs();
    let t = (mag / MAX_REL_SPEED).clamp(0.0, 1.0);
    let color = if state.surge >= 0.0 {
        // front-coming: red -> yellow -> green
        if t < 0.5 {
            let k = t / 0.5;
            // red (1,0,0) to yellow (1,1,0) => (1, k, 0)
            Color::srgba(1.0, k, 0.0, 1.0)
        } else {
            let k = (t - 0.5) / 0.5;
            // yellow (1,1,0) to green (0,1,0) => (1-k, 1, 0)
            Color::srgba(1.0 - k, 1.0, 0.0, 1.0)
        }
    } else {
        // back-coming: red -> deep magenta
        let k = t;
        // red (1,0,0) to magenta (0.85,0,0.85) => (1 - 0.15k, 0, 0.85k)
        Color::srgba(1.0 - 0.15 * k, 0.0, 0.85 * k, 1.0)
    };
    *dot_color = BackgroundColor(color);
}

// -------------------- Ballast gauges --------------------

const GAUGE_H: f32 = 120.0; // px height of gauge interior
const GAUGE_W: f32 = 20.0;  // px width of each gauge
const GAUGE_GAP: f32 = 8.0; // gap between gauges

#[derive(Component)]
struct BallastHudRoot;

#[derive(Component)]
struct BallastFwdFill;

#[derive(Component)]
struct BallastAftFill;

#[derive(Component)]
struct BallastBuoyText;

fn spawn_ballast_hud(mut commands: Commands) {
    // Bottom-right container
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                right: Val::Px(24.0),
                width: Val::Px(GAUGE_W * 2.0 + GAUGE_GAP + 8.0),
                height: Val::Px(GAUGE_H + 40.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::End,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..Default::default()
            },
            BackgroundColor(Color::NONE),
            BallastHudRoot,
            Name::new("Ballast HUD Root"),
        ))
        .with_children(|root| {
            // Gauges row
            root
                .spawn((
                    Node {
                        width: Val::Px(GAUGE_W * 2.0 + GAUGE_GAP),
                        height: Val::Px(GAUGE_H),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::End,
                        ..Default::default()
                    },
                    BackgroundColor(Color::NONE),
                    Name::new("Ballast Gauges Row"),
                ))
                .with_children(|row| {
                    // FWD gauge
                    row
                        .spawn((
                            Node {
                                width: Val::Px(GAUGE_W),
                                height: Val::Px(GAUGE_H),
                                border: UiRect::all(Val::Px(RING_THICKNESS)),
                                align_items: AlignItems::End,
                                ..Default::default()
                            },
                            BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                            BackgroundColor(Color::NONE),
                            Name::new("Gauge FWD"),
                        ))
                        .with_children(|g| {
                            g.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(0.0), // updated at runtime
                                    ..Default::default()
                                },
                                BackgroundColor(Color::srgba(0.2, 0.8, 1.0, 0.9)),
                                BallastFwdFill,
                                Name::new("Gauge FWD Fill"),
                            ));
                        });

                    // AFT gauge
                    row
                        .spawn((
                            Node {
                                width: Val::Px(GAUGE_W),
                                height: Val::Px(GAUGE_H),
                                border: UiRect::all(Val::Px(RING_THICKNESS)),
                                align_items: AlignItems::End,
                                ..Default::default()
                            },
                            BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                            BackgroundColor(Color::NONE),
                            Name::new("Gauge AFT"),
                        ))
                        .with_children(|g| {
                            g.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(0.0), // updated at runtime
                                    ..Default::default()
                                },
                                BackgroundColor(Color::srgba(1.0, 0.6, 0.2, 0.9)),
                                BallastAftFill,
                                Name::new("Gauge AFT Fill"),
                            ));
                        });
                });

            // Buoyancy text
            root.spawn((
                Text::new(""),
                TextFont { font_size: 14.0, ..Default::default() },
                TextColor(Color::WHITE),
                BallastBuoyText,
                Name::new("Buoyancy Text"),
            ));
        });
}

fn update_ballast_hud(
    telemetry: Option<Res<crate::scene::SubTelemetry>>,
    mut q_fwd: Query<&mut Node, (With<BallastFwdFill>, Without<BallastAftFill>)>,
    mut q_aft: Query<&mut Node, (With<BallastAftFill>, Without<BallastFwdFill>)>,
    mut q_txt: Query<&mut Text, With<BallastBuoyText>>,
) {
    let Some(t) = telemetry else { return; };
    let d = &t.0;
    if let Ok(mut n) = q_fwd.single_mut() {
        n.height = Val::Px((d.fill_fwd.clamp(0.0, 1.0)) * GAUGE_H);
    }
    if let Ok(mut n) = q_aft.single_mut() {
        n.height = Val::Px((d.fill_aft.clamp(0.0, 1.0)) * GAUGE_H);
    }
    if let Ok(mut txt) = q_txt.single_mut() {
        txt.0 = format!("Buoyancy: net {:>7.1} N", d.buoy_net_n);
    }
}
