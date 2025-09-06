use std::net::UdpSocket;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use bevy::prelude::*;
use bevy_renet::{
    netcode::{NetcodeServerPlugin, NetcodeServerTransport, ServerAuthentication, ServerConfig},
    renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent},
    RenetServerPlugin,
};
use clap::Parser;
use protocol::{ClientToServer, DisconnectReason, ServerToClient, PROTOCOL_VERSION, NETCODE_PROTOCOL_ID};
use levels::{builtins::greybox_level, LevelSpec, SubState, SubInputs, step_submarine, Vec3f, Quatf};
use levels::subspecs::small_skiff_spec;
use levels::SubPhysicsSpec;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Parser, Debug, Resource)]
#[command(name = "thalassocracy-server")] 
#[command(about = "Server for Thalassocracy prototype", long_about = None)]
struct Args {
    /// Path to config file
    #[arg(long, default_value = "server/config.toml")]
    config: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Resource)]
struct Config {
    #[serde(default = "default_port")] 
    port: u16,
    #[serde(default = "default_max_clients")] 
    max_clients: usize,
    #[serde(default = "default_tick_hz")] 
    tick_hz: u32,
    #[serde(default = "default_snapshot_hz")] 
    snapshot_hz: u32,
    /// Optional public address to advertise in netcode tokens
    #[serde(default)]
    public_addr: Option<String>,
}

fn default_port() -> u16 { 61234 }
fn default_max_clients() -> usize { 64 }
fn default_tick_hz() -> u32 { 30 }
fn default_snapshot_hz() -> u32 { 20 }

fn load_config(path: &PathBuf) -> Result<Config> {
    if std::fs::metadata(path).is_ok() {
        let s = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&s)?)
    } else {
        Ok(Config {
            port: default_port(),
            max_clients: default_max_clients(),
            tick_hz: default_tick_hz(),
            snapshot_hz: default_snapshot_hz(),
            public_addr: None,
        })
    }
}

fn main() -> Result<()> {
    // logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let cfg = load_config(&args.config)?;
    info!(?cfg, "Server config loaded");

    let mut app = App::new();
    app.insert_resource(args)
        .insert_resource(cfg)
        .add_plugins(MinimalPlugins)
        .add_plugins((RenetServerPlugin, NetcodeServerPlugin))
        .add_systems(Startup, server_setup)
        .add_systems(Update, (
            server_handle_events,
            server_handle_messages,
            server_physics_tick,
            server_broadcast_state,
        ));

    app.run();
    Ok(())
}

#[derive(Resource)]
struct LevelRes(pub LevelSpec);

#[derive(Resource, Default)]
struct ClientEntities(pub HashMap<u64, Entity>);

#[derive(Component)]
struct Player { id: Uuid }

#[derive(Component)]
struct Submarine;

#[derive(Component)]
struct SubStateComp(pub SubState);

#[allow(dead_code)]
#[derive(Component, Clone)]
struct SubPhysicsComp(pub SubPhysicsSpec);

#[derive(Resource)]
struct PhysicsTiming { acc: f32, dt: f32 }

#[derive(Resource)]
struct SnapshotTiming { acc: f32, dt: f32 }

#[derive(Resource)]
struct Tick(pub u64);

#[derive(Resource, Default)]
struct SimPaused(pub bool);

#[allow(dead_code)]
#[derive(Component, Default)]
struct ControlInputComp { thrust: f32, yaw: f32, pump_fwd: f32, pump_aft: f32, last_tick: u64 }

