use bevy::prelude::Resource;
use clap::Parser;

#[derive(Parser, Debug, Resource, Clone)]
#[command(name = "thalassocracy-client")]
#[command(about = "Client for Thalassocracy prototype", long_about = None)]
pub struct Args {
    /// Server address (ip:port)
    #[arg(long, default_value = "127.0.0.1:61234")]
    pub server: String,
    /// Run without window/rendering
    #[arg(long, default_value_t = false)]
    pub headless: bool,
    /// Optional display name to send in Hello
    #[arg(long)]
    pub name: Option<String>,
    /// Seconds to wait for connect before exiting
    #[arg(long, default_value_t = 5)]
    pub connect_timeout_secs: u64,
}
