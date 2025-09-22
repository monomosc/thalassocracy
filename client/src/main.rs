use anyhow::Result;
use bevy::asset::AssetPlugin;
use bevy::pbr::wireframe::WireframePlugin;
use bevy::prelude::*;
use bevy::render::settings::{
    Backends, InstanceFlags, PowerPreference, RenderCreation, WgpuSettings,
};
use bevy::render::RenderPlugin;
#[cfg(feature = "windowing")]
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_renet::{netcode::NetcodeClientPlugin, RenetClientPlugin};
use clap::Parser;

mod net;
use net::{
    client_connect, crash_on_disconnect, enforce_connect_timeout, HelloSent, LatestStateDelta,
    MyPlayerId,
};
mod labels;
use labels::LabelPlugin;
mod debug_vis;
use debug_vis::DebugVisPlugin;
mod desync_metrics;
use desync_metrics::DesyncMetricsPlugin;
mod scene;
use scene::ScenePlugin;
use scene::SimSet;
mod hud_controls;
use hud_controls::HudControlsPlugin;
mod hud_instruments;
use hud_instruments::HudInstrumentsPlugin;
mod render_settings;
mod sim_pause;

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
        app.add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "assets".into(), // path is relative to the client-crate
                    ..Default::default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        device_label: Some("thalassocracy-client".into()),
                        // Let wgpu pick the best backend; override with env if needed
                        backends: Some(Backends::from_env().unwrap_or(Backends::all())),
                        power_preference: PowerPreference::HighPerformance,
                        instance_flags: InstanceFlags::VALIDATION | InstanceFlags::DEBUG,
                        // Write a GPU command trace you can replay with `wgpu-tools` (optional)
                        trace_path: Some(std::path::PathBuf::from("wgpu-trace")),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
        );
        #[cfg(feature = "windowing")]
        {
            use bevy_inspector_egui::bevy_egui::EguiPlugin;

            app.add_plugins(EguiPlugin::default());
            app.add_plugins(WorldInspectorPlugin::default());
            app.add_plugins(HudControlsPlugin);
            app.add_plugins(HudInstrumentsPlugin);
            app.add_plugins(render_settings::RenderSettingsPlugin);
        }
    }

    app.insert_resource(args)
        .init_resource::<HelloSent>()
        .init_resource::<MyPlayerId>()
        .init_resource::<LatestStateDelta>()
        .init_resource::<sim_pause::SimPause>()
        .configure_sets(Update, (net::NetSet, SimSet).chain())
        .add_plugins(RenetClientPlugin)
        .add_plugins(NetcodeClientPlugin)
        .add_plugins(WireframePlugin::default())
        .add_plugins(DesyncMetricsPlugin)
        .add_plugins(DebugVisPlugin)
        .add_plugins(ScenePlugin)
        .add_plugins(LabelPlugin)
        .add_systems(Startup, client_connect)
        .add_systems(
            Update,
            (net::pump_network, net::apply_state_to_sub).in_set(net::NetSet),
        )
        .add_systems(Update, (crash_on_disconnect, enforce_connect_timeout));

    app.run();
    Ok(())
}

// Networking systems and resources live in net.rs
