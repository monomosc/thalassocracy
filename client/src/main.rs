use anyhow::Result;
use clap::Parser;

use client::{build_client_app, Args};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let mut app = build_client_app(args);
    app.run();
    Ok(())
}