fn server_setup(mut commands: Commands, cfg: Res<Config>) {
    // Bind UDP socket
    let socket = UdpSocket::bind(("0.0.0.0", cfg.port)).expect("failed to bind UDP socket");

    // Load/shared level spec
    let level_spec = greybox_level();
    commands.insert_resource(LevelRes(level_spec));

    // Timings
    let physics_dt = 1.0 / cfg.tick_hz.max(1) as f32;
    let snapshot_dt = 1.0 / cfg.snapshot_hz.max(1) as f32;
    commands.insert_resource(PhysicsTiming { acc: 0.0, dt: physics_dt });
    commands.insert_resource(SnapshotTiming { acc: 0.0, dt: snapshot_dt });
    commands.insert_resource(Tick(0));
    commands.insert_resource(ClientEntities::default());
    commands.insert_resource(SimPaused(false));

    // Netcode transport (renet)
    let bound_addr = socket.local_addr().expect("udp local_addr");
    let public_addr: std::net::SocketAddr = if let Some(ref s) = cfg.public_addr {
        s.parse().expect("invalid public_addr in server config")
    } else if bound_addr.ip().is_unspecified() {
        format!("127.0.0.1:{}", bound_addr.port()).parse().unwrap()
    } else {
        bound_addr
    };
    let server_config = ServerConfig {
        current_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
        max_clients: cfg.max_clients,
        protocol_id: NETCODE_PROTOCOL_ID,
        public_addresses: vec![public_addr],
        authentication: ServerAuthentication::Unsecure,
    };
    let transport = NetcodeServerTransport::new(server_config, socket).expect("failed to create server transport");

    // Reliable server (renet)
    let server = RenetServer::new(ConnectionConfig::default());

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!(port = cfg.port, "Server running");
}

