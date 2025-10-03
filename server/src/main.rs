use anyhow::Result;
use clap::Parser;
use tracing::info;

use server::{build_server_app, load_config, Args};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let cfg = load_config(&args.config)?;
    info!(?cfg, "Server config loaded");

    let mut app = build_server_app(cfg);
    app.insert_resource(args);
    app.run();
    Ok(())
}
