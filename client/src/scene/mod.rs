use bevy::prelude::*;

pub mod setup;
pub mod world;
pub mod submarine;
pub mod camera;
pub mod water;
pub mod postprocess;
pub mod volumetric_cone_material;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimSet;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        use submarine::{ClientPhysicsTiming, SubTelemetry};

        app.register_type::<world::FlowField>()
            .init_resource::<SubTelemetry>()
            .init_resource::<ClientPhysicsTiming>()
            .add_systems(Startup, (setup::setup_scene, world::spawn_greybox))
            .add_systems(
                Update,
                (
                    camera::switch_cameras_keys,
                    camera::free_fly_camera,
                    world::draw_flow_gizmos,
                    submarine::simulate_submarine.in_set(SimSet),
                    submarine::apply_server_corrections,
                    camera::update_game_camera.after(SimSet),
                    submarine::animate_rudder,
                ),
            );

        // Lightweight underwater look and feel
        app.add_plugins(water::WaterFxPlugin);
        app.add_plugins(postprocess::WaterPostProcessPlugin);
    }
}
