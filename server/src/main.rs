use std::net::UdpSocket;
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
        .add_systems(Update, (server_handle_events, server_handle_messages));

    app.run();
    Ok(())
}

fn server_setup(mut commands: Commands, cfg: Res<Config>) {
    // Bind UDP socket
    let socket = UdpSocket::bind(("0.0.0.0", cfg.port)).expect("failed to bind UDP socket");

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

fn server_handle_events(mut server: ResMut<RenetServer>) {
    while let Some(event) = server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!(?client_id, "client connected");
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!(?client_id, ?reason, "client disconnected");
            }
        }
    }
}

fn server_handle_messages(mut server: ResMut<RenetServer>) {
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
                    // Assign a UUID and ack
                    let ack = ServerToClient::JoinAck(protocol::JoinAck { player_id: uuid::Uuid::new_v4() });
                    server.send_message(client_id, DefaultChannel::ReliableOrdered, protocol::encode(&ack).unwrap());
                    info!(?client_id, name = hello.display_name.as_deref().unwrap_or("(anon)"), "sent JoinAck");
                }
                Ok(other) => {
                    warn!(?client_id, ?other, "unexpected message before world init");
                }
                Err(err) => warn!(?client_id, ?err, "failed to decode client message"),
            }
        }
    }
}
