use bevy::prelude::*;

pub mod ballast;
pub mod flow;

pub use flow::HudInstrumentState;

pub struct HudInstrumentsPlugin;

impl Plugin for HudInstrumentsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (flow::spawn_flow_instr, ballast::spawn_ballast_hud),
        )
        .add_systems(
            Update,
            (
                sanitize_ui_nodes,
                flow::update_hud_instr_state,
                flow::draw_flow_instr,
                ballast::update_ballast_hud,
            ),
        );
    }
}

// Best-effort guard against NaN values slipping into UI nodes which can panic inside bevy_ui.
fn sanitize_ui_nodes(mut q: Query<(Entity, &mut Node, Option<&Name>)>) {
    fn fix(label: &'static str, v: &mut Val, dirty: &mut Vec<&'static str>) {
        match v {
            Val::Px(x) => {
                if !x.is_finite() || *x > 1.0e7 || *x < -1.0e7 {
                    *x = 0.0;
                    dirty.push(label);
                }
            }
            Val::Percent(p) => {
                if !p.is_finite() || *p > 1.0e6 || *p < -1.0e6 {
                    *v = Val::Px(0.0);
                    dirty.push(label);
                }
            }
            _ => {}
        }
    }
    for (e, mut n, name) in &mut q {
        let mut dirty: Vec<&'static str> = Vec::new();
        fix("width", &mut n.width, &mut dirty);
        fix("height", &mut n.height, &mut dirty);
        fix("left", &mut n.left, &mut dirty);
        fix("right", &mut n.right, &mut dirty);
        fix("top", &mut n.top, &mut dirty);
        fix("bottom", &mut n.bottom, &mut dirty);
        fix("row_gap", &mut n.row_gap, &mut dirty);
        fix("column_gap", &mut n.column_gap, &mut dirty);
        let mut m = n.margin;
        fix("margin.left", &mut m.left, &mut dirty);
        fix("margin.right", &mut m.right, &mut dirty);
        fix("margin.top", &mut m.top, &mut dirty);
        fix("margin.bottom", &mut m.bottom, &mut dirty);
        n.margin = m;
        let mut p = n.padding;
        fix("padding.left", &mut p.left, &mut dirty);
        fix("padding.right", &mut p.right, &mut dirty);
        fix("padding.top", &mut p.top, &mut dirty);
        fix("padding.bottom", &mut p.bottom, &mut dirty);
        n.padding = p;
        let mut b = n.border;
        fix("border.left", &mut b.left, &mut dirty);
        fix("border.right", &mut b.right, &mut dirty);
        fix("border.top", &mut b.top, &mut dirty);
        fix("border.bottom", &mut b.bottom, &mut dirty);
        n.border = b;

        if !dirty.is_empty() {
            let label = name
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("Entity#{e:?}"));
            tracing::warn!(target: "ui_sanitize", node=%label, fields=?dirty, "Sanitized non-finite UI values");
        }
    }
}
