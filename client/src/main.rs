use anyhow::Result;
use bevy::prelude::*;
use bevy_renet::{netcode::NetcodeClientPlugin, RenetClientPlugin};
use clap::Parser;

mod net;
use net::{client_connect, crash_on_disconnect, enforce_connect_timeout, HelloSent};
mod scene;
use scene::ScenePlugin;

#[derive(Parser, Debug, Resource)]
#[command(name = "thalassocracy-client")] 
#[command(about = "Client for Thalassocracy prototype", long_about = None)]
struct Args {
    /// Server address (ip:port)
    #[arg(long, default_value = "127.0.0.1:61234")]
    server: String,
    /// Run without window/rendering
    #[arg(long, default_value_t = false)]
    headless: bool,
    /// Optional display name to send in Hello
    #[arg(long)]
    name: Option<String>,
    /// Seconds to wait for connect before exiting
    #[arg(long, default_value_t = 5)]
    connect_timeout_secs: u64,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let mut app = App::new();

    if args.headless {
        app.add_plugins(MinimalPlugins);
    } else {
        app.add_plugins(DefaultPlugins);
    }

    app.insert_resource(args)
        .init_resource::<HelloSent>()
        .add_plugins(RenetClientPlugin)
        .add_plugins(NetcodeClientPlugin)
        .add_plugins(ScenePlugin)
        .add_systems(Startup, client_connect)
        .add_systems(Update, (net::pump_network, crash_on_disconnect, enforce_connect_timeout));

    app.run();
    Ok(())
}

// Networking systems and resources live in net.rs
