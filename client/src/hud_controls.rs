use crate::net::{ConnectStart, TimeSync};
use crate::sim_pause::SimPause;
use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use bevy_inspector_egui::bevy_egui::EguiContexts;
use bevy_renet::renet::{DefaultChannel, RenetClient};

#[derive(Resource, Debug)]
pub struct ThrustInput {
    pub value: f32, // -1.0 .. 1.0 (forward/back)
    /// Rudder in [-1,1]. Convention: +1 = right rudder (nose right under forward motion).
    pub yaw: f32, // -1.0 .. 1.0 (right rudder positive)
    /// Forward ballast pump speed in [-1,1]. +1 pumps water in, -1 pumps out.
    pub pump_fwd: f32,
    /// Aft ballast pump speed in [-1,1]. +1 pumps water in, -1 pumps out.
    pub pump_aft: f32,
    pub tick: u64,
}

impl Default for ThrustInput {
    fn default() -> Self {
        Self {
            value: 0.0,
            yaw: 0.0,
            pump_fwd: 0.0,
            pump_aft: 0.0,
            tick: 0,
        }
    }
}

pub struct HudControlsPlugin;

impl Plugin for HudControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ThrustInput>()
            // Ensure the egui UI runs between BeginPass (PreUpdate) and EndPass (PostUpdate)
            .add_systems(EguiPrimaryContextPass, ui_thrust_slider)
            .add_systems(Update, (send_thrust_input, send_pause_request));
    }
}

fn ui_thrust_slider(
    mut egui_ctx: EguiContexts,
    mut thrust: ResMut<ThrustInput>,
    mut paused: ResMut<SimPause>,
) {
    use bevy_inspector_egui::egui::*;
    let Ok(ctx) = egui_ctx.ctx_mut() else {
        return;
    };

    SidePanel::left("thrust_panel")
        .exact_width(90.0)
        .show(ctx, |ui| {
            ui.heading("Controls");
            ui.add_space(8.0);

            // Pause toggle (client + server via network message)
            let mut p = paused.0;
            if ui.checkbox(&mut p, "Pause").clicked() {
                paused.0 = p;
            }
            ui.add_space(8.0);

            ui.label("Thrust");
            let mut v = thrust.value;
            let slider = Slider::new(&mut v, -1.0..=1.0)
                .vertical()
                .clamping(SliderClamping::Always);
            ui.add(slider);
            if (v - thrust.value).abs() > f32::EPSILON {
                thrust.value = v;
            }
            ui.add_space(6.0);

            ui.label("Rudder (+ = right)");
            let mut r = thrust.yaw;
            let slider_r = Slider::new(&mut r, -1.0..=1.0)
                .vertical()
                .clamping(SliderClamping::Always);
            ui.add(slider_r);
            if (r - thrust.yaw).abs() > f32::EPSILON {
                thrust.yaw = r;
            }

            ui.add_space(6.0);

            ui.separator();
            ui.label("Pump FWD (+in)");
            let mut pf = thrust.pump_fwd;
            let slider_pf = Slider::new(&mut pf, -1.0..=1.0)
                .vertical()
                .clamping(SliderClamping::Always);
            ui.add(slider_pf);
            if (pf - thrust.pump_fwd).abs() > f32::EPSILON {
                thrust.pump_fwd = pf;
            }

            ui.label("Pump AFT (+in)");
            let mut pa = thrust.pump_aft;
            let slider_pa = Slider::new(&mut pa, -1.0..=1.0)
                .vertical()
                .clamping(SliderClamping::Always);
            ui.add(slider_pa);
            if (pa - thrust.pump_aft).abs() > f32::EPSILON {
                thrust.pump_aft = pa;
            }

            ui.add_space(6.0);
            ui.monospace(format!(
                "T {:.2} | R {:.2}\nPF {:.2} | PA {:.2}",
                thrust.value, thrust.yaw, thrust.pump_fwd, thrust.pump_aft
            ));
        });
}

fn send_pause_request(
    client: Option<ResMut<RenetClient>>,
    paused: Res<SimPause>,
    mut last: Local<Option<bool>>,
) {
    let Some(mut client) = client else {
        return;
    };
    if !client.is_connected() {
        return;
    }
    let cur = paused.0;
    if last.map(|v| v == cur).unwrap_or(false) {
        return;
    }
    let msg = protocol::ClientToServer::PauseRequest(protocol::PauseRequest { paused: cur });
    if let Ok(bytes) = protocol::encode(&msg) {
        client.send_message(DefaultChannel::ReliableOrdered, bytes);
    }
    *last = Some(cur);
}

fn send_thrust_input(
    client: Option<ResMut<RenetClient>>,
    mut thrust: ResMut<ThrustInput>,
    connect: Option<Res<ConnectStart>>,
    tsync: Option<Res<TimeSync>>,
) {
    let Some(mut client) = client else {
        return;
    };
    // For now, send every frame if connected. Later: send on change or at a fixed input rate.
    if !client.is_connected() {
        return;
    }
    thrust.tick = thrust.tick.wrapping_add(1);
    // Compute server-time stamped event scheduled slightly ahead (30 ms) to reduce timing disagreement
    let ahead_ms: u64 = 30;
    if let (Some(connect), Some(tsync)) = (connect, tsync) {
        let local_ms = connect.at.elapsed().as_millis() as u64;
        let server_now_ms = (local_ms as i64 + tsync.offset_ms as i64).max(0) as u64;
        let t_ms = server_now_ms + ahead_ms;
        let ev = protocol::InputEvent {
            t_ms,
            thrust: thrust.value,
            yaw: thrust.yaw,
            pump_fwd: thrust.pump_fwd,
            pump_aft: thrust.pump_aft,
        };
        let msg = protocol::ClientToServer::InputEvent(ev);
        if let Ok(bytes) = protocol::encode(&msg) {
            client.send_message(DefaultChannel::ReliableOrdered, bytes);
        }
    } else {
        // Fallback: send legacy tick message
        let msg = protocol::ClientToServer::InputTick(protocol::InputTick {
            tick: thrust.tick,
            thrust: thrust.value,
            yaw: thrust.yaw,
            pump_fwd: thrust.pump_fwd,
            pump_aft: thrust.pump_aft,
        });
        if let Ok(bytes) = protocol::encode(&msg) {
            client.send_message(DefaultChannel::ReliableOrdered, bytes);
        }
    }
}
