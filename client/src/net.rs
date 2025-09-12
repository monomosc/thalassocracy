use bevy::prelude::*;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::renet::{ConnectionConfig, DefaultChannel, RenetClient};
use std::net::UdpSocket;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use crate::desync_metrics::NetClientStats;
use crate::scene::submarine::ClientPhysicsTiming;
use crate::scene::submarine::{NetControlled, ServerCorrection, Submarine, Velocity};

use crate::Args;
use protocol::{ClientToServer, ClientHello, ServerToClient, StateDelta, PROTOCOL_VERSION, NETCODE_PROTOCOL_ID};

#[derive(Resource, Default)]
pub struct HelloSent(pub bool);

#[derive(Resource)]
pub struct ConnectStart {
    pub at: Instant,
    pub timeout: Duration,
}

#[derive(Resource, Default)]
pub struct MyPlayerId(pub Option<uuid::Uuid>);

#[derive(Resource, Default)]
pub struct LatestStateDelta(pub Option<StateDelta>);

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetSet;

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct TimeSync { pub offset_ms: f32, pub last_server_ms: u64 }

#[derive(Resource, Debug, Clone, Copy)]
pub struct FilteredServerState {
    pub initialized: bool,
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
}

impl Default for FilteredServerState {
    fn default() -> Self {
        Self { initialized: false, pos: Vec3::ZERO, rot: Quat::IDENTITY, vel: Vec3::ZERO }
    }
}

pub fn client_connect(mut commands: Commands, args: Res<Args>) {
    let server_addr: std::net::SocketAddr = args.server.parse().expect("invalid server addr");

    // Unsecure prototype setup
    let client = RenetClient::new(ConnectionConfig::default());
    // Generate a non-zero client id (derive from UUID bytes for simplicity)
    let uuid = uuid::Uuid::new_v4();
    let bytes = uuid.as_bytes();
    let client_id = u64::from_le_bytes(bytes[0..8].try_into().expect("uuid slice to u64"));
    let auth = ClientAuthentication::Unsecure { protocol_id: NETCODE_PROTOCOL_ID, client_id, server_addr, user_data: None };
    let socket = UdpSocket::bind(("0.0.0.0", 0)).expect("failed to bind UDP socket");
    let transport = NetcodeClientTransport::new(SystemTime::now().duration_since(UNIX_EPOCH).unwrap(), auth, socket)
        .expect("failed to create client transport");

    commands.insert_resource(client);
    commands.insert_resource(transport);
    commands.insert_resource(ConnectStart { at: Instant::now(), timeout: Duration::from_secs(args.connect_timeout_secs) });
    commands.init_resource::<TimeSync>();
    commands.init_resource::<FilteredServerState>();

    info!(?server_addr, "Client created and connecting");
}

