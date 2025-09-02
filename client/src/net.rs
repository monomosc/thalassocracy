use bevy::prelude::*;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientTransport};
use bevy_renet::renet::{ConnectionConfig, DefaultChannel, RenetClient};
use std::net::UdpSocket;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use crate::Args;
use protocol::{ClientToServer, ClientHello, ServerToClient, PROTOCOL_VERSION, NETCODE_PROTOCOL_ID};

#[derive(Resource, Default)]
pub struct HelloSent(pub bool);

#[derive(Resource)]
pub struct ConnectStart {
    pub at: Instant,
    pub timeout: Duration,
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

    info!(?server_addr, "Client created and connecting");
}

pub fn pump_network(client: Option<ResMut<RenetClient>>, args: Res<Args>, mut hello_sent: ResMut<HelloSent>) {
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
            }
            Ok(other) => {
                warn!(?other, "Unhandled server message");
            }
            Err(err) => warn!(?err, "Failed to decode server message"),
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
