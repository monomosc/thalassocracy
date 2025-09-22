use bevy::prelude::*;

pub mod camera;
pub mod flow_field;
pub mod greybox;
pub mod light_bulb;
pub mod ore;
pub mod postprocess;
pub mod proctex;
pub mod render;
pub mod setup;
pub mod submarine;
pub mod water;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimSet;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        use submarine::{ClientPhysicsTiming, SubTelemetry};

        app.register_type::<flow_field::FlowField>()
            .init_resource::<SubTelemetry>()
            .init_resource::<ClientPhysicsTiming>()
            .add_plugins(proctex::ProcTexPlugin)
            .add_plugins(light_bulb::LightBulbPlugin)
            .add_systems(Startup, (setup::setup_scene, greybox::spawn_greybox))
            .add_systems(
                Update,
                (
                    camera::switch_cameras_keys,
                    camera::free_fly_camera,
                    flow_field::draw_flow_gizmos,
                    submarine::simulate_submarine.in_set(SimSet),
                    submarine::apply_server_corrections,
                    camera::update_game_camera.after(SimSet),
                    submarine::animate_rudder,
                ),
            );

        // Lightweight underwater look and feel
        app.add_plugins(render::volumetric_floodlights::VolumetricFloodlightsPlugin);
        app.add_plugins(water::WaterFxPlugin);
        app.add_plugins(postprocess::WaterPostProcessPlugin);
        app.add_plugins(ore::OrePlugin);
    }
}
