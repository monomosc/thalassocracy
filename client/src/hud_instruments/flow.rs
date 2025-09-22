use bevy::prelude::*;
use levels::{builtins::greybox_level, sample_flow_at, Vec3f};

const INSTR_SIZE: f32 = 140.0; // px
const RING_THICKNESS: f32 = 2.0; // px
const DOT_SIZE: f32 = 12.0; // px
const SMOOTH_ALPHA: f32 = 0.2; // EMA for dot position/color

#[derive(Component)]
pub(super) struct FlowInstrRoot;

#[derive(Component)]
pub(super) struct FlowInstrRing;

#[derive(Component)]
pub(super) struct FlowInstrDot;

#[derive(Component)]
pub(super) struct FlowInstrSpeedText;

#[derive(Component, Default, Clone, Copy)]
pub struct HudInstrumentState {
    pub(crate) pos: Vec2,
    /// Longitudinal water-relative speed along body +Z (surge); >0 = coming from front, <0 = from back
    pub(crate) surge: f32,
    /// Magnitude of water-relative speed (m/s)
    pub(crate) speed: f32,
}

pub(super) fn spawn_flow_instr(mut commands: Commands) {
    // Bottom-center overlay container
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(INSTR_SIZE + 28.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..Default::default()
            },
            BackgroundColor(Color::NONE),
            FlowInstrRoot,
            Name::new("Flow Instrument Root"),
        ))
        .with_children(|root| {
            // Speed text above ring
            root.spawn((
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..Default::default()
                },
                TextColor(Color::WHITE),
                FlowInstrSpeedText,
                Name::new("Flow Instrument Speed Text"),
            ));
            // Ring
            root.spawn((
                Node {
                    width: Val::Px(INSTR_SIZE),
                    height: Val::Px(INSTR_SIZE),
                    border: UiRect::all(Val::Px(RING_THICKNESS)),
                    ..Default::default()
                },
                BackgroundColor(Color::NONE),
                BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                // Make the ring circular
                BorderRadius::all(Val::Px(INSTR_SIZE * 0.5)),
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
                    BackgroundColor(Color::WHITE),
                    // Make the dot circular
                    BorderRadius::all(Val::Px(DOT_SIZE * 0.5)),
                    FlowInstrDot,
                    Name::new("Flow Instrument Dot"),
                ));
            });
        });
}

pub(super) fn update_hud_instr_state(
    time: Res<Time>,
    mut q: Query<
        (
            &crate::scene::submarine::SubStateComp,
            &mut HudInstrumentState,
        ),
        With<crate::scene::submarine::Submarine>,
    >,
) {
    let Ok((state_comp, mut hud)) = q.single_mut() else {
        return;
    };

    let s = &state_comp.0;
    // Sample flow at sub position
    let level = greybox_level();
    let (flow, _var) = sample_flow_at(
        &level,
        Vec3f {
            x: s.position.x,
            y: s.position.y,
            z: s.position.z,
        },
        time.elapsed_secs(),
    );
    let rel = Vec3::new(
        s.velocity.x - flow.x,
        s.velocity.y - flow.y,
        s.velocity.z - flow.z,
    );
    // Convert world->body using body's orientation
    let rel_body = s.orientation.conjugate() * rel;
    // Longitudinal relative speed (+Z forward)
    let u_rel = rel_body.z;
    // Incoming flow direction towards the nose
    let n = (-rel_body).normalize_or_zero();
    // Project to instrument plane using gnomonic-like mapping to keep center stable
    let denom = (n.z).abs().max(1e-3);
    let mut dot = Vec2::new(n.x / denom, n.y / denom);
    if !dot.x.is_finite() || !dot.y.is_finite() {
        dot = Vec2::ZERO;
    }
    let mag = dot.length();
    if mag > 1.0 {
        dot /= mag;
    }

    // Smooth into HUD state
    hud.pos = hud.pos.lerp(dot, SMOOTH_ALPHA);
    hud.surge = hud.surge + (u_rel - hud.surge) * SMOOTH_ALPHA;
    let speed_mag = rel.length();
    hud.speed = hud.speed + (speed_mag - hud.speed) * SMOOTH_ALPHA;
}

