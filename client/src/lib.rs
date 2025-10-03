use bevy::asset::AssetPlugin;
use bevy::pbr::wireframe::WireframePlugin;
use bevy::prelude::*;
use bevy::render::settings::{
    Backends, InstanceFlags, PowerPreference, RenderCreation, WgpuSettings,
};
use bevy::render::RenderPlugin;
use bevy_renet::{netcode::NetcodeClientPlugin, RenetClientPlugin};

pub mod args;
pub mod debug_vis;
pub mod desync_metrics;
pub mod hud_controls;
pub mod hud_instruments;
pub mod input;
pub mod labels;
pub mod net;
pub mod render_settings;
pub mod scene;
pub mod sim_pause;

pub use args::Args;
use debug_vis::DebugVisPlugin;
use desync_metrics::{DesyncMetricsPlugin, NetClientStats};
#[cfg(feature = "windowing")]
use hud_controls::HudControlsPlugin;
#[cfg(feature = "windowing")]
use hud_instruments::HudInstrumentsPlugin;
pub use input::ThrustInput;
use labels::LabelPlugin;
use net::{
    client_connect, crash_on_disconnect, enforce_connect_timeout, HelloSent, LatestStateDelta,
    MyPlayerId, NetSet,
};
use scene::{
    submarine::{ClientPhysicsTiming, SubTelemetry},
    ScenePlugin, SimSet,
};
use sim_pause::SimPause;

#[cfg(feature = "windowing")]
use bevy_egui::EguiPlugin;
#[cfg(feature = "windowing")]
use bevy_inspector_egui::quick::WorldInspectorPlugin;

#[derive(Clone, Copy)]
struct ClientAppConfig {
    include_rendering: bool,
    include_ui: bool,
    include_scene: bool,
    include_debug: bool,
}

impl ClientAppConfig {
    fn full(args: &Args) -> Self {
        Self {
            include_rendering: !args.headless,
            include_ui: !args.headless,
            include_scene: true,
            include_debug: true,
        }
    }

    const MINIMAL: Self = Self {
        include_rendering: false,
        include_ui: false,
        include_scene: false,
        include_debug: false,
    };
}

pub fn build_client_app(args: Args) -> App {
    let config = ClientAppConfig::full(&args);
    build_client_app_with_config(args, config)
}

pub fn build_minimal_client_app(args: Args) -> App {
    build_client_app_with_config(args, ClientAppConfig::MINIMAL)
}

fn build_client_app_with_config(args: Args, config: ClientAppConfig) -> App {
    let mut app = App::new();

    if config.include_rendering {
        app.add_plugins((DefaultPlugins
            .set(AssetPlugin {
                file_path: "assets".into(),
                ..Default::default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    device_label: Some("thalassocracy-client".into()),
                    backends: Some(Backends::from_env().unwrap_or(Backends::all())),
                    power_preference: PowerPreference::HighPerformance,
                    instance_flags: InstanceFlags::VALIDATION | InstanceFlags::DEBUG,
                    trace_path: Some(std::path::PathBuf::from("wgpu-trace")),
                    ..Default::default()
                }),
                ..Default::default()
            }),));
        #[cfg(feature = "windowing")]
        if config.include_ui {
            app.add_plugins(EguiPlugin::default());
            app.add_plugins(WorldInspectorPlugin::default());
            app.add_plugins(HudControlsPlugin);
            app.add_plugins(HudInstrumentsPlugin);
            app.add_plugins(render_settings::RenderSettingsPlugin);
        }
    } else {
        app.add_plugins(MinimalPlugins);
    }

    app.insert_resource(args.clone())
        .init_resource::<HelloSent>()
        .init_resource::<MyPlayerId>()
        .init_resource::<LatestStateDelta>()
        .init_resource::<SimPause>()
        .init_resource::<NetClientStats>()
        .init_resource::<SubTelemetry>()
        .init_resource::<ClientPhysicsTiming>();

    if !config.include_ui && !app.world().contains_resource::<ThrustInput>() {
        app.world_mut().insert_resource(ThrustInput::default());
    }

    app.add_plugins(RenetClientPlugin)
        .add_plugins(NetcodeClientPlugin)
        .configure_sets(Update, (NetSet, SimSet).chain())
        .add_systems(Startup, client_connect)
        .add_systems(
            Update,
            (net::pump_network, net::apply_state_to_sub).in_set(NetSet),
        )
        .add_systems(Update, (crash_on_disconnect, enforce_connect_timeout));

    if config.include_debug {
        app.add_plugins(WireframePlugin::default());
        app.add_plugins(DesyncMetricsPlugin);
        app.add_plugins(DebugVisPlugin);
    }

    if config.include_rendering {
        app.add_plugins(LabelPlugin);
    }

    if config.include_scene {
        app.add_plugins(ScenePlugin);
    }

    app
}