#[allow(clippy::too_many_arguments)]
pub fn pump_network(
    client: Option<ResMut<RenetClient>>,
    args: Res<Args>,
    mut hello_sent: ResMut<HelloSent>,
    mut my_id: ResMut<MyPlayerId>,
    mut latest: ResMut<LatestStateDelta>,
    mut paused: ResMut<crate::sim_pause::SimPause>,
    mut net_stats: ResMut<NetClientStats>,
    mut client_tick: ResMut<ClientPhysicsTiming>,
) {
    let Some(mut client) = client else { return; };

    // Send Hello once after connection established
    if client.is_connected() && !hello_sent.0 {
        let hello = ClientToServer::Hello(ClientHello { protocol: PROTOCOL_VERSION, display_name: args.name.clone() });
        if let Ok(bytes) = protocol::encode(&hello) {
            client.send_message(DefaultChannel::ReliableOrdered, bytes);
        }
        hello_sent.0 = true;
    }

    // Read reliable messages
    while let Some(bytes) = client.receive_message(DefaultChannel::ReliableOrdered) {
        match protocol::decode::<ServerToClient>(bytes.as_ref()) {
            Ok(ServerToClient::JoinAck(ack)) => {
                info!(player_id = ?ack.player_id, "Received JoinAck");
                my_id.0 = Some(ack.player_id);
                // Configure client fixed-step dt from server tick rate
                let hz = ack.tick_hz.max(1) as f32;
                client_tick.dt = 1.0 / hz;
                info!(tick_hz = ack.tick_hz, dt = client_tick.dt, "Configured client fixed-step dt");
            }
            Ok(ServerToClient::StateDelta(delta)) => {
                // For compatibility in case server still sends reliable.
                let latest_tick = latest.0.as_ref().map(|d| d.tick).unwrap_or(0);
                if delta.tick > latest_tick {
                    latest.0 = Some(delta);
                    let now = Instant::now();
                    if let Some(prev) = net_stats.last_state_instant {
                        let dt_ms = now.saturating_duration_since(prev).as_secs_f32() * 1000.0;
                        let alpha = 0.2_f32;
                        net_stats.inter_arrival_ewma_ms = if net_stats.inter_arrival_ewma_ms == 0.0 {
                            dt_ms
                        } else {
                            net_stats.inter_arrival_ewma_ms + alpha * (dt_ms - net_stats.inter_arrival_ewma_ms)
                        };
                    }
                    net_stats.last_state_instant = Some(now);
                    // Time sync handled in apply_state_to_sub where ConnectStart is available
                    net_stats.last_server_tick = latest.0.as_ref().map(|d| d.tick);
                }
            }
            Ok(ServerToClient::PauseState(state)) => {
                paused.0 = state.paused;
            }
            Ok(ServerToClient::InputAck(ack)) => {
                net_stats.last_acked_tick = Some(ack.tick);
            }
            Ok(other) => {
                warn!(?other, "Unhandled server message");
            }
            Err(err) => warn!(?err, "Failed to decode server message"),
        }
    }

    // Read unreliable messages (snapshots)
    while let Some(bytes) = client.receive_message(DefaultChannel::Unreliable) {
        match protocol::decode::<ServerToClient>(bytes.as_ref()) {
            Ok(ServerToClient::StateDelta(delta)) => {
                let latest_tick = latest.0.as_ref().map(|d| d.tick).unwrap_or(0);
                if delta.tick > latest_tick {
                    latest.0 = Some(delta);
                    let now = Instant::now();
                    if let Some(prev) = net_stats.last_state_instant {
                        let dt_ms = now.saturating_duration_since(prev).as_secs_f32() * 1000.0;
                        let alpha = 0.2_f32;
                        net_stats.inter_arrival_ewma_ms = if net_stats.inter_arrival_ewma_ms == 0.0 {
                            dt_ms
                        } else {
                            net_stats.inter_arrival_ewma_ms + alpha * (dt_ms - net_stats.inter_arrival_ewma_ms)
                        };
                    }
                    net_stats.last_state_instant = Some(now);
                    // Update time sync (simple): offset = server_ms - local_ms
                    if let Some(ref _d) = latest.0 {
                        // Use ConnectStart.at as local epoch
                        // We don't have it here; will update in apply_state_to_sub where we have `time` resource
                    }
                    net_stats.last_server_tick = latest.0.as_ref().map(|d| d.tick);
                }
            }
            Ok(other) => {
                // Ignore other kinds on unreliable for now.
                warn!(?other, "Unhandled unreliable server message");
            }
            Err(err) => warn!(?err, "Failed to decode unreliable server message"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_state_to_sub(
    my_id: Res<MyPlayerId>,
    latest: Res<LatestStateDelta>,
    mut commands: Commands,
    mut q_sub: Query<(Entity, &mut Transform, &mut Velocity, Option<&mut ServerCorrection>), With<Submarine>>,
    mut net_stats: ResMut<NetClientStats>,
    controls: Option<Res<crate::hud_controls::ThrustInput>>,
    time: Res<Time>,
    mut filtered: ResMut<FilteredServerState>,
    connect: Option<Res<ConnectStart>>,
    mut tsync: ResMut<TimeSync>,
) {
    let Some(my_id) = my_id.0 else { return; };
    let Some(delta) = latest.0.as_ref() else { return; };
    let Some(me) = delta.players.iter().find(|p| p.id == my_id) else { return; };
    if let Ok((entity, mut t, mut v, corr_opt)) = q_sub.single_mut() {
        // Update time sync from delta.server_ms vs local monotonic
        if let Some(connect) = connect {
            let local_ms = connect.at.elapsed().as_millis() as u64;
            if let Some(d) = latest.0.as_ref() {
                let sample = d.server_ms as i64 - local_ms as i64;
                let alpha = 0.1_f32;
                let s = sample as f32;
                tsync.offset_ms = tsync.offset_ms + alpha * (s - tsync.offset_ms);
                tsync.last_server_ms = d.server_ms;
            }
        }
        // Ensure network-driven marker present
        commands.entity(entity).insert(NetControlled);
        let target_pos_raw = Vec3::new(me.position[0], me.position[1], me.position[2]);
        // Prefer full orientation from server if present
        let target_rot_raw = {
            let o = me.orientation;
            Quat::from_xyzw(o[0], o[1], o[2], o[3])
        };
        // Convert physics (body +Z forward) to mesh (visual +X forward): apply -90° yaw
        let target_rot = target_rot_raw * Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
        let target_vel_raw = Vec3::new(me.velocity[0], me.velocity[1], me.velocity[2]);

        // Initialize or low-pass filter the authoritative target to remove HF jitter
        if !filtered.initialized {
            filtered.pos = target_pos_raw;
            // Initialize in the same frame (mesh space) used for comparisons/corrections
            filtered.rot = target_rot;
            filtered.vel = target_vel_raw;
            filtered.initialized = true;
        } else {
            let dt = time.delta_secs().max(1e-3);
            // Adapt smoothing: track server more tightly while the player is steering
            let yaw_in_mag = controls.as_ref().map(|c| c.yaw.abs()).unwrap_or(0.0);
            let tau = if yaw_in_mag > 0.05 { 0.035 } else { 0.10 }; // ~35ms when steering, 100ms otherwise
            let alpha = 1.0 - (-dt / tau).exp();
            filtered.pos = filtered.pos.lerp(target_pos_raw, alpha);
            filtered.rot = filtered.rot.slerp(target_rot, alpha);
            // Blend velocity toward server but also consider current sim velocity to avoid buzz
            filtered.vel = filtered.vel.lerp(target_vel_raw, alpha);
        }

        let target_pos = filtered.pos;
        let target_rot = filtered.rot;
        let target_vel = filtered.vel;

        // If the error is huge (teleport), snap immediately; otherwise smooth via ServerCorrection
        let raw_pos_err = t.translation.distance(target_pos_raw);
        let raw_ang_err = t.rotation.angle_between(target_rot);
        let pos_err = t.translation.distance(target_pos);
        let ang_err = t.rotation.angle_between(target_rot);
        let snap_now = raw_pos_err > 10.0 || raw_ang_err > 1.0;
        let vel_err = (**v - target_vel).length();
        let yaw_in = controls.as_ref().map(|c| c.yaw.abs()).unwrap_or(0.0);
        let steering = yaw_in > 0.05;
        // Hysteresis thresholds: tighter for removal, looser for insertion; looser still when steering
        let tiny_pos = if steering { 0.08 } else { 0.04 };
        let tiny_ang = if steering { 0.05 } else { 0.03 };
        let tiny_vel = if steering { 0.08 } else { 0.04 };
        let tiny = pos_err < tiny_pos && ang_err < tiny_ang && vel_err < tiny_vel;

        let enter_pos = if steering { 0.20 } else { 0.08 };
        let enter_ang = if steering { 0.10 } else { 0.05 };
        let enter_vel = if steering { 0.20 } else { 0.08 };
        let need_corr = pos_err > enter_pos || ang_err > enter_ang || vel_err > enter_vel;
        if snap_now {
            t.translation = target_pos;
            t.rotation = target_rot;
            **v = target_vel;
            commands.entity(entity).remove::<ServerCorrection>();
            // Record the magnitude of snap for the desync indicator
            net_stats.last_snap_magnitude_m = raw_pos_err;

        } else if tiny {
            // Avoid micro-corrections. Drop any existing correction and gently align velocity.
            if corr_opt.is_some() {
                commands.entity(entity).remove::<ServerCorrection>();
            }
            **v = target_vel;
        } else if let Some(mut corr) = corr_opt {
            // Update targets in place to avoid restarting smoothing.
            corr.target_pos = target_pos;
            corr.target_rot = target_rot;
            corr.target_vel = target_vel;
            // If the existing correction is near its end, keep some time to finish the new target.
            if corr.elapsed > 0.2 { corr.elapsed = 0.2; }
        } else if need_corr {
            commands.entity(entity).insert(ServerCorrection {
                target_pos,
                target_rot,
                target_vel,
                elapsed: 0.0,
                duration: 0.25,
            });
        } else {
            // Between tiny and need_corr: do nothing (no new correction), leave state to client sim
        }
    }
}

pub fn crash_on_disconnect(transport: Option<Res<NetcodeClientTransport>>) {
    if let Some(transport) = transport {
        if let Some(reason) = transport.disconnect_reason() {
            eprintln!("Network disconnect: {reason:?}. Exiting.");
            std::process::exit(1);
        }
    }
}

pub fn enforce_connect_timeout(client: Option<Res<RenetClient>>, start: Option<Res<ConnectStart>>) {
    let (Some(client), Some(start)) = (client, start) else { return; };
    if !client.is_connected() && start.at.elapsed() >= start.timeout {
        eprintln!(
            "Connection timeout after {}s without establishing a session. Exiting.",
            start.timeout.as_secs()
        );
        std::process::exit(1);
    }
}