fn server_handle_events(
    mut server: ResMut<RenetServer>,
    mut commands: Commands,
    mut clients: ResMut<ClientEntities>,
) {
    while let Some(event) = server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!(?client_id, "client connected");
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!(?client_id, ?reason, "client disconnected");
                if let Some(entity) = clients.0.remove(&client_id) {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

fn server_handle_messages(
    mut server: ResMut<RenetServer>,
    mut commands: Commands,
    level: Res<LevelRes>,
    mut clients: ResMut<ClientEntities>,
    mut paused: ResMut<SimPaused>,
    cfg: Res<Config>,
) {
    for client_id in server.clients_id() {
        while let Some(payload) = server.receive_message(client_id, DefaultChannel::ReliableOrdered) {
            match protocol::decode::<ClientToServer>(payload.as_ref()) {
                Ok(ClientToServer::Hello(hello)) => {
                    if hello.protocol != PROTOCOL_VERSION {
                        let msg = ServerToClient::Disconnect(DisconnectReason::IncompatibleProtocol {
                            server: PROTOCOL_VERSION,
                            client: hello.protocol,
                        });
                        server.send_message(client_id, DefaultChannel::ReliableOrdered, protocol::encode(&msg).unwrap());
                        server.disconnect(client_id);
                        continue;
                    }
                    // Assign a UUID and ack, and spawn a server-side player + submarine state
                    let player_uuid = Uuid::new_v4();
                    let ack = ServerToClient::JoinAck(protocol::JoinAck { player_id: player_uuid, tick_hz: cfg.tick_hz.max(1) });
                    server.send_message(client_id, DefaultChannel::ReliableOrdered, protocol::encode(&ack).unwrap());
                    info!(?client_id, name = hello.display_name.as_deref().unwrap_or("(anon)"), "sent JoinAck");

                    // Avoid double-spawn on repeated Hello
                    if clients.0.contains_key(&client_id) { continue; }

                    // Compute a start position near tunnel entrance
                    let t = &level.0.tunnel;
                    let half_x = t.size.x * 0.5;
                    let start = Vec3f { x: t.pos.x - half_x + 6.0, y: t.pos.y, z: t.pos.z };
                    let spec = small_skiff_spec();
                    let entity = commands.spawn((
                        Player { id: player_uuid },
                        Submarine,
                        SubStateComp(SubState { position: start, velocity: Vec3f::new(0.0, 0.0, 0.0), orientation: Quatf::from_yaw(0.0), ang_vel: Vec3f::new(0.0, 0.0, 0.0), ballast_fill: vec![0.5; spec.ballast_tanks.len()] }),
                        SubPhysicsComp(spec),
                        Name::new(format!("Player {player_uuid}")),
                    )).id();
                    clients.0.insert(client_id, entity);
                }
                Ok(ClientToServer::InputTick(input)) => {
                    // For now ignore in physics; acknowledge receipt only.
                    let ack = ServerToClient::InputAck(protocol::InputAck { tick: input.tick });
                    let payload = protocol::encode(&ack).unwrap();
                    server.send_message(client_id, DefaultChannel::ReliableOrdered, payload);
                    // Update or insert control input on the client's entity
                    if let Some(&entity) = clients.0.get(&client_id) {
                        let thrust = input.thrust.clamp(-1.0, 1.0);
                        let yaw = input.yaw.clamp(-1.0, 1.0);
                        let pump_fwd = input.pump_fwd.clamp(-1.0, 1.0);
                        let pump_aft = input.pump_aft.clamp(-1.0, 1.0);
                        commands.entity(entity).insert(ControlInputComp { thrust, yaw, pump_fwd, pump_aft, last_tick: input.tick });
                    }
                }
                Ok(ClientToServer::PauseRequest(req)) => {
                    paused.0 = req.paused;
                    let msg = ServerToClient::PauseState(protocol::PauseState { paused: paused.0 });
                    let payload = protocol::encode(&msg).unwrap();
                    for id in server.clients_id() {
                        server.send_message(id, DefaultChannel::ReliableOrdered, payload.clone());
                    }
                }
                Ok(other) => {
                    warn!(?client_id, ?other, "unexpected message before world init");
                }
                Err(err) => warn!(?client_id, ?err, "failed to decode client message"),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn server_physics_tick(
    time: Res<Time>,
    mut timing: ResMut<PhysicsTiming>,
    level: Res<LevelRes>,
    mut tick: ResMut<Tick>,
    mut server: ResMut<RenetServer>,
    mut clients: ResMut<ClientEntities>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut SubStateComp, &SubPhysicsComp, Option<&ControlInputComp>)>,
    paused: Res<SimPaused>,
) {
    if paused.0 {
        // Drop accumulated dt to avoid huge catch-up on resume.
        timing.acc = 0.0;
        return;
    }
    timing.acc += time.delta_secs();
    while timing.acc >= timing.dt {
        for (entity, mut s, spec, input) in &mut q {
            let inp = if let Some(ci) = input { SubInputs { thrust: ci.thrust, yaw: ci.yaw, pump_fwd: ci.pump_fwd, pump_aft: ci.pump_aft } } else { SubInputs::default() };
            step_submarine(&level.0, &spec.0, inp, &mut s.0, timing.dt, time.elapsed_secs());
            // Collision with tunnel walls (server-authoritative): Y/Z outside interior AABB
            let c = level.0.tunnel.pos;
            let h = level.0.tunnel.size;
            let half_y = h.y * 0.5;
            let half_z = h.z * 0.5;
            let y = s.0.position.y;
            let z = s.0.position.z;
            let collide = y < c.y - half_y || y > c.y + half_y || z < c.z - half_z || z > c.z + half_z;
            if collide {
                // Find client_id for this entity and disconnect once; also cleanup entity & mapping immediately
                if let Some((&client_id, _)) = clients.0.iter().find(|(_, &e)| e == entity) {
                    tracing::warn!(?client_id, ?entity, "Disconnecting client due to tunnel wall collision");
                    server.disconnect(client_id);
                    if let Some(_removed_entity) = clients.0.remove(&client_id) {
                        commands.entity(entity).despawn();
                    }
                }
                continue; // skip further processing on collided entity this tick
            }
        }
        timing.acc -= timing.dt;
        tick.0 = tick.0.wrapping_add(1);
    }
}

fn server_broadcast_state(
    time: Res<Time>,
    mut timing: ResMut<SnapshotTiming>,
    tick: Res<Tick>,
    mut server: ResMut<RenetServer>,
    q: Query<(&Player, &SubStateComp)>,
) {
    timing.acc += time.delta_secs();
    if timing.acc < timing.dt { return; }
    timing.acc -= timing.dt;

    let mut players = Vec::new();
    for (player, state) in &q {
        players.push(protocol::NetPlayer {
            id: player.id,
            position: [state.0.position.x, state.0.position.y, state.0.position.z],
            velocity: [state.0.velocity.x, state.0.velocity.y, state.0.velocity.z],
            yaw: state.0.orientation.to_yaw(),
            orientation: [state.0.orientation.x, state.0.orientation.y, state.0.orientation.z, state.0.orientation.w],
        });
    }
    let delta = protocol::StateDelta { tick: tick.0, players };
    let payload = protocol::encode(&protocol::ServerToClient::StateDelta(delta)).unwrap();
    for client_id in server.clients_id() {
        // Use unreliable channel for snapshots to avoid HOL blocking.
        server.send_message(client_id, DefaultChannel::Unreliable, payload.clone());
    }
}
