use bevy::prelude::*;

const GAUGE_H: f32 = 120.0; // px height of gauge interior
const GAUGE_W: f32 = 20.0; // px width of each gauge
const GAUGE_GAP: f32 = 8.0; // gap between gauges
const BORDER_THICKNESS: f32 = 2.0; // px

#[derive(Component)]
pub(super) struct BallastHudRoot;

#[derive(Component)]
pub(super) struct BallastFwdFill;

#[derive(Component)]
pub(super) struct BallastAftFill;

#[derive(Component)]
pub(super) struct BallastBuoyText;

pub(super) fn spawn_ballast_hud(mut commands: Commands) {
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
            root.spawn((
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
                row.spawn((
                    Node {
                        width: Val::Px(GAUGE_W),
                        height: Val::Px(GAUGE_H),
                        border: UiRect::all(Val::Px(BORDER_THICKNESS)),
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
                row.spawn((
                    Node {
                        width: Val::Px(GAUGE_W),
                        height: Val::Px(GAUGE_H),
                        border: UiRect::all(Val::Px(BORDER_THICKNESS)),
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
                TextFont {
                    font_size: 14.0,
                    ..Default::default()
                },
                TextColor(Color::WHITE),
                BallastBuoyText,
                Name::new("Buoyancy Text"),
            ));
        });
}

pub(super) fn update_ballast_hud(
    telemetry: Option<Res<crate::scene::submarine::SubTelemetry>>,
    mut q_fwd: Query<&mut Node, (With<BallastFwdFill>, Without<BallastAftFill>)>,
    mut q_aft: Query<&mut Node, (With<BallastAftFill>, Without<BallastFwdFill>)>,
    mut q_txt: Query<&mut Text, With<BallastBuoyText>>,
) {
    let Some(t) = telemetry else {
        return;
    };
    let d = &t.0;
    let fwd = if d.fill_fwd.is_finite() {
        d.fill_fwd.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let aft = if d.fill_aft.is_finite() {
        d.fill_aft.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if let Ok(mut n) = q_fwd.single_mut() {
        n.height = Val::Px(fwd * GAUGE_H);
    }
    if let Ok(mut n) = q_aft.single_mut() {
        n.height = Val::Px(aft * GAUGE_H);
    }
    if let Ok(mut txt) = q_txt.single_mut() {
        let b = if d.buoy_net_n.is_finite() {
            d.buoy_net_n
        } else {
            0.0
        };
        txt.0 = format!("Buoyancy: net {:>7.1} N", b);
    }
}
