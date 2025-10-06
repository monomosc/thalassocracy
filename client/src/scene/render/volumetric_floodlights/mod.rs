use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use bevy::render::{
    render_graph::{RenderGraphApp, ViewNodeRunner},
    render_resource::SpecializedRenderPipelines,
    ExtractSchedule, Render, RenderApp, RenderSet,
};

pub mod debug_material;
pub use debug_material::VolumetricConeDebugMaterial;

mod cones;
mod extract;
mod pipeline;
mod render_node;
mod ui;

pub use cones::VolumetricCone;
pub use render_node::FloodlightPassLabel;

pub const CONE_VOLUME_SHADER_PATH: &str = "shaders/volumetric_floodlights/volumetric_cones.wgsl";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricLightingMode {
    Disabled,
    RaymarchCones,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct VolumetricLightingState {
    pub mode: VolumetricLightingMode,
}

impl Default for VolumetricLightingState {
    fn default() -> Self {
        Self {
            mode: VolumetricLightingMode::RaymarchCones,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct RenderVolumetricLightingMode(pub VolumetricLightingMode);

impl Default for RenderVolumetricLightingMode {
    fn default() -> Self {
        Self(VolumetricLightingMode::RaymarchCones)
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub(super) struct ExtractedVolumetricSettings {
    pub scatter_strength: f32,
    pub distance_falloff: f32,
    pub angular_softness: f32,
    pub extinction: f32,
}

impl Default for ExtractedVolumetricSettings {
    fn default() -> Self {
        Self {
            scatter_strength: 0.02,
            distance_falloff: 0.12,
            angular_softness: 0.08,
            extinction: 0.25,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub(super) struct ExtractedVolumetricDebugSettings {
    pub debug_mode: u32,
}

pub struct VolumetricFloodlightsPlugin;

impl Plugin for VolumetricFloodlightsPlugin {
    fn build(&self, app: &mut App) {
        cones::register(app);

        app.init_resource::<VolumetricLightingState>()
            .add_systems(Update, ui::toggle_volumetric_mode)
            .add_systems(Startup, ui::spawn_mode_label)
            .add_systems(Update, ui::update_mode_label);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<RenderVolumetricLightingMode>()
                .init_resource::<pipeline::ConeVolumePipeline>()
                .init_resource::<SpecializedRenderPipelines<pipeline::ConeVolumePipeline>>()
                .init_resource::<pipeline::ExtractedConeLights>()
                .init_resource::<ExtractedVolumetricSettings>()
                .init_resource::<ExtractedVolumetricDebugSettings>()
                .add_systems(ExtractSchedule, extract::extract_volumetric_mode)
                .add_systems(
                    ExtractSchedule,
                    extract::extract_volumetric_settings.after(extract::extract_volumetric_mode),
                )
                .add_systems(
                    ExtractSchedule,
                    extract::extract_volumetric_debug_settings
                        .after(extract::extract_volumetric_settings),
                )
                .add_systems(
                    ExtractSchedule,
                    extract::extract_cone_lights.after(extract::extract_volumetric_debug_settings),
                )
                .add_systems(
                    Render,
                    pipeline::prepare_view_cone_lights.in_set(RenderSet::Queue),
                )
                .add_render_graph_node::<ViewNodeRunner<render_node::FloodlightViewNode>>(
                    Core3d,
                    render_node::FloodlightPassLabel,
                )
                .add_render_graph_edges(
                    Core3d,
                    (
                        Node3d::MainTransparentPass,
                        render_node::FloodlightPassLabel,
                        Node3d::EndMainPass,
                    )
                );
        }
    }
}