pub(super) fn draw_flow_instr(
    q_hud: Query<&HudInstrumentState, With<crate::scene::submarine::Submarine>>,
    mut q_root: Query<&GlobalTransform, With<FlowInstrRing>>,
    mut q_dot: Query<(&mut Node, &mut BackgroundColor), With<FlowInstrDot>>,
    mut q_speed: Query<&mut Text, With<FlowInstrSpeedText>>,
    q_spec: Query<&crate::scene::submarine::SubPhysics, With<crate::scene::submarine::Submarine>>,
) {
    let Ok(_ring_xform) = q_root.single_mut() else {
        return;
    };
    let Ok((mut dot_node, mut dot_color)) = q_dot.single_mut() else {
        return;
    };
    let Ok(state) = q_hud.single() else {
        return;
    };

    // Compute pixel position inside the ring node
    let r = INSTR_SIZE * 0.5 - DOT_SIZE * 0.5 - RING_THICKNESS; // inner radius minus dot radius and border
    let center = Vec2::splat(INSTR_SIZE * 0.5 - DOT_SIZE * 0.5);
    let pos_px = center + state.pos * r;
    dot_node.left = Val::Px(pos_px.x);
    dot_node.top = Val::Px(INSTR_SIZE - DOT_SIZE - pos_px.y); // UI Y downwards

    // Update speed text (absolute incoming water speed)
    if let Ok(mut t) = q_speed.single_mut() {
        t.0 = format!("{:.2} m/s", state.speed.abs());
    }

    // Color by relative speed from physics-derived thresholds
    // Compute terminal surge speed from spec: solve 0.5*rho*cxd*A*u^2 + xu*u = t_max
    let u_term = if let Some(spec) = q_spec.iter().next() {
        let rho = 1025.0_f32; // seawater kg/m^3 (matches physics)
        let a = 0.5 * rho * spec.0.cxd * spec.0.s_forward;
        let b = spec.0.xu;
        let t_max = spec.0.t_max.max(0.0);
        if a > 1e-6 {
            let disc = b * b + 4.0 * a * t_max;
            ((-b) + disc.sqrt()) / (2.0 * a)
        } else if b > 1e-6 {
            // Fallback: purely linear drag
            t_max / b
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Thresholds: green at 2/3 Vmax; stay green until 3/2 Vmax; then shift to blue.
    let u_green = (2.0 / 3.0) * u_term.max(0.0);
    let u_blue_start = (3.0 / 2.0) * u_term.max(0.0);

    let mag = state.surge.abs();
    let color = if state.surge < 0.0 {
        // Back-coming flow: magenta at strong backflow, red as it approaches zero
        // Interpolate red (1,0,0) → magenta (1,0,1) using u_green as normalization
        let k = if u_green > 1e-6 {
            (mag / u_green).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Color::srgba(1.0, 0.0, k, 1.0)
    } else {
        // Front-coming flow
        if u_term <= 0.0 {
            // No spec available: degrade gracefully red→yellow→green by a soft scaler
            let t = (mag / 3.0).clamp(0.0, 1.0);
            if t < 0.5 {
                let k = t / 0.5; // red → yellow
                Color::srgba(1.0, k, 0.0, 1.0)
            } else {
                let k = (t - 0.5) / 0.5; // yellow → green
                Color::srgba(1.0 - k, 1.0, 0.0, 1.0)
            }
        } else if mag <= u_green {
            // Red → yellow → green up to 2/3 Vmax
            let t = (mag / u_green).clamp(0.0, 1.0);
            if t < 0.5 {
                let k = t / 0.5; // red → yellow
                Color::srgba(1.0, k, 0.0, 1.0)
            } else {
                let k = (t - 0.5) / 0.5; // yellow → green
                Color::srgba(1.0 - k, 1.0, 0.0, 1.0)
            }
        } else if mag <= u_blue_start {
            // Hold green between 2/3 and 3/2 Vmax
            Color::srgba(0.0, 1.0, 0.0, 1.0)
        } else {
            // Beyond 3/2 Vmax: move towards blue. Ramp to blue by 2.5× Vmax.
            let u_blue_full = (2.5_f32) * u_term;
            let denom = (u_blue_full - u_blue_start).max(1e-3);
            let k = ((mag - u_blue_start) / denom).clamp(0.0, 1.0); // 0 at start, 1 at full
                                                                    // green (0,1,0) → blue (0,0,1)
            Color::srgba(0.0, 1.0 - k, k, 1.0)
        }
    };
    *dot_color = BackgroundColor(color);
}
