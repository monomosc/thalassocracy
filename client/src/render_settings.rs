use bevy::prelude::*;
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use bevy_inspector_egui::InspectorOptions;

#[derive(Resource, Debug, Clone, Reflect, InspectorOptions)]
#[reflect(Resource)]
pub struct RenderSettings {
    pub volumetric_cones: bool,
    pub volumetric_cone_intensity: f32,
    pub water_post: bool,
    pub water_post_strength: f32,
    pub water_post_debug: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            volumetric_cones: true,
            volumetric_cone_intensity: 0.3,
            water_post: true,
            water_post_strength: 1.0,
            water_post_debug: false,
        }
    }
}

pub struct RenderSettingsPlugin;

impl Plugin for RenderSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderSettings>()
            .register_type::<RenderSettings>()
            .add_plugins(ResourceInspectorPlugin::<RenderSettings>::default())
            .init_resource::<VolumetricConeShaderDebugSettings>()
            .register_type::<VolumetricConeShaderDebugSettings>()
            .add_plugins(ResourceInspectorPlugin::<VolumetricConeShaderDebugSettings>::default());
    }
}

#[derive(Resource, Debug, Clone, Reflect, InspectorOptions, Default)]
#[reflect(Resource)]
pub struct VolumetricConeShaderDebugSettings {
    #[inspector(min = 0, max = 5)]
    pub debug_mode: u32,
}
